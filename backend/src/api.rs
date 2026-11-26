use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use common::cookie_manager::CookieManager;
use common::gen_cp::CTokenGenerator;
use common::login::QrCodeLoginStatus;
use common::ticket::*;
use rand::{thread_rng, Rng};
use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::watch;

/// 内部错误码：网络传输/超时失败（与 HTTP 412 风控严格区分）
pub const ERR_NETWORK: i32 = -1001;
/// 内部错误码：响应 JSON 解析失败或关键字段缺失
pub const ERR_PARSE: i32 = -1002;
/// 内部错误码：下单接口返回 errno=0 但 msg 含 defaultBBR 降级标记（实际未成单）
pub const ERR_DEGRADED_BBR: i32 = -1003;
/// 内部错误码：本地传参错误（不可通过重试修复）
pub const ERR_INVALID_PARAM: i32 = 999;
/// 内部错误码：id_bind 字段值非法
pub const ERR_ID_BIND_INVALID: i32 = 919;

const CREATE_ORDER_V2_URL: &str = "https://show.bilibili.com/api/ticket/order/createV2";
const GET_PAY_PARAM_URL: &str = "https://show.bilibili.com/api/ticket/order/getPayParam";
/// 获取 PC 扫码支付链接 (`code_url`) 专用桌面端 UA：
/// 当用户在设置中开启移动端自定义 UA（Android/Mobile）时，B站 createstatus/getPayParam
/// 会按移动端 H5（deviceType=2）下发收银台签名参数而不返回 code_url；
/// 仅在已下单成功后查询 getPayParam 时以标准 PC Chrome UA 请求，即可拿到直扫支付链接。
const PC_PAY_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateOrderFailure {
    pub code: i32,
    pub message: String,
    pub updated_pay_money: Option<i64>,
}

impl CreateOrderFailure {
    pub fn from_code(code: i32) -> Self {
        Self {
            code,
            message: String::new(),
            updated_pay_money: None,
        }
    }

    pub fn from_code_msg(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            updated_pay_money: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayParamError {
    /// 服务端明确返回订单不存在 / 已关闭 / 已取消（属于假票或已被回滚）
    OrderNotFound { code: i64, message: String },
    /// 订单可能有效但无直扫二维码（如仅支持App支付、0元票、未返回code_url或临时网络异常）
    Unavailable(String),
}

impl std::fmt::Display for PayParamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PayParamError::OrderNotFound { code, message } => {
                write!(f, "订单不存在或已失效({}): {}", code, message)
            }
            PayParamError::Unavailable(msg) => write!(f, "{}", msg),
        }
    }
}

