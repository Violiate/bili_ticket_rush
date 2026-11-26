use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::HeaderValue;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::cookie_manager::CookieManager;

const NAV_URL: &str = "https://api.bilibili.com/x/web-interface/nav";
const EXIT_URL: &str = "https://passport.bilibili.com/login/exit/v2";
const NAV_MAX_ATTEMPTS: usize = 3;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const FALLBACK_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36";

#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    #[serde(default)]
    pub uid: i64, // UID
    #[serde(default)]
    pub name: String, // 昵称
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub cookie: String, // cookie
    #[serde(default)]
    pub csrf: String, // csrf
    #[serde(default)]
    pub is_login: bool, // 是否登录
    #[serde(default)]
    pub account_status: String, // 账号状态
    #[serde(default)]
    pub vip_label: String, // 大会员，对应 /nav 请求中 data['vip_label']['text']
    #[serde(default)]
    pub is_active: bool, // 该账号是否启动抢票
    #[serde(default)]
    pub avatar_url: Option<String>, // 头像地址
    #[serde(skip)]
    pub avatar_texture: Option<eframe::egui::TextureHandle>, // 头像纹理
    #[serde(skip)]
    pub cookie_manager: Option<Arc<CookieManager>>, // cookie 管理器
}

fn redact_secret(value: &str) -> &'static str {
    if value.is_empty() {
        ""
    } else {
        "[REDACTED]"
    }
}

impl fmt::Debug for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Account")
            .field("uid", &self.uid)
            .field("name", &self.name)
            .field("level", &self.level)
            .field("cookie", &redact_secret(&self.cookie))
            .field("csrf", &redact_secret(&self.csrf))
            .field("is_login", &self.is_login)
            .field("account_status", &self.account_status)
            .field("vip_label", &self.vip_label)
            .field("is_active", &self.is_active)
            .field("avatar_url", &self.avatar_url)
            .field(
                "avatar_texture",
                &if self.avatar_texture.is_some() {
                    "Some(TextureHandle)"
                } else {
                    "None"
                },
            )
            .field("cookie_manager", &self.cookie_manager)
            .finish()
    }
}

fn run_async_blocking<F, T>(fut: F) -> Result<T, String>
where
    F: Future<Output = T> + Send,
    T: Send,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            Ok(tokio::task::block_in_place(|| handle.block_on(fut)))
        }
        Ok(_) => std::thread::scope(|s| {
            s.spawn(|| {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("创建异步运行时失败: {}", e))?;
                Ok(rt.block_on(fut))
            })
            .join()
            .unwrap_or_else(|_| Err("异步执行线程异常退出".to_string()))
        }),
        Err(_) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("创建异步运行时失败: {}", e))?;
            Ok(rt.block_on(fut))
        }
    }
}

pub(crate) fn sanitize_cookie_input(raw: &str) -> Result<String, String> {
    let stripped = raw.replace(['\r', '\n'], "");
    let mut cleaned = stripped
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim();

    if cleaned.len() >= 7 && cleaned[..7].eq_ignore_ascii_case("cookie:") {
        cleaned = cleaned[7..]
            .trim()
            .trim_matches(|c| c == '"' || c == '\'')
            .trim();
    }

    if cleaned.is_empty() {
        return Err("Cookie 不能为空".to_string());
    }

    if !cleaned.is_ascii() || cleaned.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return Err("Cookie 包含非ASCII或控制字符，请检查是否粘贴了非法字符".to_string());
    }

    HeaderValue::from_str(cleaned)
        .map_err(|_| "Cookie 包含无效请求头字符，请检查是否粘贴了非法字符".to_string())?;

    Ok(cleaned.to_string())
}

fn sanitize_ua(ua: &str) -> &str {
    let trimmed = ua.trim();
    if !trimmed.is_empty() && HeaderValue::from_str(trimmed).is_ok() {
        trimmed
    } else {
        FALLBACK_UA
    }
}

