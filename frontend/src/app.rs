use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{Read, Write};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::ui;
use crate::ui::error_banner::render_notification_banner;
use crate::windows;
use crate::windows::add_buyer::AddBuyerInput;
use crate::windows::login_windows::LoginTexture;

use backend::taskmanager::TaskManagerImpl;
use common::account::Account;
use common::captcha::LocalCaptcha;
use common::cookie_manager::CookieManager;
use common::login::LoginInput;
use common::push::PushConfig;
use common::show_orderlist::OrderResponse;
use common::taskmanager::{
    AddAccountRequest, FetchImageRequest, FetchPolicyRequest, GetAllorderRequest,
    GetBuyerInfoRequest, GetTicketInfoRequest, QrCodeLoginRequest, SignoutAccountRequest,
    TaskManager, TaskRequest, TaskResult, TaskStatus,
};
use common::ticket::{BilibiliTicket, BuyerInfo};
use common::utility::CustomConfig;
use common::utils::{save_config, Config, MAX_ACCOUNTS};
use common::LOG_COLLECTOR;

const MAX_LOG_LINES: usize = 5000;
const MAX_IMAGE_CACHE_ENTRIES: usize = 120;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BannerKind {
    Success,
    Error,
    Info,
}

pub struct NotificationBanner {
    pub kind: BannerKind,
    pub text: String,
    pub start_time: Instant,
    pub opacity: f32,
}

pub enum ImageLoadState {
    Loading,
    Loaded(egui::TextureHandle),
    Failed,
}

#[derive(Clone, Debug)]
pub struct PayQrItem {
    pub pay_url: String,
    pub order_id: String,
    pub account_name: String,
    pub project_name: String,
    pub screen_name: String,
    pub ticket_name: String,
    pub count: usize,
    pub pay_money_fen: i64,
    pub created_at: Instant,
}

// 账号管理
pub struct AccountManager {
    pub accounts: Vec<Account>,
}

// 获取全部订单结构体（便于区分）
#[derive(Clone)]
pub struct OrderData {
    pub account_id: String,
    pub page: usize,
    pub data: Option<OrderResponse>,
}

pub struct AccountSwitch {
    pub uid: String,
    pub switch: bool,
}

pub struct Myapp {
    pub app: String,
    pub version: String,
    pub policy: Option<Value>,
    pub permissions: Option<Value>,
    pub policy_requested: bool,
    pub policy_blocked_message: Option<String>,

    // UI
    pub left_panel_width: f32,
    pub selected_tab: usize,
    pub last_selected_tab: usize,

    // 加载动画
    pub loading_angle: f32,
    pub is_loading: bool,
    pub loading_text: String,

    // 日志记录与过滤
    pub logs: Vec<String>,
    pub show_log_window: bool,
    pub log_filter_level: u8, // 0=全部, 1=INFO+, 2=WARN+, 3=ERROR
    pub log_search_query: String,
    pub log_auto_scroll: bool,

    // 登录窗口
    pub show_login_windows: bool,
    pub login_texture: LoginTexture,
    pub login_method: String,
    pub login_qrcode_url: Option<String>,
    pub qrcode_polling_task_id: Option<String>,
    pub login_qr_gen_task_id: Option<String>,
    pub login_qr_texture_cache: Option<(String, egui::TextureHandle)>,
    pub login_qr_status: String,
    pub login_qr_requesting: bool,
    pub login_qr_error: Option<String>,
    pub login_submitting: bool,
    pub sms_sending: bool,
    pub sms_cooldown_until: Option<Instant>,
    pub login_input: LoginInput,
    pub sms_captcha_key: String,

    // 默认头像与统一异步图片缓存
    pub default_avatar_texture: Option<egui::TextureHandle>,
    pub image_cache: HashMap<String, ImageLoadState>,
    pub image_cache_order: VecDeque<String>,

    // 统一通知横幅
    pub active_banner: Option<NotificationBanner>,

    // 抢票输入
    pub ticket_id: String,

    // 任务管理与账号管理
    pub task_manager: Box<dyn TaskManager>,
    pub account_manager: AccountManager,
    pub init_client_rx: mpsc::Receiver<(i64, Arc<CookieManager>)>,
    pub initializing_account_uids: HashSet<i64>,
    pub signout_pending_uids: HashSet<i64>,

    // 推送、配置与自定义配置
    pub push_config: PushConfig,
    pub config: Config,
    pub custom_config: CustomConfig,
    pub settings_dirty: bool,
    pub settings_last_saved_at: Option<Instant>,
    pub settings_save_error: Option<String>,

    // 客户端与 UA
    pub client: Client,
    pub base_default_ua: String,
    pub default_ua: String,

    // 延迟操作
    pub delete_account: Option<String>,
    pub cookie_login: Option<String>,
    pub account_switch: Option<AccountSwitch>,

    // 添加购票人窗口
    pub add_buyer_input: AddBuyerInput,
    pub show_add_buyer_window: Option<String>,
    pub add_buyer_active_task_id: Option<String>,
    pub add_buyer_submitting: bool,
    pub add_buyer_error: Option<String>,

    // 订单列表窗口
    pub show_orderlist_window: Option<String>,
    pub total_order_data: Option<OrderData>,
    pub orderlist_page: usize,
    pub orderlist_need_reload: bool,
    pub orderlist_last_request_time: Option<Instant>,
    pub orderlist_requesting: bool,
    pub orderlist_active_task_id: Option<String>,
    pub orderlist_error: Option<String>,
    pub orderlist_detail_id: Option<String>,

    // 抢票向导状态
    pub status_delay: usize,
    pub grab_mode: u8, // 0: 自动抢票, 1: 直接抢票, 2: 捡漏回流票
    pub selected_account_uid: Option<i64>,
    pub bilibiliticket_list: Vec<BilibiliTicket>,
    pub show_screen_info: Option<i64>,
    pub selected_screen_index: Option<usize>,
    pub selected_screen_id: Option<i64>,
    pub selected_ticket_id: Option<i64>,
    pub allow_force_select_unavailable: bool,

    // 分离的项目详情 / 购票人请求状态与冷却
    pub project_info_last_request_time: Option<Instant>,
    pub project_info_requesting: bool,
    pub project_info_active_task_id: Option<String>,
    pub project_info_error: Option<String>,
    pub buyer_info_last_request_time: Option<Instant>,
    pub buyer_info_requesting: bool,
    pub buyer_info_active_task_id: Option<String>,
    pub buyer_info_error: Option<String>,

    pub confirm_ticket_info: Option<String>,
    pub selected_buyer_uid: Option<i64>,
    pub selected_buyer_list: Option<Vec<BuyerInfo>>,

    pub local_captcha: LocalCaptcha,

    // 多成功订单支付二维码队列与纹理缓存
    pub pending_pay_qrs: VecDeque<PayQrItem>,
    pub pay_qr_texture_cache: Option<(String, egui::TextureHandle)>,

    pub machine_id: String,
    pub announce1: Option<String>,
    pub announce2: Option<String>,
    pub announce3: Option<String>,
    pub announce4: Option<String>,

    pub public_key: String,
    pub skip_words: Option<Vec<String>>,
    pub skip_words_input: String,

    // 中文输入法（IME）兼容状态
    pub ime_composing: bool,
    pub ime_active_this_frame: bool,
    pub last_focused_widget: Option<egui::Id>,
    pub last_text_cursor_pos: Option<egui::Pos2>,
}