/// 过滤掉 Cookie 中仅用于移动端/App 场景的来源标识，保留全部登录与风控凭据，
/// 避免 `getPayParam` 将请求识别为移动端 H5（deviceType=2）从而省略 `code_url`。
pub(crate) fn build_pc_pay_cookie_header(all_cookies: &str) -> String {
    all_cookies
        .split(';')
        .map(str::trim)
        .filter(|pair| !pair.is_empty())
        .filter(|pair| {
            let key = pair
                .split_once('=')
                .map(|(k, _)| k.trim())
                .unwrap_or("");
            !key.eq_ignore_ascii_case("msource")
                && !key.eq_ignore_ascii_case("kfcsource")
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// 构造 B站会员购订单详情页兜底链接（仅在接口确实未下发第三方直扫 code_url 时兜底）
pub fn build_order_detail_fallback_url(order_id: &str) -> String {
    format!(
        "https://show.bilibili.com/platform/orderDetail.html?order_id={}",
        order_id.trim()
    )
}

/// 解析 `/api/ticket/order/getPayParam` 响应，提取 `code_url`（同时兼容顶层/嵌套及常见别名字段）
pub(crate) fn parse_pay_param_response(value: &Value) -> Result<CheckFakeResultData, PayParamError> {
    if let Some((err_code, err_msg)) = extract_api_error(value) {
        let is_not_found = matches!(err_code, 100018 | 100009)
            || err_msg.contains("订单不存在")
            || err_msg.contains("订单已取消")
            || err_msg.contains("订单已关闭")
            || err_msg.contains("订单已失效");
        if is_not_found {
            return Err(PayParamError::OrderNotFound {
                code: err_code,
                message: err_msg,
            });
        }
        return Err(PayParamError::Unavailable(format!(
            "getPayParam 失败({}): {}",
            err_code, err_msg
        )));
    }

    let Some(data) = value.get("data").filter(|d| d.is_object()) else {
        return Err(PayParamError::Unavailable(
            "getPayParam 响应缺少有效 data 对象".to_string(),
        ));
    };

    fn extract_url(obj: &Value) -> Option<&str> {
        for key in ["code_url", "codeUrl", "pay_url", "payUrl", "qrcode_url"] {
            if let Some(s) = obj
                .get(key)
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                return Some(s);
            }
        }
        None
    }

    let code_url = extract_url(data)
        .or_else(|| data.get("payParam").and_then(extract_url))
        .unwrap_or("");

    if code_url.is_empty() {
        let device_type_hint = data
            .get("deviceType")
            .or_else(|| data.get("payParam").and_then(|p| p.get("deviceType")))
            .and_then(|v| v.as_i64())
            .map(|dt| format!(" (deviceType={})", dt))
            .unwrap_or_default();
        return Err(PayParamError::Unavailable(format!(
            "getPayParam 未返回有效 code_url{}",
            device_type_hint
        )));
    }

    let sign = data
        .get("sign")
        .and_then(|v| v.as_str())
        .or_else(|| {
            data.get("payParam")
                .and_then(|p| p.get("sign"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("")
        .to_string();

    Ok(CheckFakeResultData {
        sign,
        code_url: code_url.to_string(),
    })
}

/// 检测 B站 createV2 在 errno=0 时返回的 `defaultBBR` 降级假成功响应
pub(crate) fn is_create_order_degraded_response(value: &Value) -> bool {
    let msg = value
        .get("msg")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("message").and_then(|v| v.as_str()))
        .unwrap_or("");
    msg.contains("defaultBBR")
}

/// 在 error=100034（票价错误）时提取接口返回的最新应付金额（单位：分）
pub(crate) fn extract_updated_pay_money(value: &Value) -> Option<i64> {
    value
        .get("data")
        .and_then(|d| d.get("pay_money"))
        .and_then(|v| v.as_i64())
        .filter(|&m| m > 0)
}

const DEFAULT_QRCODE_POLL_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub(crate) fn extract_api_message(json: &Value) -> String {
    json.get("msg")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            json.get("message")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            json.get("data")
                .and_then(|d| d.get("message"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            json.get("data")
                .and_then(|d| d.get("msg"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or("未知错误")
        .to_string()
}

/// 统一检查 B站接口返回中的 `errno` 与 `code`：
/// - 只要任一存在且非 0，返回 `Some((err_code, msg))`
/// - 若 `errno` 与 `code` 同时缺失，也判定为错误并返回 `Some((-1, msg))`
/// - 绝不将真正的 `-1` 错误码与字段缺失混淆
pub(crate) fn extract_api_error(json: &Value) -> Option<(i64, String)> {
    let errno = json.get("errno").and_then(|v| v.as_i64());
    let code = json.get("code").and_then(|v| v.as_i64());

    match (errno, code) {
        (Some(e), _) if e != 0 => Some((e, extract_api_message(json))),
        (_, Some(c)) if c != 0 => Some((c, extract_api_message(json))),
        (None, None) => Some((-1, extract_api_message(json))),
        _ => None,
    }
}

/// 从 createV2 响应中提取订单号，兼容 i64 / u64 / 大整数字符串雪花 ID
pub fn extract_order_id(order_result: &Value) -> Option<String> {
    let val = order_result
        .get("data")
        .and_then(|d| d.get("orderId").or_else(|| d.get("order_id")))
        .or_else(|| order_result.get("orderId"))
        .or_else(|| order_result.get("order_id"))?;

    if let Some(s) = val.as_str() {
        let trimmed = s.trim();
        if !trimmed.is_empty() && trimmed != "0" {
            return Some(trimmed.to_string());
        }
        return None;
    }
    if let Some(n) = val.as_i64() {
        if n > 0 {
            return Some(n.to_string());
        }
        return None;
    }
    if let Some(n) = val.as_u64() {
        if n > 0 {
            return Some(n.to_string());
        }
        return None;
    }
    None
}

fn sanitize_cache_id(project_id: &str) -> String {
    let sanitized: String = project_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

fn cache_path(project_id: &str) -> PathBuf {
    let safe_id = sanitize_cache_id(project_id);
    Path::new("cache").join(format!("project_{}.json", safe_id))
}

async fn save_project_cache_async(project_id: String, info: InfoResponse) {
    let _ = tokio::task::spawn_blocking(move || {
        let path = cache_path(&project_id);
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                log::warn!("创建项目缓存目录失败: {}", e);
                return;
            }
        }
        let json = match serde_json::to_string_pretty(&info) {
            Ok(j) => j,
            Err(e) => {
                log::warn!("序列化项目缓存失败: {}", e);
                return;
            }
        };
        let tmp_name = format!(
            "project_{}.{}.tmp",
            sanitize_cache_id(&project_id),
            uuid::Uuid::new_v4()
        );
        let tmp_path = path
            .parent()
            .map(|p| p.join(&tmp_name))
            .unwrap_or_else(|| PathBuf::from(&tmp_name));

        if let Err(e) = std::fs::write(&tmp_path, &json) {
            log::warn!("写入项目缓存临时文件失败: {}", e);
            return;
        }
        if let Err(e) = std::fs::rename(&tmp_path, &path) {
            log::warn!("原子替换项目缓存失败: {}", e);
            let _ = std::fs::remove_file(&tmp_path);
        } else {
            log::info!("已缓存项目信息到 {:?}", path);
        }
    })
    .await;
}

pub(crate) fn parse_and_validate_project_cache(json: &str) -> Result<InfoResponse, String> {
    let mut info = serde_json::from_str::<InfoResponse>(json)
        .map_err(|e| format!("解析本地缓存失败: {}", e))?;
    info.data.fill_missing_sale_begin();
    info.data.validate()?;
    Ok(info)
}

async fn load_project_cache_async(project_id: String) -> Option<InfoResponse> {
    tokio::task::spawn_blocking(move || {
        let path = cache_path(&project_id);
        match std::fs::read_to_string(&path) {
            Ok(json) => match parse_and_validate_project_cache(&json) {
                Ok(info) => {
                    log::info!("已从本地缓存加载项目信息: {:?}", path);
                    Some(info)
                }
                Err(e) => {
                    log::warn!("本地缓存 {:?} 无效，已跳过: {}", path, e);
                    None
                }
            },
            Err(_) => None,
        }
    })
    .await
    .ok()
    .flatten()
}

pub(crate) fn parse_server_now_response(text: &str) -> Result<i64, String> {
    let json_data: Value =
        serde_json::from_str(text).map_err(|e| format!("JSON解析失败: {}", e))?;
    let now_sec = json_data["data"]["now"].as_i64().unwrap_or(0);
    if now_sec > 0 {
        Ok(now_sec)
    } else {
        Err("返回时间戳为0或缺失".to_string())
    }
}

/// 严格获取 B 站服务器秒级时间戳；失败时返回 Err，绝不静默回退到本地时钟。
pub async fn fetch_server_now_sec(cookie_manager: Arc<CookieManager>) -> Result<i64, String> {
    let url = "https://api.bilibili.com/x/click-interface/click/now";
    let response = cookie_manager.get(url).await;
    let resp = response
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    log::debug!("时间接口原始响应：{}", text);

    let now_sec = parse_server_now_response(&text)?;
    log::debug!("解析出的网络时间(秒级)：{}", now_sec);
    Ok(now_sec)
}

pub async fn get_countdown_from_ts(
    cookie_manager: Arc<CookieManager>,
    sale_begin_sec: i64,
) -> Result<f64, String> {
    log::debug!("获取开始时间(秒级)：{}", sale_begin_sec);

    let now_sec = match fetch_server_now_sec(cookie_manager).await {
        Ok(ts) => ts,
        Err(reason) => {
            let local_sec = chrono::Utc::now().timestamp();
            log::warn!(
                "获取B站服务器时间失败({})，已回退使用本地系统时间({})，请确保本机时钟已校时",
                reason,
                local_sec
            );
            local_sec
        }
    };

    let countdown_sec = sale_begin_sec - now_sec;
    log::debug!(
        "计算倒计时(秒)：开始时间[{}] - 当前时间[{}] = 倒计时[{}]秒",
        sale_begin_sec,
        now_sec,
        countdown_sec
    );

    Ok(countdown_sec as f64)
}

pub async fn get_countdown(
    cookie_manager: Arc<CookieManager>,
    info: Option<TicketInfo>,
) -> Result<f64, String> {
    let sale_begin_sec = match info {
        Some(info) => match info.sale_begin {
            Some(v) => v,
            None => return Err("开售时间(sale_begin)为空".to_string()),
        },
        None => return Err("获取开始时间失败".to_string()),
    };
    get_countdown_from_ts(cookie_manager, sale_begin_sec).await
}

pub async fn get_buyer_info(
    cookie_manager: Arc<CookieManager>,
) -> Result<BuyerInfoResponse, String> {
    let req = cookie_manager
        .get("https://show.bilibili.com/api/ticket/buyer/list")
        .await;
    let resp = req
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    if !resp.status().is_success() {
        log::debug!("获取购票人请求响应失败: {:?}", resp.status());
        return Err(format!("请求响应失败: {}", resp.status()));
    }

    let text = resp
        .text()
        .await
        .map_err(|e| format!("获取购票人信息失败：{}", e))?;
    log::debug!("获取购票人信息：{}", text);

    let raw: Value = serde_json::from_str(&text)
        .map_err(|e| format!("获取购票人信息json解析失败：{}", e))?;
    if let Some((err_code, msg)) = extract_api_error(&raw) {
        return Err(format!("获取购票人信息失败({})：{}", err_code, msg));
    }

    let buyer_info: BuyerInfoResponse = serde_json::from_value(raw)
        .map_err(|e| format!("获取购票人信息数据解析失败：{}", e))?;

    if buyer_info.errno != 0 || buyer_info.code != 0 {
        let err_code = if buyer_info.errno != 0 {
            buyer_info.errno
        } else {
            buyer_info.code
        };
        let msg = if !buyer_info.msg.is_empty() {
            buyer_info.msg.clone()
        } else if !buyer_info.message.is_empty() {
            buyer_info.message.clone()
        } else {
            "未知错误".to_string()
        };
        return Err(format!("获取购票人信息失败({})：{}", err_code, msg));
    }

    Ok(buyer_info)
}

pub async fn add_buyer(
    cookie_manager: Arc<CookieManager>,
    name: &str,
    phone: &str,
    id_type: i32,
    id_number: &str,
    as_default_buyer: bool,
) -> Result<String, String> {
    let name = name.trim();
    let phone = phone.trim();
    let id_number = id_number.trim();

    if name.is_empty() {
        return Err("购票人姓名不能为空".to_string());
    }
    if phone.is_empty() {
        return Err("购票人手机号不能为空".to_string());
    }
    if id_number.is_empty() {
        return Err("购票人证件号码不能为空".to_string());
    }
    if !(0..=3).contains(&id_type) {
        return Err(format!("不支持的证件类型: {}", id_type));
    }

    let payload = json!({
        "name": name,
        "tel": phone,
        "id_type": id_type,
        "personal_id": id_number,
        "is_default": if as_default_buyer { "1" } else { "0" },
        "src": "ticket",
    });

    let resp = cookie_manager
        .post("https://show.bilibili.com/api/ticket/buyer/create")
        .await
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("添加购票人请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("添加购票人 HTTP 请求失败: {}", resp.status()));
    }

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取添加购票人响应失败: {}", e))?;

    let json_val: Value = serde_json::from_str(&text)
        .map_err(|e| format!("解析添加购票人响应失败: {}", e))?;

    if let Some((err_code, msg)) = extract_api_error(&json_val) {
        return Err(format!("添加购票人失败({}): {}", err_code, msg));
    }

    Ok("添加购票人成功".to_string())
}

pub async fn get_project_with_options(
    cookie_manager: Arc<CookieManager>,
    project_id: &str,
    referer_link: Option<String>,
    allow_cache_fallback: bool,
) -> Result<InfoResponse, String> {
    let base_url = "https://show.bilibili.com/api/ticket/project/getV2";
    let req = match referer_link.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(referer) => {
            let headers: HashMap<&str, &str> = HashMap::from([("Referer", referer)]);
            cookie_manager.get_with_headers(base_url, headers).await
        }
        None => cookie_manager.get(base_url).await,
    };

    let response = req
        .query(&[("id", project_id), ("project_id", project_id)])
        .send()
        .await;

    let result = match response {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.text().await {
                    Ok(text) => {
                        log::debug!("获取项目详情：{}", text);
                        match serde_json::from_str::<Value>(&text) {
                            Ok(raw) => {
                                if let Some((err_code, msg)) = extract_api_error(&raw) {
                                    Err(format!("获取项目详情失败({})：{}", err_code, msg))
                                } else {
                                    match serde_json::from_value::<InfoResponse>(raw) {
                                        Ok(mut ticket_info) => {
                                            if ticket_info.errno != 0 || ticket_info.code != 0 {
                                                let err_code = if ticket_info.errno != 0 {
                                                    ticket_info.errno
                                                } else {
                                                    ticket_info.code
                                                };
                                                let msg = if !ticket_info.msg.is_empty() {
                                                    ticket_info.msg.clone()
                                                } else if !ticket_info.message.is_empty() {
                                                    ticket_info.message.clone()
                                                } else {
                                                    "未知错误".to_string()
                                                };
                                                Err(format!(
                                                    "获取项目详情失败({})：{}",
                                                    err_code, msg
                                                ))
                                            } else {
                                                ticket_info.data.fill_missing_sale_begin();
                                                if let Err(e) = ticket_info.data.validate() {
                                                    Err(e)
                                                } else {
                                                    Ok(ticket_info)
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("获取项目详情json解析失败：{}", e);
                                            Err(format!("获取项目详情json解析失败：{}", e))
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("获取项目详情json解析失败：{}", e);
                                Err(format!("获取项目详情json解析失败：{}", e))
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("获取项目详情失败：{}", e);
                        Err(format!("获取项目详情失败：{}", e))
                    }
                }
            } else {
                log::debug!("请求响应失败: {:?}", resp.status());
                Err(format!("请求响应失败: {}", resp.status()))
            }
        }
        Err(e) => Err(format!("请求失败: {}", e)),
    };

    match result {
        Ok(info) => {
            if allow_cache_fallback {
                save_project_cache_async(project_id.to_string(), info.clone()).await;
            }
            Ok(info)
        }
        Err(e) => {
            if allow_cache_fallback {
                log::warn!("在线获取项目信息失败({}), 尝试从本地缓存加载...", e);
                match load_project_cache_async(project_id.to_string()).await {
                    Some(cached) => {
                        log::info!("已降级使用本地缓存的项目信息");
                        Ok(cached)
                    }
                    None => Err(e),
                }
            } else {
                Err(e)
            }
        }
    }
}

pub async fn get_project(
    cookie_manager: Arc<CookieManager>,
    project_id: &str,
    referer_link: &str,
) -> Result<InfoResponse, String> {
    let referer = if referer_link.trim().is_empty() {
        None
    } else {
        Some(referer_link.to_string())
    };
    get_project_with_options(cookie_manager, project_id, referer, true).await
}

fn upsert_cookie(cookies: &mut Vec<(String, String)>, name: &str, value: &str) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    if let Some(existing) = cookies.iter_mut().find(|(k, _)| k == name) {
        existing.1 = value.to_string();
    } else {
        cookies.push((name.to_string(), value.to_string()));
    }
}

async fn wait_cancel_rx(cancel_rx: &mut watch::Receiver<bool>) {
    if *cancel_rx.borrow() {
        return;
    }
    while cancel_rx.changed().await.is_ok() {
        if *cancel_rx.borrow() {
            return;
        }
    }
    // 发送端已被丢弃且未发送 true，避免误触发取消
    std::future::pending::<()>().await;
}

async fn poll_qrcode_login_with_config(
    poll_url: &str,
    qrcode_key: &str,
    user_agent: Option<&str>,
    poll_interval: Duration,
    mut cancel_rx: watch::Receiver<bool>,
    mut on_scanning: impl FnMut(),
) -> QrCodeLoginStatus {
    if *cancel_rx.borrow() {
        return QrCodeLoginStatus::Failed("任务已取消".to_string());
    }

    let ua = user_agent
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_QRCODE_POLL_UA);

    let mut client_builder = Client::builder()
        .user_agent(ua)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(15));
    if poll_url.starts_with("http://127.0.0.1") || poll_url.starts_with("http://localhost") {
        client_builder = client_builder.no_proxy();
    }
    let client = client_builder.build().unwrap_or_default();

    let max_attempts: u32 = 60;
    const MAX_CONSECUTIVE_ERRORS: u32 = 3;
    let mut consecutive_errors: u32 = 0;
    let mut scanning_notified = false;

    for attempt in 1..=max_attempts {
        if *cancel_rx.borrow() {
            return QrCodeLoginStatus::Failed("任务已取消".to_string());
        }

        let send_fut = client
            .get(poll_url)
            .query(&[("qrcode_key", qrcode_key)])
            .send();

        let response = tokio::select! {
            res = send_fut => match res {
                Ok(resp) => resp,
                Err(e) => {
                    consecutive_errors += 1;
                    log::warn!(
                        "轮询二维码状态网络错误 ({}/{}): {}",
                        consecutive_errors,
                        MAX_CONSECUTIVE_ERRORS,
                        e
                    );
                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                        return QrCodeLoginStatus::Failed(format!("网络错误: {}", e));
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(poll_interval) => {}
                        _ = wait_cancel_rx(&mut cancel_rx) => {
                            return QrCodeLoginStatus::Failed("任务已取消".to_string());
                        }
                    }
                    continue;
                }
            },
            _ = wait_cancel_rx(&mut cancel_rx) => {
                return QrCodeLoginStatus::Failed("任务已取消".to_string());
            }
        };

        if !response.status().is_success() {
            consecutive_errors += 1;
            let status = response.status();
            log::warn!(
                "轮询二维码状态返回非成功 HTTP 状态 ({}/{}): {}",
                consecutive_errors,
                MAX_CONSECUTIVE_ERRORS,
                status
            );
            if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                return QrCodeLoginStatus::Failed(format!("服务器返回错误状态: {}", status));
            }
            tokio::select! {
                _ = tokio::time::sleep(poll_interval) => {}
                _ = wait_cancel_rx(&mut cancel_rx) => {
                    return QrCodeLoginStatus::Failed("任务已取消".to_string());
                }
            }
            continue;
        }

        let mut deduped_cookies: Vec<(String, String)> = Vec::new();
        for value in response.headers().get_all(reqwest::header::SET_COOKIE) {
            if let Ok(cookie_str) = value.to_str() {
                let kv = match cookie_str.find(';') {
                    Some(end_pos) => &cookie_str[0..end_pos],
                    None => cookie_str,
                };
                if let Some((k, v)) = kv.split_once('=') {
                    upsert_cookie(&mut deduped_cookies, k, v);
                }
            }
        }

        let json = tokio::select! {
            res = response.json::<Value>() => match res {
                Ok(j) => j,
                Err(e) => {
                    consecutive_errors += 1;
                    log::warn!(
                        "解析二维码轮询响应失败 ({}/{}): {}",
                        consecutive_errors,
                        MAX_CONSECUTIVE_ERRORS,
                        e
                    );
                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                        return QrCodeLoginStatus::Failed(format!("解析响应失败: {}", e));
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(poll_interval) => {}
                        _ = wait_cancel_rx(&mut cancel_rx) => {
                            return QrCodeLoginStatus::Failed("任务已取消".to_string());
                        }
                    }
                    continue;
                }
            },
            _ = wait_cancel_rx(&mut cancel_rx) => {
                return QrCodeLoginStatus::Failed("任务已取消".to_string());
            }
        };

        // 收到合法响应后清零连续瞬时错误计数
        consecutive_errors = 0;

        let code = extract_qrcode_poll_code(&json);
        match code {
            0 => {
                if let Some(cookie_info) = json["data"]["cookie_info"].as_object() {
                    for (key, value) in cookie_info {
                        if let Some(val_str) = value["value"].as_str() {
                            upsert_cookie(&mut deduped_cookies, key, val_str);
                        }
                    }
                }

                if !deduped_cookies.is_empty() {
                    let cookie_str = deduped_cookies
                        .into_iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<_>>()
                        .join("; ");
                    return QrCodeLoginStatus::Success(cookie_str);
                } else {
                    return QrCodeLoginStatus::Failed("无法获取Cookie信息".to_string());
                }
            }
            86038 => return QrCodeLoginStatus::Expired,
            86090 => {
                if !scanning_notified {
                    scanning_notified = true;
                    on_scanning();
                }
                log::info!(
                    "二维码已扫描，等待确认 (尝试 {} / {} 次)",
                    attempt,
                    max_attempts
                );
            }
            86101 => {
                log::info!(
                    "二维码已生成，等待扫描 (尝试 {} / {} 次)",
                    attempt,
                    max_attempts
                );
            }
            _ => {
                let message = json["data"]["message"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .or_else(|| json["message"].as_str().filter(|s| !s.is_empty()))
                    .unwrap_or("未知错误");
                return QrCodeLoginStatus::Failed(message.to_string());
            }
        }

        if attempt < max_attempts {
            tokio::select! {
                _ = tokio::time::sleep(poll_interval) => {}
                _ = wait_cancel_rx(&mut cancel_rx) => {
                    return QrCodeLoginStatus::Failed("任务已取消".to_string());
                }
            }
        }
    }

    QrCodeLoginStatus::Expired
}

// 可取消的二维码登录轮询（在首次检测到 86090 已扫码未确认时触发一次 on_scanning 回调）
pub async fn poll_qrcode_login_with_progress(
    qrcode_key: &str,
    user_agent: Option<&str>,
    cancel_rx: watch::Receiver<bool>,
    on_scanning: impl FnMut(),
) -> QrCodeLoginStatus {
    poll_qrcode_login_with_config(
        "https://passport.bilibili.com/x/passport-login/web/qrcode/poll",
        qrcode_key,
        user_agent,
        Duration::from_secs(1),
        cancel_rx,
        on_scanning,
    )
    .await
}

// 可取消的二维码登录轮询
pub async fn poll_qrcode_login_cancellable(
    qrcode_key: &str,
    user_agent: Option<&str>,
    cancel_rx: watch::Receiver<bool>,
) -> QrCodeLoginStatus {
    poll_qrcode_login_with_progress(qrcode_key, user_agent, cancel_rx, || {}).await
}

// 兼容无取消信号调用方的轮询入口
pub async fn poll_qrcode_login(
    qrcode_key: &str,
    user_agent: Option<&str>,
) -> QrCodeLoginStatus {
    let (_tx, rx) = watch::channel(false);
    poll_qrcode_login_cancellable(qrcode_key, user_agent, rx).await
}

pub(crate) fn build_token_risk_param(
    code: i32,
    message: String,
    risk_params: &Value,
) -> TokenRiskParam {
    TokenRiskParam {
        code,
        message,
        mid: Some(risk_params["mid"].as_str().unwrap_or("").to_string()),
        decision_type: Some(
            risk_params["decision_type"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        ),
        buvid: Some(risk_params["buvid"].as_str().unwrap_or("").to_string()),
        ip: Some(risk_params["ip"].as_str().unwrap_or("").to_string()),
        scene: Some(risk_params["scene"].as_str().unwrap_or("").to_string()),
        ua: Some(risk_params["ua"].as_str().unwrap_or("").to_string()),
        v_voucher: Some(risk_params["v_voucher"].as_str().unwrap_or("").to_string()),
        risk_param: if risk_params.is_null() {
            None
        } else {
            Some(risk_params.clone())
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmOrderFailure {
    pub code: Option<i64>,
    pub message: String,
}

impl std::fmt::Display for ConfirmOrderFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.code {
            Some(code) => write!(f, "确认订单失败({}): {}", code, self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

pub(crate) fn parse_confirm_ticket_response(
    json: &Value,
) -> Result<ConfirmTicketResult, ConfirmOrderFailure> {
    if let Some((err_code, msg)) = extract_api_error(json) {
        return Err(ConfirmOrderFailure {
            code: Some(err_code),
            message: msg,
        });
    }

    let Some(data) = json.get("data").filter(|d| d.is_object()) else {
        return Err(ConfirmOrderFailure {
            code: None,
            message: "确认订单响应缺少有效 data 对象".to_string(),
        });
    };

    let confirm_result: ConfirmTicketResult = serde_json::from_value(data.clone()).map_err(|e| {
        ConfirmOrderFailure {
            code: None,
            message: format!("解析确认订单结果失败: {}", e),
        }
    })?;

    if confirm_result.count <= 0 {
        return Err(ConfirmOrderFailure {
            code: None,
            message: format!(
                "确认订单数据不完整：购票数量无效 (count={})",
                confirm_result.count
            ),
        });
    }

    Ok(confirm_result)
}

fn simple_risk_err(code: i32, message: impl Into<String>) -> TokenRiskParam {
    TokenRiskParam {
        code,
        message: message.into(),
        mid: None,
        decision_type: None,
        buvid: None,
        ip: None,
        scene: None,
        ua: None,
        v_voucher: None,
        risk_param: None,
    }
}

pub(crate) fn build_prepare_order_payload(
    project_id_int: i64,
    screen_id_int: i64,
    sku_id_int: i64,
    count: i16,
    buyer_info: &[BuyerInfo],
    ctoken: &str,
) -> Value {
    json!({
        "count": count,
        "screen_id": screen_id_int,
        "order_type": 1,
        "project_id": project_id_int,
        "sku_id": sku_id_int,
        "buyer_info": buyer_info,
        "ignoreRequestLimit": true,
        "ticket_agent": "",
        "token": ctoken,
        "newRisk": true,
        "requestSource": "neul-next",
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn get_ticket_token(
    cookie_manager: Arc<CookieManager>,
    cpdd: Arc<Mutex<CTokenGenerator>>,
    project_id: &str,
    screen_id: &str,
    ticket_id: &str,
    count: i16,
    is_hot: bool,
    buyer_info: &[BuyerInfo],
) -> Result<(String, String), TokenRiskParam> {
    let project_id_int = project_id
        .parse::<i64>()
        .map_err(|_| simple_risk_err(ERR_INVALID_PARAM, format!("无效的 project_id: {}", project_id)))?;
    let screen_id_int = screen_id
        .parse::<i64>()
        .map_err(|_| simple_risk_err(ERR_INVALID_PARAM, format!("无效的 screen_id: {}", screen_id)))?;
    let sku_id_int = ticket_id
        .parse::<i64>()
        .map_err(|_| simple_risk_err(ERR_INVALID_PARAM, format!("无效的 ticket_id: {}", ticket_id)))?;

    let ctoken = if is_hot {
        cpdd.lock()
            .unwrap_or_else(|e| e.into_inner())
            .generate_ctoken(false)
    } else {
        String::new()
    };

    let params = build_prepare_order_payload(
        project_id_int,
        screen_id_int,
        sku_id_int,
        count,
        buyer_info,
        &ctoken,
    );

    log::debug!("获取票token参数：{:?}", params);
    let base_url = "https://show.bilibili.com/api/ticket/order/prepare";
    // 热门项目 prepare 走 HTTP/2（与 createV2 同一个 h2 客户端）
    let req = if is_hot {
        cookie_manager
            .post_with_headers_h2(base_url, HashMap::new())
            .await
    } else {
        cookie_manager.post(base_url).await
    };

    let resp = req
        .query(&[("project_id", project_id)])
        .json(&params)
        .send()
        .await
        .map_err(|e| {
            log::error!("获取票token网络请求失败，错误信息：{}", e);
            simple_risk_err(ERR_NETWORK, e.to_string())
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        log::error!("获取票token失败，服务器不期待响应，状态码：{}", status);
        return Err(simple_risk_err(status.as_u16() as i32, status.to_string()));
    }

    let json = resp.json::<Value>().await.map_err(|e| {
        log::error!("解析票务token响应失败: {}", e);
        simple_risk_err(ERR_PARSE, e.to_string())
    })?;

    log::debug!("获取票token响应：{}", json);

    if let Some((code, msg)) = extract_api_error(&json) {
        if code == -401 || code == 401 {
            log::info!("需要进行人机验证");
            let risk_params = &json["data"]["ga_data"]["riskParams"];
            let token_risk_param = build_token_risk_param(code as i32, msg, risk_params);
            log::debug!("{:?}", token_risk_param);
            return Err(token_risk_param);
        }

        log::error!(
            "获取token失败，错误码：{}，错误信息：{}",
            code,
            msg
        );
        log::debug!("获取token错误响应详情: {:?}", json);
        return Err(simple_risk_err(code as i32, msg));
    }

    let token = json["data"]["token"].as_str().unwrap_or("").trim();
    if token.is_empty() {
        log::error!("获取token响应成功但 token 字段为空");
        return Err(simple_risk_err(ERR_PARSE, "接口返回空 token"));
    }

    // 无论项目详情初始是否标记 hotProject，只要 prepare 下发了 ptoken 就保留透传
    let ptoken = json["data"]["ptoken"].as_str().unwrap_or("").trim();
    Ok((token.to_string(), ptoken.to_string()))
}

pub async fn confirm_ticket_order(
    cookie_manager: Arc<CookieManager>,
    project_id: &str,
    token: &str,
) -> Result<ConfirmTicketResult, ConfirmOrderFailure> {
    let response = cookie_manager
        .get("https://show.bilibili.com/api/ticket/order/confirmInfo")
        .await
        .query(&[
            ("token", token),
            ("voucher", ""),
            ("project_id", project_id),
            ("requestSource", "neul-next"),
        ])
        .send()
        .await
        .map_err(|e| ConfirmOrderFailure {
            code: None,
            message: format!("请求失败: {}", e),
        })?;

    if !response.status().is_success() {
        return Err(ConfirmOrderFailure {
            code: Some(response.status().as_u16() as i64),
            message: format!("请求失败: {}", response.status()),
        });
    }

    let text = response.text().await.map_err(|e| ConfirmOrderFailure {
        code: None,
        message: format!("获取响应文本失败: {}", e),
    })?;
    log::debug!("确认订单响应：{}", text);

    let json: Value = serde_json::from_str(&text).map_err(|e| ConfirmOrderFailure {
        code: None,
        message: format!("解析响应文本失败: {}", e),
    })?;

    parse_confirm_ticket_response(&json)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_create_order_payload(
    project_id: &str,
    screen_id: &str,
    ticket_id: Option<&str>,
    token: &str,
    ptoken_clean: &str,
    ctoken: Option<String>,
    is_hot: bool,
    id_bind: usize,
    no_bind_buyer_info: Option<&NoBindBuyerInfo>,
    buyer_info: &[BuyerInfo],
    click_position: Value,
    is_mobile: bool,
    device_id: &str,
    pay_money: i64,
    count: i32,
    timestamp: u64,
) -> Result<Value, i32> {
    let project_id_int = project_id.parse::<i64>().map_err(|_| ERR_INVALID_PARAM)?;
    let screen_id_int = screen_id.parse::<i64>().map_err(|_| ERR_INVALID_PARAM)?;
    let ticket_id_str = ticket_id.ok_or(ERR_INVALID_PARAM)?;
    let ticket_id_int = ticket_id_str.parse::<i64>().map_err(|_| ERR_INVALID_PARAM)?;
    let request_source = if is_mobile { "neul-next" } else { "pc-new" };

    let include_hot_tokens = is_hot || !ptoken_clean.is_empty();

    match id_bind {
        0 => {
            let no_bind = match no_bind_buyer_info {
                Some(info) => info,
                None => {
                    log::error!("非实名购票人信息缺失，无法下单");
                    return Err(ERR_INVALID_PARAM);
                }
            };
            let mut body = json!({
                "project_id": project_id_int,
                "screen_id": screen_id_int,
                "sku_id": ticket_id_int,
                "token": token,
                "buyer": no_bind.name,
                "tel": no_bind.tel,
                "clickPosition": click_position,
                "again": 1,
                "newRisk": true,
                "requestSource": request_source,
                "deviceId": device_id,
                "pay_money": pay_money,
                "count": count,
                "timestamp": timestamp,
                "order_type": 1,
                "orderCreateUrl": CREATE_ORDER_V2_URL,
            });
            if include_hot_tokens {
                if let Some(obj) = body.as_object_mut() {
                    if let Some(ct) = ctoken.filter(|s| !s.is_empty()) {
                        obj.insert("ctoken".to_string(), json!(ct));
                    } else if is_hot {
                        obj.insert("ctoken".to_string(), json!(""));
                    }
                    obj.insert("ptoken".to_string(), json!(ptoken_clean));
                }
            }
            Ok(body)
        }
        1 | 2 => {
            let mut body = json!({
                "project_id": project_id_int,
                "screen_id": screen_id_int,
                "sku_id": ticket_id_int,
                "token": token,
                "buyer_info": serde_json::to_string(buyer_info).unwrap_or_default(),
                "clickPosition": click_position,
                "again": 1,
                "newRisk": true,
                "requestSource": request_source,
                "deviceId": device_id,
                "pay_money": pay_money,
                "count": count,
                "timestamp": timestamp,
                "order_type": 1,
                "orderCreateUrl": CREATE_ORDER_V2_URL,
            });
            if include_hot_tokens {
                if let Some(obj) = body.as_object_mut() {
                    if let Some(ct) = ctoken.filter(|s| !s.is_empty()) {
                        obj.insert("ctoken".to_string(), json!(ct));
                    } else if is_hot {
                        obj.insert("ctoken".to_string(), json!(""));
                    }
                    obj.insert("ptoken".to_string(), json!(ptoken_clean));
                }
            }
            Ok(body)
        }
        _ => {
            log::error!("购票人信息错误，id_bind: {}", id_bind);
            Err(ERR_ID_BIND_INVALID)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn create_order(
    cookie_manager: Arc<CookieManager>,
    cpdd: Arc<Mutex<CTokenGenerator>>,
    project_id: &str,
    token: &str,
    ptoken: &str,
    confirm_result: &ConfirmTicketResult,
    pay_money_override: Option<i64>,
    is_hot: bool,
    biliticket: &BilibiliTicket,
    buyer_info: &Vec<BuyerInfo>,
    is_mobile: bool,
    need_retry: bool,
    fast_mode: bool,
    screen_size: Option<(u32, u32)>,
) -> Result<Value, CreateOrderFailure> {
    let ptoken_clean = ptoken.replace('=', "");
    let use_hot_tokens = is_hot || !ptoken_clean.is_empty();
    let base_url = CREATE_ORDER_V2_URL;

    let position_type = if need_retry && is_mobile {
        ClickPositionType::RetryButton
    } else if is_mobile {
        ClickPositionType::MobileConfirm
    } else {
        ClickPositionType::PcConfirm
    };

    let risk_header = format!(
        "platform/{} uid/{} deviceId/{}",
        "h5",
        cookie_manager.get_cookie("DedeUserID").unwrap_or_default(),
        cookie_manager.get_cookie("buvid3").unwrap_or_default(),
    );
    let mut input_risk_header = HashMap::new();
    input_risk_header.insert("X-Risk-Header", risk_header.as_str());

    let (width, height) = screen_size.unwrap_or((1080, 2400));
    let click_position = random_click_position(position_type, fast_mode, Some(width), Some(height));

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let ctoken = if use_hot_tokens {
        Some(
            cpdd.lock()
                .unwrap_or_else(|e| e.into_inner())
                .generate_ctoken(true),
        )
    } else {
        None
    };

    let device_id = cookie_manager
        .get_cookie("deviceFingerprint")
        .unwrap_or_default();

    let effective_pay_money = pay_money_override.unwrap_or(confirm_result.pay_money);

    let data = build_create_order_payload(
        project_id,
        &biliticket.screen_id,
        biliticket.select_ticket_id.as_deref(),
        token,
        &ptoken_clean,
        ctoken,
        is_hot,
        biliticket.id_bind,
        biliticket.no_bind_buyer_info.as_ref(),
        buyer_info,
        click_position,
        is_mobile,
        &device_id,
        effective_pay_money,
        confirm_result.count,
        timestamp,
    )
    .map_err(CreateOrderFailure::from_code)?;

    log::debug!("抢票data：{:?}", data);

    let req = cookie_manager
        .post_with_headers_h2(base_url, input_risk_header)
        .await;
    let req = if use_hot_tokens {
        req.query(&[("project_id", project_id), ("ptoken", ptoken_clean.as_str())])
    } else {
        req.query(&[("project_id", project_id)])
    };

    let response = req.json(&data).send().await.map_err(|e| {
        log::error!("createV2 网络请求失败: {}", e);
        CreateOrderFailure::from_code(ERR_NETWORK)
    })?;

    log::debug!("createV2 实际协议版本: {:?}", response.version());
    if response.status() != 200 {
        log::error!("createV2 HTTP 失败: {}", response.status());
        return Err(CreateOrderFailure::from_code(
            response.status().as_u16() as i32
        ));
    }

    let text = response.text().await.map_err(|e| {
        log::error!("createV2 获取响应文本失败: {}", e);
        CreateOrderFailure::from_code(ERR_NETWORK)
    })?;
    log::debug!("createV2 原始响应: {}", text);

    let value: Value = serde_json::from_str(&text).map_err(|e| {
        log::error!("createV2 解析响应文本失败: {}", e);
        CreateOrderFailure::from_code(ERR_PARSE)
    })?;

    if let Some((err_code, msg)) = extract_api_error(&value) {
        log::debug!("createV2 返回错误码({}): {}", err_code, msg);
        let updated_pay_money = if err_code == 100034 {
            extract_updated_pay_money(&value)
        } else {
            None
        };
        return Err(CreateOrderFailure {
            code: err_code as i32,
            message: msg,
            updated_pay_money,
        });
    }

    if is_create_order_degraded_response(&value) {
        log::info!("createV2 返回 defaultBBR 降级空包，视为未成单继续重试");
        return Err(CreateOrderFailure::from_code_msg(
            ERR_DEGRADED_BBR,
            extract_api_message(&value),
        ));
    }

    Ok(value)
}

pub async fn check_fake_ticket(
    cookie_manager: Arc<CookieManager>,
    project_id: &str,
    pay_token: &str,
    order_id: &str,
) -> Result<Value, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let timestamp_str = timestamp.to_string();

    let mut query_params: Vec<(&str, &str)> = vec![
        ("project_id", project_id),
        ("token", pay_token),
        ("timestamp", timestamp_str.as_str()),
    ];
    let trimmed_order_id = order_id.trim();
    if !trimmed_order_id.is_empty() && trimmed_order_id != "0" {
        query_params.push(("orderId", trimmed_order_id));
    }

    let response = cookie_manager
        .get("https://show.bilibili.com/api/ticket/order/createstatus")
        .await
        .query(&query_params)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    log::debug!("check_fake_ticket status: {:?}", response.status());
    if !response.status().is_success() {
        return Err(format!("检测订单状态返回 HTTP {}", response.status()));
    }

    let text = response
        .text()
        .await
        .map_err(|e| format!("读取检测响应文本失败: {}", e))?;
    log::debug!("check_fake_ticket 原始响应: {}", text);

    let data =
        serde_json::from_str::<Value>(&text).map_err(|e| format!("解析响应文本失败: {}", e))?;
    Ok(data)
}

async fn fetch_order_pay_param_from_url(
    cookie_manager: Arc<CookieManager>,
    endpoint_url: &str,
    order_id: &str,
) -> Result<CheckFakeResultData, PayParamError> {
    let trimmed = order_id.trim();
    if trimmed.is_empty() || trimmed == "0" {
        return Err(PayParamError::Unavailable(
            "订单号为空，无法查询支付参数".to_string(),
        ));
    }

    // 显式使用桌面 PC 浏览器身份请求 getPayParam：
    // 即使抢票阶段开启了 Android/Mobile 自定义 UA，也能让 B站返回 deviceType=1 的直扫 code_url，
    // 而不是返回仅供移动端 H5 收银台使用的无 code_url 表单参数（deviceType=2）。
    let referer = format!(
        "https://show.bilibili.com/platform/orderDetail.html?order_id={}",
        trimmed
    );
    let pc_cookie = build_pc_pay_cookie_header(&cookie_manager.get_all_cookies());
    let mut headers: HashMap<&str, &str> = HashMap::new();
    headers.insert("User-Agent", PC_PAY_USER_AGENT);
    headers.insert("Referer", referer.as_str());
    headers.insert("Origin", "https://show.bilibili.com");
    if !pc_cookie.is_empty() {
        headers.insert("Cookie", pc_cookie.as_str());
    }

    let response = cookie_manager
        .get_with_headers(endpoint_url, headers)
        .await
        .query(&[("order_id", trimmed)])
        .send()
        .await
        .map_err(|e| PayParamError::Unavailable(format!("getPayParam 网络请求失败: {}", e)))?;

    if !response.status().is_success() {
        return Err(PayParamError::Unavailable(format!(
            "getPayParam 返回 HTTP {}",
            response.status()
        )));
    }

    let text = response
        .text()
        .await
        .map_err(|e| PayParamError::Unavailable(format!("读取 getPayParam 响应失败: {}", e)))?;
    log::debug!("getPayParam 原始响应: {}", text);

    let value: Value = serde_json::from_str(&text)
        .map_err(|e| PayParamError::Unavailable(format!("解析 getPayParam 响应失败: {}", e)))?;
    parse_pay_param_response(&value)
}

/// 通过 `/api/ticket/order/getPayParam?order_id=...` 获取已创建订单的直扫支付二维码链接
pub async fn fetch_order_pay_param(
    cookie_manager: Arc<CookieManager>,
    order_id: &str,
) -> Result<CheckFakeResultData, PayParamError> {
    fetch_order_pay_param_from_url(cookie_manager, GET_PAY_PARAM_URL, order_id).await
}

/// 点击位置类型枚举
#[derive(Debug, Clone, Copy)]
pub enum ClickPositionType {
    /// PC端确认下单按钮位置
    PcConfirm,
    /// 手机端确认下单按钮位置
    MobileConfirm,
    /// "再试一次"按钮位置（屏幕中间）
    RetryButton,
}

fn random_click_position_with_now(
    position_type: ClickPositionType,
    fast_mode: bool,
    screen_width: Option<u32>,
    screen_height: Option<u32>,
    now: u64,
) -> Value {
    let mut rng = thread_rng();

    let mobile_width = screen_width.unwrap_or(1080);
    let mobile_height = screen_height.unwrap_or(2400);

    let (base_x, base_y, offset_range) = match position_type {
        ClickPositionType::PcConfirm => (1131, 636, 10),
        ClickPositionType::MobileConfirm => {
            let x_ratio = rng.gen_range(0.55..0.9);
            let y_ratio = rng.gen_range(0.9..0.95);

            let x = (mobile_width as f32 * x_ratio) as i32;
            let y = (mobile_height as f32 * y_ratio) as i32;

            (x, y, (mobile_width as i32 / 4).clamp(1, 20))
        }
        ClickPositionType::RetryButton => {
            let x_ratio = rng.gen_range(0.33..0.67);
            let y_ratio = rng.gen_range(0.6..0.7);

            let x = (mobile_width as f32 * x_ratio) as i32;
            let y = (mobile_height as f32 * y_ratio) as i32;

            (x, y, (mobile_width as i32 / 4).clamp(1, 30))
        }
    };

    let offset_x = rng.gen_range(-offset_range..=offset_range);
    let offset_y = rng.gen_range(-offset_range..=offset_range);

    let final_x = base_x + offset_x;
    let final_y = base_y + offset_y;

    let random_delay: u64 = if fast_mode {
        rng.gen_range(800..4600)
    } else {
        rng.gen_range(4000..12000)
    };

    let origin = now.saturating_sub(random_delay);

    json!({
        "x": final_x,
        "y": final_y,
        "origin": origin,
        "now": now
    })
}

/// 生成随机点击位置
pub fn random_click_position(
    position_type: ClickPositionType,
    fast_mode: bool,
    screen_width: Option<u32>,
    screen_height: Option<u32>,
) -> Value {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    random_click_position_with_now(position_type, fast_mode, screen_width, screen_height, now)
}

fn extract_qrcode_poll_code(json: &Value) -> i64 {
    json["data"]["code"]
        .as_i64()
        .or_else(|| json["code"].as_i64().filter(|&c| c != 0))
        .unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qrcode_poll_uses_nested_data_code() {
        let unscanned = json!({
            "code": 0,
            "message": "0",
            "ttl": 1,
            "data": {
                "url": "",
                "refresh_token": "",
                "timestamp": 0,
                "code": 86101,
                "message": "未扫码"
            }
        });
        assert_eq!(extract_qrcode_poll_code(&unscanned), 86101);

        let scanned = json!({
            "code": 0,
            "data": { "code": 86090, "message": "二维码已扫码未确认" }
        });
        assert_eq!(extract_qrcode_poll_code(&scanned), 86090);

        let expired = json!({
            "code": 0,
            "data": { "code": 86038, "message": "二维码已失效" }
        });
        assert_eq!(extract_qrcode_poll_code(&expired), 86038);

        let success = json!({
            "code": 0,
            "data": { "code": 0, "message": "" }
        });
        assert_eq!(extract_qrcode_poll_code(&success), 0);

        let api_err = json!({
            "code": -400,
            "message": "请求错误"
        });
        assert_eq!(extract_qrcode_poll_code(&api_err), -400);

        let missing = json!({ "code": 0, "data": {} });
        assert_eq!(extract_qrcode_poll_code(&missing), -1);
    }

    #[test]
    fn extract_api_error_distinguishes_minus_one_and_missing_fields() {
        // errno: -1 with code: 0 must fail (never treat -1 as missing!)
        let err1 = json!({ "errno": -1, "code": 0, "msg": "系统错误" });
        assert_eq!(
            extract_api_error(&err1),
            Some((-1, "系统错误".to_string()))
        );

        // errno: 0 with code: -400 must fail
        let err2 = json!({ "errno": 0, "code": -400, "message": "参数错误" });
        assert_eq!(
            extract_api_error(&err2),
            Some((-400, "参数错误".to_string()))
        );

        // errno: 0 with absent code succeeds
        let ok1 = json!({ "errno": 0, "data": {} });
        assert_eq!(extract_api_error(&ok1), None);

        // code: 0 with absent errno succeeds
        let ok2 = json!({ "code": 0, "data": {} });
        assert_eq!(extract_api_error(&ok2), None);

        // both absent must fail
        let both_missing = json!({ "data": { "message": "缺少状态码" } });
        assert_eq!(
            extract_api_error(&both_missing),
            Some((-1, "缺少状态码".to_string()))
        );

        // empty object falls back to default message
        assert_eq!(
            extract_api_error(&json!({})),
            Some((-1, "未知错误".to_string()))
        );
    }

    #[test]
    fn extract_order_id_handles_i64_u64_string_and_missing() {
        // i64
        let from_i64 = json!({ "data": { "orderId": 12345678901234i64 } });
        assert_eq!(
            extract_order_id(&from_i64),
            Some("12345678901234".to_string())
        );

        // u64 larger than i64::MAX
        let big_u64: u64 = (i64::MAX as u64) + 12345;
        let from_u64 = json!({ "data": { "orderId": big_u64 } });
        assert_eq!(extract_order_id(&from_u64), Some(big_u64.to_string()));

        // large snowflake string
        let from_str = json!({ "data": { "orderId": "9988776655443322110099" } });
        assert_eq!(
            extract_order_id(&from_str),
            Some("9988776655443322110099".to_string())
        );

        // missing or zero
        assert_eq!(extract_order_id(&json!({ "data": {} })), None);
        assert_eq!(extract_order_id(&json!({ "data": { "orderId": 0 } })), None);
        assert_eq!(
            extract_order_id(&json!({ "data": { "orderId": "" } })),
            None
        );
    }

    #[test]
    fn create_order_payload_includes_ctoken_ptoken_for_id_bind_0_hot_project() {
        let no_bind = NoBindBuyerInfo {
            name: "张三".to_string(),
            tel: "13800138000".to_string(),
            uid: 10001,
        };
        let payload = build_create_order_payload(
            "10001",
            "20002",
            Some("30003"),
            "tok_abc",
            "ptok_clean",
            Some("ctok_xyz".to_string()),
            true,
            0,
            Some(&no_bind),
            &[],
            json!({"x": 100, "y": 200, "origin": 1000, "now": 2000}),
            true,
            "",
            9900,
            1,
            1700000000000,
        )
        .expect("id_bind=0 + is_hot=true should build payload");

        assert_eq!(payload["project_id"], json!(10001));
        assert_eq!(payload["screen_id"], json!(20002));
        assert_eq!(payload["sku_id"], json!(30003));
        assert_eq!(payload["again"], json!(1));
        assert_eq!(
            payload["orderCreateUrl"],
            json!("https://show.bilibili.com/api/ticket/order/createV2")
        );
        assert_eq!(payload["ctoken"], json!("ctok_xyz"));
        assert_eq!(payload["ptoken"], json!("ptok_clean"));
        assert_eq!(payload["buyer"], json!("张三"));
        assert_eq!(payload["tel"], json!("13800138000"));
        assert_eq!(payload["deviceId"], json!(""));

        // non-hot id_bind=0 without ptoken should still carry again & orderCreateUrl, omitting ctoken/ptoken
        let cold_payload = build_create_order_payload(
            "10001",
            "20002",
            Some("30003"),
            "tok_abc",
            "",
            None,
            false,
            0,
            Some(&no_bind),
            &[],
            json!({"x": 100, "y": 200, "origin": 1000, "now": 2000}),
            false,
            "fp123",
            9900,
            1,
            1700000000000,
        )
        .expect("cold id_bind=0 should build");
        assert_eq!(cold_payload["again"], json!(1));
        assert_eq!(
            cold_payload["orderCreateUrl"],
            json!("https://show.bilibili.com/api/ticket/order/createV2")
        );
        assert!(cold_payload.get("ctoken").is_none());
        assert!(cold_payload.get("ptoken").is_none());
        assert_eq!(cold_payload["requestSource"], json!("pc-new"));

        // non-hot project that still received a server ptoken must forward ptoken & ctoken
        let dynamic_ptoken_payload = build_create_order_payload(
            "10001",
            "20002",
            Some("30003"),
            "tok_abc",
            "ptok_from_prepare",
            Some("ctok_dyn".to_string()),
            false,
            1,
            None,
            &[],
            json!({}),
            true,
            "fp123",
            9900,
            1,
            1700000000000,
        )
        .expect("dynamic ptoken payload should build");
        assert_eq!(dynamic_ptoken_payload["ptoken"], json!("ptok_from_prepare"));
        assert_eq!(dynamic_ptoken_payload["ctoken"], json!("ctok_dyn"));

        // prepare payload includes buyer_info, ignoreRequestLimit and ticket_agent
        let prepare_payload = build_prepare_order_payload(10001, 20002, 30003, 2, &[], "ctok_p");
        assert_eq!(prepare_payload["ignoreRequestLimit"], json!(true));
        assert_eq!(prepare_payload["ticket_agent"], json!(""));
        assert_eq!(prepare_payload["buyer_info"], json!([]));
        assert_eq!(prepare_payload["token"], json!("ctok_p"));
        assert_eq!(prepare_payload["requestSource"], json!("neul-next"));

        // degraded defaultBBR detection & 100034 pay_money extraction
        assert!(is_create_order_degraded_response(&json!({
            "errno": 0,
            "msg": "defaultBBR fallback"
        })));
        assert!(!is_create_order_degraded_response(&json!({
            "errno": 0,
            "msg": ""
        })));
        assert_eq!(
            extract_updated_pay_money(&json!({"errno": 100034, "data": {"pay_money": 18800}})),
            Some(18800)
        );
        assert_eq!(
            extract_updated_pay_money(&json!({"errno": 100034, "data": {"pay_money": 0}})),
            None
        );

        // getPayParam parsing & orderDetail fallback URL
        let top_level_pay = parse_pay_param_response(&json!({
            "errno": 0,
            "data": {"code_url": "https://qr.alipay.com/bax123", "sign": "sig_1"}
        }))
        .expect("top-level data.code_url should parse");
        assert_eq!(top_level_pay.code_url, "https://qr.alipay.com/bax123");
        assert_eq!(top_level_pay.sign, "sig_1");

        let nested_pay = parse_pay_param_response(&json!({
            "errno": 0,
            "data": {"payParam": {"code_url": "weixin://wxpay/abc"}}
        }))
        .expect("nested data.payParam.code_url should parse");
        assert_eq!(nested_pay.code_url, "weixin://wxpay/abc");

        assert!(matches!(
            parse_pay_param_response(&json!({"errno": 0, "data": {"code_url": ""}})),
            Err(PayParamError::Unavailable(_))
        ));
        assert!(matches!(
            parse_pay_param_response(&json!({"errno": 100018, "msg": "订单不存在"})),
            Err(PayParamError::OrderNotFound { code: 100018, .. })
        ));
        assert!(matches!(
            parse_pay_param_response(&json!({"errno": 99999, "msg": "仅支持APP支付"})),
            Err(PayParamError::Unavailable(_))
        ));
        assert_eq!(
            build_order_detail_fallback_url(" 8001478508219364 "),
            "https://show.bilibili.com/platform/orderDetail.html?order_id=8001478508219364"
        );

        // invalid numeric IDs fail fast with ERR_INVALID_PARAM (999)
        let bad = build_create_order_payload(
            "not_a_num",
            "20002",
            Some("30003"),
            "tok",
            "",
            None,
            false,
            0,
            Some(&no_bind),
            &[],
            json!({}),
            true,
            "",
            9900,
            1,
            1700000000000,
        );
        assert_eq!(bad, Err(ERR_INVALID_PARAM));
    }

    #[test]
    fn cache_path_sanitizes_traversal_and_special_chars() {
        let p = cache_path("../../etc/passwd");
        assert_eq!(p, PathBuf::from("cache").join("project_etcpasswd.json"));

        let p2 = cache_path("12345_abc-9");
        assert_eq!(p2, PathBuf::from("cache").join("project_12345_abc-9.json"));

        let p3 = cache_path("../../../");
        assert_eq!(p3, PathBuf::from("cache").join("project_unknown.json"));
    }

    #[test]
    fn random_click_position_does_not_underflow_and_handles_small_screens() {
        // now < delay must saturate to 0 rather than panic with u64 underflow
        let pos = random_click_position_with_now(
            ClickPositionType::MobileConfirm,
            false,
            Some(0),
            Some(0),
            100,
        );
        assert_eq!(pos["origin"], json!(0));
        assert_eq!(pos["now"], json!(100));

        let retry_pos = random_click_position_with_now(
            ClickPositionType::RetryButton,
            true,
            Some(1080),
            Some(2400),
            500,
        );
        assert_eq!(retry_pos["origin"], json!(0));

        // Normal timestamp produces origin <= now
        let normal = random_click_position(ClickPositionType::PcConfirm, true, None, None);
        let origin = normal["origin"].as_u64().unwrap();
        let now = normal["now"].as_u64().unwrap();
        assert!(origin <= now);
    }

    #[tokio::test]
    async fn poll_qrcode_login_cancellable_exits_immediately_when_cancelled() {
        let (_tx, rx) = watch::channel(true);
        let status = poll_qrcode_login_cancellable("dummy_key", Some("   "), rx).await;
        assert!(matches!(status, QrCodeLoginStatus::Failed(_)));
    }

    #[test]
    fn parse_confirm_ticket_response_rejects_null_and_empty_data() {
        assert!(parse_confirm_ticket_response(&json!({"errno": 0, "data": null})).is_err());
        assert!(parse_confirm_ticket_response(&json!({"errno": 0, "data": {}})).is_err());
        assert!(
            parse_confirm_ticket_response(&json!({"errno": 0, "data": {"count": 0, "pay_money": 9900}}))
                .is_err()
        );
        assert!(
            parse_confirm_ticket_response(&json!({"errno": 100001, "msg": "限流", "data": {"count": 1}}))
                .is_err()
        );

        let ok = parse_confirm_ticket_response(&json!({
            "errno": 0,
            "data": {
                "count": 2,
                "pay_money": 19800,
                "project_name": "测试漫展",
                "screen_name": "5月1日",
                "ticket_info": {
                    "name": "普通票",
                    "count": 2,
                    "price": 9900
                }
            }
        }))
        .expect("valid confirm response should succeed");
        assert_eq!(ok.count, 2);
        assert_eq!(ok.pay_money, 19800);
    }

    #[test]
    fn project_cache_validation_rejects_incomplete_cached_json() {
        let missing_id_bind = r#"{
            "errno": 0,
            "data": {
                "id": 10001,
                "name": "Incomplete Show",
                "sale_begin": 1780000000,
                "screen_list": [{"id": 1, "clickable": true}]
            }
        }"#;
        assert!(parse_and_validate_project_cache(missing_id_bind).is_err());

        let valid_cache = r#"{
            "errno": 0,
            "data": {
                "id": 10001,
                "name": "Complete Show",
                "id_bind": 2,
                "sale_flag_number": 8,
                "sale_begin": null,
                "screen_list": [{"id": 1, "clickable": false, "sale_start": 0, "sale_flag_number": 8}]
            }
        }"#;
        let parsed =
            parse_and_validate_project_cache(valid_cache).expect("valid cache should pass");
        assert_eq!(parsed.data.id, Some(10001));
    }

    #[test]
    fn parse_server_now_response_requires_positive_timestamp() {
        assert_eq!(
            parse_server_now_response(r#"{"code":0,"data":{"now":1776079512}}"#).unwrap(),
            1776079512
        );
        assert!(parse_server_now_response(r#"{"code":0,"data":{"now":0}}"#).is_err());
        assert!(parse_server_now_response(r#"{"code":0,"data":{}}"#).is_err());
        assert!(parse_server_now_response("not json").is_err());
    }

    #[test]
    fn build_token_risk_param_never_wraps_json_null() {
        let null_param = build_token_risk_param(-401, "need risk".to_string(), &Value::Null);
        assert!(null_param.risk_param.is_none());

        let obj_val = json!({"v_voucher": "voucher_123", "mid": "42"});
        let valid_param = build_token_risk_param(-401, "need risk".to_string(), &obj_val);
        assert_eq!(valid_param.risk_param, Some(obj_val));
        assert_eq!(valid_param.v_voucher.as_deref(), Some("voucher_123"));
    }

    #[tokio::test]
    async fn poll_qrcode_login_with_progress_emits_scanning_once() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local mock qr server");
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let responses = [
                (
                    r#"{"code":0,"data":{"code":86101,"message":"未扫码"}}"#,
                    None,
                ),
                (
                    r#"{"code":0,"data":{"code":86090,"message":"已扫码未确认"}}"#,
                    None,
                ),
                (
                    r#"{"code":0,"data":{"code":86090,"message":"已扫码未确认"}}"#,
                    None,
                ),
                (
                    r#"{"code":0,"data":{"code":0,"message":""}}"#,
                    Some("SESSDATA=qr_ok_cookie; Path=/"),
                ),
            ];
            for (body, set_cookie) in responses {
                let (mut sock, _) = listener.accept().await.expect("accept poll request");
                let mut req_buf = [0u8; 1024];
                let _ = sock.read(&mut req_buf).await;
                let cookie_hdr = match set_cookie {
                    Some(c) => format!("Set-Cookie: {}\r\n", c),
                    None => String::new(),
                };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                    cookie_hdr,
                    body.len(),
                    body
                );
                sock.write_all(resp.as_bytes()).await.unwrap();
                let _ = sock.shutdown().await;
            }
        });

        let (_tx, rx) = watch::channel(false);
        let mut scan_notifications = 0usize;
        let url = format!("http://{}/poll", addr);
        let status = poll_qrcode_login_with_config(
            &url,
            "mock_key",
            None,
            Duration::from_millis(10),
            rx,
            || {
                scan_notifications += 1;
            },
        )
        .await;

        server.await.unwrap();
        assert_eq!(scan_notifications, 1, "on_scanning should fire exactly once");
        assert_eq!(
            status,
            QrCodeLoginStatus::Success("SESSDATA=qr_ok_cookie".to_string())
        );
    }

    #[tokio::test]
    async fn fetch_order_pay_param_forces_desktop_pc_ua_and_strips_mobile_source_cookies() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local mock pay server");
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.expect("accept getPayParam request");
            let mut buf = vec![0u8; 4096];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let raw_req = String::from_utf8_lossy(&buf[..n]).to_string();

            let body = r#"{"errno":0,"msg":"","data":{"code_url":"https://pay.bilibili.com/payplatform-h5/qrpay.html?payToken=tok_ok","sign":"sig_pc"}}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
            let _ = sock.shutdown().await;
            raw_req
        });

        let mobile_ua = "Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Mobile Safari/537.36";
        let client = reqwest::Client::builder()
            .user_agent(mobile_ua)
            .no_proxy()
            .build()
            .unwrap();
        let cm = Arc::new(CookieManager::from_client(
            Arc::new(client),
            "SESSDATA=real_sess; DedeUserID=401924460; msource=bilibiliapp; mSource=bilibiliapp; kfcSource=bilibiliapp",
        ));
        cm.set_ua(mobile_ua);

        let endpoint = format!("http://{}/api/ticket/order/getPayParam", addr);
        let pay = fetch_order_pay_param_from_url(cm, &endpoint, "8001478555289364")
            .await
            .expect("mock getPayParam should return code_url");

        let captured_req = server.await.unwrap();
        let captured_lower = captured_req.to_ascii_lowercase();

        assert_eq!(
            pay.code_url,
            "https://pay.bilibili.com/payplatform-h5/qrpay.html?payToken=tok_ok"
        );
        assert_eq!(pay.sign, "sig_pc");
        assert!(
            captured_lower.contains("windows nt 10.0; win64; x64"),
            "must override mobile UA with desktop PC UA, got:\n{}",
            captured_req
        );
        assert!(
            !captured_lower.contains("android") && !captured_lower.contains("nexus 5"),
            "must NOT send Android/Mobile UA to getPayParam, got:\n{}",
            captured_req
        );
        assert!(
            captured_lower.contains("referer: https://show.bilibili.com/platform/orderdetail.html?order_id=8001478555289364"),
            "must send PC orderDetail Referer, got:\n{}",
            captured_req
        );
        assert!(
            captured_req.contains("SESSDATA=real_sess") && captured_req.contains("DedeUserID=401924460"),
            "must preserve auth cookies, got:\n{}",
            captured_req
        );
        assert!(
            !captured_lower.contains("msource=") && !captured_lower.contains("kfcsource="),
            "must strip mobile app source cookies, got:\n{}",
            captured_req
        );
    }
}