pub(crate) fn normalize_avatar_url(raw: Option<&str>) -> Option<String> {
    let trimmed = raw?.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("//") {
        Some(format!("https:{}", trimmed))
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        Some(format!("https://{}", rest))
    } else {
        Some(trimmed.to_string())
    }
}

pub async fn add_account_async(cookie: &str, client: &Client, ua: &str) -> Result<Account, String> {
    let clean_cookie = sanitize_cookie_input(cookie)?;
    let clean_ua = sanitize_ua(ua);

    let json = fetch_nav_json(client, &clean_cookie, clean_ua).await?;

    let code = json.get("code").and_then(|v| v.as_i64());
    if code != Some(0) {
        let msg = json
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("未知错误");
        return Err(format!("获取账号信息失败: {} (code: {:?})", msg, code));
    }

    let data = json
        .get("data")
        .ok_or_else(|| "无法获取用户信息".to_string())?;

    if !data["isLogin"].as_bool().unwrap_or(false) {
        return Err("Cookie 未登录或已失效，请重新登录".to_string());
    }

    let uid = data["mid"].as_i64().unwrap_or(0);
    if uid <= 0 {
        return Err("无法获取有效的用户 UID".to_string());
    }

    // 仅在 /nav 校验成功后才初始化 CookieManager
    let cookie_manager = Arc::new(CookieManager::new(&clean_cookie, Some(clean_ua), 0).await);
    let csrf = cookie_manager
        .get_cookie("bili_jct")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| extract_csrf(&clean_cookie));

    let name = data["uname"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("未知用户")
        .to_string();

    log::info!("添加账号成功: UID {}", uid);

    Ok(Account {
        uid,
        name,
        level: data["level_info"]["current_level"]
            .as_i64()
            .unwrap_or(0)
            .to_string(),
        cookie: cookie_manager.get_all_cookies(),
        csrf,
        is_login: true,
        account_status: "空闲".to_string(),
        vip_label: data["vip_label"]["text"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        is_active: true,
        avatar_url: normalize_avatar_url(data["face"].as_str()),
        avatar_texture: None,
        cookie_manager: Some(cookie_manager),
    })
}

pub fn add_account(cookie: &str, client: &Client, ua: &str) -> Result<Account, String> {
    run_async_blocking(add_account_async(cookie, client, ua))?
}

async fn fetch_nav_json(
    client: &Client,
    cookie: &str,
    ua: &str,
) -> Result<serde_json::Value, String> {
    let cookie_header = HeaderValue::from_str(cookie)
        .map_err(|_| "Cookie 包含无效请求头字符".to_string())?;
    let ua_header = HeaderValue::from_str(ua)
        .map_err(|_| "User-Agent 包含无效请求头字符".to_string())?;

    let mut last_error = String::new();

    for attempt in 1..=NAV_MAX_ATTEMPTS {
        let response = match client
            .get(NAV_URL)
            .timeout(REQUEST_TIMEOUT)
            .header(reqwest::header::COOKIE, cookie_header.clone())
            .header(reqwest::header::USER_AGENT, ua_header.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => {
                last_error = format!("请求账号信息失败: {}", error);
                if attempt < NAV_MAX_ATTEMPTS {
                    log::warn!("{}，正在重试 ({}/{})", last_error, attempt, NAV_MAX_ATTEMPTS);
                    tokio::time::sleep(Duration::from_millis(200 * attempt as u64)).await;
                    continue;
                }
                break;
            }
        };

        let status = response.status();
        let content_length = response.content_length();
        let body = match response.bytes().await {
            Ok(body) => body,
            Err(error) => {
                last_error = format!(
                    "读取账号信息响应失败 (HTTP {}, Content-Length {:?}): {}",
                    status, content_length, error
                );
                if attempt < NAV_MAX_ATTEMPTS {
                    log::warn!("{}，正在重试 ({}/{})", last_error, attempt, NAV_MAX_ATTEMPTS);
                    tokio::time::sleep(Duration::from_millis(200 * attempt as u64)).await;
                    continue;
                }
                break;
            }
        };

        if status.is_client_error() {
            // 4xx 属于确定性客户端错误，直接快速失败而不重试
            return Err(format!(
                "获取账号信息被拒绝 (HTTP {}, 响应体 {} 字节)",
                status,
                body.len()
            ));
        }

        if !status.is_success() {
            last_error = format!(
                "获取账号信息失败 (HTTP {}, 响应体 {} 字节)",
                status,
                body.len()
            );
        } else {
            match serde_json::from_slice(&body) {
                Ok(json) => return Ok(json),
                Err(error) => {
                    last_error = format!(
                        "解析账号信息响应失败 (HTTP {}, 响应体 {} 字节, Content-Length {:?}): {}",
                        status,
                        body.len(),
                        content_length,
                        error
                    );
                }
            }
        }

        if attempt < NAV_MAX_ATTEMPTS {
            log::warn!("{}，正在重试 ({}/{})", last_error, attempt, NAV_MAX_ATTEMPTS);
            tokio::time::sleep(Duration::from_millis(200 * attempt as u64)).await;
        }
    }

    Err(format!("{}（已尝试 {} 次）", last_error, NAV_MAX_ATTEMPTS))
}

async fn signout_account_parts(
    uid: i64,
    csrf_field: String,
    cookie: String,
    cookie_manager: Option<Arc<CookieManager>>,
) -> Result<bool, String> {
    let csrf = if !csrf_field.trim().is_empty() {
        csrf_field.trim().to_string()
    } else {
        extract_csrf(&cookie)
    };

    if csrf.is_empty() {
        return Err("缺少 biliCSRF，无法执行退出登录".to_string());
    }

    let form = [("biliCSRF", csrf.as_str())];

    let resp = if let Some(cm) = cookie_manager {
        cm.post(EXIT_URL)
            .await
            .timeout(REQUEST_TIMEOUT)
            .form(&form)
            .send()
            .await
            .map_err(|e| format!("退出登录请求失败: {}", e))?
    } else {
        let clean_cookie = sanitize_cookie_input(&cookie)?;
        let temp_client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(REQUEST_TIMEOUT)
            .user_agent(FALLBACK_UA)
            .build()
            .map_err(|e| format!("创建临时HTTP客户端失败: {}", e))?;

        temp_client
            .post(EXIT_URL)
            .header(reqwest::header::COOKIE, clean_cookie)
            .form(&form)
            .send()
            .await
            .map_err(|e| format!("退出登录请求失败: {}", e))?
    };

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("退出登录失败，HTTP 状态码: {}", status));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析退出登录响应失败: {}", e))?;

    let ok = json.get("code").and_then(|v| v.as_i64()) == Some(0)
        || json.get("status").and_then(|v| v.as_bool()) == Some(true);

    if ok {
        log::info!("账号 UID {} 退出登录成功", uid);
        Ok(true)
    } else {
        let msg = json
            .get("message")
            .and_then(|v| v.as_str())
            .or_else(|| json["data"]["message"].as_str())
            .unwrap_or("未知错误");
        Err(format!("退出登录失败: {}", msg))
    }
}