impl Myapp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        ui::fonts::configure_fonts(&cc.egui_ctx);

        let mut startup_warning: Option<String> = None;
        let config = match Config::load_config() {
            Ok(load_config) => {
                log::info!("配置文件加载成功");
                load_config
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    log::info!("未找到加密配置文件，尝试迁移旧 JSON 配置");
                    match Config::load_json_config() {
                        Ok(load_config) => {
                            if let Err(save_err) = load_config.save_config() {
                                log::error!("迁移配置保存失败: {}", save_err);
                            } else {
                                if let Err(del_err) = Config::delete_json_config() {
                                    log::warn!("旧配置文件删除失败: {}", del_err);
                                }
                                log::info!("旧配置迁移成功");
                            }
                            load_config
                        }
                        Err(_) => {
                            let cfg = Config::new();
                            if let Err(save_err) = cfg.save_config() {
                                log::error!("初始化配置文件保存失败: {}", save_err);
                            }
                            cfg
                        }
                    }
                } else {
                    log::error!("配置文件读取或解密失败: {}", e);
                    // 备份损坏配置，严禁无备份直接覆盖用户原有账号和设置
                    let backup_name = format!(
                        "./config.corrupt.{}",
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()
                    );
                    if std::path::Path::new("./config").exists() {
                        match std::fs::rename("./config", &backup_name) {
                            Ok(_) => {
                                log::warn!("已将异常配置文件备份至 {}", backup_name);
                                startup_warning = Some(format!(
                                    "原配置文件损坏或设备环境变更，已备份至 {} 并使用默认配置启动",
                                    backup_name
                                ));
                            }
                            Err(rename_err) => {
                                log::error!("备份异常配置文件失败: {}", rename_err);
                                startup_warning = Some(
                                    "配置文件读取失败，已使用内存默认配置启动（未覆盖原文件）"
                                        .to_string(),
                                );
                            }
                        }
                    }
                    Config::new()
                }
            }
        };

        let push_config = match serde_json::from_value::<PushConfig>(config["push_config"].clone())
        {
            Ok(cfg) => cfg,
            Err(e) => {
                log::warn!("无法解析推送配置: {}, 使用默认值", e);
                PushConfig::new()
            }
        };

        let custom_config =
            match serde_json::from_value::<CustomConfig>(config["custom_config"].clone()) {
                Ok(cfg) => cfg,
                Err(e) => {
                    log::warn!("无法解析自定义配置: {}, 使用默认值", e);
                    CustomConfig::new()
                }
            };

        let random_value = generate_random_string(8);
        let base_default_ua = format!(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36 Edg/134.0.0.0 {}",
            random_value
        );
        let default_ua = if custom_config.open_custom_ua && !custom_config.custom_ua.trim().is_empty()
        {
            custom_config.custom_ua.trim().to_string()
        } else {
            base_default_ua.clone()
        };

        let client = create_client(default_ua.clone());

        let task_manager: Box<dyn TaskManager> = Box::new(TaskManagerImpl::new());
        task_manager.update_push_config(push_config.clone());

        let accounts = config.load_accounts();
        let (init_client_tx, init_client_rx) = mpsc::channel::<(i64, Arc<CookieManager>)>();
        let mut initializing_account_uids = HashSet::new();

        // 在后台线程异步并发初始化所有已保存账号的 CookieManager，彻底消除启动卡顿
        let accounts_to_init: Vec<(i64, String)> = accounts
            .iter()
            .filter(|a| a.cookie_manager.is_none() && !a.cookie.trim().is_empty())
            .map(|a| {
                initializing_account_uids.insert(a.uid);
                (a.uid, a.cookie.clone())
            })
            .collect();

        if !accounts_to_init.is_empty() {
            let ua_override = if custom_config.open_custom_ua
                && !custom_config.custom_ua.trim().is_empty()
            {
                Some(custom_config.custom_ua.trim().to_string())
            } else {
                None
            };
            std::thread::spawn(move || {
                if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    rt.block_on(async move {
                        let mut handles = Vec::with_capacity(accounts_to_init.len());
                        for (uid, cookie) in accounts_to_init {
                            let tx = init_client_tx.clone();
                            let ua = ua_override.clone();
                            handles.push(tokio::spawn(async move {
                                let cm = Arc::new(CookieManager::new(&cookie, ua.as_deref(), 0).await);
                                let _ = tx.send((uid, cm));
                            }));
                        }
                        for h in handles {
                            let _ = h.await;
                        }
                    });
                }
            });
        }

        let machine_id = common::machine_id::get_machine_id_ob();

        let mut app = Self {
            app: String::from("BRT"),
            version: String::from("6.6.6"),
            policy: None,
            permissions: None,
            policy_requested: false,
            policy_blocked_message: None,
            public_key: String::from("-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEApTAS0RElXIs4Kr0bO4n8\nJB+eBFF/TwXUlvtOM9FNgHjK8m13EdwXaLy9zjGTSQr8tshSRr0dQ6iaCG19Zo2Y\nXfvJrwQLqdezMN+ayMKFy58/S9EGG3Np2eGgKHUPnCOAlRicqWvBdQ/cxzTDNCxa\nORMZdJRoBvya7JijLLIC3CoqmMc6Fxe5i8eIP0zwlyZ0L0C1PQ82BcWn58y7tlPY\nTCz12cWnuKwiQ9LSOfJ4odJJQK0k7rXxwBBsYxULRno0CJ3rKfApssW4cfITYVax\nFtdbu0IUsgEeXs3EzNw8yIYnsaoZlFwLS8SMVsiAFOy2y14lR9043PYAQHm1Cjaf\noQIDAQAB\n-----END PUBLIC KEY-----"),
            left_panel_width: 210.0,
            selected_tab: 0,
            last_selected_tab: 0,
            is_loading: false,
            loading_angle: 0.0,
            loading_text: String::from("加载中..."),
            show_log_window: false,
            log_filter_level: 0,
            log_search_query: String::new(),
            log_auto_scroll: true,
            show_login_windows: false,
            logs: Vec::new(),
            client,
            base_default_ua,
            default_ua,
            default_avatar_texture: None,
            image_cache: HashMap::new(),
            image_cache_order: VecDeque::new(),
            active_banner: None,
            ticket_id: String::new(),
            task_manager,
            account_manager: AccountManager { accounts },
            init_client_rx,
            initializing_account_uids,
            signout_pending_uids: HashSet::new(),
            push_config,
            custom_config,
            settings_dirty: false,
            settings_last_saved_at: None,
            settings_save_error: None,
            config,
            login_texture: LoginTexture {
                left_conrner_texture: None,
                right_conrner_texture: None,
            },
            login_method: "扫码登录".to_string(),
            login_qrcode_url: None,
            qrcode_polling_task_id: None,
            login_qr_gen_task_id: None,
            login_qr_texture_cache: None,
            login_qr_status: String::new(),
            login_qr_requesting: false,
            login_qr_error: None,
            login_submitting: false,
            sms_sending: false,
            sms_cooldown_until: None,
            login_input: LoginInput {
                phone: String::new(),
                account: String::new(),
                password: String::new(),
                cookie: String::new(),
                sms_code: String::new(),
            },
            sms_captcha_key: String::new(),
            delete_account: None,
            cookie_login: None,
            account_switch: None,
            add_buyer_input: AddBuyerInput {
                name: String::new(),
                phone: String::new(),
                id_type: 0,
                id_number: String::new(),
                as_default_buyer: false,
            },
            show_add_buyer_window: None,
            add_buyer_active_task_id: None,
            add_buyer_submitting: false,
            add_buyer_error: None,
            show_orderlist_window: None,
            total_order_data: None,
            orderlist_page: 0,
            orderlist_need_reload: false,
            orderlist_last_request_time: None,
            orderlist_requesting: false,
            orderlist_active_task_id: None,
            orderlist_error: None,
            orderlist_detail_id: None,
            status_delay: 2,
            grab_mode: 0,
            selected_account_uid: None,
            bilibiliticket_list: Vec::new(),
            show_screen_info: None,
            selected_screen_index: None,
            selected_screen_id: None,
            selected_ticket_id: None,
            allow_force_select_unavailable: false,
            project_info_last_request_time: None,
            project_info_requesting: false,
            project_info_active_task_id: None,
            project_info_error: None,
            buyer_info_last_request_time: None,
            buyer_info_requesting: false,
            buyer_info_active_task_id: None,
            buyer_info_error: None,
            confirm_ticket_info: None,
            selected_buyer_uid: None,
            selected_buyer_list: None,
            local_captcha: LocalCaptcha::new(),
            pending_pay_qrs: VecDeque::new(),
            pay_qr_texture_cache: None,
            announce1: None,
            announce2: None,
            announce3: None,
            announce4: None,
            machine_id,
            skip_words: None,
            skip_words_input: String::new(),
            ime_composing: false,
            ime_active_this_frame: false,
            last_focused_widget: None,
            last_text_cursor_pos: None,
        };

        if let Some(msg) = startup_warning {
            app.show_error(msg);
        }

        // 异步提交策略拉取任务，避免启动时阻塞主线程
        app.submit_policy_request();

        app
    }

    pub fn show_error(&mut self, msg: impl Into<String>) {
        self.active_banner = Some(NotificationBanner {
            kind: BannerKind::Error,
            text: msg.into(),
            start_time: Instant::now(),
            opacity: 1.0,
        });
    }

    pub fn show_success(&mut self, msg: impl Into<String>) {
        self.active_banner = Some(NotificationBanner {
            kind: BannerKind::Success,
            text: msg.into(),
            start_time: Instant::now(),
            opacity: 1.0,
        });
    }

    pub fn show_info(&mut self, msg: impl Into<String>) {
        self.active_banner = Some(NotificationBanner {
            kind: BannerKind::Info,
            text: msg.into(),
            start_time: Instant::now(),
            opacity: 1.0,
        });
    }

    pub fn is_ime_composing(&self) -> bool {
        self.ime_composing || self.ime_active_this_frame
    }

    fn fix_ime_events(&mut self, ctx: &egui::Context) {
        let current_focus = ctx.memory(|m| m.focus());
        if current_focus != self.last_focused_widget {
            self.ime_composing = false;
            self.last_focused_widget = current_focus;
        }
        let mut composing = self.ime_composing;
        let active_this_frame = ctx.input_mut(|input| normalize_ime_events(&mut input.events, &mut composing));
        self.ime_composing = composing;
        self.ime_active_this_frame = active_this_frame;
    }

    fn stabilize_ime_output(&mut self, ctx: &egui::Context) {
        let current_focus = ctx.memory(|m| m.focus());
        if current_focus != self.last_focused_widget {
            self.ime_composing = false;
            self.last_focused_widget = current_focus;
        }

        ctx.output_mut(|o| {
            if let Some(pos) = o.text_cursor_pos {
                self.last_text_cursor_pos = Some(pos);
            } else {
                // 在 Windows 上，egui-winit 0.23 每次在 text_cursor_pos == None 时都会调用
                // window.set_ime_allowed(false) -> ImmAssociateContextEx(hwnd, 0, IACE_CHILDREN)，
                // 频繁拆装 TSF/IME 上下文会导致默认中文输入法（微软拼音/搜狗/微信输入法）卡死无响应。
                // 保持 text_cursor_pos 稳定为 Some 可让窗口 IME 上下文持久就绪，点击任意输入框即可直接输入中文。
                #[cfg(target_os = "windows")]
                {
                    o.text_cursor_pos =
                        Some(self.last_text_cursor_pos.unwrap_or(egui::pos2(16.0, 16.0)));
                }
                #[cfg(not(target_os = "windows"))]
                {
                    if current_focus.is_some() {
                        if let Some(pos) = self.last_text_cursor_pos {
                            o.text_cursor_pos = Some(pos);
                        }
                    } else {
                        self.last_text_cursor_pos = None;
                    }
                }
            }
        });
    }

    /// 持久化当前设置（并在写入前规范化数字类字段中的全角数字），实时同步 UA 与推送配置。
    pub fn auto_save_settings(&mut self) -> bool {
        self.custom_config.preinput_phone1 =
            normalize_fullwidth_ascii(&self.custom_config.preinput_phone1)
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
        self.custom_config.preinput_phone2 =
            normalize_fullwidth_ascii(&self.custom_config.preinput_phone2)
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
        self.push_config.smtp_config.smtp_port =
            normalize_fullwidth_ascii(&self.push_config.smtp_config.smtp_port)
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();

        match save_config(
            &mut self.config,
            Some(&self.push_config),
            Some(&self.custom_config),
            None,
        ) {
            Ok(_) => {
                self.settings_dirty = false;
                self.settings_save_error = None;
                self.settings_last_saved_at = Some(Instant::now());
                self.apply_ua_settings();
                self.task_manager.update_push_config(self.push_config.clone());
                log::info!("设置已自动保存并生效");
                true
            }
            Err(e) => {
                let err_msg = e.to_string();
                self.settings_dirty = true;
                self.settings_save_error = Some(err_msg.clone());
                log::error!("设置自动保存失败: {}", err_msg);
                self.show_error(format!("设置自动保存失败: {}", err_msg));
                false
            }
        }
    }

    pub fn effective_ua(&self) -> String {
        if self.custom_config.open_custom_ua && !self.custom_config.custom_ua.trim().is_empty() {
            self.custom_config.custom_ua.trim().to_string()
        } else {
            self.base_default_ua.clone()
        }
    }

    pub fn apply_ua_settings(&mut self) {
        let new_ua = self.effective_ua();
        if new_ua != self.default_ua {
            self.default_ua = new_ua.clone();
            self.client = create_client(new_ua.clone());
            for account in &self.account_manager.accounts {
                if let Some(cm) = &account.cookie_manager {
                    cm.set_ua(&new_ua);
                }
            }
            for ticket in &self.bilibiliticket_list {
                if let Some(cm) = &ticket.account.cookie_manager {
                    cm.set_ua(&new_ua);
                }
            }
        }
    }

    pub fn upsert_ticket_draft(&mut self, ticket: BilibiliTicket) {
        self.bilibiliticket_list.retain(|t| t.uid != ticket.uid);
        self.bilibiliticket_list.push(ticket);
    }

    pub fn invalidate_buyer_cache(&mut self, uid: i64) {
        for ticket in &mut self.bilibiliticket_list {
            if ticket.uid == uid {
                ticket.all_buyer_info = None;
            }
        }
        self.buyer_info_requesting = false;
        self.buyer_info_active_task_id = None;
        self.buyer_info_last_request_time = None;
        self.buyer_info_error = None;
    }

    pub fn reset_buyer_selection(&mut self) {
        self.selected_buyer_list = None;
        self.selected_buyer_uid = None;
    }

    pub fn reset_login_state(&mut self) {
        if let Some(task_id) = self.qrcode_polling_task_id.take() {
            let _ = self.task_manager.cancel_task(&task_id);
        }
        self.login_qr_gen_task_id = None;
        self.login_qrcode_url = None;
        self.login_qr_texture_cache = None;
        self.login_qr_status.clear();
        self.login_qr_requesting = false;
        self.login_qr_error = None;
        self.login_submitting = false;
        self.sms_sending = false;
        self.sms_captcha_key.clear();
        self.login_input.password.clear();
        self.login_input.cookie.clear();
        self.login_input.sms_code.clear();
        // 重建登录专用 HTTP 客户端，确保每次新登录会话拥有独立的空 cookie store
        self.client = create_client(self.effective_ua());
    }

    pub fn get_preinput_phone_for_uid(&self, uid: i64) -> Option<String> {
        let idx = self
            .account_manager
            .accounts
            .iter()
            .position(|a| a.uid == uid)?;
        let raw = match idx {
            0 => &self.custom_config.preinput_phone1,
            1 => &self.custom_config.preinput_phone2,
            _ => return None,
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    pub fn request_image(&mut self, key: &str, url: &str) {
        let trimmed = url.trim();
        if trimmed.is_empty() || self.image_cache.contains_key(key) {
            return;
        }
        self.image_cache
            .insert(key.to_string(), ImageLoadState::Loading);
        self.image_cache_order.push_back(key.to_string());
        self.evict_image_cache_if_needed();

        let req = TaskRequest::FetchImageRequest(FetchImageRequest {
            key: key.to_string(),
            url: trimmed.to_string(),
        });
        if let Err(e) = self.task_manager.submit_task(req) {
            log::warn!("提交图片下载任务失败 ({}): {}", key, e);
            self.image_cache
                .insert(key.to_string(), ImageLoadState::Failed);
        }
    }

    pub fn clear_order_thumbnails(&mut self) {
        self.image_cache.retain(|k, _| !k.starts_with("order:"));
        self.image_cache_order.retain(|k| !k.starts_with("order:"));
    }

    fn evict_image_cache_if_needed(&mut self) {
        while self.image_cache.len() > MAX_IMAGE_CACHE_ENTRIES {
            // 优先淘汰订单缩略图，保留账号头像
            if let Some(pos) = self
                .image_cache_order
                .iter()
                .position(|k| k.starts_with("order:"))
            {
                if let Some(old_key) = self.image_cache_order.remove(pos) {
                    self.image_cache.remove(&old_key);
                    continue;
                }
            }
            if let Some(old_key) = self.image_cache_order.pop_front() {
                self.image_cache.remove(&old_key);
            } else {
                break;
            }
        }
    }

    pub fn request_orderlist(&mut self, uid: i64, page: usize) {
        let Some(account) = self
            .account_manager
            .accounts
            .iter()
            .find(|a| a.uid == uid)
            .cloned()
        else {
            self.orderlist_error = Some(format!("未找到账号 {}", uid));
            return;
        };

        let Some(cookie_manager) = account.cookie_manager.clone() else {
            if self.initializing_account_uids.contains(&uid) {
                self.orderlist_error = Some("账号客户端正在初始化，请稍后点击刷新".to_string());
            } else {
                self.orderlist_error = Some("账号客户端未初始化，请重新登录".to_string());
            }
            return;
        };

        self.orderlist_requesting = true;
        self.orderlist_active_task_id = None;
        self.orderlist_error = None;
        self.orderlist_need_reload = false;
        self.orderlist_page = page;
        self.orderlist_last_request_time = Some(Instant::now());

        let request = TaskRequest::GetAllorderRequest(GetAllorderRequest {
            task_id: String::new(),
            account_id: account.uid.to_string(),
            cookie_manager,
            cookies: String::new(),
            status: TaskStatus::Pending,
            start_time: None,
            page,
            page_size: 10,
        });
        match self.task_manager.submit_task(request) {
            Ok(task_id) => {
                self.orderlist_active_task_id = Some(task_id);
            }
            Err(e) => {
                self.orderlist_requesting = false;
                self.orderlist_active_task_id = None;
                self.orderlist_error = Some(format!("提交订单请求失败: {}", e));
                log::error!("查看全部订单请求提交失败：{}", e);
            }
        }
    }

    pub fn request_signout(&mut self, uid: i64) {
        if self.signout_pending_uids.contains(&uid) {
            return;
        }
        let Some(account) = self
            .account_manager
            .accounts
            .iter()
            .find(|a| a.uid == uid)
            .cloned()
        else {
            self.show_error(format!("未找到账号 {}", uid));
            return;
        };

        // 先取消该账号下正在运行的所有任务
        let cancelled = self.task_manager.cancel_tasks_for_uid(uid);
        if cancelled > 0 {
            log::info!("登出账号 {} 前已取消 {} 个运行中任务", uid, cancelled);
        }

        self.signout_pending_uids.insert(uid);
        match self
            .task_manager
            .submit_task(TaskRequest::SignoutAccountRequest(SignoutAccountRequest {
                account,
            })) {
            Ok(_) => {
                log::info!("正在登出账号 {}...", uid);
            }
            Err(e) => {
                self.signout_pending_uids.remove(&uid);
                log::warn!("提交远端登出任务失败，直接从本地移除账号 {}: {}", uid, e);
                self.remove_account_locally(uid);
            }
        }
    }

    pub fn upsert_account_and_save(&mut self, account: Account) {
        let uid = account.uid;
        let name = account.name.clone();
        let existing_pos = self
            .account_manager
            .accounts
            .iter()
            .position(|a| a.uid == uid);

        if existing_pos.is_none() && self.account_manager.accounts.len() >= MAX_ACCOUNTS {
            let msg = format!(
                "为防止项目被黄牛滥用，最多仅支持同时保存 {} 个账号。如需更换，请先登出已有账号。",
                MAX_ACCOUNTS
            );
            log::warn!("拒绝新增第 {} 个账号 ({}, UID={}): 已达上限", self.account_manager.accounts.len() + 1, name, uid);
            self.show_error(msg);
            return;
        }

        if let Some(pos) = existing_pos {
            self.account_manager.accounts[pos] = account.clone();
        } else {
            self.account_manager.accounts.push(account.clone());
        }
        self.initializing_account_uids.remove(&uid);
        // 清理该账号可能缓存的旧头像状态，促使重新加载新头像
        let avatar_key = format!("avatar:{}", uid);
        self.image_cache.remove(&avatar_key);
        self.image_cache_order.retain(|k| k != &avatar_key);

        match save_config(&mut self.config, None, None, Some(account)) {
            Ok(_) => {
                log::info!("账号 {} ({}) 登录并保存成功", name, uid);
                self.show_success(format!("账号 {} 登录成功", name));
            }
            Err(e) => {
                log::error!("账号 {} 登录成功，但写入配置失败: {}", name, e);
                self.show_error(format!("账号已登录，但保存配置失败: {}", e));
            }
        }
    }

    pub fn remove_account_locally(&mut self, uid: i64) {
        self.task_manager.cancel_tasks_for_uid(uid);
        self.account_manager.accounts.retain(|a| a.uid != uid);
        self.initializing_account_uids.remove(&uid);
        self.signout_pending_uids.remove(&uid);
        self.bilibiliticket_list.retain(|t| t.uid != uid);

        if self.selected_account_uid == Some(uid) {
            self.selected_account_uid = self
                .account_manager
                .accounts
                .iter()
                .find(|a| a.is_active)
                .or_else(|| self.account_manager.accounts.first())
                .map(|a| a.uid);
        }
        if self.show_screen_info == Some(uid) {
            self.show_screen_info = None;
            self.project_info_requesting = false;
            self.project_info_active_task_id = None;
            self.project_info_error = None;
            self.is_loading = false;
        }
        if self.confirm_ticket_info.as_deref() == Some(&uid.to_string()) {
            self.confirm_ticket_info = None;
            self.buyer_info_requesting = false;
            self.buyer_info_active_task_id = None;
            self.buyer_info_error = None;
            self.is_loading = false;
        }
        if self.selected_buyer_uid == Some(uid) {
            self.reset_buyer_selection();
        }
        if self.show_add_buyer_window.as_deref() == Some(&uid.to_string()) {
            self.show_add_buyer_window = None;
            self.add_buyer_input = AddBuyerInput::default();
            self.add_buyer_active_task_id = None;
            self.add_buyer_submitting = false;
            self.add_buyer_error = None;
        }
        if self.show_orderlist_window.as_deref() == Some(&uid.to_string()) {
            self.show_orderlist_window = None;
            self.orderlist_requesting = false;
            self.orderlist_active_task_id = None;
            self.orderlist_error = None;
            self.orderlist_detail_id = None;
        }
        if self
            .total_order_data
            .as_ref()
            .map(|d| d.account_id == uid.to_string())
            .unwrap_or(false)
        {
            self.total_order_data = None;
        }

        let avatar_key = format!("avatar:{}", uid);
        self.image_cache.remove(&avatar_key);
        self.image_cache_order.retain(|k| k != &avatar_key);

        let _ = self.config.delete_account(uid);
        log::info!("账号 {} 已移除", uid);
        self.show_success(format!("账号 {} 已移除", uid));
    }

    pub fn add_log(&mut self, message: &str) {
        // 将多行日志按行拆分，确保 log_windows 的 ScrollArea::show_rows 行高严格准确
        for line in message.lines() {
            self.logs.push(line.to_string());
        }
        if self.logs.len() > MAX_LOG_LINES {
            let overflow = self.logs.len() - MAX_LOG_LINES / 2;
            self.logs.drain(0..overflow);
        }

        // 仅在真正错误日志出现时自动弹出错误横幅，避免对包含"成功"的调试/启动日志误弹绿色横幅
        if message.contains("] ERROR") {
            let display = message
                .split("] ERROR")
                .nth(1)
                .map(|s| s.trim_start_matches([':', ' ']))
                .unwrap_or(message);
            self.show_error(display);
        }
    }

    fn submit_policy_request(&mut self) {
        if self.policy_requested {
            return;
        }
        self.policy_requested = true;
        let req = TaskRequest::FetchPolicyRequest(FetchPolicyRequest {
            client: self.client.clone(),
            app: self.app.clone(),
            version: self.version.clone(),
            machine_id: self.machine_id.clone(),
        });
        if let Err(e) = self.task_manager.submit_task(req) {
            log::warn!("提交策略请求失败: {}，将默认允许运行", e);
            self.policy = Some(json!({"allow_run": true}));
        }
    }

    fn apply_policy_response(&mut self, resp: Value) {
        let mut resolved_policy = json!({"allow_run": true});
        if let Some(code) = resp["code"].as_i64() {
            if code != 0 {
                log::warn!("获取策略失败: {}", resp["message"]);
            } else {
                match decode_policy(resp["data"]["data"].as_str().unwrap_or(""), &self.public_key) {
                    Ok(policy) => {
                        if let Some(permission_token) = resp["data"]["permission"].as_str() {
                            match decode_permissions(permission_token, &self.public_key) {
                                Ok(perms) => {
                                    if let Ok(mut file) = File::create("permissions") {
                                        let _ = file.write_all(permission_token.as_bytes());
                                    }
                                    self.permissions = Some(perms);
                                }
                                Err(e) => {
                                    log::warn!("权限签名无效: {}", e);
                                    self.permissions = Some(load_local_permissions(&self.public_key));
                                }
                            }
                        }
                        resolved_policy = policy;
                    }
                    Err(e) => {
                        log::error!("策略签名无效: {}", e);
                        resolved_policy = json!({"allow_run": false});
                    }
                }
            }
        }

        self.ticket_id = if self.ticket_id.is_empty() {
            resolved_policy["ticket_id"]
                .as_str()
                .unwrap_or("")
                .to_string()
        } else {
            self.ticket_id.clone()
        };
        self.policy = Some(resolved_policy);
        self.check_policy();
    }

    fn check_policy(&mut self) {
        let Some(policy) = self.policy.clone() else {
            return;
        };

        if let Some(announcement) = policy.get("announcement1").and_then(|v| v.as_str()) {
            if !announcement.trim().is_empty() {
                log::info!("公告: {}", announcement);
                self.announce1 = Some(announcement.to_string());
            }
        }
        if let Some(announcement) = policy.get("announcement2").and_then(|v| v.as_str()) {
            if !announcement.trim().is_empty() {
                self.announce2 = Some(announcement.to_string());
            }
        }
        if let Some(announcement) = policy.get("announcement3").and_then(|v| v.as_str()) {
            if !announcement.trim().is_empty() {
                self.announce3 = Some(announcement.to_string());
            }
        }
        if let Some(announcement) = policy.get("announcement4").and_then(|v| v.as_str()) {
            if !announcement.trim().is_empty() {
                self.announce4 = Some(announcement.to_string());
            }
        }

        let allow_run = policy
            .get("allow_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        if !allow_run {
            let reason = self
                .announce4
                .clone()
                .unwrap_or_else(|| "根据策略配置，当前版本不允许运行".to_string());
            log::error!("策略限制: {}", reason);
            self.policy_blocked_message = Some(reason);
        }
    }

    pub fn submit_add_account(&mut self, cookie: String, source: &str) {
        self.login_submitting = true;
        let req = TaskRequest::AddAccountRequest(AddAccountRequest {
            cookie,
            client: self.client.clone(),
            user_agent: self.effective_ua(),
            source: source.to_string(),
        });
        if let Err(e) = self.task_manager.submit_task(req) {
            self.login_submitting = false;
            log::error!("提交登录验证任务失败: {}", e);
            self.show_error(format!("登录提交失败: {}", e));
        }
    }

    fn drain_initialized_clients(&mut self) {
        while let Ok((uid, cm)) = self.init_client_rx.try_recv() {
            if !self.initializing_account_uids.remove(&uid) {
                log::debug!("忽略账号 {} 已过期的后台客户端初始化结果", uid);
                continue;
            }
            if let Some(acc) = self
                .account_manager
                .accounts
                .iter_mut()
                .find(|a| a.uid == uid)
            {
                if acc.cookie_manager.is_none() {
                    if self.custom_config.open_custom_ua
                        && !self.custom_config.custom_ua.trim().is_empty()
                    {
                        cm.set_ua(self.custom_config.custom_ua.trim());
                    }
                    acc.cookie_manager = Some(cm);
                    log::debug!("账号 {} 后台客户端初始化完成", acc.name);
                }
            }
        }
    }

    fn process_task_results(&mut self, ctx: &egui::Context) {
        let results = self.task_manager.get_results();

        for result in results {
            match result {
                TaskResult::GenerateQrCodeResult(qr_gen) => {
                    if !self.show_login_windows
                        || self.login_method != "扫码登录"
                        || self.login_qr_gen_task_id.as_deref() != Some(qr_gen.task_id.as_str())
                    {
                        continue;
                    }
                    self.login_qr_gen_task_id = None;
                    self.login_qr_requesting = false;
                    if qr_gen.success {
                        if let (Some(key), Some(url)) = (qr_gen.qrcode_key, qr_gen.qrcode_url) {
                            self.login_qrcode_url = Some(url.clone());
                            self.login_qr_texture_cache = None;
                            self.login_qr_error = None;
                            self.login_qr_status = "请使用哔哩哔哩 App 扫码登录".to_string();

                            // 先取消可能残留的旧轮询任务
                            if let Some(old_id) = self.qrcode_polling_task_id.take() {
                                let _ = self.task_manager.cancel_task(&old_id);
                            }

                            let poll_req = TaskRequest::QrCodeLoginRequest(QrCodeLoginRequest {
                                qrcode_key: key,
                                qrcode_url: url,
                                user_agent: Some(self.effective_ua()),
                            });
                            match self.task_manager.submit_task(poll_req) {
                                Ok(task_id) => {
                                    self.qrcode_polling_task_id = Some(task_id);
                                }
                                Err(e) => {
                                    self.login_qr_error =
                                        Some(format!("启动扫码轮询失败: {}", e));
                                    self.login_qr_status = "扫码轮询启动失败".to_string();
                                }
                            }
                        } else {
                            self.login_qr_error = Some("未获取到有效的二维码数据".to_string());
                        }
                    } else {
                        self.login_qr_error = Some(qr_gen.message.clone());
                        self.login_qr_status = format!("获取二维码失败: {}", qr_gen.message);
                        log::error!("获取登录二维码失败: {}", qr_gen.message);
                    }
                }
                TaskResult::QrCodeLoginResult(qrcode_result) => {
                    // 忽略已被替换的旧轮询任务的晚到结果
                    if self.qrcode_polling_task_id.as_deref() != Some(qrcode_result.task_id.as_str())
                    {
                        continue;
                    }
                    match qrcode_result.status {
                        common::login::QrCodeLoginStatus::Success(_) => {
                            self.qrcode_polling_task_id = None;
                            self.login_qrcode_url = None;
                            self.login_qr_texture_cache = None;
                            self.login_qr_status = "扫码确认成功，正在获取账号信息...".to_string();
                            log::info!("二维码扫码确认成功，正在获取账号信息");
                            if let Some(cookie_str) = qrcode_result.cookie {
                                self.submit_add_account(cookie_str, "qrcode");
                            }
                        }
                        common::login::QrCodeLoginStatus::Scanning => {
                            self.login_qr_status = "已扫码，请在手机上点击确认登录".to_string();
                        }
                        common::login::QrCodeLoginStatus::Failed(err) => {
                            self.qrcode_polling_task_id = None;
                            self.login_qr_status = format!("扫码登录失败: {}", err);
                            self.login_qr_error = Some(err.clone());
                            log::error!("二维码登录失败: {}", err);
                        }
                        common::login::QrCodeLoginStatus::Expired => {
                            self.qrcode_polling_task_id = None;
                            self.login_qr_status = "二维码已过期，请点击「刷新二维码」".to_string();
                            log::warn!("二维码已过期，请刷新");
                        }
                        _ => {}
                    }
                }
                TaskResult::LoginSmsResult(sms_result) => {
                    self.sms_sending = false;
                    if sms_result.success {
                        self.sms_captcha_key = sms_result.message.clone();
                        self.sms_cooldown_until = Some(Instant::now() + Duration::from_secs(60));
                        log::info!("短信验证码发送成功");
                        self.show_success("短信验证码已发送，请注意查收");
                    } else {
                        log::error!("短信发送失败: {}", sms_result.message);
                    }
                }
                TaskResult::SubmitSmsLoginResult(submit_result) => {
                    if submit_result.success {
                        if let Some(cookie_str) = submit_result.cookie {
                            self.submit_add_account(cookie_str, "sms");
                        } else {
                            self.login_submitting = false;
                            log::error!("短信登录返回成功但未获取到 Cookie");
                        }
                    } else {
                        self.login_submitting = false;
                        log::error!("短信登录失败: {}", submit_result.message);
                    }
                }
                TaskResult::AddAccountResult(add_res) => {
                    self.login_submitting = false;
                    if add_res.success {
                        if let Some(account) = add_res.account {
                            self.upsert_account_and_save(account);
                            self.show_login_windows = false;
                            self.reset_login_state();
                        } else {
                            log::error!("登录验证成功但账号数据为空");
                            if add_res.source == "qrcode" {
                                let msg = "登录验证成功但账号数据为空".to_string();
                                self.login_qr_error = Some(msg.clone());
                                self.login_qr_status = format!("登录失败: {}", msg);
                            }
                        }
                    } else {
                        log::error!("登录添加账号失败 ({}): {}", add_res.source, add_res.message);
                        if add_res.source == "qrcode" {
                            self.login_qr_error = Some(add_res.message.clone());
                            self.login_qr_status = format!("登录失败: {}", add_res.message);
                        }
                    }
                }
                TaskResult::SignoutAccountResult(signout_res) => {
                    self.signout_pending_uids.remove(&signout_res.uid);
                    if !signout_res.remote_success {
                        log::warn!(
                            "账号 {} 远端登出未成功（{}），已强制从本地移除",
                            signout_res.uid,
                            signout_res.message
                        );
                    }
                    self.remove_account_locally(signout_res.uid);
                }
                TaskResult::AddBuyerResult(buyer_res) => {
                    let is_active = self.add_buyer_active_task_id.as_deref()
                        == Some(buyer_res.task_id.as_str());
                    if buyer_res.success {
                        self.invalidate_buyer_cache(buyer_res.uid);
                        log::info!("购票人添加成功: {}", buyer_res.message);
                        self.show_success("购票人添加成功，列表将自动刷新");
                        if is_active {
                            self.add_buyer_active_task_id = None;
                            self.add_buyer_submitting = false;
                            self.show_add_buyer_window = None;
                            self.add_buyer_error = None;
                            self.add_buyer_input = AddBuyerInput::default();
                        }
                    } else if is_active {
                        self.add_buyer_active_task_id = None;
                        self.add_buyer_submitting = false;
                        self.add_buyer_error = Some(buyer_res.message.clone());
                        log::error!("添加购票人失败: {}", buyer_res.message);
                    } else {
                        log::warn!(
                            "忽略已过期的添加购票人失败结果 (uid={}): {}",
                            buyer_res.uid,
                            buyer_res.message
                        );
                    }
                }
                TaskResult::PushResult(push_result) => {
                    if push_result.success {
                        log::info!("推送成功: {}", push_result.message);
                        self.show_success(format!("推送成功: {}", push_result.message));
                    } else {
                        log::error!("推送失败: {}", push_result.message);
                    }
                }
                TaskResult::GetAllorderRequestResult(order_result) => {
                    // 仅当结果匹配当前活跃请求且订单窗口仍指向同一账号时写入，避免乱序/跨账号串数据
                    if self.orderlist_active_task_id.as_deref()
                        != Some(order_result.task_id.as_str())
                        || self.show_orderlist_window.as_deref()
                            != Some(order_result.account_id.as_str())
                    {
                        continue;
                    }
                    self.orderlist_requesting = false;
                    self.orderlist_active_task_id = None;
                    if order_result.success {
                        self.orderlist_error = None;
                        self.total_order_data = Some(OrderData {
                            account_id: order_result.account_id.clone(),
                            page: order_result.page,
                            data: order_result.order_info,
                        });
                        log::info!("账号 {} 第 {} 页订单获取成功", order_result.account_id, order_result.page + 1);
                    } else {
                        self.orderlist_error = Some(order_result.message.clone());
                        log::error!(
                            "账号 {} 订单请求失败: {}",
                            order_result.account_id,
                            order_result.message
                        );
                    }
                }
                TaskResult::GetTicketInfoResult(ticket_result) => {
                    if self.project_info_active_task_id.as_deref()
                        != Some(ticket_result.task_id.as_str())
                    {
                        continue;
                    }
                    self.project_info_requesting = false;
                    self.project_info_active_task_id = None;
                    self.is_loading = false;
                    if ticket_result.success {
                        let Some(inforesponse) = ticket_result.ticket_info else {
                            let msg = format!("项目信息为空: {}", ticket_result.message);
                            self.project_info_error = Some(msg.clone());
                            log::error!("{}", msg);
                            continue;
                        };
                        let project_info = inforesponse.data;
                        if let Some(bilibili_ticket) = self
                            .bilibiliticket_list
                            .iter_mut()
                            .find(|ticket| ticket.uid == ticket_result.uid)
                        {
                            bilibili_ticket.project_info = Some(project_info);
                            self.project_info_error = None;
                            // 每次成功加载新项目详情时重置场次索引，防止切项目越界空白
                            self.selected_screen_index = Some(0);
                            log::info!("获取项目详情成功");
                        } else {
                            log::warn!(
                                "未找到账号ID为 {} 的抢票草稿，忽略项目详情结果",
                                ticket_result.uid
                            );
                            self.show_screen_info = None;
                        }
                    } else {
                        self.project_info_error = Some(ticket_result.message.clone());
                        log::error!("获取项目信息失败: {}", ticket_result.message);
                    }
                }
                TaskResult::GetBuyerInfoResult(buyer_result) => {
                    if self.buyer_info_active_task_id.as_deref()
                        != Some(buyer_result.task_id.as_str())
                    {
                        continue;
                    }
                    self.buyer_info_requesting = false;
                    self.buyer_info_active_task_id = None;
                    self.is_loading = false;
                    if buyer_result.success {
                        let Some(response) = buyer_result.buyer_info else {
                            self.buyer_info_error = Some("购票人返回数据为空".to_string());
                            log::error!("获取购票人信息失败: 数据为空");
                            continue;
                        };
                        if response.errno != 0 {
                            let msg = format!(
                                "获取购票人信息失败 (errno={}): {}",
                                response.errno, response.msg
                            );
                            self.buyer_info_error = Some(msg.clone());
                            log::error!("{}", msg);
                            continue;
                        }
                        if let Some(bilibili_ticket) = self
                            .bilibiliticket_list
                            .iter_mut()
                            .find(|ticket| ticket.uid == buyer_result.uid)
                        {
                            bilibili_ticket.all_buyer_info = Some(response.data);
                            self.buyer_info_error = None;
                        } else {
                            log::warn!(
                                "未找到账号ID为 {} 的抢票草稿，忽略购票人结果",
                                buyer_result.uid
                            );
                            self.confirm_ticket_info = None;
                        }
                    } else {
                        self.buyer_info_error = Some(buyer_result.message.clone());
                        log::error!("获取购票人信息失败: {}", buyer_result.message);
                    }
                }
                TaskResult::GrabTicketResult(grab_res) => {
                    if grab_res.success {
                        let Some(pay_data) = grab_res.pay_result else {
                            log::error!("抢票成功但缺少支付链接: {}", grab_res.message);
                            continue;
                        };
                        let confirm = grab_res.confirm_result.unwrap_or_default();
                        let pay_money_fen = if confirm.pay_money > 0 {
                            confirm.pay_money
                        } else {
                            confirm.ticket_info.price * confirm.count as i64
                        };
                        let order_id = grab_res.order_id.unwrap_or_default();
                        let item = PayQrItem {
                            pay_url: pay_data.code_url,
                            order_id: order_id.clone(),
                            account_name: grab_res.account_name.clone(),
                            project_name: confirm.project_name.clone(),
                            screen_name: confirm.screen_name.clone(),
                            ticket_name: confirm.ticket_info.name.clone(),
                            count: confirm.count.max(1) as usize,
                            pay_money_fen,
                            created_at: Instant::now(),
                        };
                        self.pending_pay_qrs.push_back(item);
                        let yuan = pay_money_fen as f64 / 100.0;
                        let title = format!("恭喜！{} 抢票成功（¥{:.2}）", confirm.project_name, yuan);
                        log::info!("{} 订单号: {}", title, order_id);
                        self.show_success(title);
                        // 注意：推送通知已由 backend/src/taskmanager.rs 使用实时 PushConfig 单次发出，此处不再重复提交 PushRequest
                    } else {
                        log::error!(
                            "抢票任务结束 [{}]: {}",
                            grab_res.account_name,
                            grab_res.message
                        );
                    }
                }
                TaskResult::FetchPolicyResult(policy_res) => {
                    if policy_res.success {
                        if let Some(raw) = policy_res.raw_response {
                            self.apply_policy_response(raw);
                        } else {
                            self.policy = Some(json!({"allow_run": true}));
                        }
                    } else {
                        log::warn!(
                            "请求策略失败: {}，将默认允许运行",
                            policy_res.error.unwrap_or_default()
                        );
                        self.policy = Some(json!({"allow_run": true}));
                    }
                }
                TaskResult::FetchImageResult(img_res) => {
                    if !matches!(self.image_cache.get(&img_res.key), Some(ImageLoadState::Loading)) {
                        continue;
                    }
                    if let Some(bytes) = img_res.bytes {
                        let is_avatar = img_res.key.starts_with("avatar:");
                        match decode_image_to_color_image(&bytes, is_avatar) {
                            Ok(color_image) => {
                                let texture = ctx.load_texture(
                                    img_res.key.clone(),
                                    color_image,
                                    egui::TextureOptions::LINEAR,
                                );
                                self.image_cache
                                    .insert(img_res.key, ImageLoadState::Loaded(texture));
                            }
                            Err(e) => {
                                log::debug!("图片解码失败 ({}): {}", img_res.key, e);
                                self.image_cache
                                    .insert(img_res.key, ImageLoadState::Failed);
                            }
                        }
                    } else {
                        log::debug!(
                            "图片下载失败 ({}): {}",
                            img_res.key,
                            img_res.error.unwrap_or_default()
                        );
                        self.image_cache
                            .insert(img_res.key, ImageLoadState::Failed);
                    }
                }
            }
        }
    }

    pub fn add_log_windows(&mut self) {
        let logs = {
            let mut collector = LOG_COLLECTOR
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            collector.get_logs()
        };
        if let Some(logs) = logs {
            for line in logs {
                self.add_log(&line);
            }
        }
    }

    fn process_deferred_actions(&mut self) {
        if let Some(account_id) = self.delete_account.take() {
            if let Ok(uid) = account_id.parse::<i64>() {
                self.remove_account_locally(uid);
            }
        }

        if let Some(cookie) = self.cookie_login.take() {
            self.submit_add_account(cookie, "cookie");
        }

        if let Some(account_switch) = self.account_switch.take() {
            if let Ok(uid) = account_switch.uid.parse::<i64>() {
                if let Some(account) = self
                    .account_manager
                    .accounts
                    .iter_mut()
                    .find(|a| a.uid == uid)
                {
                    account.is_active = account_switch.switch;
                    let updated = account.clone();
                    if let Err(e) = save_config(&mut self.config, None, None, Some(updated)) {
                        log::error!("保存账号 {} 抢票开关状态失败: {}", uid, e);
                    } else {
                        log::info!(
                            "账号 {} 已{}",
                            uid,
                            if account_switch.switch {
                                "开启抢票"
                            } else {
                                "关闭抢票"
                            }
                        );
                    }
                } else {
                    log::warn!("未找到账号 {}，忽略开关切换", account_switch.uid);
                }
            }
        }
    }

    fn update_banner_animation(&mut self) -> bool {
        let Some(banner) = &mut self.active_banner else {
            return false;
        };
        let elapsed = banner.start_time.elapsed().as_secs_f32();
        let hold_secs = match banner.kind {
            BannerKind::Error => 4.0,
            BannerKind::Success | BannerKind::Info => 3.0,
        };
        let fade_secs = 0.6;
        if elapsed < hold_secs + fade_secs {
            if elapsed > hold_secs {
                banner.opacity = (1.0 - (elapsed - hold_secs) / fade_secs).clamp(0.0, 1.0);
            } else {
                banner.opacity = 1.0;
            }
            true
        } else {
            self.active_banner = None;
            false
        }
    }

    fn render_policy_block_modal(&self, ctx: &egui::Context) {
        let Some(reason) = &self.policy_blocked_message else {
            return;
        };
        egui::Window::new("版本限制提示")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_min_width(380.0);
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("当前版本暂不可继续运行")
                        .strong()
                        .size(16.0)
                        .color(egui::Color32::from_rgb(220, 68, 55)),
                );
                ui.add_space(8.0);
                ui.label(reason);
                ui.add_space(16.0);
                ui.vertical_centered(|ui| {
                    if ui.button("退出程序").clicked() {
                        log::logger().flush();
                        std::process::exit(0);
                    }
                });
            });
    }
}

