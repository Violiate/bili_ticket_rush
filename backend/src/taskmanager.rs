use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use common::account::{add_account_async, signout_account_async};
use common::captcha::handle_risk_verification;
use common::cookie_manager::CookieManager;
use common::gen_cp::CTokenGenerator;
use common::login::{qrcode_login_async, send_loginsms, sms_login};
use common::push::PushConfig;
use common::record_log::TASK_LABEL;
use common::taskmanager::*;
use common::ticket::*;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde_json::json;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, watch};

use crate::api::*;
use crate::show_orderlist::get_orderlist;

const MAX_RECENT_TASKS: usize = 50;
const MAX_CONFIRM_RETRY: usize = 4;
const MAX_FAKE_CHECK_POLLS: usize = 5;

struct GrabTaskEntry {
    info_seed: RunningTaskInfo,
    start_time: Instant,
    cancel_flag: Arc<AtomicBool>,
}

struct CancelTokenEntry {
    uid: Option<i64>,
    cancel_flag: Arc<AtomicBool>,
    cancel_tx: watch::Sender<bool>,
}

struct FinalTaskOutcome {
    status: TaskStatus,
    message: String,
}

/// RAII 守卫：普通可取消任务（如二维码轮询）结束时自动从 `cancel_tokens` 移除。
struct CancelTokenGuard {
    cancel_tokens: Arc<Mutex<HashMap<String, CancelTokenEntry>>>,
    task_id: String,
}

impl Drop for CancelTokenGuard {
    fn drop(&mut self) {
        let mut tokens = self
            .cancel_tokens
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        tokens.remove(&self.task_id);
    }
}

/// RAII 守卫：抢票任务无论正常结束、取消还是 panic unwind，
/// 均会清理 `grab_registry` / `cancel_tokens` 并将结束快照写入 `recent_tasks`。
struct GrabTaskGuard {
    grab_registry: Arc<Mutex<HashMap<String, GrabTaskEntry>>>,
    cancel_tokens: Arc<Mutex<HashMap<String, CancelTokenEntry>>>,
    recent_tasks: Arc<Mutex<VecDeque<RecentTaskInfo>>>,
    task_id: String,
    uid: i64,
    account_name: String,
    project_id: String,
    project_name: String,
    grab_mode: u8,
    started_at: Instant,
    cancel_flag: Arc<AtomicBool>,
    final_outcome: Arc<Mutex<Option<FinalTaskOutcome>>>,
}

impl Drop for GrabTaskGuard {
    fn drop(&mut self) {
        {
            let mut reg = self
                .grab_registry
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            reg.remove(&self.task_id);
        }
        {
            let mut tokens = self
                .cancel_tokens
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            tokens.remove(&self.task_id);
        }

        let explicit = self
            .final_outcome
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();

        let (status, message) = match explicit {
            Some(outcome) => (outcome.status, outcome.message),
            None => {
                if self.cancel_flag.load(Ordering::Relaxed) {
                    (TaskStatus::Cancelled, "任务已取消".to_string())
                } else {
                    (
                        TaskStatus::Failed("任务异常终止".to_string()),
                        "任务异常终止".to_string(),
                    )
                }
            }
        };

        let record = RecentTaskInfo {
            task_id: self.task_id.clone(),
            uid: self.uid,
            account_name: self.account_name.clone(),
            project_id: self.project_id.clone(),
            project_name: self.project_name.clone(),
            grab_mode: self.grab_mode,
            status,
            message,
            started_at: self.started_at,
            finished_at: Instant::now(),
        };

        let mut recent = self.recent_tasks.lock().unwrap_or_else(|e| e.into_inner());
        recent.push_front(record);
        while recent.len() > MAX_RECENT_TASKS {
            recent.pop_back();
        }
    }
}