pub async fn signout_account_async(account: &Account) -> Result<bool, String> {
    signout_account_parts(
        account.uid,
        account.csrf.clone(),
        account.cookie.clone(),
        account.cookie_manager.clone(),
    )
    .await
}

pub fn signout_account(account: &Account) -> Result<bool, String> {
    run_async_blocking(signout_account_parts(
        account.uid,
        account.csrf.clone(),
        account.cookie.clone(),
        account.cookie_manager.clone(),
    ))?
}

// 提取 csrf（不记录原始 cookie / csrf 值）
pub(crate) fn extract_csrf(cookie: &str) -> String {
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some((raw_key, raw_val)) = part.split_once('=') {
            let key = raw_key.trim().trim_matches(|c| c == '"' || c == '\'');
            if key.eq_ignore_ascii_case("bili_jct") {
                let value = raw_val.trim().trim_matches(|c| c == '"' || c == '\'');
                if !value.is_empty() {
                    return value.to_string();
                }
            }
        }
    }

    log::warn!("无法从 Cookie 中提取 bili_jct (CSRF)");
    String::new()
}

impl Account {
    // 确保每个账号都有自己的 CookieManager（已有则直接返回，不创建运行时）
    pub fn ensure_client(&mut self) {
        if self.cookie_manager.is_some() {
            return;
        }
        self.ensure_client_with_ua(None);
    }