impl eframe::App for Myapp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 0. 修正 egui 0.23 / egui-winit 0.23 的中文输入法（IME）事件缺陷
        self.fix_ime_events(ctx);

        // 1. 先处理后台状态与结果，再渲染 UI（消除 1 帧滞后与早返回跳过弹窗的问题）
        self.drain_initialized_clients();
        self.process_task_results(ctx);
        self.add_log_windows();
        self.process_deferred_actions();

        // 若被策略拦截，展示明确阻断弹窗而不是静默闪退
        if self.policy_blocked_message.is_some() {
            self.render_policy_block_modal(ctx);
            self.stabilize_ime_output(ctx);
            return;
        }

        // 2. 侧栏与主内容区（离开设置页时自动落盘未保存修改）
        ui::sidebar::render_sidebar(self, ctx);
        if self.last_selected_tab == 3 && self.selected_tab != 3 && self.settings_dirty {
            self.auto_save_settings();
        }
        self.last_selected_tab = self.selected_tab;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui::tabs::render_tab_content(self, ui);
        });

        // 3. 各悬浮弹窗（全流程无提前 return，确保所有弹窗都能正常绘制）
        if self.show_log_window {
            windows::log_windows::show(self, ctx);
        }

        if self.show_login_windows {
            windows::login_windows::show(self, ctx);
        }

        if let Some(account_id) = self.show_add_buyer_window.clone() {
            if account_id == "0" || account_id.is_empty() {
                self.show_add_buyer_window = None;
                self.show_info("请先登录账号后再添加购票人");
            } else {
                windows::add_buyer::show(self, ctx, &account_id);
            }
        }

        if let Some(uid_str) = self.show_orderlist_window.clone() {
            let account_id = uid_str.parse::<i64>().unwrap_or(0);
            if account_id <= 0 {
                self.show_orderlist_window = None;
                self.orderlist_requesting = false;
                self.orderlist_active_task_id = None;
                self.show_info("请先登录账号后再查看订单");
            } else {
                // 切换查看另一个账号时立即清空旧账号订单缓存，避免跨账号误展示
                if self
                    .total_order_data
                    .as_ref()
                    .map(|d| d.account_id != uid_str)
                    .unwrap_or(false)
                {
                    self.total_order_data = None;
                    self.orderlist_page = 0;
                    self.orderlist_error = None;
                    self.orderlist_requesting = false;
                    self.orderlist_active_task_id = None;
                    self.orderlist_need_reload = true;
                } else if self.total_order_data.is_none()
                    && !self.orderlist_requesting
                    && self.orderlist_error.is_none()
                {
                    self.orderlist_need_reload = true;
                }

                if self.orderlist_need_reload && !self.orderlist_requesting {
                    let page = self.orderlist_page;
                    self.request_orderlist(account_id, page);
                }

                windows::show_orderlist::show(self, ctx);
            }
        }

        if let Some(account_id) = self.show_screen_info {
            let needs_fetch = self
                .bilibiliticket_list
                .iter()
                .find(|t| t.uid == account_id)
                .map(|t| t.project_info.is_none())
                .unwrap_or(false);

            if !self.bilibiliticket_list.iter().any(|t| t.uid == account_id) {
                log::warn!("未找到账号ID为 {} 的抢票对象，关闭项目详情窗口", account_id);
                self.show_screen_info = None;
                self.project_info_requesting = false;
                self.project_info_active_task_id = None;
                self.is_loading = false;
            } else {
                if needs_fetch
                    && !self.project_info_requesting
                    && self.project_info_error.is_none()
                {
                    let cooldown_ok = self
                        .project_info_last_request_time
                        .map(|t| t.elapsed() >= Duration::from_secs(2))
                        .unwrap_or(true);
                    if cooldown_ok {
                        if let Some(ticket) = self
                            .bilibiliticket_list
                            .iter()
                            .find(|t| t.uid == account_id)
                            .cloned()
                        {
                            if let Some(cm) = ticket.account.cookie_manager.clone() {
                                let req = TaskRequest::GetTicketInfoRequest(GetTicketInfoRequest {
                                    task_id: String::new(),
                                    uid: ticket.uid,
                                    project_id: ticket.project_id.clone(),
                                    cookie_manager: cm,
                                    referer_link: ticket.referer.clone(),
                                });
                                match self.task_manager.submit_task(req) {
                                    Ok(task_id) => {
                                        self.project_info_requesting = true;
                                        self.project_info_active_task_id = Some(task_id);
                                        self.project_info_last_request_time = Some(Instant::now());
                                    }
                                    Err(e) => {
                                        self.project_info_active_task_id = None;
                                        self.project_info_error =
                                            Some(format!("提交项目请求失败: {}", e));
                                    }
                                }
                            } else {
                                self.project_info_error = Some(if self.initializing_account_uids.contains(&account_id) {
                                    "账号客户端正在初始化，请稍后点击重试".to_string()
                                } else {
                                    "账号客户端未初始化，请重新登录".to_string()
                                });
                            }
                        }
                    }
                }
                windows::screen_info::show(self, ctx, account_id);
            }
        }

        if let Some(uid_str) = self.confirm_ticket_info.clone() {
            let confirm_uid = uid_str.parse::<i64>().unwrap_or(0);
            let ticket_opt = self
                .bilibiliticket_list
                .iter()
                .find(|t| t.uid == confirm_uid)
                .cloned();

            match ticket_opt {
                None => {
                    log::warn!(
                        "确认信息窗口未找到账号ID {} 的抢票对象，自动关闭",
                        confirm_uid
                    );
                    self.confirm_ticket_info = None;
                    self.buyer_info_requesting = false;
                    self.buyer_info_active_task_id = None;
                    self.is_loading = false;
                    self.reset_buyer_selection();
                }
                Some(ticket) => {
                    // 切换目标账号时立即清空上一个账号的购票人选择
                    if self.selected_buyer_uid != Some(confirm_uid) {
                        self.selected_buyer_list = None;
                        self.selected_buyer_uid = Some(confirm_uid);
                    }

                    let mut id_bind = ticket
                        .project_info
                        .as_ref()
                        .and_then(|p| p.id_bind)
                        .unwrap_or(9);
                    if ticket.method == 2 {
                        id_bind = 1;
                    }

                    if (id_bind == 1 || id_bind == 2)
                        && ticket.all_buyer_info.is_none()
                        && !self.buyer_info_requesting
                        && self.buyer_info_error.is_none()
                    {
                        let cooldown_ok = self
                            .buyer_info_last_request_time
                            .map(|t| t.elapsed() >= Duration::from_secs(2))
                            .unwrap_or(true);
                        if cooldown_ok {
                            if let Some(cm) = ticket.account.cookie_manager.clone() {
                                let req = TaskRequest::GetBuyerInfoRequest(GetBuyerInfoRequest {
                                    task_id: String::new(),
                                    uid: ticket.uid,
                                    cookie_manager: cm,
                                });
                                match self.task_manager.submit_task(req) {
                                    Ok(task_id) => {
                                        self.buyer_info_requesting = true;
                                        self.buyer_info_active_task_id = Some(task_id);
                                        self.buyer_info_last_request_time = Some(Instant::now());
                                    }
                                    Err(e) => {
                                        self.buyer_info_active_task_id = None;
                                        self.buyer_info_error =
                                            Some(format!("提交购票人请求失败: {}", e));
                                    }
                                }
                            } else {
                                self.buyer_info_error = Some(if self.initializing_account_uids.contains(&confirm_uid) {
                                    "账号客户端正在初始化，请稍后点击重试".to_string()
                                } else {
                                    "账号客户端未初始化，请重新登录".to_string()
                                });
                            }
                        }
                    }

                    match ticket.method {
                        0 | 1 => windows::confirm_ticket::show(self, ctx, &confirm_uid),
                        2 => windows::confirm_ticket2::show(self, ctx, &confirm_uid),
                        other => {
                            log::error!("未知的抢票方式: {}", other);
                            self.confirm_ticket_info = None;
                            self.buyer_info_requesting = false;
                            self.buyer_info_active_task_id = None;
                            self.reset_buyer_selection();
                        }
                    }
                }
            }
        }

        if !self.pending_pay_qrs.is_empty() {
            windows::show_qrcode::show(self, ctx);
        }

        // 4. 加载遮罩与通知横幅绘制在最顶层
        if self.is_loading {
            ui::loading::render_loading_overlay(self, ctx);
        }

        let banner_animating = self.update_banner_animation();
        if banner_animating {
            render_notification_banner(self, ctx);
        }

        // 5. 稳定 IME 光标输出与动态按需重绘，避免完全空闲时持续高频消耗 CPU/GPU
        self.stabilize_ime_output(ctx);

        if self.sms_cooldown_until.is_some_and(|t| t <= Instant::now()) {
            self.sms_cooldown_until = None;
        }
        let has_active_bg = !self.task_manager.list_running_tasks().is_empty()
            || !self.initializing_account_uids.is_empty()
            || !self.signout_pending_uids.is_empty()
            || self.orderlist_requesting
            || self.project_info_requesting
            || self.buyer_info_requesting
            || self.login_qr_requesting
            || self.qrcode_polling_task_id.is_some()
            || self.login_submitting
            || self.sms_sending
            || self.add_buyer_submitting
            || self.sms_cooldown_until.is_some()
            || self
                .image_cache
                .values()
                .any(|s| matches!(s, ImageLoadState::Loading));

        if banner_animating || self.is_loading {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else if has_active_bg || self.show_log_window {
            ctx.request_repaint_after(Duration::from_millis(150));
        } else {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.settings_dirty {
            let _ = self.auto_save_settings();
        }
        log::info!("程序即将退出，正在优雅关闭后台任务...");
        self.task_manager.shutdown();
        log::logger().flush();
    }
}

