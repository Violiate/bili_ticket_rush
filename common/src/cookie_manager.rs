use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rand::seq::SliceRandom;

use crate::web_ck_obfuscated::{
    gen_01x88, gen_buvid3and4, gen_ckbili_ticket, gen_fp, gen_uuid_infoc,
};

fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

fn redact_non_empty(value: &str) -> &'static str {
    if value.is_empty() {
        ""
    } else {
        "[REDACTED]"
    }
}

#[derive(Debug, Clone)]
pub struct AppData {
    pub brand: String,
    pub model: String,
    pub buvid: String,
}

#[derive(Clone)]
pub struct WebData {
    pub browser_version: String,
    pub os_version: String,
    pub ua: String,
    pub buvid3: String,
    pub buvid4: String,
    pub b_nut: String,
    pub buvid_fp: String,
    pub _uuid: String,
    pub bili_ticket: String,
    pub bili_ticket_expires: String,
    pub msource: String,
}

impl fmt::Debug for WebData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebData")
            .field("browser_version", &self.browser_version)
            .field("os_version", &self.os_version)
            .field("ua", &self.ua)
            .field("buvid3", &redact_non_empty(&self.buvid3))
            .field("buvid4", &redact_non_empty(&self.buvid4))
            .field("b_nut", &self.b_nut)
            .field("buvid_fp", &redact_non_empty(&self.buvid_fp))
            .field("_uuid", &redact_non_empty(&self._uuid))
            .field("bili_ticket", &redact_non_empty(&self.bili_ticket))
            .field("bili_ticket_expires", &self.bili_ticket_expires)
            .field("msource", &self.msource)
            .finish()
    }
}

#[derive(Clone)]
pub struct CookiesData {
    pub cookies_map: Arc<Mutex<HashMap<String, String>>>,
}

impl fmt::Debug for CookiesData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let map = lock_or_recover(&self.cookies_map);
        let mut keys: Vec<&String> = map.keys().collect();
        keys.sort();
        f.debug_struct("CookiesData")
            .field("count", &map.len())
            .field("keys", &keys)
            .finish()
    }
}

impl CookiesData {
    pub fn insert(&self, key: String, value: String) {
        lock_or_recover(&self.cookies_map).insert(key, value);
    }

    pub fn clear(&self) {
        lock_or_recover(&self.cookies_map).clear();
    }
}

#[derive(Clone)]
pub struct CookieManager {
    pub client: Arc<reqwest::Client>,
    pub h2_client: Arc<reqwest::Client>,
    pub create_type: usize,
    app_data: Option<AppData>,
    pub web_data: Option<WebData>,
    pub cookies: CookiesData,
    ua: Arc<RwLock<String>>,
}

impl fmt::Debug for CookieManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CookieManager")
            .field("create_type", &self.create_type)
            .field("app_data", &self.app_data)
            .field("web_data", &self.web_data)
            .field("ua", &self.get_ua())
            .field("cookies", &self.cookies)
            .finish()
    }
}

pub(crate) fn should_send_bilibili_cookies(url: &str) -> bool {
    match reqwest::Url::parse(url) {
        Ok(parsed) => {
            if parsed.scheme() != "https" {
                return false;
            }
            match parsed.host_str() {
                Some(host) => {
                    let host = host.to_ascii_lowercase();
                    host == "bilibili.com" || host.ends_with(".bilibili.com")
                }
                None => false,
            }
        }
        Err(_) => false,
    }
}

fn is_set_cookie_attribute(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "path"
            | "domain"
            | "expires"
            | "max-age"
            | "samesite"
            | "secure"
            | "httponly"
            | "priority"
            | "partitioned"
            | "comment"
            | "version"
    )
}

fn has_header_ignore_ascii_case(headers: &HashMap<&str, &str>, name: &str) -> bool {
    headers.keys().any(|k| k.eq_ignore_ascii_case(name))
}