fn set_final_outcome(
    holder: &Arc<Mutex<Option<FinalTaskOutcome>>>,
    status: TaskStatus,
    message: impl Into<String>,
) {
    let mut guard = holder.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(FinalTaskOutcome {
        status,
        message: message.into(),
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GrabOutcome {
    Success,
    Cancelled,
    NeedRefreshToken,
    RetryLimitReached,
    FatalError(String),
    ConfirmFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CreateOrderErrorAction {
    Retry(Duration),
    NeedRefreshToken,
    Fatal(String),
}

pub(crate) fn classify_create_order_error(
    code: i32,
    default_delay: Duration,
) -> CreateOrderErrorAction {
    match code {
        100001 | 429 | 900001 => {
            log::info!("B站限速({})，正常现象，稍后重试", code);
            CreateOrderErrorAction::Retry(Duration::from_millis(300))
        }
        100009 => {
            log::info!("当前票种库存不足");
            CreateOrderErrorAction::Retry(Duration::from_millis(600))
        }
        211 => {
            log::info!("很遗憾，差一点点抢到票，继续加油吧！");
            CreateOrderErrorAction::Retry(default_delay)
        }
        3 => {
            log::info!("抢票速度过快，即将被硬控5秒，暂停4.8秒");
            CreateOrderErrorAction::Retry(Duration::from_millis(4800))
        }
        100041 | 100050 | 900002 => {
            log::info!("token失效({})，即将重新获取token", code);
            CreateOrderErrorAction::NeedRefreshToken
        }
        100017 | 100016 => {
            CreateOrderErrorAction::Fatal("当前项目/类型/场次已停售".to_string())
        }
        1 => CreateOrderErrorAction::Fatal(
            "超人请慢一点，这是仅限1人抢票的项目或抢票格式有误，请重新提交任务".to_string(),
        ),
        83000004 => {
            CreateOrderErrorAction::Fatal("没有配置购票人信息，请重新配置".to_string())
        }
        100079 | 100003 => CreateOrderErrorAction::Fatal(
            "购票人存在待付款订单，请前往支付或取消后重新下单".to_string(),
        ),
        100039 => CreateOrderErrorAction::Fatal("活动已收摊/停售，下次要快点哦".to_string()),
        209001 => CreateOrderErrorAction::Fatal(
            "当前项目只能选择一个购票人，不支持多选，请重新提交任务".to_string(),
        ),
        ERR_INVALID_PARAM => CreateOrderErrorAction::Fatal(
            "程序内部传参错误（项目/场次/票种ID或购票人信息无效）".to_string(),
        ),
        ERR_ID_BIND_INVALID => CreateOrderErrorAction::Fatal(
            "该项目实名绑定类型(id_bind)异常，请尝试重新提交任务".to_string(),
        ),
        ERR_NETWORK => {
            log::warn!("下单网络请求失败，准备重试...");
            CreateOrderErrorAction::Retry(default_delay)
        }
        ERR_PARSE => {
            log::warn!("下单响应解析失败，准备重试...");
            CreateOrderErrorAction::Retry(default_delay)
        }
        412 => {
            log::warn!("触发B站 412 风控拦截，放缓请求后重试");
            CreateOrderErrorAction::Retry(Duration::from_millis(1200))
        }
        737 => {
            log::warn!("B站返回空数据(737)，将继续重试，若持续异常请取消任务");
            CreateOrderErrorAction::Retry(default_delay)
        }
        _ => {
            log::error!(
                "下单失败，未知错误码：{}，将继续重试（可提 issue 上报）",
                code
            );
            CreateOrderErrorAction::Retry(default_delay)
        }
    }
}

/// 将分（整数）格式化为带两位小数的元字符串（如 `9990` -> `"99.90"`, `50` -> `"0.50"`）
pub(crate) fn format_price_yuan(cents: i64) -> String {
    let abs_cents = cents.unsigned_abs();
    let yuan = abs_cents / 100;
    let fen = abs_cents % 100;
    if cents < 0 {
        format!("-{}.{:02}", yuan, fen)
    } else {
        format!("{}.{:02}", yuan, fen)
    }
}

/// 优先使用 `confirm_result.pay_money`（> 0），否则回退 `ticket_info.price * count`
pub(crate) fn resolve_order_price_cents(confirm_result: &ConfirmTicketResult) -> i64 {
    if confirm_result.pay_money > 0 {
        confirm_result.pay_money
    } else {
        confirm_result
            .ticket_info
            .price
            .saturating_mul(confirm_result.count.max(0) as i64)
    }
}

pub(crate) fn should_skip_by_words(text: &str, skip_words_lower: &[String]) -> bool {
    if skip_words_lower.is_empty() || text.is_empty() {
        return false;
    }
    let lowered = text.to_lowercase();
    skip_words_lower.iter().any(|w| lowered.contains(w))
}

pub(crate) fn extract_verified_pay_param(
    check_result: &serde_json::Value,
) -> Result<CheckFakeResultData, String> {
    if let Some((err_code, err_msg)) = extract_api_error(check_result) {
        return Err(format!("code={}, msg={}", err_code, err_msg));
    }

    if !check_result.get("data").is_some_and(|d| d.is_object()) {
        return Err("payParam.code_url 为空，订单支付信息尚未生成或为假票".to_string());
    }

    let analyze_result = serde_json::from_value::<CheckFakeResult>(check_result.clone())
        .map_err(|e| format!("解析核验响应失败: {}", e))?;

    if analyze_result.data.pay_param.code_url.trim().is_empty() {
        return Err("payParam.code_url 为空，订单支付信息尚未生成或为假票".to_string());
    }

    Ok(analyze_result.data.pay_param)
}

pub(crate) fn is_transient_mode2_token_error(code: i64) -> bool {
    matches!(code, 100001 | 100009 | 100017 | 429 | 412 | -1001 | -1002)
}

pub(crate) fn has_actionable_risk_verification(risk_param: &TokenRiskParam) -> bool {
    (risk_param.code == -401 || risk_param.code == 401)
        && risk_param
            .risk_param
            .as_ref()
            .is_some_and(|v| !v.is_null())
}

fn is_cancelled(cancel_flag: &Arc<AtomicBool>, cancel_rx: &watch::Receiver<bool>) -> bool {
    cancel_flag.load(Ordering::Relaxed) || *cancel_rx.borrow()
}

async fn wait_for_cancel(cancel_flag: &Arc<AtomicBool>, cancel_rx: &mut watch::Receiver<bool>) {
    if is_cancelled(cancel_flag, cancel_rx) {
        return;
    }
    while cancel_rx.changed().await.is_ok() {
        if is_cancelled(cancel_flag, cancel_rx) {
            return;
        }
    }
    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn cancellable_sleep(
    duration: Duration,
    cancel_flag: &Arc<AtomicBool>,
    cancel_rx: &mut watch::Receiver<bool>,
) -> bool {
    if is_cancelled(cancel_flag, cancel_rx) {
        return true;
    }
    tokio::select! {
        _ = tokio::time::sleep(duration) => is_cancelled(cancel_flag, cancel_rx),
        _ = wait_for_cancel(cancel_flag, cancel_rx) => true,
    }
}

fn build_ctoken_generator(
    sale_begin: Option<i64>,
    ua: Option<String>,
) -> Arc<Mutex<CTokenGenerator>> {
    let mut rng = StdRng::from_entropy();
    let now_sec = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let ticket_collection_t = sale_begin.unwrap_or(now_sec);
    Arc::new(Mutex::new(CTokenGenerator::new(
        ticket_collection_t,
        0,
        rng.gen_range(2000..10000),
        ua,
    )))
}

fn emit_grab_failure(
    result_tx: &mpsc::UnboundedSender<TaskResult>,
    task_id: &str,
    uid: i64,
    project_id: &str,
    account_name: &str,
    message: &str,
) {
    let _ = result_tx.send(TaskResult::GrabTicketResult(GrabTicketResult {
        task_id: task_id.to_string(),
        uid,
        project_id: project_id.to_string(),
        account_name: account_name.to_string(),
        success: false,
        message: message.to_string(),
        order_id: None,
        pay_token: None,
        confirm_result: None,
        pay_result: None,
    }));
}

enum TaskMessage {
    SubmitTask {
        task_id: String,
        request: TaskRequest,
    },
    SubmitQrCodeLogin {
        task_id: String,
        request: QrCodeLoginRequest,
        cancel_rx: watch::Receiver<bool>,
    },
    SubmitGrabTask {
        task_id: String,
        request: GrabTicketRequest,
        seq: u64,
        account_name: String,
        project_name: String,
        started_at: Instant,
        cancel_flag: Arc<AtomicBool>,
        cancel_rx: watch::Receiver<bool>,
    },
    Shutdown,
}

pub struct TaskManagerImpl {
    task_sender: mpsc::UnboundedSender<TaskMessage>,
    result_receiver: mpsc::UnboundedReceiver<TaskResult>,
    grab_registry: Arc<Mutex<HashMap<String, GrabTaskEntry>>>,
    cancel_tokens: Arc<Mutex<HashMap<String, CancelTokenEntry>>>,
    recent_tasks: Arc<Mutex<VecDeque<RecentTaskInfo>>>,
    live_push_config: Arc<RwLock<PushConfig>>,
    push_config_updated: Arc<AtomicBool>,
    seq_counter: Arc<AtomicU64>,
    _worker_thread: Option<thread::JoinHandle<()>>,
}

impl TaskManagerImpl {
    #[cfg(test)]
    fn new_without_worker() -> (Self, mpsc::UnboundedReceiver<TaskMessage>) {
        let (task_tx, task_rx) = mpsc::unbounded_channel();
        let (_result_tx, result_rx) = mpsc::unbounded_channel();
        let tm = Self {
            task_sender: task_tx,
            result_receiver: result_rx,
            grab_registry: Arc::new(Mutex::new(HashMap::new())),
            cancel_tokens: Arc::new(Mutex::new(HashMap::new())),
            recent_tasks: Arc::new(Mutex::new(VecDeque::new())),
            live_push_config: Arc::new(RwLock::new(PushConfig::new())),
            push_config_updated: Arc::new(AtomicBool::new(false)),
            seq_counter: Arc::new(AtomicU64::new(0)),
            _worker_thread: None,
        };
        (tm, task_rx)
    }
}

impl Drop for TaskManagerImpl {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl TaskManager for TaskManagerImpl {
    fn new() -> Self {
        let (task_tx, mut task_rx) = mpsc::unbounded_channel::<TaskMessage>();
        let (result_tx, result_rx) = mpsc::unbounded_channel::<TaskResult>();

        let runtime = Runtime::new().expect("创建 Tokio 运行时失败");

        let grab_registry: Arc<Mutex<HashMap<String, GrabTaskEntry>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let cancel_tokens: Arc<Mutex<HashMap<String, CancelTokenEntry>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let recent_tasks: Arc<Mutex<VecDeque<RecentTaskInfo>>> =
            Arc::new(Mutex::new(VecDeque::new()));
        let live_push_config: Arc<RwLock<PushConfig>> =
            Arc::new(RwLock::new(PushConfig::new()));
        let push_config_updated: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let seq_counter: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));

        let grab_registry_worker = Arc::clone(&grab_registry);
        let cancel_tokens_worker = Arc::clone(&cancel_tokens);
        let recent_tasks_worker = Arc::clone(&recent_tasks);
        let live_push_config_worker = Arc::clone(&live_push_config);
        let push_config_updated_worker = Arc::clone(&push_config_updated);

        // Runtime 的唯一所有权移交到独立 OS 工作线程，避免主线程或测试异步上下文 Drop Runtime 时 panic
        let worker = thread::spawn(move || {
            runtime.block_on(async {
                while let Some(msg) = task_rx.recv().await {
                    match msg {
                        TaskMessage::SubmitQrCodeLogin {
                            task_id,
                            request: qrcode_req,
                            cancel_rx,
                        } => {
                            let result_tx = result_tx.clone();
                            let cancel_tokens_cleanup = Arc::clone(&cancel_tokens_worker);
                            let task_id_cleanup = task_id.clone();

                            tokio::spawn(async move {
                                let _guard = CancelTokenGuard {
                                    cancel_tokens: cancel_tokens_cleanup,
                                    task_id: task_id_cleanup,
                                };

                                let progress_tx = result_tx.clone();
                                let progress_task_id = task_id.clone();
                                let status = poll_qrcode_login_with_progress(
                                    &qrcode_req.qrcode_key,
                                    qrcode_req.user_agent.as_deref(),
                                    cancel_rx,
                                    move || {
                                        let _ = progress_tx.send(TaskResult::QrCodeLoginResult(
                                            TaskQrCodeLoginResult {
                                                task_id: progress_task_id.clone(),
                                                status: common::login::QrCodeLoginStatus::Scanning,
                                                cookie: None,
                                                error: None,
                                            },
                                        ));
                                    },
                                )
                                .await;

                                let (cookie, error) = match &status {
                                    common::login::QrCodeLoginStatus::Success(cookie) => {
                                        (Some(cookie.clone()), None)
                                    }
                                    common::login::QrCodeLoginStatus::Failed(err) => {
                                        (None, Some(err.clone()))
                                    }
                                    _ => (None, None),
                                };

                                let _ = result_tx.send(TaskResult::QrCodeLoginResult(
                                    TaskQrCodeLoginResult {
                                        task_id,
                                        status,
                                        cookie,
                                        error,
                                    },
                                ));
                            });
                        }
                        TaskMessage::SubmitGrabTask {
                            task_id,
                            request: grab_ticket_req,
                            seq,
                            account_name,
                            project_name,
                            started_at,
                            cancel_flag,
                            mut cancel_rx,
                        } => {
                            let result_tx = result_tx.clone();
                            let grab_registry_cleanup = Arc::clone(&grab_registry_worker);
                            let cancel_tokens_cleanup = Arc::clone(&cancel_tokens_worker);
                            let recent_tasks_cleanup = Arc::clone(&recent_tasks_worker);
                            let live_push_cfg = Arc::clone(&live_push_config_worker);
                            let push_cfg_updated = Arc::clone(&push_config_updated_worker);

                            let uid = grab_ticket_req.uid;
                            let project_id = grab_ticket_req.project_id.clone();
                            let grab_mode = grab_ticket_req.grab_mode;
                            let task_label = format!("#{} {}", seq, account_name);

                            tokio::spawn(async move {
                                let final_outcome = Arc::new(Mutex::new(None));
                                let _guard = GrabTaskGuard {
                                    grab_registry: grab_registry_cleanup,
                                    cancel_tokens: cancel_tokens_cleanup,
                                    recent_tasks: recent_tasks_cleanup,
                                    task_id: task_id.clone(),
                                    uid,
                                    account_name: account_name.clone(),
                                    project_id: project_id.clone(),
                                    project_name: project_name.clone(),
                                    grab_mode,
                                    started_at,
                                    cancel_flag: Arc::clone(&cancel_flag),
                                    final_outcome: Arc::clone(&final_outcome),
                                };

                                TASK_LABEL
                                    .scope(task_label, async move {
                                        execute_grab_ticket_task(
                                            task_id,
                                            grab_ticket_req,
                                            account_name,
                                            result_tx,
                                            cancel_flag,
                                            &mut cancel_rx,
                                            live_push_cfg,
                                            push_cfg_updated,
                                            final_outcome,
                                        )
                                        .await;
                                    })
                                    .await;
                            });
                        }
                        TaskMessage::SubmitTask { task_id, request } => {
                            let result_tx = result_tx.clone();

                            match request {
                                TaskRequest::LoginSmsRequest(login_sms_req) => {
                                    let phone = login_sms_req.phone;
                                    let client = login_sms_req.client;
                                    let custom_config = login_sms_req.custom_config;
                                    let local_captcha = login_sms_req.local_captcha;

                                    tokio::spawn(async move {
                                        log::info!("开始发送短信验证码 ID: {}", task_id);
                                        let response = send_loginsms(
                                            &phone,
                                            &client,
                                            custom_config,
                                            local_captcha,
                                        )
                                        .await;
                                        let success = response.is_ok();
                                        let message = match response {
                                            Ok(msg) => msg,
                                            Err(err) => {
                                                log::error!("发送短信验证码失败: {}", err);
                                                err
                                            }
                                        };
                                        log::info!(
                                            "发送短信任务完成 ID: {}, 结果: {}",
                                            task_id,
                                            if success { "成功" } else { "失败" }
                                        );

                                        let _ = result_tx.send(TaskResult::LoginSmsResult(
                                            LoginSmsRequestResult {
                                                task_id,
                                                phone,
                                                success,
                                                message,
                                            },
                                        ));
                                    });
                                }
                                TaskRequest::SubmitLoginSmsRequest(login_sms_req) => {
                                    let phone = login_sms_req.phone;
                                    let client = login_sms_req.client;
                                    let captcha_key = login_sms_req.captcha_key;
                                    let code = login_sms_req.code;

                                    tokio::spawn(async move {
                                        log::info!("短信验证码登录进行中 ID: {}", task_id);
                                        let response =
                                            sms_login(&phone, &code, &captcha_key, &client).await;
                                        let (success, message, cookie) = match response {
                                            Ok(cookie_str) => {
                                                (true, "短信登录成功".to_string(), Some(cookie_str))
                                            }
                                            Err(err) => {
                                                log::error!("提交短信验证码失败: {}", err);
                                                (false, err, None)
                                            }
                                        };
                                        log::info!(
                                            "提交短信任务完成 ID: {}, 结果: {}",
                                            task_id,
                                            if success { "成功" } else { "失败" }
                                        );

                                        let _ = result_tx.send(TaskResult::SubmitSmsLoginResult(
                                            SubmitSmsLoginResult {
                                                task_id,
                                                phone,
                                                success,
                                                message,
                                                cookie,
                                            },
                                        ));
                                    });
                                }
                                TaskRequest::PushRequest(push_req) => {
                                    let push_config = push_req.push_config;
                                    let title = push_req.title;
                                    let message = push_req.message;
                                    let jump_url = push_req.jump_url;
                                    let push_type = push_req.push_type;

                                    tokio::spawn(async move {
                                        log::info!(
                                            "开始处理推送任务 ID: {}, 类型: {:?}",
                                            task_id,
                                            push_type
                                        );

                                        let (success, result_message) = match push_type {
                                            PushType::All => {
                                                if !push_config.enabled {
                                                    (false, "推送未启用".to_string())
                                                } else {
                                                    push_config
                                                        .push_all_async(&title, &message, &jump_url)
                                                        .await
                                                }
                                            }
                                            PushType::Bark => {
                                                push_config
                                                    .push_bark_with_url(
                                                        &title, &message, &jump_url,
                                                    )
                                                    .await
                                            }
                                            PushType::PushPlus => {
                                                push_config.push_pushplus(&title, &message).await
                                            }
                                            PushType::Fangtang => {
                                                push_config.push_fangtang(&title, &message).await
                                            }
                                            PushType::Dingtalk => {
                                                push_config.push_dingtalk(&title, &message).await
                                            }
                                            PushType::WeChat => {
                                                push_config.push_wechat(&title, &message).await
                                            }
                                            PushType::Gotify => {
                                                push_config
                                                    .push_gotify(&title, &message, &jump_url)
                                                    .await
                                            }
                                            PushType::Smtp => {
                                                push_config.push_smtp(&title, &message).await
                                            }
                                        };

                                        log::info!(
                                            "推送任务 ID: {} 完成, 结果: {}",
                                            task_id,
                                            if success { "成功" } else { "失败" }
                                        );

                                        let _ = result_tx.send(TaskResult::PushResult(
                                            PushRequestResult {
                                                task_id,
                                                success,
                                                message: result_message,
                                                push_type,
                                            },
                                        ));
                                    });
                                }
                                TaskRequest::GetAllorderRequest(get_order_req) => {
                                    let cookie_manager = get_order_req.cookie_manager;
                                    let account_id = get_order_req.account_id;
                                    let page = get_order_req.page;
                                    let page_size = get_order_req.page_size;

                                    tokio::spawn(async move {
                                        log::info!(
                                            "正在获取全部订单 ID: {}, page={}, page_size={}",
                                            task_id,
                                            page,
                                            page_size
                                        );
                                        let (success, message, order_info) =
                                            match get_orderlist(cookie_manager, page, page_size)
                                                .await
                                            {
                                                Ok(order_resp) => {
                                                    let msg = format!(
                                                        "获取全部订单成功: {}",
                                                        order_resp.data.total
                                                    );
                                                    (true, msg, Some(order_resp))
                                                }
                                                Err(err) => {
                                                    log::error!("获取全部订单失败: {}", err);
                                                    (false, err, None)
                                                }
                                            };

                                        let _ =
                                            result_tx.send(TaskResult::GetAllorderRequestResult(
                                                GetAllorderRequestResult {
                                                    task_id,
                                                    account_id,
                                                    page,
                                                    success,
                                                    message,
                                                    order_info,
                                                    timestamp: Instant::now(),
                                                },
                                            ));
                                    });
                                }
                                TaskRequest::GetTicketInfoRequest(get_ticketinfo_req) => {
                                    let cookie_manager = get_ticketinfo_req.cookie_manager;
                                    let project_id = get_ticketinfo_req.project_id;
                                    let uid = get_ticketinfo_req.uid;
                                    let referer_link = get_ticketinfo_req.referer_link;

                                    tokio::spawn(async move {
                                        log::debug!("正在获取项目 {} (task_id={})", project_id, task_id);
                                        let response =
                                            get_project(cookie_manager, &project_id, &referer_link)
                                                .await;
                                        let (success, ticket_info, message) = match response {
                                            Ok(info) => {
                                                if info.data.screen_list.is_empty() {
                                                    log::warn!("项目信息获取成功但场次列表为空，可能是API格式变化");
                                                }
                                                let msg =
                                                    format!("项目 {} 请求成功", project_id);
                                                (true, Some(info), msg)
                                            }
                                            Err(e) => {
                                                log::error!("获取项目 {} 失败，原因：{}", project_id, e);
                                                (false, None, e)
                                            }
                                        };

                                        let _ = result_tx.send(TaskResult::GetTicketInfoResult(
                                            GetTicketInfoResult {
                                                task_id,
                                                uid,
                                                ticket_info,
                                                success,
                                                message,
                                            },
                                        ));
                                    });
                                }
                                TaskRequest::GetBuyerInfoRequest(get_buyerinfo_req) => {
                                    let cookie_manager = get_buyerinfo_req.cookie_manager;
                                    let uid = get_buyerinfo_req.uid;

                                    tokio::spawn(async move {
                                        log::debug!("正在获取购票人信息 task_id={}", task_id);
                                        let response = get_buyer_info(cookie_manager).await;
                                        let (success, buyer_info, message) = match response {
                                            Ok(info) => {
                                                if !info.data.list.is_empty() {
                                                    let list_to_save = info.data.list.clone();
                                                    match tokio::task::spawn_blocking(move || {
                                                        common::utils::save_buyer_cache(
                                                            uid,
                                                            &list_to_save,
                                                        )
                                                    })
                                                    .await
                                                    {
                                                        Ok(Ok(())) => {}
                                                        Ok(Err(e)) => {
                                                            log::warn!("保存购票人缓存失败：{}", e);
                                                        }
                                                        Err(e) => {
                                                            log::warn!(
                                                                "保存购票人缓存阻塞任务失败：{}",
                                                                e
                                                            );
                                                        }
                                                    }
                                                }
                                                (true, Some(info), "购票人信息请求成功".to_string())
                                            }
                                            Err(e) => {
                                                log::error!("获取购票人信息失败，原因：{}", e);
                                                let cached_res =
                                                    tokio::task::spawn_blocking(move || {
                                                        common::utils::load_buyer_cache(uid)
                                                    })
                                                    .await;
                                                match cached_res {
                                                    Ok(Ok(list)) if !list.is_empty() => {
                                                        log::warn!(
                                                            "已从本地缓存读取账号 {} 的购票人信息（{} 人）",
                                                            uid,
                                                            list.len()
                                                        );
                                                        let cached = BuyerInfoResponse {
                                                            errno: 0,
                                                            errtag: 0,
                                                            msg: String::new(),
                                                            code: 0,
                                                            message: String::new(),
                                                            data: BuyerInfoData { list },
                                                        };
                                                        (
                                                            true,
                                                            Some(cached),
                                                            "网络获取购票人失败，已从本地缓存读取"
                                                                .to_string(),
                                                        )
                                                    }
                                                    _ => (false, None, e),
                                                }
                                            }
                                        };

                                        let _ = result_tx.send(TaskResult::GetBuyerInfoResult(
                                            GetBuyerInfoResult {
                                                task_id,
                                                uid,
                                                buyer_info,
                                                success,
                                                message,
                                            },
                                        ));
                                    });
                                }
                                TaskRequest::GenerateQrCodeRequest(req) => {
                                    tokio::spawn(async move {
                                        let (success, qrcode_key, qrcode_url, message) =
                                            match qrcode_login_async(&req.client).await {
                                                Ok((key, url)) => (
                                                    true,
                                                    Some(key),
                                                    Some(url),
                                                    "生成登录二维码成功".to_string(),
                                                ),
                                                Err(e) => {
                                                    log::error!("生成登录二维码失败: {}", e);
                                                    (false, None, None, e)
                                                }
                                            };
                                        let _ = result_tx.send(TaskResult::GenerateQrCodeResult(
                                            GenerateQrCodeResult {
                                                task_id,
                                                success,
                                                qrcode_key,
                                                qrcode_url,
                                                message,
                                            },
                                        ));
                                    });
                                }
                                TaskRequest::AddAccountRequest(req) => {
                                    tokio::spawn(async move {
                                        let (success, account, message) = match add_account_async(
                                            &req.cookie,
                                            &req.client,
                                            &req.user_agent,
                                        )
                                        .await
                                        {
                                            Ok(acc) => (
                                                true,
                                                Some(acc),
                                                "添加账号成功".to_string(),
                                            ),
                                            Err(e) => {
                                                log::error!("添加账号失败: {}", e);
                                                (false, None, e)
                                            }
                                        };
                                        let _ = result_tx.send(TaskResult::AddAccountResult(
                                            AddAccountResult {
                                                task_id,
                                                success,
                                                account,
                                                source: req.source,
                                                message,
                                            },
                                        ));
                                    });
                                }
                                TaskRequest::SignoutAccountRequest(req) => {
                                    tokio::spawn(async move {
                                        let uid = req.account.uid;
                                        let (remote_success, message) =
                                            match signout_account_async(&req.account).await {
                                                Ok(ok) => (
                                                    ok,
                                                    if ok {
                                                        "远程退出登录成功".to_string()
                                                    } else {
                                                        "远程退出登录未确认成功".to_string()
                                                    },
                                                ),
                                                Err(e) => {
                                                    log::warn!(
                                                        "账号 {} 远程退出登录失败: {}",
                                                        uid,
                                                        e
                                                    );
                                                    (false, e)
                                                }
                                            };
                                        let _ = result_tx.send(TaskResult::SignoutAccountResult(
                                            SignoutAccountResult {
                                                task_id,
                                                uid,
                                                remote_success,
                                                message,
                                            },
                                        ));
                                    });
                                }
                                TaskRequest::AddBuyerRequest(req) => {
                                    tokio::spawn(async move {
                                        let (success, message) = match add_buyer(
                                            req.cookie_manager,
                                            &req.name,
                                            &req.phone,
                                            req.id_type,
                                            &req.id_number,
                                            req.as_default_buyer,
                                        )
                                        .await
                                        {
                                            Ok(msg) => (true, msg),
                                            Err(e) => {
                                                log::error!("添加购票人失败: {}", e);
                                                (false, e)
                                            }
                                        };
                                        let _ = result_tx.send(TaskResult::AddBuyerResult(
                                            AddBuyerResult {
                                                task_id,
                                                uid: req.uid,
                                                success,
                                                message,
                                            },
                                        ));
                                    });
                                }
                                TaskRequest::FetchPolicyRequest(req) => {
                                    tokio::spawn(async move {
                                        let timestamp = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_secs();
                                        let body = json!({
                                            "ts": timestamp,
                                            "machine_id": req.machine_id,
                                        });
                                        let url = format!(
                                            "https://policy.nexaorion.cn/api/client/{}/{}/dispatch.json",
                                            req.app, req.version
                                        );

                                        let res = req
                                            .client
                                            .post(&url)
                                            .timeout(Duration::from_secs(15))
                                            .json(&body)
                                            .send()
                                            .await;

                                        let (success, raw_response, error) = match res {
                                            Ok(resp) => {
                                                if !resp.status().is_success() {
                                                    (
                                                        false,
                                                        None,
                                                        Some(format!(
                                                            "策略服务器返回 HTTP {}",
                                                            resp.status()
                                                        )),
                                                    )
                                                } else {
                                                    match resp.json::<serde_json::Value>().await {
                                                        Ok(val) => (true, Some(val), None),
                                                        Err(e) => (
                                                            false,
                                                            None,
                                                            Some(format!("解析策略响应失败: {}", e)),
                                                        ),
                                                    }
                                                }
                                            }
                                            Err(e) => (
                                                false,
                                                None,
                                                Some(format!("请求策略失败: {}", e)),
                                            ),
                                        };

                                        let _ = result_tx.send(TaskResult::FetchPolicyResult(
                                            FetchPolicyResult {
                                                task_id,
                                                success,
                                                raw_response,
                                                error,
                                            },
                                        ));
                                    });
                                }
                                TaskRequest::FetchImageRequest(req) => {
                                    tokio::spawn(async move {
                                        let (bytes, error) =
                                            match common::utils::fetch_image_bytes(&req.url).await {
                                                Ok(b) => (Some(b), None),
                                                Err(e) => (None, Some(e)),
                                            };
                                        let _ = result_tx.send(TaskResult::FetchImageResult(
                                            FetchImageResult {
                                                task_id,
                                                key: req.key,
                                                url: req.url,
                                                bytes,
                                                error,
                                            },
                                        ));
                                    });
                                }
                                TaskRequest::QrCodeLoginRequest(_)
                                | TaskRequest::GrabTicketRequest(_) => {
                                    log::warn!(
                                        "收到未预注册的任务消息类型，忽略 task_id={}",
                                        task_id
                                    );
                                }
                            }
                        }
                        TaskMessage::Shutdown => break,
                    }
                }
            });
            runtime.shutdown_timeout(Duration::from_secs(2));
        });

        Self {
            task_sender: task_tx,
            result_receiver: result_rx,
            grab_registry,
            cancel_tokens,
            recent_tasks,
            live_push_config,
            push_config_updated,
            seq_counter,
            _worker_thread: Some(worker),
        }
    }

    fn submit_task(&mut self, request: TaskRequest) -> Result<String, String> {
        let task_id = uuid::Uuid::new_v4().to_string();

        match request {
            TaskRequest::GrabTicketRequest(mut grab_req) => {
                log::info!("提交抢票任务 ID: {}", task_id);
                grab_req.task_id = task_id.clone();

                let seq = self.seq_counter.fetch_add(1, Ordering::Relaxed) + 1;
                let started_at = Instant::now();
                let cancel_flag = Arc::new(AtomicBool::new(false));
                let (cancel_tx, cancel_rx) = watch::channel(false);

                let account_name = grab_req.biliticket.account.name.clone();
                let project_name = grab_req
                    .biliticket
                    .project_info
                    .as_ref()
                    .and_then(|p| p.name.clone())
                    .unwrap_or_else(|| grab_req.project_id.clone());

                let entry = GrabTaskEntry {
                    info_seed: RunningTaskInfo {
                        task_id: task_id.clone(),
                        seq,
                        uid: grab_req.uid,
                        account_name: account_name.clone(),
                        project_name: project_name.clone(),
                        grab_mode: grab_req.grab_mode,
                        status: TaskStatus::Running,
                        elapsed_secs: 0,
                    },
                    start_time: started_at,
                    cancel_flag: Arc::clone(&cancel_flag),
                };

                // 同步预注册到 grab_registry 与 cancel_tokens，消除 submit -> cancel/list 竞态
                {
                    let mut reg = self
                        .grab_registry
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    reg.insert(task_id.clone(), entry);
                }
                {
                    let mut tokens = self
                        .cancel_tokens
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    tokens.insert(
                        task_id.clone(),
                        CancelTokenEntry {
                            uid: Some(grab_req.uid),
                            cancel_flag: Arc::clone(&cancel_flag),
                            cancel_tx,
                        },
                    );
                }

                if let Err(e) = self.task_sender.send(TaskMessage::SubmitGrabTask {
                    task_id: task_id.clone(),
                    request: grab_req,
                    seq,
                    account_name,
                    project_name,
                    started_at,
                    cancel_flag,
                    cancel_rx,
                }) {
                    // 发送失败时回滚预注册条目
                    self.grab_registry
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(&task_id);
                    self.cancel_tokens
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(&task_id);
                    return Err(format!("无法提交抢票任务: {}", e));
                }

                Ok(task_id)
            }
            TaskRequest::QrCodeLoginRequest(qrcode_req) => {
                log::info!("提交二维码登录任务 ID: {}", task_id);
                let cancel_flag = Arc::new(AtomicBool::new(false));
                let (cancel_tx, cancel_rx) = watch::channel(false);

                {
                    let mut tokens = self
                        .cancel_tokens
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    tokens.insert(
                        task_id.clone(),
                        CancelTokenEntry {
                            uid: None,
                            cancel_flag,
                            cancel_tx,
                        },
                    );
                }

                if let Err(e) = self.task_sender.send(TaskMessage::SubmitQrCodeLogin {
                    task_id: task_id.clone(),
                    request: qrcode_req,
                    cancel_rx,
                }) {
                    self.cancel_tokens
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(&task_id);
                    return Err(format!("无法提交二维码登录任务: {}", e));
                }

                Ok(task_id)
            }
            other => {
                log::debug!("提交异步任务 ID: {}", task_id);
                if let Err(e) = self.task_sender.send(TaskMessage::SubmitTask {
                    task_id: task_id.clone(),
                    request: other,
                }) {
                    return Err(format!("无法提交任务: {}", e));
                }
                Ok(task_id)
            }
        }
    }

    fn get_results(&mut self) -> Vec<TaskResult> {
        let mut results = Vec::new();
        while let Ok(result) = self.result_receiver.try_recv() {
            results.push(result);
        }
        results
    }

    fn cancel_task(&mut self, task_id: &str) -> Result<(), String> {
        let tokens = self
            .cancel_tokens
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match tokens.get(task_id) {
            Some(entry) => {
                entry.cancel_flag.store(true, Ordering::SeqCst);
                let _ = entry.cancel_tx.send(true);
                Ok(())
            }
            None => Err("任务不存在".to_string()),
        }
    }

    fn get_task_status(&self, task_id: &str) -> Option<TaskStatus> {
        {
            let registry = self
                .grab_registry
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(entry) = registry.get(task_id) {
                return Some(if entry.cancel_flag.load(Ordering::Relaxed) {
                    TaskStatus::Cancelling
                } else {
                    TaskStatus::Running
                });
            }
        }
        {
            let tokens = self
                .cancel_tokens
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(entry) = tokens.get(task_id) {
                return Some(if entry.cancel_flag.load(Ordering::Relaxed) {
                    TaskStatus::Cancelling
                } else {
                    TaskStatus::Running
                });
            }
        }
        {
            let recent = self
                .recent_tasks
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(item) = recent.iter().find(|t| t.task_id == task_id) {
                return Some(item.status.clone());
            }
        }
        None
    }

    fn list_running_tasks(&self) -> Vec<RunningTaskInfo> {
        let registry = self
            .grab_registry
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut tasks: Vec<RunningTaskInfo> = registry
            .values()
            .map(|entry| {
                let mut info = entry.info_seed.clone();
                info.elapsed_secs = entry.start_time.elapsed().as_secs();
                if entry.cancel_flag.load(Ordering::Relaxed) {
                    info.status = TaskStatus::Cancelling;
                }
                info
            })
            .collect();
        tasks.sort_by_key(|t| t.seq);
        tasks
    }

    fn list_recent_tasks(&self) -> Vec<RecentTaskInfo> {
        let recent = self
            .recent_tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        recent.iter().cloned().collect()
    }

    fn clear_recent_tasks(&self) {
        let mut recent = self
            .recent_tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        recent.clear();
    }

    fn update_push_config(&self, push_config: PushConfig) {
        let mut cfg = self
            .live_push_config
            .write()
            .unwrap_or_else(|e| e.into_inner());
        *cfg = push_config;
        self.push_config_updated.store(true, Ordering::Release);
    }

    fn cancel_tasks_for_uid(&self, uid: i64) -> usize {
        let tokens = self
            .cancel_tokens
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut count = 0usize;
        for entry in tokens.values() {
            if entry.uid == Some(uid) {
                entry.cancel_flag.store(true, Ordering::SeqCst);
                let _ = entry.cancel_tx.send(true);
                count += 1;
            }
        }
        count
    }

    fn shutdown(&mut self) {
        {
            let tokens = self
                .cancel_tokens
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            for entry in tokens.values() {
                entry.cancel_flag.store(true, Ordering::SeqCst);
                let _ = entry.cancel_tx.send(true);
            }
        }
        let _ = self.task_sender.send(TaskMessage::Shutdown);
        if let Some(handle) = self._worker_thread.take() {
            let _ = handle.join();
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_grab_ticket_task(
    task_id: String,
    grab_ticket_req: GrabTicketRequest,
    account_name: String,
    result_tx: mpsc::UnboundedSender<TaskResult>,
    cancel_flag: Arc<AtomicBool>,
    cancel_rx: &mut watch::Receiver<bool>,
    live_push_config: Arc<RwLock<PushConfig>>,
    push_config_updated: Arc<AtomicBool>,
    final_outcome: Arc<Mutex<Option<FinalTaskOutcome>>>,
) {
    let project_id = grab_ticket_req.project_id.clone();
    let screen_id = grab_ticket_req.screen_id.clone();
    let ticket_id = grab_ticket_req.ticket_id.clone();
    let buyer_info = grab_ticket_req.buyer_info.clone();
    let cookie_manager = Arc::clone(&grab_ticket_req.cookie_manager);
    let uid = grab_ticket_req.uid;
    let mode = grab_ticket_req.grab_mode;
    let custom_config = grab_ticket_req.biliticket.config.clone();
    let csrf = grab_ticket_req.biliticket.account.csrf.clone();
    let local_captcha = grab_ticket_req.local_captcha.clone();
    let count = grab_ticket_req.count;
    let project_info = grab_ticket_req.biliticket.project_info.clone();
    let skip_words = grab_ticket_req.skip_words.clone();
    let is_hot = grab_ticket_req.is_hot;
    let referer_link = grab_ticket_req.biliticket.referer.clone();

    log::debug!("开始分析抢票任务：{}", task_id);

    match mode {
        0 => {
            log::debug!("定时抢票模式");
            let sale_begin_opt = project_info.as_ref().and_then(|p| p.sale_begin);

            let initial_countdown = match tokio::select! {
                res = get_countdown(Arc::clone(&cookie_manager), project_info.clone()) => res,
                _ = wait_for_cancel(&cancel_flag, cancel_rx) => {
                    log::info!("获取倒计时期间任务已取消");
                    set_final_outcome(&final_outcome, TaskStatus::Cancelled, "任务已取消");
                    return;
                }
            } {
                Ok(c) => c,
                Err(e) => {
                    let msg = format!("获取倒计时失败: {}", e);
                    log::error!("{}", msg);
                    emit_grab_failure(&result_tx, &task_id, uid, &project_id, &account_name, &msg);
                    set_final_outcome(&final_outcome, TaskStatus::Failed(msg.clone()), msg);
                    return;
                }
            };

            if initial_countdown > 0.0 {
                log::info!("距离开售时间还有 {:.1} 秒", initial_countdown);
                let mut deadline = tokio::time::Instant::now()
                    + Duration::from_secs_f64(initial_countdown.max(0.0));
                let mut resynced = initial_countdown <= 45.0;
                let mut last_log_secs: Option<u64> = None;

                loop {
                    if is_cancelled(&cancel_flag, cancel_rx) {
                        log::info!("倒计时期间任务已取消");
                        set_final_outcome(&final_outcome, TaskStatus::Cancelled, "任务已取消");
                        return;
                    }

                    let remaining =
                        deadline.saturating_duration_since(tokio::time::Instant::now());
                    if remaining.is_zero() {
                        break;
                    }

                    // 初始倒计时超过 45 秒时，在 T-30 秒附近执行一次服务器时间重校准，消除时钟漂移
                    if !resynced && remaining <= Duration::from_secs(30) {
                        resynced = true;
                        if let Some(sale_begin_sec) = sale_begin_opt {
                            log::info!("距开售约 30 秒，正在向 B站服务器重校准时间...");
                            let sync_res = tokio::select! {
                                res = fetch_server_now_sec(Arc::clone(&cookie_manager)) => res,
                                _ = wait_for_cancel(&cancel_flag, cancel_rx) => {
                                    log::info!("校准时间期间任务已取消");
                                    set_final_outcome(&final_outcome, TaskStatus::Cancelled, "任务已取消");
                                    return;
                                }
                            };
                            match sync_res {
                                Ok(server_now) => {
                                    let synced_secs = (sale_begin_sec - server_now) as f64;
                                    let clamped = synced_secs.max(0.0);
                                    deadline = tokio::time::Instant::now()
                                        + Duration::from_secs_f64(clamped);
                                    log::info!("服务器时间重校准完成，剩余 {:.1} 秒", clamped);
                                    continue;
                                }
                                Err(e) => {
                                    log::warn!("T-30s 重校准时间失败({}), 继续沿用原倒计时", e);
                                }
                            }
                        }
                    }

                    let rem_secs = remaining.as_secs();
                    let should_log = if rem_secs <= 15 {
                        last_log_secs != Some(rem_secs)
                    } else {
                        last_log_secs.map_or(true, |prev| prev.saturating_sub(rem_secs) >= 15)
                    };
                    if should_log {
                        log::info!("距离抢票时间还有 {} 秒", rem_secs);
                        last_log_secs = Some(rem_secs);
                    }

                    let slice = remaining.min(Duration::from_millis(250));
                    if cancellable_sleep(slice, &cancel_flag, cancel_rx).await {
                        log::info!("倒计时期间任务已取消");
                        set_final_outcome(&final_outcome, TaskStatus::Cancelled, "任务已取消");
                        return;
                    }
                }
            }

            if is_cancelled(&cancel_flag, cancel_rx) {
                log::info!("开售前任务已取消");
                set_final_outcome(&final_outcome, TaskStatus::Cancelled, "任务已取消");
                return;
            }

            log::info!("开始抢票！");
            // 倒计时结束后立即构造 CTokenGenerator，确保 when_gen 贴近开抢时刻
            let cpdd = build_ctoken_generator(
                sale_begin_opt,
                Some(cookie_manager.get_ua()).filter(|s| !s.is_empty()),
            );

            run_direct_grab_loop(
                "定时抢票模式",
                5,
                cookie_manager,
                cpdd,
                &project_id,
                &screen_id,
                &ticket_id,
                count,
                is_hot,
                &task_id,
                uid,
                &account_name,
                &csrf,
                &custom_config,
                local_captcha,
                &grab_ticket_req,
                &buyer_info,
                &result_tx,
                &cancel_flag,
                cancel_rx,
                &live_push_config,
                &push_config_updated,
                &final_outcome,
            )
            .await;
        }
        1 => {
            log::debug!("直接抢票模式");
            let sale_begin_opt = project_info.as_ref().and_then(|p| p.sale_begin);
            let cpdd = build_ctoken_generator(
                sale_begin_opt,
                Some(cookie_manager.get_ua()).filter(|s| !s.is_empty()),
            );

            run_direct_grab_loop(
                "直接抢票模式",
                10,
                cookie_manager,
                cpdd,
                &project_id,
                &screen_id,
                &ticket_id,
                count,
                is_hot,
                &task_id,
                uid,
                &account_name,
                &csrf,
                &custom_config,
                local_captcha,
                &grab_ticket_req,
                &buyer_info,
                &result_tx,
                &cancel_flag,
                cancel_rx,
                &live_push_config,
                &push_config_updated,
                &final_outcome,
            )
            .await;
        }
        2 => {
            log::debug!("捡漏模式");
            let mut local_grab_request = grab_ticket_req.clone();
            let skip_words_lower: Vec<String> = skip_words
                .unwrap_or_default()
                .into_iter()
                .map(|w| w.trim().to_lowercase())
                .filter(|w| !w.is_empty())
                .collect();
            let poll_interval =
                Duration::from_millis(custom_config.grab_interval_ms.clamp(500, 30000));
            let referer_opt = {
                let trimmed = referer_link.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            };

            let mut consecutive_token_failures: usize = 0;
            const MAX_CONSECUTIVE_TOKEN_FAILURES: usize = 10;
            const MAX_TICKET_TOKEN_REFRESH: usize = 3;

            'main_loop: loop {
                if is_cancelled(&cancel_flag, cancel_rx) {
                    log::info!("任务已取消，退出捡漏模式");
                    set_final_outcome(&final_outcome, TaskStatus::Cancelled, "任务已取消");
                    return;
                }

                log::debug!(
                    "捡漏轮询 project_id={}, screen_id={}, ticket_id={}",
                    project_id,
                    screen_id,
                    ticket_id
                );

                // 捡漏模式禁用本地缓存回退与磁盘写入，确保实时库存且不频繁刷盘
                let project_res = tokio::select! {
                    res = get_project_with_options(
                        Arc::clone(&cookie_manager),
                        &project_id,
                        referer_opt.clone(),
                        false,
                    ) => res,
                    _ = wait_for_cancel(&cancel_flag, cancel_rx) => {
                        log::info!("轮询项目期间任务已取消，退出捡漏模式");
                        set_final_outcome(&final_outcome, TaskStatus::Cancelled, "任务已取消");
                        return;
                    }
                };

                let project_data = match project_res {
                    Ok(data) => data,
                    Err(e) => {
                        log::error!("捡漏获取实时项目数据失败，原因：{}", e);
                        if cancellable_sleep(poll_interval, &cancel_flag, cancel_rx).await {
                            log::info!("任务已取消，退出捡漏模式");
                            set_final_outcome(&final_outcome, TaskStatus::Cancelled, "任务已取消");
                            return;
                        }
                        continue 'main_loop;
                    }
                };

                let current_is_hot = project_data.data.hot_project.unwrap_or(false);
                let id_bind = project_data.data.id_bind.unwrap_or(0);
                if ![1, 2].contains(&id_bind) {
                    let msg = "暂不支持非实名票捡漏模式".to_string();
                    log::error!("{}", msg);
                    emit_grab_failure(&result_tx, &task_id, uid, &project_id, &account_name, &msg);
                    set_final_outcome(&final_outcome, TaskStatus::Failed(msg.clone()), msg);
                    break 'main_loop;
                }
                local_grab_request.biliticket.id_bind = id_bind;

                'screen_loop: for screen_data in project_data.data.screen_list {
                    if is_cancelled(&cancel_flag, cancel_rx) {
                        log::info!("任务已取消，退出捡漏模式");
                        set_final_outcome(&final_outcome, TaskStatus::Cancelled, "任务已取消");
                        return;
                    }
                    if !screen_data.clickable {
                        continue 'screen_loop;
                    }

                    // 场次级关键词过滤
                    let screen_name_raw = screen_data.name.as_deref().unwrap_or("");
                    if should_skip_by_words(screen_name_raw, &skip_words_lower) {
                        log::info!("跳过包含过滤关键词的场次: {}", screen_name_raw);
                        continue 'screen_loop;
                    }

                    let current_screen_id = screen_data.id.unwrap_or(0).to_string();
                    local_grab_request.screen_id = current_screen_id.clone();
                    local_grab_request.biliticket.screen_id = current_screen_id;
                    log::info!(
                        "当前项目场次 [{}] 有可抢票种，开始检测！",
                        if screen_name_raw.is_empty() {
                            &local_grab_request.screen_id
                        } else {
                            screen_name_raw
                        }
                    );

                    'ticket_loop: for ticket_data in screen_data.ticket_list {
                        if is_cancelled(&cancel_flag, cancel_rx) {
                            log::info!("任务已取消，退出捡漏模式");
                            set_final_outcome(&final_outcome, TaskStatus::Cancelled, "任务已取消");
                            return;
                        }
                        if !ticket_data.clickable.unwrap_or(false) {
                            continue 'ticket_loop;
                        }

                        // 票种附带场次名与票种描述关键词过滤
                        let ticket_screen_name = ticket_data.screen_name.as_deref().unwrap_or("");
                        if should_skip_by_words(ticket_screen_name, &skip_words_lower) {
                            log::info!("跳过包含过滤关键词的场次: {}", ticket_screen_name);
                            continue 'ticket_loop;
                        }
                        let ticket_desc = ticket_data.desc.as_deref().unwrap_or("未知票种");
                        if should_skip_by_words(ticket_desc, &skip_words_lower) {
                            log::info!("跳过包含过滤关键词的票种: {}", ticket_desc);
                            continue 'ticket_loop;
                        }

                        log::info!(
                            "当前 {} {} 票种可售，开始抢票！",
                            ticket_data.screen_name.as_deref().unwrap_or(screen_name_raw),
                            ticket_desc
                        );
                        let current_ticket_id = ticket_data.id.unwrap_or(0).to_string();
                        local_grab_request.ticket_id = current_ticket_id.clone();
                        local_grab_request.biliticket.select_ticket_id = Some(current_ticket_id);

                        let cpdd = build_ctoken_generator(
                            project_data.data.sale_begin,
                            Some(cookie_manager.get_ua()).filter(|s| !s.is_empty()),
                        );

                        let mut ticket_token_attempts: usize = 0;
                        'ticket_token_loop: loop {
                            if is_cancelled(&cancel_flag, cancel_rx) {
                                log::info!("任务已取消，退出捡漏模式");
                                set_final_outcome(
                                    &final_outcome,
                                    TaskStatus::Cancelled,
                                    "任务已取消",
                                );
                                return;
                            }

                            let token_result = tokio::select! {
                                res = get_ticket_token(
                                    Arc::clone(&cookie_manager),
                                    Arc::clone(&cpdd),
                                    &project_id,
                                    &local_grab_request.screen_id,
                                    &local_grab_request.ticket_id,
                                    count,
                                    current_is_hot,
                                ) => res,
                                _ = wait_for_cancel(&cancel_flag, cancel_rx) => {
                                    log::info!("获取token期间任务已取消，退出捡漏模式");
                                    set_final_outcome(&final_outcome, TaskStatus::Cancelled, "任务已取消");
                                    return;
                                }
                            };

                            match token_result {
                                Ok((token, ptoken)) => {
                                    consecutive_token_failures = 0;
                                    log::info!("获取抢票token成功！");
                                    log::debug!(
                                        "token长度={}, ptoken长度={}",
                                        token.len(),
                                        ptoken.len()
                                    );

                                    let mut confirm_retry_count: usize = 0;

                                    'confirm_loop: loop {
                                        if is_cancelled(&cancel_flag, cancel_rx) {
                                            log::info!("任务已取消，退出捡漏模式");
                                            set_final_outcome(
                                                &final_outcome,
                                                TaskStatus::Cancelled,
                                                "任务已取消",
                                            );
                                            return;
                                        }

                                        match handle_grab_ticket(
                                            Arc::clone(&cookie_manager),
                                            Arc::clone(&cpdd),
                                            &project_id,
                                            &token,
                                            &ptoken,
                                            current_is_hot,
                                            &task_id,
                                            uid,
                                            &account_name,
                                            &result_tx,
                                            &local_grab_request,
                                            &buyer_info,
                                            &cancel_flag,
                                            cancel_rx,
                                            &live_push_config,
                                            &push_config_updated,
                                        )
                                        .await
                                        {
                                            GrabOutcome::Success => {
                                                log::info!("抢票成功，退出捡漏模式");
                                                set_final_outcome(
                                                    &final_outcome,
                                                    TaskStatus::Completed(true),
                                                    "抢票成功",
                                                );
                                                break 'main_loop;
                                            }
                                            GrabOutcome::Cancelled => {
                                                log::info!("任务已取消，退出捡漏模式");
                                                set_final_outcome(
                                                    &final_outcome,
                                                    TaskStatus::Cancelled,
                                                    "任务已取消",
                                                );
                                                return;
                                            }
                                            GrabOutcome::FatalError(reason) => {
                                                log::error!("遇到致命错误，退出捡漏模式: {}", reason);
                                                emit_grab_failure(
                                                    &result_tx,
                                                    &task_id,
                                                    uid,
                                                    &project_id,
                                                    &account_name,
                                                    &reason,
                                                );
                                                set_final_outcome(
                                                    &final_outcome,
                                                    TaskStatus::Failed(reason.clone()),
                                                    reason,
                                                );
                                                break 'main_loop;
                                            }
                                            GrabOutcome::NeedRefreshToken => {
                                                log::warn!("token已失效，重新获取当前票种token...");
                                                break 'confirm_loop;
                                            }
                                            GrabOutcome::RetryLimitReached => {
                                                log::info!("该票种已达最大重试次数，继续尝试同场次及其他场次票种");
                                                break 'ticket_token_loop;
                                            }
                                            GrabOutcome::ConfirmFailed => {
                                                confirm_retry_count += 1;
                                                if confirm_retry_count >= MAX_CONFIRM_RETRY {
                                                    log::error!("确认订单失败已达最大重试次数，尝试其他票种");
                                                    break 'ticket_token_loop;
                                                }
                                                if cancellable_sleep(
                                                    Duration::from_millis(300),
                                                    &cancel_flag,
                                                    cancel_rx,
                                                )
                                                .await
                                                {
                                                    log::info!("任务已取消，退出捡漏模式");
                                                    set_final_outcome(
                                                        &final_outcome,
                                                        TaskStatus::Cancelled,
                                                        "任务已取消",
                                                    );
                                                    return;
                                                }
                                            }
                                        }
                                    }

                                    ticket_token_attempts += 1;
                                    if ticket_token_attempts >= MAX_TICKET_TOKEN_REFRESH {
                                        log::warn!("当前票种刷新token达上限，尝试其他票种");
                                        break 'ticket_token_loop;
                                    }
                                    if cancellable_sleep(
                                        Duration::from_millis(300),
                                        &cancel_flag,
                                        cancel_rx,
                                    )
                                    .await
                                    {
                                        log::info!("任务已取消，退出捡漏模式");
                                        set_final_outcome(
                                            &final_outcome,
                                            TaskStatus::Cancelled,
                                            "任务已取消",
                                        );
                                        return;
                                    }
                                    continue 'ticket_token_loop;
                                }
                                Err(risk_param) => {
                                    if has_actionable_risk_verification(&risk_param) {
                                        if is_cancelled(&cancel_flag, cancel_rx) {
                                            log::info!("任务已取消，退出捡漏模式");
                                            set_final_outcome(
                                                &final_outcome,
                                                TaskStatus::Cancelled,
                                                "任务已取消",
                                            );
                                            return;
                                        }
                                        log::warn!("需要验证码，开始处理验证码...");
                                        let verify_res = tokio::select! {
                                            res = handle_risk_verification(
                                                Arc::clone(&cookie_manager),
                                                risk_param,
                                                &custom_config,
                                                &csrf,
                                                local_captcha.clone(),
                                            ) => res,
                                            _ = wait_for_cancel(&cancel_flag, cancel_rx) => {
                                                log::info!("验证码处理期间任务已取消，退出捡漏模式");
                                                set_final_outcome(&final_outcome, TaskStatus::Cancelled, "任务已取消");
                                                return;
                                            }
                                        };
                                        match verify_res {
                                            Ok(()) => {
                                                log::info!("验证码处理成功！");
                                                ticket_token_attempts += 1;
                                                if ticket_token_attempts >= MAX_TICKET_TOKEN_REFRESH {
                                                    break 'ticket_token_loop;
                                                }
                                                continue 'ticket_token_loop;
                                            }
                                            Err(e) => {
                                                log::error!("验证码处理失败: {}", e);
                                                consecutive_token_failures += 1;
                                                if consecutive_token_failures
                                                    >= MAX_CONSECUTIVE_TOKEN_FAILURES
                                                {
                                                    let msg = format!(
                                                        "验证码处理连续失败达到上限({}): {}",
                                                        MAX_CONSECUTIVE_TOKEN_FAILURES, e
                                                    );
                                                    emit_grab_failure(
                                                        &result_tx,
                                                        &task_id,
                                                        uid,
                                                        &project_id,
                                                        &account_name,
                                                        &msg,
                                                    );
                                                    set_final_outcome(
                                                        &final_outcome,
                                                        TaskStatus::Failed(msg.clone()),
                                                        msg,
                                                    );
                                                    break 'main_loop;
                                                }
                                            }
                                        }
                                    } else {
                                        let reason = match risk_param.code {
                                            100080 | 100082 => {
                                                "场次/项目/日期选择有误".to_string()
                                            }
                                            100039 => "该场次已停售".to_string(),
                                            _ => format!(
                                                "获取token失败，错误代码: {}，错误信息：{}",
                                                risk_param.code, risk_param.message
                                            ),
                                        };
                                        if is_transient_mode2_token_error(risk_param.code as i64) {
                                            log::warn!(
                                                "捡漏获取token遇到瞬时/余票波动错误，不计入连续失败：{}",
                                                reason
                                            );
                                        } else {
                                            log::error!("捡漏获取token失败：{}", reason);
                                            consecutive_token_failures += 1;
                                            if consecutive_token_failures
                                                >= MAX_CONSECUTIVE_TOKEN_FAILURES
                                            {
                                                emit_grab_failure(
                                                    &result_tx,
                                                    &task_id,
                                                    uid,
                                                    &project_id,
                                                    &account_name,
                                                    &reason,
                                                );
                                                set_final_outcome(
                                                    &final_outcome,
                                                    TaskStatus::Failed(reason.clone()),
                                                    reason,
                                                );
                                                break 'main_loop;
                                            }
                                        }
                                    }

                                    if cancellable_sleep(
                                        Duration::from_secs(1),
                                        &cancel_flag,
                                        cancel_rx,
                                    )
                                    .await
                                    {
                                        log::info!("任务已取消，退出捡漏模式");
                                        set_final_outcome(
                                            &final_outcome,
                                            TaskStatus::Cancelled,
                                            "任务已取消",
                                        );
                                        return;
                                    }
                                    break 'ticket_token_loop;
                                }
                            }
                        }
                    }
                }

                log::info!(
                    "所有场次和票种检查完毕，等待 {}ms 后重新检查",
                    poll_interval.as_millis()
                );
                if cancellable_sleep(poll_interval, &cancel_flag, cancel_rx).await {
                    log::info!("任务已取消，退出捡漏模式");
                    set_final_outcome(&final_outcome, TaskStatus::Cancelled, "任务已取消");
                    return;
                }
            }

            log::info!("捡漏模式任务已退出");
        }
        _ => {
            let msg = format!("未知抢票模式: {}", mode);
            log::error!("{}", msg);
            emit_grab_failure(&result_tx, &task_id, uid, &project_id, &account_name, &msg);
            set_final_outcome(&final_outcome, TaskStatus::Failed(msg.clone()), msg);
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_direct_grab_loop(
    mode_name: &str,
    max_token_retry: usize,
    cookie_manager: Arc<CookieManager>,
    cpdd: Arc<Mutex<CTokenGenerator>>,
    project_id: &str,
    screen_id: &str,
    ticket_id: &str,
    count: i16,
    is_hot: bool,
    task_id: &str,
    uid: i64,
    account_name: &str,
    csrf: &str,
    custom_config: &common::utility::CustomConfig,
    local_captcha: common::captcha::LocalCaptcha,
    grab_ticket_req: &GrabTicketRequest,
    buyer_info: &Vec<BuyerInfo>,
    result_tx: &mpsc::UnboundedSender<TaskResult>,
    cancel_flag: &Arc<AtomicBool>,
    cancel_rx: &mut watch::Receiver<bool>,
    live_push_config: &Arc<RwLock<PushConfig>>,
    push_config_updated: &Arc<AtomicBool>,
    final_outcome: &Arc<Mutex<Option<FinalTaskOutcome>>>,
) {
    let mut token_retry_count: usize = 0;
    let mut consecutive_captcha_ok: usize = 0;
    const MAX_CONSECUTIVE_CAPTCHA_RETRY: usize = 5;

    'token_loop: loop {
        if is_cancelled(cancel_flag, cancel_rx) {
            log::info!("任务已取消，退出{}", mode_name);
            set_final_outcome(final_outcome, TaskStatus::Cancelled, "任务已取消");
            return;
        }

        let token_result = tokio::select! {
            res = get_ticket_token(
                Arc::clone(&cookie_manager),
                Arc::clone(&cpdd),
                project_id,
                screen_id,
                ticket_id,
                count,
                is_hot,
            ) => res,
            _ = wait_for_cancel(cancel_flag, cancel_rx) => {
                log::info!("获取token过程中任务已取消，退出{}", mode_name);
                set_final_outcome(final_outcome, TaskStatus::Cancelled, "任务已取消");
                return;
            }
        };

        match token_result {
            Ok((token, ptoken)) => {
                consecutive_captcha_ok = 0;
                log::info!("获取抢票token成功！");
                log::debug!("token长度={}, ptoken长度={}", token.len(), ptoken.len());

                let mut confirm_retry_count: usize = 0;

                'confirm_loop: loop {
                    if is_cancelled(cancel_flag, cancel_rx) {
                        log::info!("任务已取消，退出{}", mode_name);
                        set_final_outcome(final_outcome, TaskStatus::Cancelled, "任务已取消");
                        return;
                    }

                    match handle_grab_ticket(
                        Arc::clone(&cookie_manager),
                        Arc::clone(&cpdd),
                        project_id,
                        &token,
                        &ptoken,
                        is_hot,
                        task_id,
                        uid,
                        account_name,
                        result_tx,
                        grab_ticket_req,
                        buyer_info,
                        cancel_flag,
                        cancel_rx,
                        live_push_config,
                        push_config_updated,
                    )
                    .await
                    {
                        GrabOutcome::Success => {
                            log::info!("抢票成功，退出{}", mode_name);
                            set_final_outcome(
                                final_outcome,
                                TaskStatus::Completed(true),
                                "抢票成功",
                            );
                            return;
                        }
                        GrabOutcome::Cancelled => {
                            log::info!("任务已取消，退出{}", mode_name);
                            set_final_outcome(final_outcome, TaskStatus::Cancelled, "任务已取消");
                            return;
                        }
                        GrabOutcome::FatalError(reason) => {
                            log::error!("遇到致命错误，退出{}: {}", mode_name, reason);
                            emit_grab_failure(
                                result_tx,
                                task_id,
                                uid,
                                project_id,
                                account_name,
                                &reason,
                            );
                            set_final_outcome(
                                final_outcome,
                                TaskStatus::Failed(reason.clone()),
                                reason,
                            );
                            return;
                        }
                        GrabOutcome::RetryLimitReached => {
                            let max_retry = custom_config.max_retry_count.clamp(10, 200);
                            let msg = format!("下单失败，已达最大重试次数({})", max_retry);
                            log::error!("{}，退出{}", msg, mode_name);
                            emit_grab_failure(
                                result_tx,
                                task_id,
                                uid,
                                project_id,
                                account_name,
                                &msg,
                            );
                            set_final_outcome(
                                final_outcome,
                                TaskStatus::Failed(msg.clone()),
                                msg,
                            );
                            return;
                        }
                        GrabOutcome::NeedRefreshToken => {
                            log::warn!("token已失效，准备重新获取token...");
                            break 'confirm_loop;
                        }
                        GrabOutcome::ConfirmFailed => {
                            confirm_retry_count += 1;
                            if confirm_retry_count >= MAX_CONFIRM_RETRY {
                                log::warn!(
                                    "确认订单连续失败 {} 次，尝试重新获取token...",
                                    confirm_retry_count
                                );
                                break 'confirm_loop;
                            }
                            if cancellable_sleep(
                                Duration::from_millis(300),
                                cancel_flag,
                                cancel_rx,
                            )
                            .await
                            {
                                log::info!("任务已取消，退出{}", mode_name);
                                set_final_outcome(
                                    final_outcome,
                                    TaskStatus::Cancelled,
                                    "任务已取消",
                                );
                                return;
                            }
                        }
                    }
                }

                token_retry_count += 1;
                if token_retry_count >= max_token_retry {
                    let msg = format!(
                        "token刷新/确认订单重试已达上限({})，退出{}",
                        max_token_retry, mode_name
                    );
                    log::error!("{}", msg);
                    emit_grab_failure(result_tx, task_id, uid, project_id, account_name, &msg);
                    set_final_outcome(final_outcome, TaskStatus::Failed(msg.clone()), msg);
                    return;
                }
                if cancellable_sleep(Duration::from_millis(300), cancel_flag, cancel_rx).await {
                    log::info!("任务已取消，退出{}", mode_name);
                    set_final_outcome(final_outcome, TaskStatus::Cancelled, "任务已取消");
                    return;
                }
                continue 'token_loop;
            }
            Err(risk_param) => {
                if has_actionable_risk_verification(&risk_param) {
                    if is_cancelled(cancel_flag, cancel_rx) {
                        log::info!("任务已取消，退出{}", mode_name);
                        set_final_outcome(final_outcome, TaskStatus::Cancelled, "任务已取消");
                        return;
                    }
                    log::warn!("需要验证码，开始处理验证码...");
                    let verify_res = tokio::select! {
                        res = handle_risk_verification(
                            Arc::clone(&cookie_manager),
                            risk_param,
                            custom_config,
                            csrf,
                            local_captcha.clone(),
                        ) => res,
                        _ = wait_for_cancel(cancel_flag, cancel_rx) => {
                            log::info!("验证码处理期间任务已取消，退出{}", mode_name);
                            set_final_outcome(final_outcome, TaskStatus::Cancelled, "任务已取消");
                            return;
                        }
                    };
                    match verify_res {
                        Ok(()) => {
                            consecutive_captcha_ok += 1;
                            log::info!(
                                "验证码处理成功 ({}/{})",
                                consecutive_captcha_ok,
                                MAX_CONSECUTIVE_CAPTCHA_RETRY
                            );
                            if consecutive_captcha_ok >= MAX_CONSECUTIVE_CAPTCHA_RETRY {
                                let msg = format!(
                                    "连续触发人机验证超过上限({})仍无法获取token，已停止重试",
                                    MAX_CONSECUTIVE_CAPTCHA_RETRY
                                );
                                log::error!("{}", msg);
                                emit_grab_failure(
                                    result_tx,
                                    task_id,
                                    uid,
                                    project_id,
                                    account_name,
                                    &msg,
                                );
                                set_final_outcome(
                                    final_outcome,
                                    TaskStatus::Failed(msg.clone()),
                                    msg,
                                );
                                return;
                            }
                            if cancellable_sleep(
                                Duration::from_millis(300),
                                cancel_flag,
                                cancel_rx,
                            )
                            .await
                            {
                                log::info!("任务已取消，退出{}", mode_name);
                                set_final_outcome(
                                    final_outcome,
                                    TaskStatus::Cancelled,
                                    "任务已取消",
                                );
                                return;
                            }
                            continue 'token_loop;
                        }
                        Err(e) => {
                            log::error!("验证码处理失败: {}", e);
                            token_retry_count += 1;
                            if token_retry_count >= max_token_retry {
                                let msg = format!("验证码处理失败，已达最大重试次数: {}", e);
                                emit_grab_failure(
                                    result_tx,
                                    task_id,
                                    uid,
                                    project_id,
                                    account_name,
                                    &msg,
                                );
                                set_final_outcome(
                                    final_outcome,
                                    TaskStatus::Failed(msg.clone()),
                                    msg,
                                );
                                return;
                            }
                        }
                    }
                } else {
                    let (is_fatal, reason) = match risk_param.code {
                        100080 | 100082 => (
                            true,
                            "抢票失败，场次/项目/日期选择有误，请重新提交任务".to_string(),
                        ),
                        100039 => (true, "抢票失败，该场次已停售，请重新提交任务".to_string()),
                        ERR_INVALID_PARAM => (true, format!("参数错误: {}", risk_param.message)),
                        _ => (
                            false,
                            format!(
                                "获取token失败，错误代码: {}，错误信息：{}",
                                risk_param.code, risk_param.message
                            ),
                        ),
                    };
                    log::error!("{}", reason);
                    if is_fatal {
                        emit_grab_failure(
                            result_tx,
                            task_id,
                            uid,
                            project_id,
                            account_name,
                            &reason,
                        );
                        set_final_outcome(
                            final_outcome,
                            TaskStatus::Failed(reason.clone()),
                            reason,
                        );
                        return;
                    }

                    token_retry_count += 1;
                    if token_retry_count >= max_token_retry {
                        emit_grab_failure(
                            result_tx,
                            task_id,
                            uid,
                            project_id,
                            account_name,
                            &reason,
                        );
                        set_final_outcome(
                            final_outcome,
                            TaskStatus::Failed(reason.clone()),
                            reason,
                        );
                        return;
                    }
                }

                if cancellable_sleep(Duration::from_secs(1), cancel_flag, cancel_rx).await {
                    log::info!("任务已取消，退出{}", mode_name);
                    set_final_outcome(final_outcome, TaskStatus::Cancelled, "任务已取消");
                    return;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_grab_ticket(
    cookie_manager: Arc<CookieManager>,
    cpdd: Arc<Mutex<CTokenGenerator>>,
    project_id: &str,
    token: &str,
    ptoken: &str,
    is_hot: bool,
    task_id: &str,
    uid: i64,
    account_name: &str,
    result_tx: &mpsc::UnboundedSender<TaskResult>,
    grab_ticket_req: &GrabTicketRequest,
    buyer_info: &Vec<BuyerInfo>,
    cancel_flag: &Arc<AtomicBool>,
    cancel_rx: &mut watch::Receiver<bool>,
    live_push_config: &Arc<RwLock<PushConfig>>,
    push_config_updated: &Arc<AtomicBool>,
) -> GrabOutcome {
    if is_cancelled(cancel_flag, cancel_rx) {
        return GrabOutcome::Cancelled;
    }

    let confirm_res = tokio::select! {
        res = confirm_ticket_order(Arc::clone(&cookie_manager), project_id, token) => res,
        _ = wait_for_cancel(cancel_flag, cancel_rx) => {
            log::info!("确认订单期间任务已取消");
            return GrabOutcome::Cancelled;
        }
    };

    match confirm_res {
        Ok(confirm_result) => {
            log::info!("确认订单成功！准备下单");
            try_create_order(
                cookie_manager,
                cpdd,
                project_id,
                token,
                ptoken,
                &confirm_result,
                is_hot,
                grab_ticket_req,
                buyer_info,
                task_id,
                uid,
                account_name,
                result_tx,
                cancel_flag,
                cancel_rx,
                live_push_config,
                push_config_updated,
            )
            .await
        }
        Err(e) => {
            log::error!("确认订单失败，原因：{}，正在重试...", e);
            GrabOutcome::ConfirmFailed
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn try_create_order(
    cookie_manager: Arc<CookieManager>,
    cpdd: Arc<Mutex<CTokenGenerator>>,
    project_id: &str,
    token: &str,
    ptoken: &str,
    confirm_result: &ConfirmTicketResult,
    is_hot: bool,
    grab_ticket_req: &GrabTicketRequest,
    buyer_info: &Vec<BuyerInfo>,
    task_id: &str,
    uid: i64,
    account_name: &str,
    result_tx: &mpsc::UnboundedSender<TaskResult>,
    cancel_flag: &Arc<AtomicBool>,
    cancel_rx: &mut watch::Receiver<bool>,
    live_push_config: &Arc<RwLock<PushConfig>>,
    push_config_updated: &Arc<AtomicBool>,
) -> GrabOutcome {
    let base_max_retry = grab_ticket_req
        .biliticket
        .config
        .max_retry_count
        .clamp(10, 200);
    let max_order_retry: u32 = if grab_ticket_req.grab_mode == 2 {
        (base_max_retry / 2).max(10)
    } else {
        base_max_retry
    };
    let order_retry_delay = Duration::from_millis(
        grab_ticket_req
            .biliticket
            .config
            .order_retry_delay_ms
            .clamp(200, 5000),
    );

    let mut order_retry_count: u32 = 0;
    let mut need_retry = false;

    loop {
        if is_cancelled(cancel_flag, cancel_rx) {
            log::info!("下单任务已取消");
            return GrabOutcome::Cancelled;
        }
        if order_retry_count >= 3 {
            need_retry = true;
        }

        // 注意：create_order 一旦发出不中途 select! 取消，避免已创建订单丢失
        let retry_sleep = match create_order(
            Arc::clone(&cookie_manager),
            Arc::clone(&cpdd),
            project_id,
            token,
            ptoken,
            confirm_result,
            is_hot,
            &grab_ticket_req.biliticket,
            buyer_info,
            true,
            need_retry,
            false,
            None,
        )
        .await
        {
            Ok(order_result) => {
                // CRITICAL SAFETY: 一旦 create_order 返回 Ok，必须完成核验与成功上报，绝不因取消而丢单！
                let order_id_opt = extract_order_id(&order_result);
                let order_id_str = order_id_opt.clone().unwrap_or_default();
                let pay_token = order_result
                    .get("data")
                    .and_then(|d| d.get("token"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                log::info!(
                    "下单接口返回成功（订单号: {}），正在核验订单状态...",
                    if order_id_str.is_empty() {
                        "(待生成)"
                    } else {
                        &order_id_str
                    }
                );

                let mut verified_pay_result: Option<CheckFakeResultData> = None;
                let mut last_check_err = String::new();

                for poll in 1..=MAX_FAKE_CHECK_POLLS {
                    match check_fake_ticket(
                        Arc::clone(&cookie_manager),
                        project_id,
                        pay_token,
                        &order_id_str,
                    )
                    .await
                    {
                        Ok(check_result) => match extract_verified_pay_param(&check_result) {
                            Ok(pay_param) => {
                                verified_pay_result = Some(pay_param);
                                break;
                            }
                            Err(err_msg) => {
                                last_check_err = err_msg;
                                log::warn!(
                                    "订单核验第 {}/{} 次未通过({})，订单号: {}",
                                    poll,
                                    MAX_FAKE_CHECK_POLLS,
                                    last_check_err,
                                    order_id_str
                                );
                            }
                        },
                        Err(e) => {
                            last_check_err = e.clone();
                            log::warn!(
                                "订单核验第 {}/{} 次请求失败: {}",
                                poll,
                                MAX_FAKE_CHECK_POLLS,
                                e
                            );
                        }
                    }

                    if poll < MAX_FAKE_CHECK_POLLS {
                        tokio::time::sleep(Duration::from_millis(400)).await;
                    }
                }

                if let Some(pay_result) = verified_pay_result {
                    let task_result = TaskResult::GrabTicketResult(GrabTicketResult {
                        task_id: task_id.to_string(),
                        uid,
                        project_id: project_id.to_string(),
                        account_name: account_name.to_string(),
                        success: true,
                        message: "抢票成功".to_string(),
                        order_id: order_id_opt.clone(),
                        pay_token: if pay_token.is_empty() {
                            None
                        } else {
                            Some(pay_token.to_string())
                        },
                        confirm_result: Some(confirm_result.clone()),
                        pay_result: Some(pay_result.clone()),
                    });
                    let _ = result_tx.send(task_result);

                    // 单一权威推送通道：读取最新动态 PushConfig，脱离主任务异步推送（不阻塞任务结束与注销）
                    let effective_push_config = if push_config_updated.load(Ordering::Acquire) {
                        live_push_config
                            .read()
                            .unwrap_or_else(|e| e.into_inner())
                            .clone()
                    } else {
                        grab_ticket_req.biliticket.push_self.clone()
                    };

                    if effective_push_config.enabled {
                        let jump_url = Some(format!(
                            "bilibili://mall/web?url=https://mall.bilibili.com/neul-next/ticket/orderDetail.html?order_id={}",
                            order_id_str
                        ));
                        let pay_url = pay_result.code_url.clone();
                        let price_cents = resolve_order_price_cents(confirm_result);
                        let price_formatted = format_price_yuan(price_cents);
                        let title = format!("恭喜{}抢票成功！", confirm_result.project_name);
                        let message = format!(
                            "抢票成功！\n项目：{}\n场次：{}\n票类型：{}\n订单号：{}\n支付链接：{}\n请尽快支付 ¥{}，以免支付超时导致票丢失\n本项目完全免费开源，仅供学习使用，开发组不承担使用本软件造成的一切后果",
                            confirm_result.project_name,
                            confirm_result.screen_name,
                            confirm_result.ticket_info.name,
                            order_id_str,
                            pay_url,
                            price_formatted,
                        );

                        tokio::spawn(async move {
                            let (ok, msg) = effective_push_config
                                .push_all_async(&title, &message, &jump_url)
                                .await;
                            if ok {
                                log::info!("抢票成功通知推送完成: {}", msg);
                            } else {
                                log::error!("抢票成功通知推送失败: {}", msg);
                            }
                        });
                    }

                    return GrabOutcome::Success;
                }

                // 5次轮询均未通过：若存在订单号，输出醒目告警提示用户检查订单列表；绝不无延迟死循环 continue
                if !order_id_str.is_empty() {
                    log::warn!(
                        "订单可能已创建，请检查订单列表！订单号: {}（连续 {} 次核验未通过: {}）",
                        order_id_str,
                        MAX_FAKE_CHECK_POLLS,
                        last_check_err
                    );
                } else {
                    log::warn!(
                        "订单核验连续 {} 次未通过({})，继续重试抢票",
                        MAX_FAKE_CHECK_POLLS,
                        last_check_err
                    );
                }
                order_retry_delay
            }
            Err(code) => match classify_create_order_error(code, order_retry_delay) {
                CreateOrderErrorAction::Retry(delay) => delay,
                CreateOrderErrorAction::NeedRefreshToken => {
                    return GrabOutcome::NeedRefreshToken;
                }
                CreateOrderErrorAction::Fatal(reason) => {
                    log::error!("{}", reason);
                    return GrabOutcome::FatalError(reason);
                }
            },
        };

        order_retry_count += 1;
        if order_retry_count >= max_order_retry {
            if grab_ticket_req.grab_mode == 2 {
                log::warn!(
                    "捡漏模式下单已达本票种最大重试次数({})，放弃该票种继续检测其他票种",
                    max_order_retry
                );
            } else {
                log::error!("下单失败，已达最大重试次数({})，停止本任务", max_order_retry);
            }
            return GrabOutcome::RetryLimitReached;
        }

        if cancellable_sleep(retry_sleep, cancel_flag, cancel_rx).await {
            log::info!("下单任务已取消");
            return GrabOutcome::Cancelled;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::account::Account;
    use common::captcha::LocalCaptcha;
    use common::utility::CustomConfig;
    use std::sync::OnceLock;

    fn test_local_captcha() -> LocalCaptcha {
        static INSTANCE: OnceLock<LocalCaptcha> = OnceLock::new();
        INSTANCE.get_or_init(LocalCaptcha::new).clone()
    }

    fn make_dummy_grab_request(uid: i64, grab_mode: u8, project_id: &str) -> GrabTicketRequest {
        let cookie_manager = Arc::new(CookieManager::from_client(
            Arc::new(reqwest::Client::new()),
            "DedeUserID=12345; buvid3=test_buvid",
        ));
        let account = Account {
            uid,
            name: format!("user_{}", uid),
            level: "6".to_string(),
            cookie: String::new(),
            csrf: String::new(),
            is_login: true,
            account_status: "空闲".to_string(),
            vip_label: String::new(),
            is_active: true,
            avatar_url: None,
            avatar_texture: None,
            cookie_manager: Some(Arc::clone(&cookie_manager)),
        };
        let config = CustomConfig::new();
        let push_self = PushConfig::new();
        let biliticket = BilibiliTicket {
            uid,
            method: grab_mode,
            ua: "test-ua".to_string(),
            config,
            account,
            push_self,
            status_delay: 0,
            captcha_use_type: 0,
            cookie_manager: Some(Arc::clone(&cookie_manager)),
            project_id: project_id.to_string(),
            referer: String::new(),
            screen_id: "100".to_string(),
            id_bind: 1,
            project_info: None,
            all_buyer_info: None,
            buyer_info: None,
            no_bind_buyer_info: None,
            select_ticket_id: Some("200".to_string()),
            pay_money: None,
            count: Some(1),
            device_id: String::new(),
        };

        GrabTicketRequest {
            task_id: String::new(),
            uid,
            project_id: project_id.to_string(),
            screen_id: "100".to_string(),
            ticket_id: "200".to_string(),
            count: 1,
            buyer_info: Vec::new(),
            cookie_manager,
            biliticket,
            grab_mode,
            status: TaskStatus::Pending,
            start_time: None,
            is_hot: false,
            local_captcha: test_local_captcha(),
            skip_words: None,
        }
    }

    #[test]
    fn preregistration_and_cancel_transition_to_cancelling() {
        let (mut tm, mut rx) = TaskManagerImpl::new_without_worker();

        let req = make_dummy_grab_request(1001, 0, "88888");
        let task_id = tm
            .submit_task(TaskRequest::GrabTicketRequest(req))
            .expect("submit should succeed");

        // Pre-registered synchronously: immediately visible before worker even polls rx
        let running = tm.list_running_tasks();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].task_id, task_id);
        assert_eq!(running[0].seq, 1);
        assert_eq!(running[0].uid, 1001);
        assert_eq!(running[0].status, TaskStatus::Running);
        assert_eq!(tm.get_task_status(&task_id), Some(TaskStatus::Running));

        // Pull message from queue to inspect cancel_rx
        let msg = rx.try_recv().expect("message should be queued");
        let cancel_rx = match msg {
            TaskMessage::SubmitGrabTask { cancel_rx, .. } => cancel_rx,
            _ => panic!("expected SubmitGrabTask"),
        };
        assert!(!*cancel_rx.borrow());

        // Cancel transitions status to Cancelling and flips watch channel immediately
        tm.cancel_task(&task_id).expect("cancel should succeed");
        assert_eq!(tm.get_task_status(&task_id), Some(TaskStatus::Cancelling));
        let running_after = tm.list_running_tasks();
        assert_eq!(running_after[0].status, TaskStatus::Cancelling);
        assert!(*cancel_rx.borrow());
    }

    #[test]
    fn cancel_task_works_on_qrcode_login_request() {
        let (mut tm, mut rx) = TaskManagerImpl::new_without_worker();

        let task_id = tm
            .submit_task(TaskRequest::QrCodeLoginRequest(QrCodeLoginRequest {
                qrcode_key: "key_123".to_string(),
                qrcode_url: "https://example.com/qr".to_string(),
                user_agent: None,
            }))
            .expect("submit qrcode task should succeed");

        assert_eq!(tm.get_task_status(&task_id), Some(TaskStatus::Running));

        let msg = rx.try_recv().expect("qrcode message should be queued");
        let cancel_rx = match msg {
            TaskMessage::SubmitQrCodeLogin { cancel_rx, .. } => cancel_rx,
            _ => panic!("expected SubmitQrCodeLogin"),
        };
        assert!(!*cancel_rx.borrow());

        tm.cancel_task(&task_id).expect("cancel qrcode should succeed");
        assert_eq!(tm.get_task_status(&task_id), Some(TaskStatus::Cancelling));
        assert!(*cancel_rx.borrow());
    }

    #[test]
    fn cancel_tasks_for_uid_only_cancels_matching_account() {
        let (mut tm, _rx) = TaskManagerImpl::new_without_worker();

        let id1 = tm
            .submit_task(TaskRequest::GrabTicketRequest(make_dummy_grab_request(
                1001, 1, "111",
            )))
            .unwrap();
        let id2 = tm
            .submit_task(TaskRequest::GrabTicketRequest(make_dummy_grab_request(
                1001, 2, "222",
            )))
            .unwrap();
        let id3 = tm
            .submit_task(TaskRequest::GrabTicketRequest(make_dummy_grab_request(
                2002, 1, "333",
            )))
            .unwrap();

        let cancelled = tm.cancel_tasks_for_uid(1001);
        assert_eq!(cancelled, 2);
        assert_eq!(tm.get_task_status(&id1), Some(TaskStatus::Cancelling));
        assert_eq!(tm.get_task_status(&id2), Some(TaskStatus::Cancelling));
        assert_eq!(tm.get_task_status(&id3), Some(TaskStatus::Running));
    }

    #[test]
    fn update_push_config_updates_live_config() {
        let (tm, _rx) = TaskManagerImpl::new_without_worker();
        assert!(!tm.push_config_updated.load(Ordering::Acquire));

        let mut cfg = PushConfig::new();
        cfg.enabled = true;
        cfg.bark_token = "bark_live_token".to_string();
        tm.update_push_config(cfg);

        assert!(tm.push_config_updated.load(Ordering::Acquire));
        let stored = tm.live_push_config.read().unwrap();
        assert!(stored.enabled);
        assert_eq!(stored.bark_token, "bark_live_token");
    }

    #[test]
    fn recent_tasks_records_finished_and_cancelled_and_respects_cap() {
        // 1. Live worker offline failure (mode 0 with project_info=None fails immediately with zero network)
        let mut tm = TaskManagerImpl::new();
        let req = make_dummy_grab_request(3003, 0, "77777");
        let task_id = tm
            .submit_task(TaskRequest::GrabTicketRequest(req))
            .expect("submit should succeed");

        let mut got_result = None;
        for _ in 0..100 {
            let results = tm.get_results();
            if let Some(r) = results.into_iter().next() {
                got_result = Some(r);
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }

        match got_result.expect("should receive failure GrabTicketResult") {
            TaskResult::GrabTicketResult(res) => {
                assert_eq!(res.task_id, task_id);
                assert_eq!(res.uid, 3003);
                assert_eq!(res.project_id, "77777");
                assert_eq!(res.account_name, "user_3003");
                assert!(!res.success);
            }
            _ => panic!("expected GrabTicketResult"),
        }

        // Wait briefly for GrabTaskGuard Drop to complete
        for _ in 0..50 {
            if !tm.list_recent_tasks().is_empty() && tm.list_running_tasks().is_empty() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(tm.list_running_tasks().is_empty());
        let recent = tm.list_recent_tasks();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].task_id, task_id);
        assert!(matches!(recent[0].status, TaskStatus::Failed(_)));

        tm.clear_recent_tasks();
        assert!(tm.list_recent_tasks().is_empty());

        // 2. Cancelled guard + cap enforcement at MAX_RECENT_TASKS (50)
        for i in 0..(MAX_RECENT_TASKS + 10) {
            let cancel_flag = Arc::new(AtomicBool::new(i % 2 == 0));
            let final_outcome = Arc::new(Mutex::new(if i % 2 == 1 {
                Some(FinalTaskOutcome {
                    status: TaskStatus::Completed(true),
                    message: "抢票成功".to_string(),
                })
            } else {
                None
            }));
            let guard = GrabTaskGuard {
                grab_registry: Arc::clone(&tm.grab_registry),
                cancel_tokens: Arc::clone(&tm.cancel_tokens),
                recent_tasks: Arc::clone(&tm.recent_tasks),
                task_id: format!("task_{}", i),
                uid: 42,
                account_name: "tester".to_string(),
                project_id: "999".to_string(),
                project_name: "Test Show".to_string(),
                grab_mode: 1,
                started_at: Instant::now(),
                cancel_flag,
                final_outcome,
            };
            drop(guard);
        }

        let capped = tm.list_recent_tasks();
        assert_eq!(capped.len(), MAX_RECENT_TASKS);
        // Newest entry is at front
        assert_eq!(capped[0].task_id, format!("task_{}", MAX_RECENT_TASKS + 9));
        assert_eq!(capped[0].status, TaskStatus::Completed(true));
        assert_eq!(capped[1].status, TaskStatus::Cancelled);

        tm.shutdown();
    }

    #[test]
    fn price_formatting_and_pay_money_preference() {
        assert_eq!(format_price_yuan(9990), "99.90");
        assert_eq!(format_price_yuan(50), "0.50");
        assert_eq!(format_price_yuan(0), "0.00");
        assert_eq!(format_price_yuan(12800), "128.00");
        assert_eq!(format_price_yuan(5), "0.05");

        let confirm_with_pay_money = ConfirmTicketResult {
            count: 2,
            pay_money: 19980,
            project_name: "Show".to_string(),
            screen_name: "Day 1".to_string(),
            ticket_info: ConfirmTicketInfo {
                name: "VIP".to_string(),
                count: 2,
                price: 9000,
            },
        };
        assert_eq!(resolve_order_price_cents(&confirm_with_pay_money), 19980);
        assert_eq!(
            format_price_yuan(resolve_order_price_cents(&confirm_with_pay_money)),
            "199.80"
        );

        let confirm_fallback = ConfirmTicketResult {
            count: 2,
            pay_money: 0,
            project_name: "Show".to_string(),
            screen_name: "Day 1".to_string(),
            ticket_info: ConfirmTicketInfo {
                name: "Standard".to_string(),
                count: 2,
                price: 9990,
            },
        };
        assert_eq!(resolve_order_price_cents(&confirm_fallback), 19980);
    }

    #[test]
    fn grab_outcome_and_error_classification_logic() {
        let default_delay = Duration::from_millis(400);

        // Token refresh triggers
        for code in [100041, 100050, 900002] {
            assert_eq!(
                classify_create_order_error(code, default_delay),
                CreateOrderErrorAction::NeedRefreshToken
            );
        }

        // Fast-fail fatal errors including ERR_INVALID_PARAM (999)
        for code in [100017, 100016, 1, 83000004, 100079, 100003, 100039, 209001, ERR_INVALID_PARAM, ERR_ID_BIND_INVALID] {
            assert!(matches!(
                classify_create_order_error(code, default_delay),
                CreateOrderErrorAction::Fatal(_)
            ));
        }

        // Distinct single sleep durations (no double sleeps, ERR_NETWORK distinct from 412)
        assert_eq!(
            classify_create_order_error(100001, default_delay),
            CreateOrderErrorAction::Retry(Duration::from_millis(300))
        );
        assert_eq!(
            classify_create_order_error(100009, default_delay),
            CreateOrderErrorAction::Retry(Duration::from_millis(600))
        );
        assert_eq!(
            classify_create_order_error(3, default_delay),
            CreateOrderErrorAction::Retry(Duration::from_millis(4800))
        );
        assert_eq!(
            classify_create_order_error(412, default_delay),
            CreateOrderErrorAction::Retry(Duration::from_millis(1200))
        );
        assert_eq!(
            classify_create_order_error(ERR_NETWORK, default_delay),
            CreateOrderErrorAction::Retry(default_delay)
        );

        // Skip-word case-insensitive filtering
        let words = vec!["vip".to_string(), "看台".to_string()];
        assert!(should_skip_by_words("VIP前排票", &words));
        assert!(should_skip_by_words("二楼看台B区", &words));
        assert!(!should_skip_by_words("普通内场票", &words));
        assert!(!should_skip_by_words("", &words));

        // GrabOutcome equality check
        assert_ne!(GrabOutcome::NeedRefreshToken, GrabOutcome::RetryLimitReached);
        assert_eq!(
            GrabOutcome::FatalError("err".into()),
            GrabOutcome::FatalError("err".into())
        );
    }

    #[test]
    fn extract_verified_pay_param_rejects_empty_or_null_pay_param() {
        let empty_data = extract_verified_pay_param(&json!({"errno": 0, "data": {}}));
        assert!(empty_data.is_err());
        assert!(empty_data.unwrap_err().contains("payParam.code_url"));

        let null_data = extract_verified_pay_param(&json!({"errno": 0, "data": null}));
        assert!(null_data.is_err());
        assert!(null_data.unwrap_err().contains("payParam.code_url"));

        let empty_code_url = extract_verified_pay_param(&json!({
            "errno": 0,
            "data": {"payParam": {"code_url": ""}}
        }));
        assert!(empty_code_url.is_err());
        assert!(empty_code_url.unwrap_err().contains("payParam.code_url"));

        let blank_code_url = extract_verified_pay_param(&json!({
            "errno": 0,
            "data": {"payParam": {"code_url": "   "}}
        }));
        assert!(blank_code_url.is_err());

        let api_err = extract_verified_pay_param(&json!({
            "errno": 100001,
            "msg": "排队中"
        }));
        assert!(api_err.is_err());

        let valid = extract_verified_pay_param(&json!({
            "errno": 0,
            "data": {
                "payParam": {
                    "sign": "sig_abc",
                    "code_url": "https://pay.bilibili.com/qr?id=999"
                }
            }
        }))
        .expect("valid non-empty code_url should pass");
        assert_eq!(valid.sign, "sig_abc");
        assert_eq!(valid.code_url, "https://pay.bilibili.com/qr?id=999");
    }

    #[test]
    fn mode2_transient_token_error_classification() {
        for transient_code in [100001, 100009, 100017, 429, 412, -1001, -1002] {
            assert!(
                is_transient_mode2_token_error(transient_code),
                "code {} should be transient in Mode 2",
                transient_code
            );
        }

        for non_transient in [100080, 100082, 100039, ERR_INVALID_PARAM as i64, 401, -401, 500] {
            assert!(
                !is_transient_mode2_token_error(non_transient),
                "code {} should NOT be treated as transient in Mode 2",
                non_transient
            );
        }
    }

    #[test]
    fn actionable_risk_verification_requires_401_and_non_null_risk_param() {
        let valid_neg = TokenRiskParam {
            code: -401,
            risk_param: Some(json!({"v_voucher": "voucher_1"})),
            ..Default::default()
        };
        assert!(has_actionable_risk_verification(&valid_neg));

        let valid_pos = TokenRiskParam {
            code: 401,
            risk_param: Some(json!({"v_voucher": "voucher_2"})),
            ..Default::default()
        };
        assert!(has_actionable_risk_verification(&valid_pos));

        let http_401_no_risk = TokenRiskParam {
            code: 401,
            risk_param: None,
            ..Default::default()
        };
        assert!(!has_actionable_risk_verification(&http_401_no_risk));

        let null_risk = TokenRiskParam {
            code: -401,
            risk_param: Some(serde_json::Value::Null),
            ..Default::default()
        };
        assert!(!has_actionable_risk_verification(&null_risk));

        let non_401 = TokenRiskParam {
            code: 100001,
            risk_param: Some(json!({"v_voucher": "voucher_3"})),
            ..Default::default()
        };
        assert!(!has_actionable_risk_verification(&non_401));
    }
}