pub fn create_client(user_agent: String) -> Client {
    let mut headers = header::HeaderMap::new();
    let ua_val = header::HeaderValue::from_str(&user_agent).unwrap_or_else(|_| {
        header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
    });
    headers.insert(header::USER_AGENT, ua_val);

    Client::builder()
        .default_headers(headers)
        .cookie_store(true)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_default()
}

/// 将图片字节解码为 `egui::ColorImage`，可选应用真实圆形抗锯齿遮罩（用于头像）
pub fn decode_image_to_color_image(
    bytes: &[u8],
    circular_mask: bool,
) -> Result<egui::ColorImage, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("图片格式解码失败: {}", e))?;
    let mut rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return Err("图片尺寸无效".to_string());
    }

    if circular_mask {
        // 先居中裁剪为正方形再做平滑圆形 alpha 遮罩
        let side = w.min(h);
        let x0 = (w - side) / 2;
        let y0 = (h - side) / 2;
        let mut square = image::imageops::crop_imm(&rgba, x0, y0, side, side).to_image();
        let center = (side as f32 - 1.0) / 2.0;
        let radius = side as f32 / 2.0;
        for (x, y, px) in square.enumerate_pixels_mut() {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > radius {
                px[3] = 0;
            } else if dist > radius - 1.5 {
                let factor = ((radius - dist) / 1.5).clamp(0.0, 1.0);
                px[3] = (px[3] as f32 * factor) as u8;
            }
        }
        rgba = square;
    }

    let (fw, fh) = rgba.dimensions();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [fw as usize, fh as usize],
        rgba.as_raw(),
    ))
}