fn build_http_client(ua: Option<&str>, use_native_tls: bool) -> reqwest::Client {
    let make_builder = |with_ua: bool| {
        let mut builder = reqwest::Client::builder()
            .cookie_store(true)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20));
        if use_native_tls {
            builder = builder.use_native_tls();
        }
        if with_ua {
            if let Some(ua_str) = ua.map(str::trim).filter(|s| !s.is_empty()) {
                builder = builder.user_agent(ua_str);
            }
        }
        builder
    };

    make_builder(true)
        .build()
        .or_else(|e| {
            log::warn!(
                "使用指定 UA 构建 HTTP 客户端失败 ({}), 回退为默认 UA 并保留 CookieStore 与超时设置",
                e
            );
            make_builder(false).build()
        })
        .unwrap_or_else(|_| reqwest::Client::new())
}

pub(crate) fn parse_cookie_pairs(cookie_str: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let no_newlines = cookie_str.replace(['\r', '\n'], "");
    let mut cleaned = no_newlines
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim();

    if cleaned.len() >= 7 && cleaned[..7].eq_ignore_ascii_case("cookie:") {
        cleaned = cleaned[7..]
            .trim()
            .trim_matches(|c| c == '"' || c == '\'')
            .trim();
    }

    for segment in cleaned.split(';') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if let Some((raw_key, raw_val)) = segment.split_once('=') {
            let key = raw_key.trim().trim_matches(|c| c == '"' || c == '\'');
            let val = raw_val.trim().trim_matches(|c| c == '"' || c == '\'');
            if !key.is_empty() {
                map.insert(key.to_string(), val.to_string());
            }
        }
    }

    map
}

pub(crate) fn parse_set_cookie_header(header: &str) -> Option<(String, String)> {
    let cleaned = header.replace(['\r', '\n'], "");
    let first_pair = cleaned.split(';').next()?.trim();
    let (raw_key, raw_val) = first_pair.split_once('=')?;
    let key = raw_key.trim().trim_matches(|c| c == '"' || c == '\'');
    let val = raw_val.trim().trim_matches(|c| c == '"' || c == '\'');
    if key.is_empty() || is_set_cookie_attribute(key) {
        return None;
    }
    Some((key.to_string(), val.to_string()))
}