    pub fn ensure_client_with_ua(&mut self, custom_ua: Option<String>) {
        if self.cookie_manager.is_some() {
            return;
        }
        let cookie = self.cookie.clone();
        match run_async_blocking(async move {
            let ua_ref = custom_ua.as_deref().map(str::trim).filter(|s| !s.is_empty());
            Arc::new(CookieManager::new(&cookie, ua_ref, 0).await)
        }) {
            Ok(cm) => {
                self.cookie_manager = Some(cm);
            }
            Err(e) => {
                log::error!("初始化账号 {} 的 CookieManager 失败: {}", self.uid, e);
            }
        }
    }

    pub async fn ensure_client_async(&mut self, custom_ua: Option<String>) {
        if self.cookie_manager.is_some() {
            return;
        }
        let ua_ref = custom_ua.as_deref().map(str::trim).filter(|s| !s.is_empty());
        self.cookie_manager = Some(Arc::new(CookieManager::new(&self.cookie, ua_ref, 0).await));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_cookie_input_cleans_and_validates() {
        assert_eq!(
            sanitize_cookie_input("  Cookie: SESSDATA=abc==; bili_jct=123\r\n").unwrap(),
            "SESSDATA=abc==; bili_jct=123"
        );
        assert!(sanitize_cookie_input("   ").is_err());
        assert!(sanitize_cookie_input("SESSDATA=包含中文非法头").is_err());
    }

    #[test]
    fn extract_csrf_handles_whitespace_and_quotes() {
        assert_eq!(
            extract_csrf("SESSDATA=aaa;  BILI_JCT = \"token_xyz\" ; DedeUserID=1"),
            "token_xyz"
        );
        assert_eq!(extract_csrf("SESSDATA=aaa"), "");
    }

    #[test]
    fn normalize_avatar_url_upgrades_http_and_drops_empty() {
        assert_eq!(normalize_avatar_url(None), None);
        assert_eq!(normalize_avatar_url(Some("   ")), None);
        assert_eq!(
            normalize_avatar_url(Some("http://i0.hdslb.com/bfs/face/1.jpg")),
            Some("https://i0.hdslb.com/bfs/face/1.jpg".to_string())
        );
        assert_eq!(
            normalize_avatar_url(Some("//i0.hdslb.com/bfs/face/1.jpg")),
            Some("https://i0.hdslb.com/bfs/face/1.jpg".to_string())
        );
    }

    #[test]
    fn account_debug_redacts_secrets_and_deserializes_partial_json() {
        let partial = r#"{"uid": 1001, "name": "alice", "cookie": "SESSDATA=secret_val", "csrf": "secret_csrf"}"#;
        let acc: Account = serde_json::from_str(partial).expect("partial account should parse");
        assert_eq!(acc.uid, 1001);
        assert_eq!(acc.name, "alice");
        assert!(!acc.is_login);

        let rendered = format!("{:?}", acc);
        assert!(!rendered.contains("secret_val"));
        assert!(!rendered.contains("secret_csrf"));
        assert!(rendered.contains("[REDACTED]"));
    }
}