#[derive(Debug, Serialize, Deserialize)]
struct PolicyPayload {
    policy: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct PermissionsPayload {
    permissions: Value,
}

fn decode_policy(token: &str, public_key: &str) -> Result<Value, String> {
    let decoding_key = DecodingKey::from_rsa_pem(public_key.as_bytes())
        .map_err(|e| format!("无效的公钥: {}", e))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = false;
    validation.required_spec_claims.clear();

    decode::<PolicyPayload>(token, &decoding_key, &validation)
        .map(|token_data| token_data.claims.policy)
        .map_err(|e| format!("解码JWT失败: {}", e))
}

fn decode_permissions(token: &str, public_key: &str) -> Result<Value, String> {
    let decoding_key = DecodingKey::from_rsa_pem(public_key.as_bytes())
        .map_err(|e| format!("无效的公钥: {}", e))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = false;
    validation.required_spec_claims.clear();

    decode::<PermissionsPayload>(token, &decoding_key, &validation)
        .map(|token_data| token_data.claims.permissions)
        .map_err(|e| format!("解码JWT失败: {}", e))
}

fn load_local_permissions(public_key: &str) -> Value {
    if let Ok(mut file) = File::open("permissions") {
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_ok() {
            if let Ok(decoded) = decode_permissions(&contents, public_key) {
                return decoded;
            }
        }
    }
    json!({})
}

fn generate_random_string(length: usize) -> String {
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};

    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(|c| c as char)
        .collect()
}