impl CookieManager {
    pub async fn new(
        original_cookie: &str,
        user_agent: Option<&str>,
        create_type: usize, // 0：默认网页浏览器 1：app
    ) -> Self {
        let cookies = Self::parse_cookie_string(original_cookie);

        match create_type {
            0 => {
                // 浏览器 UA 列表
                let browser_version_list = [
                    // chrome
                    "Chrome/126.0.6478.55",
                    "Chrome/127.0.6520.0",
                    "Chrome/125.0.6422.61",
                    "Chrome/124.0.6367.60",
                    "Chrome/135.0.0.0",
                    // edge
                    "Chrome/126.0.6478.55 Edg/126.0.2578.55",
                    "Chrome/127.0.6520.0 Edg/127.0.2610.0",
                    "Chrome/125.0.6422.61 Edg/125.0.2535.51",
                    "Chrome/124.0.6367.60 Edg/124.0.2478.67",
                    "Chrome/135.0.0.0 Edg/135.0.0.0",
                ];
                let os_version_list = ["(Windows NT 10.0; Win64; x64)"];

                let browser_version = browser_version_list
                    .choose(&mut rand::thread_rng())
                    .map(|&s| s.to_string())
                    .unwrap_or_else(|| "Chrome/135.0.0.0".to_string());
                let os_version = os_version_list
                    .choose(&mut rand::thread_rng())
                    .map(|&s| s.to_string())
                    .unwrap_or_else(|| "(Windows NT 10.0; Win64; x64)".to_string());

                let default_ua = format!(
                    "Mozilla/5.0 {} AppleWebKit/537.36 (KHTML, like Gecko) {} Safari/537.36",
                    os_version, browser_version
                );
                let ua = match user_agent
                    .map(str::trim)
                    .filter(|s| !s.is_empty() && reqwest::header::HeaderValue::from_str(s).is_ok())
                {
                    Some(ua) => ua.to_string(),
                    None => default_ua,
                };

                log::debug!("初始化 CookieManager，UA: {}", ua);

                let client = build_http_client(Some(&ua), false);
                // createV2 专用客户端：使用系统原生 TLS（Windows SChannel），保留超时与 CookieStore
                let h2_client = build_http_client(Some(&ua), true);

                // buvid3 / buvid4 / b_nut：仅在缺失或为空时拉取，失败时绝不覆盖已有有效值
                let (buvid3, buvid4, b_nut) = {
                    let (existing_buvid3, existing_buvid4, existing_b_nut) = {
                        let cookies_map = lock_or_recover(&cookies.cookies_map);
                        (
                            cookies_map
                                .get("buvid3")
                                .filter(|s| !s.is_empty())
                                .cloned(),
                            cookies_map
                                .get("buvid4")
                                .filter(|s| !s.is_empty())
                                .cloned(),
                            cookies_map
                                .get("b_nut")
                                .filter(|s| !s.is_empty())
                                .cloned(),
                        )
                    };

                    if let (Some(b3), Some(b4), Some(bn)) =
                        (&existing_buvid3, &existing_buvid4, &existing_b_nut)
                    {
                        (b3.clone(), b4.clone(), bn.clone())
                    } else {
                        match gen_buvid3and4(client.clone()).await {
                            Ok((b3, b4, bn)) => (
                                if b3.is_empty() {
                                    existing_buvid3.unwrap_or_default()
                                } else {
                                    b3
                                },
                                if b4.is_empty() {
                                    existing_buvid4.unwrap_or_default()
                                } else {
                                    b4
                                },
                                if bn.is_empty() {
                                    existing_b_nut.unwrap_or_default()
                                } else {
                                    bn
                                },
                            ),
                            Err(err) => {
                                log::warn!("生成 buvid3/buvid4 失败，保留已有值: {}", err);
                                (
                                    existing_buvid3.unwrap_or_default(),
                                    existing_buvid4.unwrap_or_default(),
                                    existing_b_nut.unwrap_or_default(),
                                )
                            }
                        }
                    }
                };

                let fp = {
                    let existing_fp = {
                        let cookies_map = lock_or_recover(&cookies.cookies_map);
                        cookies_map
                            .get("buvid_fp")
                            .filter(|s| !s.is_empty())
                            .cloned()
                    };
                    existing_fp.unwrap_or_else(gen_fp)
                };

                let _uuid = {
                    let existing_uuid = {
                        let cookies_map = lock_or_recover(&cookies.cookies_map);
                        cookies_map
                            .get("_uuid")
                            .filter(|s| !s.is_empty())
                            .cloned()
                    };
                    existing_uuid.unwrap_or_else(gen_uuid_infoc)
                };

                // bili_ticket / bili_ticket_expires：未过期复用；刷新失败时不覆盖已有有效值
                let (bili_ticket, bili_ticket_expires) = {
                    let (existing_ticket, existing_expires) = {
                        let cookies_map = lock_or_recover(&cookies.cookies_map);
                        (
                            cookies_map
                                .get("bili_ticket")
                                .filter(|s| !s.is_empty())
                                .cloned(),
                            cookies_map
                                .get("bili_ticket_expires")
                                .filter(|s| !s.is_empty())
                                .cloned(),
                        )
                    };

                    let current_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;

                    let is_valid = match (&existing_ticket, &existing_expires) {
                        (Some(_), Some(exp)) => {
                            exp.parse::<i64>().map(|t| current_time < t).unwrap_or(false)
                        }
                        _ => false,
                    };

                    if is_valid {
                        log::debug!("复用现有未过期的 bili_ticket");
                        (
                            existing_ticket.unwrap_or_default(),
                            existing_expires.unwrap_or_default(),
                        )
                    } else {
                        log::debug!("生成或刷新 bili_ticket");
                        match gen_ckbili_ticket(client.clone()).await {
                            Ok((ticket, expires)) if !ticket.is_empty() => (ticket, expires),
                            Ok(_) => (
                                existing_ticket.unwrap_or_default(),
                                existing_expires.unwrap_or_default(),
                            ),
                            Err(err) => {
                                log::warn!("生成 bili_ticket 失败，保留已有值: {}", err);
                                (
                                    existing_ticket.unwrap_or_default(),
                                    existing_expires.unwrap_or_default(),
                                )
                            }
                        }
                    }
                };

                let msource = {
                    let cookies_map = lock_or_recover(&cookies.cookies_map);
                    let existing_msource = cookies_map
                        .get("msource")
                        .filter(|s| !s.is_empty())
                        .cloned();
                    drop(cookies_map);
                    existing_msource.unwrap_or_else(|| "bilibiliapp".to_string())
                };

                let device_fp = {
                    let cookies_map = lock_or_recover(&cookies.cookies_map);
                    let existing = cookies_map
                        .get("deviceFingerprint")
                        .filter(|s| !s.is_empty())
                        .cloned();
                    drop(cookies_map);
                    existing.unwrap_or_else(gen_01x88)
                };

                if !buvid3.is_empty() {
                    cookies.insert("buvid3".to_string(), buvid3.clone());
                }
                if !buvid4.is_empty() {
                    cookies.insert("buvid4".to_string(), buvid4.clone());
                }
                if !b_nut.is_empty() {
                    cookies.insert("b_nut".to_string(), b_nut.clone());
                }
                cookies.insert("buvid_fp".to_string(), fp.clone());
                cookies.insert("_uuid".to_string(), _uuid.clone());
                if !bili_ticket.is_empty() {
                    cookies.insert("bili_ticket".to_string(), bili_ticket.clone());
                }
                if !bili_ticket_expires.is_empty() {
                    cookies.insert(
                        "bili_ticket_expires".to_string(),
                        bili_ticket_expires.clone(),
                    );
                }
                cookies.insert("header_theme_version".to_string(), "CLOSE".to_string());
                cookies.insert("enable_web_push".to_string(), "DISABLE".to_string());
                cookies.insert("enable_feed_channel".to_string(), "ENABLE".to_string());
                cookies.insert("msource".to_string(), msource.clone());
                cookies.insert("deviceFingerprint".to_string(), device_fp);

                let canvas_fp = crate::fe_sign::random_hex_32();
                let webgl_fp = crate::fe_sign::random_hex_32();
                let fe_sign = crate::fe_sign::get_fe_sign(
                    crate::fe_sign::WEBVIEW_UA,
                    &canvas_fp,
                    &webgl_fp,
                );
                cookies.insert("feSign".to_string(), fe_sign);
                cookies.insert(
                    "screenInfo".to_string(),
                    crate::fe_sign::SCREEN_INFO.to_string(),
                );
                cookies.insert("kfcFrom".to_string(), "mall_home_searchhis".to_string());
                cookies.insert("from".to_string(), "mall_search_discovery".to_string());
                cookies.insert("kfcSource".to_string(), "bilibiliapp".to_string());
                cookies.insert("mSource".to_string(), "bilibiliapp".to_string());

                let web_data = WebData {
                    browser_version,
                    os_version,
                    ua: ua.clone(),
                    buvid3,
                    buvid4,
                    b_nut,
                    buvid_fp: fp,
                    _uuid,
                    bili_ticket,
                    bili_ticket_expires,
                    msource,
                };

                Self {
                    client: Arc::new(client),
                    h2_client: Arc::new(h2_client),
                    create_type,
                    app_data: None,
                    web_data: Some(web_data),
                    cookies,
                    ua: Arc::new(RwLock::new(ua)),
                }
            }
            _ => {
                log::warn!("未知的 CookieManager 创建类型: {}", create_type);
                let fallback = Arc::new(build_http_client(None, false));
                let init_ua = user_agent
                    .map(str::trim)
                    .filter(|s| !s.is_empty() && reqwest::header::HeaderValue::from_str(s).is_ok())
                    .unwrap_or_default()
                    .to_string();
                Self {
                    client: fallback.clone(),
                    h2_client: fallback,
                    create_type: 0,
                    app_data: None,
                    web_data: None,
                    cookies,
                    ua: Arc::new(RwLock::new(init_ua)),
                }
            }
        }
    }