/// 将中文输入法可能输出的全角 ASCII 字符（０-９、Ａ-Ｚ、ａ-ｚ、标点）和全角空格转换为标准半角 ASCII，
/// 保留中文汉字与其他 Unicode 字符不变。
pub fn normalize_fullwidth_ascii(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            '\u{3000}' => ' ',
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            other => other,
        })
        .collect()
}

/// 修正 `egui 0.23.0` + `egui-winit 0.23.0` 在中文输入法（IME）下的三个核心缺陷：
/// 1. 孤立 `CompositionEnd(text)`（未先收到 `CompositionStart` 时被 `egui::TextEdit` 整段丢弃）-> 改写为 `Event::Text(text)`；
/// 2. 孤立 `CompositionUpdate(text)` -> 在其前补发 `Event::CompositionStart`；
/// 3. 与组合事件同帧到达的 `Enter`/`Escape`/`Tab`/`Backspace`（会导致单行输入框在落字前先失焦并丢弃内容）-> 在组合帧内屏蔽。
///
/// 返回当前帧是否包含活跃的 IME 组合事件。
pub(crate) fn normalize_ime_events(
    events: &mut Vec<egui::Event>,
    ime_composing: &mut bool,
) -> bool {
    let frame_has_composition = events.iter().any(|e| {
        matches!(
            e,
            egui::Event::CompositionStart
                | egui::Event::CompositionUpdate(_)
                | egui::Event::CompositionEnd(_)
        )
    });

    if !frame_has_composition && !*ime_composing {
        return false;
    }

    let mut out = Vec::with_capacity(events.len() + 2);
    for event in std::mem::take(events) {
        match event {
            egui::Event::WindowFocused(false) => {
                *ime_composing = false;
                out.push(event);
            }
            egui::Event::PointerButton { pressed: true, .. } if !frame_has_composition => {
                *ime_composing = false;
                out.push(event);
            }
            egui::Event::CompositionStart => {
                *ime_composing = true;
                out.push(egui::Event::CompositionStart);
            }
            egui::Event::CompositionUpdate(text) => {
                if !*ime_composing {
                    if text.is_empty() {
                        continue;
                    }
                    *ime_composing = true;
                    out.push(egui::Event::CompositionStart);
                }
                out.push(egui::Event::CompositionUpdate(text));
            }
            egui::Event::CompositionEnd(text) => {
                if *ime_composing {
                    *ime_composing = false;
                    out.push(egui::Event::CompositionEnd(text));
                } else if !text.is_empty() && text != "\n" && text != "\r" {
                    out.push(egui::Event::Text(text));
                }
            }
            egui::Event::Text(text) => {
                if *ime_composing && !frame_has_composition {
                    // 外部输入法已取消组合（egui-winit 忽略了 Ime::Disabled）并恢复直接字符输入
                    *ime_composing = false;
                    out.push(egui::Event::CompositionEnd(String::new()));
                }
                out.push(egui::Event::Text(text));
            }
            egui::Event::Key { key, pressed, .. }
                if frame_has_composition
                    && matches!(
                        key,
                        egui::Key::Enter
                            | egui::Key::Escape
                            | egui::Key::Tab
                            | egui::Key::Backspace
                    ) =>
            {
                // 同帧内存在 IME 组合/上屏事件时，吞掉可能提前触发失焦或误删的按键事件
                let _ = pressed;
            }
            egui::Event::Key {
                key: egui::Key::Escape | egui::Key::Enter | egui::Key::Tab,
                pressed: true,
                ..
            } if !frame_has_composition && *ime_composing => {
                *ime_composing = false;
                out.push(event);
            }
            other => out.push(other),
        }
    }

    *events = out;
    frame_has_composition
}