    pub fn get_all_cookies(&self) -> String {
        let cookies_map = lock_or_recover(&self.cookies.cookies_map);
        cookies_map
            .iter()
            .map(|(key, value)| format!("{}={}", key, value))
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn parse_cookie_string(cookie_str: &str) -> CookiesData {
        let map = parse_cookie_pairs(cookie_str);
        CookiesData {
            cookies_map: Arc::new(Mutex::new(map)),
        }
    }

    // 现有 client 创建 ck 管理器（已封进 client 的 ck 无法读取）
    pub fn from_client(client: Arc<reqwest::Client>, original_cookie: &str) -> Self {
        let cookies = Self::parse_cookie_string(original_cookie);
        Self {
            client: client.clone(),
            h2_client: client,
            create_type: 0,
            app_data: None,
            web_data: None,
            cookies,
            ua: Arc::new(RwLock::new(String::new())),
        }
    }

    // 更新单个字段
    pub fn update_cookie(&self, key: &str, value: &str) {
        let clean_key = key.trim();
        if clean_key.is_empty() {
            return;
        }
        self.cookies
            .insert(clean_key.to_string(), value.trim().to_string());
        log::debug!("更新 Cookie 键: {}", clean_key);
    }

    // 移除某个键对应的值
    pub fn remove_cookie(&self, key: &str) -> bool {
        let existed = lock_or_recover(&self.cookies.cookies_map)
            .remove(key)
            .is_some();
        if existed {
            log::debug!("删除 Cookie 键: {}", key);
        } else {
            log::debug!("Cookie 键不存在: {}", key);
        }
        existed
    }

    // 批量更新 Cookie（兼容普通 Cookie 串与 Set-Cookie 响应头，自动忽略 Path/Domain/Expires 等属性）
    pub fn update_cookies(&self, cookies_str: &str) {
        let parsed = parse_cookie_pairs(cookies_str);
        let mut updated_count = 0usize;
        for (key, value) in parsed {
            if is_set_cookie_attribute(&key) {
                continue;
            }
            self.cookies.insert(key, value);
            updated_count += 1;
        }
        log::debug!("批量更新 Cookie 完成，共 {} 项", updated_count);
    }

    // 清除所有 ck
    pub fn clear_all_cookies(&self) {
        self.cookies.clear();
        log::debug!("清除所有 Cookie");
    }

    // 获取某个键的值
    pub fn get_cookie(&self, key: &str) -> Option<String> {
        let cookies = lock_or_recover(&self.cookies.cookies_map);
        cookies.get(key).cloned()
    }

    /// 动态更新 User-Agent（自动忽略空值或非法 HTTP 请求头字符），立即对后续所有请求生效。
    pub fn set_ua(&self, ua: &str) {
        let trimmed = ua.trim();
        if trimmed.is_empty() {
            return;
        }
        if reqwest::header::HeaderValue::from_str(trimmed).is_err() {
            log::warn!("忽略无效的 User-Agent 请求头值");
            return;
        }
        let mut guard = self.ua.write().unwrap_or_else(|e| e.into_inner());
        *guard = trimmed.to_string();
    }

    /// 读取当前动态 User-Agent（自动兼容 poisoned RwLock）。
    pub fn get_ua(&self) -> String {
        self.ua.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    // 发送 GET 请求
    pub async fn get(&self, url: &str) -> reqwest::RequestBuilder {
        let builder = self.client.get(url);
        self.prepare_request(url, builder)
    }

    // 发送 POST 请求
    pub async fn post(&self, url: &str) -> reqwest::RequestBuilder {
        let builder = self.client.post(url);
        self.prepare_request(url, builder)
    }

    // 创建具有自定义标头的请求 - 优先使用传入的 headers
    pub async fn get_with_headers(
        &self,
        url: &str,
        headers: HashMap<&str, &str>,
    ) -> reqwest::RequestBuilder {
        let builder = self.client.get(url);
        self.prepare_request_with_overrides(url, builder, &headers)
    }

    pub async fn post_with_headers(
        &self,
        url: &str,
        headers: HashMap<&str, &str>,
    ) -> reqwest::RequestBuilder {
        let builder = self.client.post(url);
        self.prepare_request_with_overrides(url, builder, &headers)
    }

    /// 与 [`post_with_headers`] 完全相同的 cookie / header 处理逻辑，
    /// 仅把底层客户端换成系统原生 TLS 后端的 `h2_client`（通过 ALPN 协商 HTTP/2）。
    pub async fn post_with_headers_h2(
        &self,
        url: &str,
        headers: HashMap<&str, &str>,
    ) -> reqwest::RequestBuilder {
        let builder = self.h2_client.post(url);
        self.prepare_request_with_overrides(url, builder, &headers)
    }

    // 处理请求头，允许传入的 headers（不区分大小写）覆盖默认值；
    // 安全隔离：仅向 HTTPS 的 *.bilibili.com / bilibili.com 注入 B 站 Cookie、Referer 和 Origin。
    fn prepare_request_with_overrides(
        &self,
        url: &str,
        mut builder: reqwest::RequestBuilder,
        custom_headers: &HashMap<&str, &str>,
    ) -> reqwest::RequestBuilder {
        let is_bili_https = should_send_bilibili_cookies(url);

        // 1. 仅向 HTTPS B 站域名注入会话 Cookie（且未被自定义头覆盖）
        if is_bili_https && !has_header_ignore_ascii_case(custom_headers, "cookie") {
            let cookies = lock_or_recover(&self.cookies.cookies_map);
            if !cookies.is_empty() {
                let cookie_header = cookies
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("; ");
                builder = builder.header(reqwest::header::COOKIE, cookie_header);
            }
        }

        // 2. 添加自定义 headers
        for (key, value) in custom_headers {
            builder = builder.header(*key, *value);
        }

        // 3. 注入动态 User-Agent（当自定义头未指定 User-Agent 时）
        if !has_header_ignore_ascii_case(custom_headers, "user-agent") {
            let current_ua = self.get_ua();
            if !current_ua.is_empty() {
                builder = builder.header(reqwest::header::USER_AGENT, current_ua);
            }
        }

        // 4. 根据创建类型添加其他默认头（不区分大小写检查是否已被自定义头覆盖）
        match self.create_type {
            0 => {
                if self.web_data.is_some() && is_bili_https {
                    if !has_header_ignore_ascii_case(custom_headers, "referer") {
                        builder =
                            builder.header(reqwest::header::REFERER, "https://show.bilibili.com/");
                    }
                    if !has_header_ignore_ascii_case(custom_headers, "origin") {
                        builder =
                            builder.header(reqwest::header::ORIGIN, "https://show.bilibili.com");
                    }
                }
                builder
            }
            1 => {
                if self.app_data.is_some()
                    && is_bili_https
                    && !has_header_ignore_ascii_case(custom_headers, "x-bili-aurora-zone")
                {
                    builder = builder.header("x-bili-aurora-zone", "sh");
                }
                builder
            }
            _ => builder,
        }
    }

    // 临时覆盖 UA
    pub async fn with_custom_ua(
        &self,
        builder: reqwest::RequestBuilder,
        ua: &str,
    ) -> reqwest::RequestBuilder {
        builder.header(reqwest::header::USER_AGENT, ua)
    }

    fn prepare_request(
        &self,
        url: &str,
        builder: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        self.prepare_request_with_overrides(url, builder, &HashMap::new())
    }

    // 处理响应中的 Set-Cookie（仅提取首段 name=value，不把 Path/Domain/Expires 等属性写入 Cookie Map）
    pub async fn execute(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let response = request.send().await?;

        for cookie_header in response.headers().get_all(reqwest::header::SET_COOKIE) {
            if let Ok(cookie_str) = cookie_header.to_str() {
                if let Some((key, val)) = parse_set_cookie_header(cookie_str) {
                    self.update_cookie(&key, &val);
                }
            }
        }

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_send_bilibili_cookies_enforces_https_and_domain_boundary() {
        assert!(should_send_bilibili_cookies("https://show.bilibili.com/api/ticket/project/getV2"));
        assert!(should_send_bilibili_cookies("https://api.bilibili.com/x/web-interface/nav"));
        assert!(should_send_bilibili_cookies("https://bilibili.com/"));
        assert!(should_send_bilibili_cookies("https://PASSPORT.BILIBILI.COM/login/exit/v2"));

        assert!(!should_send_bilibili_cookies("https://i0.hdslb.com/bfs/face/avatar.jpg"));
        assert!(!should_send_bilibili_cookies("http://show.bilibili.com/api/ticket"));
        assert!(!should_send_bilibili_cookies("https://evil-bilibili.com/steal"));
        assert!(!should_send_bilibili_cookies("https://bilibili.com.evil.com/steal"));
        assert!(!should_send_bilibili_cookies("not-a-url"));
    }

    #[test]
    fn parse_cookie_string_sanitizes_and_preserves_base64_padding() {
        let raw = "  \"Cookie: SESSDATA=abc123==; bili_jct = deadbeef; DedeUserID=42; =bad; empty_val=\r\n\"  ";
        let map = parse_cookie_pairs(raw);
        assert_eq!(map.get("SESSDATA").map(String::as_str), Some("abc123=="));
        assert_eq!(map.get("bili_jct").map(String::as_str), Some("deadbeef"));
        assert_eq!(map.get("DedeUserID").map(String::as_str), Some("42"));
        assert_eq!(map.get("empty_val").map(String::as_str), Some(""));
        assert!(!map.contains_key(""));
    }

    #[test]
    fn set_cookie_parsing_ignores_attributes() {
        let header = "SESSDATA=xyz%2C123==; Path=/; Domain=.bilibili.com; Expires=Wed, 21 Oct 2026 07:28:00 GMT; Max-Age=3600; HttpOnly; Secure; SameSite=Lax";
        let parsed = parse_set_cookie_header(header).expect("should parse Set-Cookie pair");
        assert_eq!(parsed.0, "SESSDATA");
        assert_eq!(parsed.1, "xyz%2C123==");

        let cm = CookieManager::from_client(Arc::new(reqwest::Client::new()), "old=1");
        cm.update_cookies(header);
        assert_eq!(cm.get_cookie("SESSDATA").as_deref(), Some("xyz%2C123=="));
        assert!(cm.get_cookie("Path").is_none());
        assert!(cm.get_cookie("Domain").is_none());
        assert!(cm.get_cookie("Expires").is_none());
        assert!(cm.get_cookie("Max-Age").is_none());
        assert!(cm.get_cookie("SameSite").is_none());
        assert!(cm.get_cookie("HttpOnly").is_none());
        assert!(cm.get_cookie("Secure").is_none());
    }

    #[test]
    fn debug_redacts_cookie_values() {
        let cm = CookieManager::from_client(
            Arc::new(reqwest::Client::new()),
            "SESSDATA=top_secret_sess; bili_jct=top_secret_csrf",
        );
        let rendered = format!("{:?}", cm);
        assert!(!rendered.contains("top_secret_sess"));
        assert!(!rendered.contains("top_secret_csrf"));
        assert!(rendered.contains("SESSDATA"));
    }

    #[test]
    fn case_insensitive_header_override_check() {
        let mut custom = HashMap::new();
        custom.insert("user-agent", "CustomUA");
        custom.insert("referer", "https://show.bilibili.com/platform/detail.html");
        assert!(has_header_ignore_ascii_case(&custom, "User-Agent"));
        assert!(has_header_ignore_ascii_case(&custom, "Referer"));
        assert!(!has_header_ignore_ascii_case(&custom, "Origin"));
    }

    #[tokio::test]
    async fn prepare_request_never_leaks_cookies_to_cdn_or_http() {
        let cm = CookieManager::from_client(
            Arc::new(reqwest::Client::new()),
            "SESSDATA=secret_session; bili_jct=csrf_token",
        );

        let bili_req = cm
            .get("https://show.bilibili.com/api/ticket/buyer/list")
            .await
            .build()
            .expect("build bilibili request");
        assert!(bili_req.headers().contains_key(reqwest::header::COOKIE));

        let cdn_req = cm
            .get("https://i0.hdslb.com/bfs/face/avatar.jpg")
            .await
            .build()
            .expect("build cdn request");
        assert!(!cdn_req.headers().contains_key(reqwest::header::COOKIE));
        assert!(!cdn_req.headers().contains_key(reqwest::header::REFERER));
        assert!(!cdn_req.headers().contains_key(reqwest::header::ORIGIN));

        let http_req = cm
            .get("http://show.bilibili.com/api/ticket")
            .await
            .build()
            .expect("build plaintext http request");
        assert!(!http_req.headers().contains_key(reqwest::header::COOKIE));
    }

    #[tokio::test]
    async fn set_ua_updates_outgoing_user_agent_header() {
        let cm = CookieManager::from_client(
            Arc::new(reqwest::Client::new()),
            "SESSDATA=test_sess",
        );
        assert_eq!(cm.get_ua(), "");

        cm.set_ua("CustomAgent/1.0");
        assert_eq!(cm.get_ua(), "CustomAgent/1.0");

        let req1 = cm
            .get("https://show.bilibili.com/api/ticket/project/getV2")
            .await
            .build()
            .expect("build first request");
        assert_eq!(
            req1.headers()
                .get(reqwest::header::USER_AGENT)
                .and_then(|v| v.to_str().ok()),
            Some("CustomAgent/1.0")
        );

        // 动态更新 UA 后所有克隆实例与后续请求立即生效
        let cm_clone = cm.clone();
        cm.set_ua("  UpdatedAgent/2.0  ");
        assert_eq!(cm_clone.get_ua(), "UpdatedAgent/2.0");

        // 空值与非法 HTTP header 值应被静默忽略，不覆盖已有 UA
        cm.set_ua("   ");
        cm.set_ua("Bad\r\nInjectedAgent");
        assert_eq!(cm.get_ua(), "UpdatedAgent/2.0");

        let req2 = cm_clone
            .post("https://show.bilibili.com/api/ticket/order/prepare")
            .await
            .build()
            .expect("build second request");
        assert_eq!(
            req2.headers()
                .get(reqwest::header::USER_AGENT)
                .and_then(|v| v.to_str().ok()),
            Some("UpdatedAgent/2.0")
        );

        // 自定义 User-Agent 请求头仍优先覆盖动态 UA
        let mut custom_headers = HashMap::new();
        custom_headers.insert("User-Agent", "OverrideUA/3.0");
        let req3 = cm
            .get_with_headers(
                "https://show.bilibili.com/api/ticket/buyer/list",
                custom_headers,
            )
            .await
            .build()
            .expect("build override request");
        assert_eq!(
            req3.headers()
                .get(reqwest::header::USER_AGENT)
                .and_then(|v| v.to_str().ok()),
            Some("OverrideUA/3.0")
        );
    }
}