#[cfg(test)]
mod tests {
    use super::{
        decode_image_to_color_image, normalize_fullwidth_ascii, normalize_ime_events,
    };
    use eframe::egui;

    #[test]
    fn circular_mask_makes_corners_transparent_and_center_opaque() {
        let img = image::RgbaImage::from_pixel(16, 16, image::Rgba([255, 0, 0, 255]));
        let mut png_bytes = Vec::new();
        {
            let mut cursor = std::io::Cursor::new(&mut png_bytes);
            img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        }
        let color = decode_image_to_color_image(&png_bytes, true).unwrap();
        assert_eq!(color.size, [16, 16]);
        // 左上角应在圆外 -> 透明
        assert_eq!(color.pixels[0].a(), 0);
        // 中心应在圆内 -> 不透明
        assert_eq!(color.pixels[8 * 16 + 8].a(), 255);
    }

    #[test]
    fn normalize_fullwidth_ascii_converts_digits_and_url_punctuation() {
        assert_eq!(normalize_fullwidth_ascii("１００３００"), "100300");
        assert_eq!(
            normalize_fullwidth_ascii("ｈｔｔｐｓ：／／ｓｈｏｗ．ｂｉｌｉｂｉｌｉ．ｃｏｍ？ｉｄ＝１２３４５"),
            "https://show.bilibili.com?id=12345"
        );
        assert_eq!(normalize_fullwidth_ascii("张三　１３８００１３８０００"), "张三 13800138000");
    }

    #[test]
    fn normalize_ime_events_fixes_orphan_commit_and_blocks_enter_during_composition() {
        // 1. 孤立 CompositionEnd（has_ime == false）必须转为 Event::Text，避免被 egui::TextEdit 丢弃
        let mut composing = false;
        let mut events = vec![egui::Event::CompositionEnd("你好123".to_string())];
        assert!(normalize_ime_events(&mut events, &mut composing));
        assert!(!composing);
        assert_eq!(events, vec![egui::Event::Text("你好123".to_string())]);

        // 2. 孤立 CompositionUpdate 必须补发 CompositionStart
        let mut events = vec![egui::Event::CompositionUpdate("ni".to_string())];
        assert!(normalize_ime_events(&mut events, &mut composing));
        assert!(composing);
        assert_eq!(
            events,
            vec![
                egui::Event::CompositionStart,
                egui::Event::CompositionUpdate("ni".to_string()),
            ]
        );

        // 3. 组合中按 Enter 上屏：必须吞掉 Enter，保留 CompositionEnd
        let mut events = vec![
            egui::Event::Key {
                key: egui::Key::Enter,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::CompositionEnd("nihao".to_string()),
        ];
        assert!(normalize_ime_events(&mut events, &mut composing));
        assert!(!composing);
        assert_eq!(events, vec![egui::Event::CompositionEnd("nihao".to_string())]);

        // 4. 非组合帧的正常 Enter 不得被吞掉
        let mut events = vec![egui::Event::Key {
            key: egui::Key::Enter,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }];
        assert!(!normalize_ime_events(&mut events, &mut composing));
        assert_eq!(events.len(), 1);
    }
}
