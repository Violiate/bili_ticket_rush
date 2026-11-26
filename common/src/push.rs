use std::fmt;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use reqwest::header::HeaderValue;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::taskmanager::{PushRequest, PushType, TaskManager, TaskRequest};

const PUSH_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const PUSH_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

fn redact_token(value: &str) -> &'static str {
    if value.is_empty() {
        ""
    } else {
        "[REDACTED]"
    }
}

fn build_push_client() -> Client {
    Client::builder()
        .connect_timeout(PUSH_CONNECT_TIMEOUT)
        .timeout(PUSH_REQUEST_TIMEOUT)
        .build()
        .unwrap_or_else(|_| Client::new())
}

// 推送配置
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PushConfig {
    pub enabled: bool,
    pub bark_token: String,
    pub pushplus_token: String,
    pub fangtang_token: String,
    pub dingtalk_token: String,
    pub wechat_token: String,
    pub gotify_config: GotifyConfig,
    pub smtp_config: SmtpConfig,
}

impl fmt::Debug for PushConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PushConfig")
            .field("enabled", &self.enabled)
            .field("bark_token", &redact_token(&self.bark_token))
            .field("pushplus_token", &redact_token(&self.pushplus_token))
            .field("fangtang_token", &redact_token(&self.fangtang_token))
            .field("dingtalk_token", &redact_token(&self.dingtalk_token))
            .field("wechat_token", &redact_token(&self.wechat_token))
            .field("gotify_config", &self.gotify_config)
            .field("smtp_config", &self.smtp_config)
            .finish()
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GotifyConfig {
    pub gotify_url: String,
    pub gotify_token: String,
}

impl fmt::Debug for GotifyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GotifyConfig")
            .field("gotify_url", &self.gotify_url)
            .field("gotify_token", &redact_token(&self.gotify_token))
            .finish()
    }
}

impl GotifyConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

// 邮箱配置（属于 PushConfig）
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SmtpConfig {
    pub smtp_server: String,
    pub smtp_port: String,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_from: String,
    pub smtp_to: String,
}

impl fmt::Debug for SmtpConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmtpConfig")
            .field("smtp_server", &self.smtp_server)
            .field("smtp_port", &self.smtp_port)
            .field("smtp_username", &self.smtp_username)
            .field("smtp_password", &redact_token(&self.smtp_password))
            .field("smtp_from", &self.smtp_from)
            .field("smtp_to", &self.smtp_to)
            .finish()
    }
}

impl SmtpConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

pub(crate) fn normalize_gotify_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Gotify 服务器地址为空".to_string());
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Ok(trimmed.to_string())
    } else if trimmed.contains("://") {
        Err("Gotify 服务器协议不受支持，仅支持 http:// 或 https://".to_string())
    } else {
        let use_http = if let Ok(probe) = reqwest::Url::parse(&format!("https://{}", trimmed)) {
            let host = probe.host_str().unwrap_or("");
            let host_unbracketed = host.trim_start_matches('[').trim_end_matches(']');
            let is_localhost = host_unbracketed.eq_ignore_ascii_case("localhost");
            let is_ip = host_unbracketed.parse::<std::net::IpAddr>().is_ok();
            // 使用 https:// 探测时，仅当存在显式非 443 端口，probe.port() 才为 Some(_)
            let has_non_443_port = probe.port().is_some();
            is_localhost || is_ip || has_non_443_port
        } else {
            false
        };
        let scheme = if use_http { "http" } else { "https" };
        Ok(format!("{}://{}", scheme, trimmed))
    }
}

pub(crate) fn build_bark_request(
    raw_token: &str,
    title: &str,
    message: &str,
    jump_url: &Option<String>,
) -> Result<(String, serde_json::Value), String> {
    let trimmed = raw_token.trim();
    if trimmed.is_empty() {
        return Err("Bark Token 为空".to_string());
    }

    let (base_part, query_opt) = match trimmed.split_once('?') {
        Some((base, query)) => (base, Some(query)),
        None => (trimmed, None),
    };

    let base_clean = base_part.trim().trim_end_matches('/');
    if base_clean.is_empty() {
        return Err("Bark Token 为空".to_string());
    }

    let url = if base_clean.starts_with("http://") || base_clean.starts_with("https://") {
        base_clean.to_string()
    } else {
        format!("https://api.day.app/{}", base_clean.trim_start_matches('/'))
    };

    let mut data = serde_json::json!({
        "title": title,
        "body": message,
        "level": "timeSensitive",
        "badge": 1,
        "icon": "https://sr.mihoyo.com/favicon-mi.ico",
        "group": "biliticket",
        "isArchive": 1,
    });

    if let Some(target_url) = jump_url.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        data["url"] = serde_json::json!(target_url);
    }

    if let Some(query_string) = query_opt {
        if let Some(json_object) = data.as_object_mut() {
            for pair in query_string.split('&') {
                if pair.is_empty() {
                    continue;
                }
                if let Some((key, value)) = pair.split_once('=') {
                    let key = key.trim();
                    if key.is_empty() {
                        continue;
                    }
                    let json_val = if matches!(key, "badge" | "isArchive" | "volume") {
                        match value.trim().parse::<i64>() {
                            Ok(num) => serde_json::json!(num),
                            Err(_) => serde_json::json!(value),
                        }
                    } else {
                        serde_json::json!(value)
                    };
                    json_object.insert(key.to_string(), json_val);
                }
            }
        }
    }

    Ok((url, data))
}

pub(crate) fn parse_bark_response(status: StatusCode, text: &str) -> (bool, String) {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(code) = json.get("code").and_then(|v| v.as_i64()) {
            if status.is_success() && code == 200 {
                return (true, "推送成功".to_string());
            }
            let msg = json
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("未知错误");
            return (false, format!("Bark 推送失败 (code {}): {}", code, msg));
        }
    }
    if status.is_success() {
        (false, "Bark 响应格式异常，缺少有效状态码".to_string())
    } else {
        (false, format!("推送失败，状态码: {}", status))
    }
}

pub(crate) fn parse_pushplus_response(status: StatusCode, text: &str) -> (bool, String) {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(code) = json.get("code").and_then(|v| v.as_i64()) {
            if status.is_success() && code == 200 {
                return (true, "推送成功".to_string());
            }
            let msg = json
                .get("msg")
                .and_then(|v| v.as_str())
                .unwrap_or("未知错误");
            return (false, format!("PushPlus 推送失败 (code {}): {}", code, msg));
        }
    }
    if status.is_success() {
        (false, "PushPlus 响应格式异常".to_string())
    } else {
        (false, format!("推送失败，状态码: {}", status))
    }
}

pub(crate) fn parse_fangtang_response(status: StatusCode, text: &str) -> (bool, String) {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(code) = json.get("code").and_then(|v| v.as_i64()) {
            if status.is_success() && code == 0 {
                return (true, "推送成功".to_string());
            }
            let msg = json
                .get("message")
                .and_then(|v| v.as_str())
                .or_else(|| json.get("info").and_then(|v| v.as_str()))
                .unwrap_or("未知错误");
            return (false, format!("方糖推送失败 (code {}): {}", code, msg));
        }
    }
    if status.is_success() {
        (false, "方糖响应格式异常".to_string())
    } else {
        (false, format!("推送失败，状态码: {}", status))
    }
}

pub(crate) fn parse_dingtalk_response(status: StatusCode, text: &str) -> (bool, String) {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(errcode) = json.get("errcode").and_then(|v| v.as_i64()) {
            if status.is_success() && errcode == 0 {
                return (true, "推送成功".to_string());
            }
            let errmsg = json
                .get("errmsg")
                .and_then(|v| v.as_str())
                .unwrap_or("未知错误");
            return (
                false,
                format!("钉钉推送失败 (errcode {}): {}", errcode, errmsg),
            );
        }
    }
    if status.is_success() {
        (false, "钉钉响应格式异常".to_string())
    } else {
        (false, format!("推送失败，状态码: {}", status))
    }
}

pub(crate) fn parse_wechat_response(status: StatusCode, text: &str) -> (bool, String) {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(errcode) = json.get("errcode").and_then(|v| v.as_i64()) {
            if status.is_success() && errcode == 0 {
                return (true, "推送成功".to_string());
            }
            let errmsg = json
                .get("errmsg")
                .and_then(|v| v.as_str())
                .unwrap_or("未知错误");
            return (
                false,
                format!("企业微信推送失败 (errcode {}): {}", errcode, errmsg),
            );
        }
    }
    if status.is_success() {
        (false, "企业微信响应格式异常".to_string())
    } else {
        (false, format!("推送失败，状态码: {}", status))
    }
}

fn contains_crlf(s: &str) -> bool {
    s.contains('\r') || s.contains('\n')
}

struct SmtpConn<S> {
    reader: BufReader<S>,
}

impl<S: AsyncRead + AsyncWrite + Unpin> SmtpConn<S> {
    fn new(stream: S) -> Self {
        Self {
            reader: BufReader::new(stream),
        }
    }

    fn into_inner(self) -> S {
        self.reader.into_inner()
    }

    async fn read_response(&mut self) -> Result<(u16, Vec<String>), String> {
        let mut lines = Vec::new();

        for _ in 0..100 {
            let mut buf = Vec::new();
            let bytes = self
                .reader
                .read_until(b'\n', &mut buf)
                .await
                .map_err(|e| format!("读取 SMTP 响应失败: {}", e))?;
            if bytes == 0 {
                return Err("SMTP 服务器意外关闭连接".to_string());
            }
            let line = String::from_utf8_lossy(&buf);
            let trimmed = line.trim_end_matches(['\r', '\n']).to_string();
            if trimmed.len() < 3 || !trimmed.as_bytes()[..3].iter().all(|b| b.is_ascii_digit()) {
                return Err(format!("无效的 SMTP 响应行: {}", trimmed));
            }
            let code: u16 = trimmed[..3]
                .parse()
                .map_err(|_| format!("无法解析 SMTP 状态码: {}", trimmed))?;
            let is_multiline = trimmed.as_bytes().get(3) == Some(&b'-');
            lines.push(trimmed);
            if !is_multiline {
                return Ok((code, lines));
            }
        }

        Err("SMTP 多行响应超出上限".to_string())
    }

    async fn send_command(&mut self, cmd: &str) -> Result<(u16, Vec<String>), String> {
        let writer = self.reader.get_mut();
        writer
            .write_all(cmd.as_bytes())
            .await
            .map_err(|e| format!("发送 SMTP 命令失败: {}", e))?;
        writer
            .write_all(b"\r\n")
            .await
            .map_err(|e| format!("发送 SMTP 换行失败: {}", e))?;
        writer
            .flush()
            .await
            .map_err(|e| format!("刷新 SMTP 发送缓冲区失败: {}", e))?;
        self.read_response().await
    }

    async fn write_raw_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        let writer = self.reader.get_mut();
        writer
            .write_all(bytes)
            .await
            .map_err(|e| format!("写入邮件正文失败: {}", e))?;
        writer
            .flush()
            .await
            .map_err(|e| format!("刷新邮件正文失败: {}", e))
    }
}

fn ehlo_supports_starttls(ehlo_lines: &[String]) -> bool {
    ehlo_lines.iter().any(|line| {
        let rest = line.get(4..).unwrap_or("").trim();
        rest.eq_ignore_ascii_case("STARTTLS")
            || rest
                .split_whitespace()
                .next()
                .map(|w| w.eq_ignore_ascii_case("STARTTLS"))
                .unwrap_or(false)
    })
}

fn ehlo_auth_methods(ehlo_lines: &[String]) -> (bool, bool) {
    let mut has_login = false;
    let mut has_plain = false;
    for line in ehlo_lines {
        let rest = line.get(4..).unwrap_or("").trim();
        let mut tokens = rest.split_whitespace();
        if let Some(first) = tokens.next() {
            if first.eq_ignore_ascii_case("AUTH")
                || first.to_ascii_uppercase().starts_with("AUTH=")
            {
                if let Some(after_eq) = first.split_once('=').map(|(_, v)| v) {
                    if after_eq.eq_ignore_ascii_case("LOGIN") {
                        has_login = true;
                    } else if after_eq.eq_ignore_ascii_case("PLAIN") {
                        has_plain = true;
                    }
                }
                for tok in tokens {
                    if tok.eq_ignore_ascii_case("LOGIN") {
                        has_login = true;
                    } else if tok.eq_ignore_ascii_case("PLAIN") {
                        has_plain = true;
                    }
                }
            }
        }
    }
    (has_login, has_plain)
}

pub(crate) fn encode_rfc2047_utf8_subject(subject: &str) -> String {
    if subject.is_empty() {
        return String::new();
    }
    const MAX_CHUNK_BYTES: usize = 45;
    let mut words = Vec::new();
    let mut start = 0;

    while start < subject.len() {
        let mut end = start;
        for (offset, ch) in subject[start..].char_indices() {
            let next_end = start + offset + ch.len_utf8();
            if next_end - start > MAX_CHUNK_BYTES {
                break;
            }
            end = next_end;
        }
        if end == start {
            if let Some(ch) = subject[start..].chars().next() {
                end = start + ch.len_utf8();
            } else {
                break;
            }
        }
        let chunk = &subject[start..end];
        words.push(format!("=?UTF-8?B?{}?=", BASE64.encode(chunk.as_bytes())));
        start = end;
    }

    words.join("\r\n ")
}

fn extract_domain_from_address(addr: &str) -> &str {
    addr.rsplit_once('@')
        .map(|(_, domain)| domain.trim())
        .filter(|d| !d.is_empty() && !d.chars().any(char::is_whitespace))
        .unwrap_or("localhost")
}

pub(crate) fn format_smtp_data_payload(
    from: &str,
    recipients: &[&str],
    title: &str,
    message: &str,
) -> String {
    let encoded_subject = encode_rfc2047_utf8_subject(title);
    let to_header = recipients
        .iter()
        .map(|r| format!("<{}>", r))
        .collect::<Vec<_>>()
        .join(", ");
    let date_header = chrono::Utc::now().to_rfc2822();
    let msg_id_domain = extract_domain_from_address(from);
    let message_id = format!("<{}@{}>", uuid::Uuid::new_v4(), msg_id_domain);

    let mut payload = String::new();
    payload.push_str(&format!("Date: {}\r\n", date_header));
    payload.push_str(&format!("Message-ID: {}\r\n", message_id));
    payload.push_str(&format!("From: <{}>\r\n", from));
    payload.push_str(&format!("To: {}\r\n", to_header));
    payload.push_str(&format!("Subject: {}\r\n", encoded_subject));
    payload.push_str("MIME-Version: 1.0\r\n");
    payload.push_str("Content-Type: text/plain; charset=UTF-8\r\n");
    payload.push_str("Content-Transfer-Encoding: 8bit\r\n");
    payload.push_str("\r\n");

    let normalized = message.replace("\r\n", "\n").replace('\r', "\n");
    for line in normalized.split('\n') {
        if line.starts_with('.') {
            payload.push('.');
        }
        payload.push_str(line);
        payload.push_str("\r\n");
    }
    payload.push_str(".\r\n");
    payload
}

async fn finish_smtp_delivery<S: AsyncRead + AsyncWrite + Unpin>(
    conn: &mut SmtpConn<S>,
    ehlo_lines: &[String],
    username: &str,
    password: &str,
    from: &str,
    recipients: &[&str],
    title: &str,
    message: &str,
) -> Result<(), String> {
    // 认证（当用户名和密码均非空时）
    if !username.is_empty() && !password.is_empty() {
        let (has_login, has_plain) = ehlo_auth_methods(ehlo_lines);
        if has_login || !has_plain {
            let (code, lines) = conn.send_command("AUTH LOGIN").await?;
            if code == 334 {
                let (u_code, u_lines) = conn.send_command(&BASE64.encode(username.as_bytes())).await?;
                if u_code != 334 {
                    return Err(format!("SMTP AUTH LOGIN 用户名被拒绝 ({}): {}", u_code, u_lines.join(" ")));
                }
                let (p_code, p_lines) = conn.send_command(&BASE64.encode(password.as_bytes())).await?;
                if p_code != 235 {
                    return Err(format!("SMTP AUTH LOGIN 认证失败 ({}): {}", p_code, p_lines.join(" ")));
                }
            } else if has_plain {
                let mut raw = Vec::with_capacity(username.len() + password.len() + 2);
                raw.push(0);
                raw.extend_from_slice(username.as_bytes());
                raw.push(0);
                raw.extend_from_slice(password.as_bytes());
                let (p_code, p_lines) = conn
                    .send_command(&format!("AUTH PLAIN {}", BASE64.encode(&raw)))
                    .await?;
                if p_code != 235 {
                    return Err(format!("SMTP AUTH PLAIN 认证失败 ({}): {}", p_code, p_lines.join(" ")));
                }
            } else {
                return Err(format!("SMTP AUTH LOGIN 被拒绝 ({}): {}", code, lines.join(" ")));
            }
        } else {
            let mut raw = Vec::with_capacity(username.len() + password.len() + 2);
            raw.push(0);
            raw.extend_from_slice(username.as_bytes());
            raw.push(0);
            raw.extend_from_slice(password.as_bytes());
            let (p_code, p_lines) = conn
                .send_command(&format!("AUTH PLAIN {}", BASE64.encode(&raw)))
                .await?;
            if p_code != 235 {
                return Err(format!("SMTP AUTH PLAIN 认证失败 ({}): {}", p_code, p_lines.join(" ")));
            }
        }
    }

    // MAIL FROM
    let (code, lines) = conn.send_command(&format!("MAIL FROM:<{}>", from)).await?;
    if code != 250 {
        return Err(format!("SMTP MAIL FROM 失败 ({}): {}", code, lines.join(" ")));
    }

    // RCPT TO（支持多收件人）
    for rcpt in recipients {
        let (code, lines) = conn.send_command(&format!("RCPT TO:<{}>", rcpt)).await?;
        if code != 250 && code != 251 {
            return Err(format!(
                "SMTP RCPT TO <{}> 失败 ({}): {}",
                rcpt,
                code,
                lines.join(" ")
            ));
        }
    }

    // DATA
    let (code, lines) = conn.send_command("DATA").await?;
    if code != 354 {
        return Err(format!("SMTP DATA 被拒绝 ({}): {}", code, lines.join(" ")));
    }

    let payload = format_smtp_data_payload(from, recipients, title, message);
    conn.write_raw_bytes(payload.as_bytes()).await?;
    let (code, lines) = conn.read_response().await?;
    if code != 250 {
        return Err(format!("SMTP 邮件正文发送失败 ({}): {}", code, lines.join(" ")));
    }

    // 优雅关闭会话
    let _ = conn.send_command("QUIT").await;
    Ok(())
}

async fn send_smtp_inner(config: &SmtpConfig, title: &str, message: &str) -> Result<(), String> {
    let server = config.smtp_server.trim();
    if server.is_empty() {
        return Err("SMTP 服务器地址为空".to_string());
    }
    if contains_crlf(server) || server.chars().any(char::is_whitespace) {
        return Err("SMTP 服务器地址包含非法字符".to_string());
    }

    let port: u16 = if config.smtp_port.trim().is_empty() {
        465
    } else {
        config
            .smtp_port
            .trim()
            .parse::<u16>()
            .map_err(|_| format!("无效的 SMTP 端口号: {}", config.smtp_port.trim()))?
    };
    if port == 0 {
        return Err("SMTP 端口号不能为 0".to_string());
    }

    let username = config.smtp_username.trim();
    let password = config.smtp_password.as_str();
    let from = if config.smtp_from.trim().is_empty() {
        username
    } else {
        config.smtp_from.trim()
    };

    if from.is_empty() {
        return Err("SMTP 发件人地址为空".to_string());
    }
    if contains_crlf(from) || from.contains('<') || from.contains('>') {
        return Err("SMTP 发件人地址包含非法字符".to_string());
    }

    if contains_crlf(title) {
        return Err("邮件标题包含非法换行符".to_string());
    }

    if contains_crlf(&config.smtp_to) {
        return Err("SMTP 收件人地址包含非法换行符".to_string());
    }

    let recipients: Vec<&str> = config
        .smtp_to
        .split([',', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if recipients.is_empty() {
        return Err("SMTP 收件人地址为空".to_string());
    }
    for rcpt in &recipients {
        if contains_crlf(rcpt) || rcpt.contains('<') || rcpt.contains('>') {
            return Err(format!("SMTP 收件人地址包含非法字符: {}", rcpt));
        }
    }

    let tcp = TcpStream::connect((server, port))
        .await
        .map_err(|e| format!("连接 SMTP 服务器 {}:{} 失败: {}", server, port, e))?;

    if port == 465 {
        // 隐式 TLS
        let native_tls_connector = native_tls::TlsConnector::new()
            .map_err(|e| format!("初始化 TLS 连接器失败: {}", e))?;
        let tls_connector = tokio_native_tls::TlsConnector::from(native_tls_connector);
        let tls_stream = tls_connector
            .connect(server, tcp)
            .await
            .map_err(|e| format!("SMTP TLS 握手失败: {}", e))?;

        let mut conn = SmtpConn::new(tls_stream);
        let (code, lines) = conn.read_response().await?;
        if code != 220 {
            return Err(format!("SMTP 服务器欢迎消息异常 ({}): {}", code, lines.join(" ")));
        }
        let (ehlo_code, ehlo_lines) = conn.send_command("EHLO localhost").await?;
        if ehlo_code != 250 {
            return Err(format!("SMTP EHLO 失败 ({}): {}", ehlo_code, ehlo_lines.join(" ")));
        }
        finish_smtp_delivery(
            &mut conn,
            &ehlo_lines,
            username,
            password,
            from,
            &recipients,
            title,
            message,
        )
        .await
    } else {
        let mut conn = SmtpConn::new(tcp);
        let (code, lines) = conn.read_response().await?;
        if code != 220 {
            return Err(format!("SMTP 服务器欢迎消息异常 ({}): {}", code, lines.join(" ")));
        }
        let (ehlo_code, ehlo_lines) = conn.send_command("EHLO localhost").await?;
        if ehlo_code != 250 {
            return Err(format!("SMTP EHLO 失败 ({}): {}", ehlo_code, ehlo_lines.join(" ")));
        }

        if port == 587 || ehlo_supports_starttls(&ehlo_lines) {
            let (tls_code, tls_lines) = conn.send_command("STARTTLS").await?;
            if tls_code != 220 {
                return Err(format!(
                    "SMTP STARTTLS 被拒绝 ({}): {}",
                    tls_code,
                    tls_lines.join(" ")
                ));
            }
            if !conn.reader.buffer().is_empty() {
                return Err("STARTTLS 响应后存在多余明文数据，已中止连接".to_string());
            }
            let raw_tcp = conn.into_inner();
            let native_tls_connector = native_tls::TlsConnector::new()
                .map_err(|e| format!("初始化 STARTTLS 连接器失败: {}", e))?;
            let tls_connector = tokio_native_tls::TlsConnector::from(native_tls_connector);
            let tls_stream = tls_connector
                .connect(server, raw_tcp)
                .await
                .map_err(|e| format!("SMTP STARTTLS 握手失败: {}", e))?;

            let mut tls_conn = SmtpConn::new(tls_stream);
            let (ehlo2_code, ehlo2_lines) = tls_conn.send_command("EHLO localhost").await?;
            if ehlo2_code != 250 {
                return Err(format!(
                    "SMTP STARTTLS 后 EHLO 失败 ({}): {}",
                    ehlo2_code,
                    ehlo2_lines.join(" ")
                ));
            }
            finish_smtp_delivery(
                &mut tls_conn,
                &ehlo2_lines,
                username,
                password,
                from,
                &recipients,
                title,
                message,
            )
            .await
        } else {
            finish_smtp_delivery(
                &mut conn,
                &ehlo_lines,
                username,
                password,
                from,
                &recipients,
                title,
                message,
            )
            .await
        }
    }
}

impl PushConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_all(
        &self,
        title: &str,
        message: &str,
        jump_url: &Option<String>,
        task_manager: &mut dyn TaskManager,
    ) {
        if !self.enabled {
            return;
        }
        let push_request = TaskRequest::PushRequest(PushRequest {
            title: title.to_string(),
            message: message.to_string(),
            jump_url: jump_url.clone(),
            push_config: self.clone(),
            push_type: PushType::All,
        });
        match task_manager.submit_task(push_request) {
            Ok(task_id) => {
                log::debug!("提交全渠道推送任务成功，任务ID: {}", task_id);
            }
            Err(e) => {
                log::error!("提交推送任务失败: {}", e);
            }
        }
    }

    pub async fn push_all_async(
        &self,
        title: &str,
        message: &str,
        jump_url: &Option<String>,
    ) -> (bool, String) {
        if !self.enabled {
            return (false, "推送功能未开启".to_string());
        }

        let mut handles: Vec<tokio::task::JoinHandle<(&'static str, (bool, String))>> = Vec::new();

        if !self.bark_token.trim().is_empty() {
            let cfg = self.clone();
            let t = title.to_string();
            let m = message.to_string();
            let j = jump_url.clone();
            handles.push(tokio::spawn(async move {
                ("Bark", cfg.push_bark_with_url(&t, &m, &j).await)
            }));
        }

        if !self.pushplus_token.trim().is_empty() {
            let cfg = self.clone();
            let t = title.to_string();
            let m = message.to_string();
            handles.push(tokio::spawn(async move {
                ("PushPlus", cfg.push_pushplus(&t, &m).await)
            }));
        }

        if !self.fangtang_token.trim().is_empty() {
            let cfg = self.clone();
            let t = title.to_string();
            let m = message.to_string();
            handles.push(tokio::spawn(async move {
                ("Fangtang", cfg.push_fangtang(&t, &m).await)
            }));
        }

        if !self.dingtalk_token.trim().is_empty() {
            let cfg = self.clone();
            let t = title.to_string();
            let m = message.to_string();
            handles.push(tokio::spawn(async move {
                ("DingTalk", cfg.push_dingtalk(&t, &m).await)
            }));
        }

        if !self.wechat_token.trim().is_empty() {
            let cfg = self.clone();
            let t = title.to_string();
            let m = message.to_string();
            handles.push(tokio::spawn(async move {
                ("WeChat", cfg.push_wechat(&t, &m).await)
            }));
        }

        if !self.smtp_config.smtp_server.trim().is_empty()
            && !self.smtp_config.smtp_to.trim().is_empty()
        {
            let cfg = self.clone();
            let t = title.to_string();
            let m = message.to_string();
            handles.push(tokio::spawn(async move {
                ("SMTP", cfg.push_smtp(&t, &m).await)
            }));
        }

        if !self.gotify_config.gotify_url.trim().is_empty()
            && !self.gotify_config.gotify_token.trim().is_empty()
        {
            let cfg = self.clone();
            let t = title.to_string();
            let m = message.to_string();
            let j = jump_url.clone();
            handles.push(tokio::spawn(async move {
                ("Gotify", cfg.push_gotify(&t, &m, &j).await)
            }));
        }

        if handles.is_empty() {
            return (false, "未配置任何推送渠道".to_string());
        }

        let mut success_count = 0usize;
        let mut fail_count = 0usize;
        let mut failures = Vec::new();

        for handle in handles {
            match handle.await {
                Ok((channel, (true, _))) => {
                    log::debug!("{} 推送成功", channel);
                    success_count += 1;
                }
                Ok((channel, (false, msg))) => {
                    fail_count += 1;
                    failures.push(format!("{}: {}", channel, msg));
                }
                Err(join_err) => {
                    fail_count += 1;
                    failures.push(format!("推送任务异常: {}", join_err));
                }
            }
        }

        if fail_count == 0 {
            (true, format!("全部 {} 个渠道推送成功", success_count))
        } else if success_count > 0 {
            (
                true,
                format!(
                    "{} 个渠道成功，{} 个失败: {}",
                    success_count,
                    fail_count,
                    failures.join("; ")
                ),
            )
        } else {
            (
                false,
                format!("全部 {} 个渠道推送失败: {}", fail_count, failures.join("; ")),
            )
        }
    }

    pub async fn push_gotify(
        &self,
        title: &str,
        message: &str,
        jump_url: &Option<String>,
    ) -> (bool, String) {
        let base_url = match normalize_gotify_url(&self.gotify_config.gotify_url) {
            Ok(url) => url,
            Err(e) => return (false, e),
        };

        let token = self.gotify_config.gotify_token.trim();
        if token.is_empty() {
            return (false, "Gotify Token 为空".to_string());
        }

        let token_header = match HeaderValue::from_str(token) {
            Ok(val) => val,
            Err(_) => {
                return (
                    false,
                    "Gotify Token 包含无效字符，无法写入请求头".to_string(),
                )
            }
        };

        let jump_url_real = jump_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("bilibili://mall/web?url=https://www.bilibili.com");

        let data = serde_json::json!({
            "message": message,
            "title": title,
            "priority": 9,
            "extras": {
                "client::notification": {
                    "click": {"url": jump_url_real},
                },
                "android::action": {
                    "onReceive": {"intentUrl": jump_url_real}
                }
            }
        });

        let client = build_push_client();
        let url = format!("{}/message", base_url);

        match client
            .post(&url)
            .header("X-Gotify-Key", token_header)
            .json(&data)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                match resp.text().await {
                    Ok(text) => {
                        log::debug!("Gotify 推送响应: 状态码 {}, 内容: {}", status, text);
                        if status.is_success() {
                            (true, "推送成功".to_string())
                        } else {
                            let detail = serde_json::from_str::<serde_json::Value>(&text)
                                .ok()
                                .and_then(|j| {
                                    j["errorDescription"]
                                        .as_str()
                                        .or_else(|| j["error"].as_str())
                                        .map(str::to_string)
                                })
                                .unwrap_or_else(|| format!("状态码: {}", status));
                            (false, format!("Gotify 推送失败: {}", detail))
                        }
                    }
                    Err(e) => (false, format!("读取响应失败: {}", e)),
                }
            }
            Err(e) => (false, format!("推送失败: {}", e)),
        }
    }

    pub async fn push_bark(&self, title: &str, message: &str) -> (bool, String) {
        self.push_bark_with_url(title, message, &None).await
    }

    pub async fn push_bark_with_url(
        &self,
        title: &str,
        message: &str,
        jump_url: &Option<String>,
    ) -> (bool, String) {
        let (url, data) = match build_bark_request(&self.bark_token, title, message, jump_url) {
            Ok(req) => req,
            Err(e) => return (false, e),
        };

        let client = build_push_client();
        match client.post(&url).json(&data).send().await {
            Ok(resp) => {
                let status = resp.status();
                match resp.text().await {
                    Ok(text) => {
                        log::debug!("Bark 推送响应: 状态码 {}, 内容: {}", status, text);
                        parse_bark_response(status, &text)
                    }
                    Err(e) => (false, format!("读取响应失败: {}", e)),
                }
            }
            Err(e) => (false, format!("推送失败: {}", e)),
        }
    }

    pub async fn push_pushplus(&self, title: &str, message: &str) -> (bool, String) {
        let token = self.pushplus_token.trim();
        if token.is_empty() {
            return (false, "PushPlus Token 为空".to_string());
        }

        let client = build_push_client();
        let url = "https://www.pushplus.plus/send";
        let data = serde_json::json!({
            "token": token,
            "title": title,
            "content": message,
        });

        match client.post(url).json(&data).send().await {
            Ok(resp) => {
                let status = resp.status();
                match resp.text().await {
                    Ok(text) => {
                        log::debug!("PushPlus 推送响应: 状态码 {}, 内容: {}", status, text);
                        parse_pushplus_response(status, &text)
                    }
                    Err(e) => (false, format!("读取响应失败: {}", e)),
                }
            }
            Err(e) => (false, format!("推送失败: {}", e)),
        }
    }

    pub async fn push_fangtang(&self, title: &str, message: &str) -> (bool, String) {
        let token = self.fangtang_token.trim();
        if token.is_empty() {
            return (false, "方糖 SendKey 为空".to_string());
        }

        let client = build_push_client();
        let url = format!("https://sctapi.ftqq.com/{}.send", token);
        let data = serde_json::json!({
            "title": title,
            "desp": message,
            "noip": 1
        });

        match client.post(&url).json(&data).send().await {
            Ok(resp) => {
                let status = resp.status();
                match resp.text().await {
                    Ok(text) => {
                        log::debug!("Fangtang 推送响应: 状态码 {}, 内容: {}", status, text);
                        parse_fangtang_response(status, &text)
                    }
                    Err(e) => (false, format!("读取响应失败: {}", e)),
                }
            }
            Err(e) => (false, format!("推送失败: {}", e)),
        }
    }

    pub async fn push_dingtalk(&self, title: &str, message: &str) -> (bool, String) {
        let token = self.dingtalk_token.trim();
        if token.is_empty() {
            return (false, "钉钉 Token 为空".to_string());
        }

        let client = build_push_client();
        let url = format!(
            "https://oapi.dingtalk.com/robot/send?access_token={}",
            token
        );
        let data = serde_json::json!({
            "msgtype": "text",
            "text": {
                "content": format!("{} \n {}", title, message)
            }
        });

        match client.post(&url).json(&data).send().await {
            Ok(resp) => {
                let status = resp.status();
                match resp.text().await {
                    Ok(text) => {
                        log::debug!("钉钉推送响应: 状态码 {}, 内容: {}", status, text);
                        parse_dingtalk_response(status, &text)
                    }
                    Err(e) => (false, format!("读取响应失败: {}", e)),
                }
            }
            Err(e) => (false, format!("推送失败: {}", e)),
        }
    }

    pub async fn push_wechat(&self, title: &str, message: &str) -> (bool, String) {
        let token = self.wechat_token.trim();
        if token.is_empty() {
            return (false, "企业微信 Webhook Key 为空".to_string());
        }

        let client = build_push_client();
        let url = format!(
            "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key={}",
            token
        );
        let data = serde_json::json!({
            "msgtype": "text",
            "text": {
                "content": format!("{} \n {}", title, message)
            }
        });

        match client.post(&url).json(&data).send().await {
            Ok(resp) => {
                let status = resp.status();
                match resp.text().await {
                    Ok(text) => {
                        log::debug!("微信推送响应: 状态码 {}, 内容: {}", status, text);
                        parse_wechat_response(status, &text)
                    }
                    Err(e) => (false, format!("读取响应失败: {}", e)),
                }
            }
            Err(e) => (false, format!("推送失败: {}", e)),
        }
    }

    pub async fn push_smtp(&self, title: &str, message: &str) -> (bool, String) {
        match tokio::time::timeout(
            PUSH_REQUEST_TIMEOUT,
            send_smtp_inner(&self.smtp_config, title, message),
        )
        .await
        {
            Ok(Ok(())) => (true, "SMTP 邮件推送成功".to_string()),
            Ok(Err(e)) => (false, e),
            Err(_) => (false, "SMTP 推送超时（15 秒）".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn push_all_async_respects_disabled_and_zero_channels() {
        let mut cfg = PushConfig::new();
        cfg.bark_token = "some_token".to_string();
        let (ok, msg) = cfg.push_all_async("title", "msg", &None).await;
        assert!(!ok);
        assert!(msg.contains("未开启"));

        let mut cfg_enabled_empty = PushConfig::new();
        cfg_enabled_empty.enabled = true;
        let (ok2, msg2) = cfg_enabled_empty.push_all_async("title", "msg", &None).await;
        assert!(!ok2);
        assert!(msg2.contains("未配置"));
    }

    #[test]
    fn bark_request_strips_query_from_path_and_parses_numbers() {
        let (url, data) = build_bark_request(
            "myDeviceKey/?badge=5&isArchive=0&sound=bell",
            "抢票成功",
            "请尽快付款",
            &Some("https://show.bilibili.com/order".to_string()),
        )
        .expect("should build bark request");

        assert_eq!(url, "https://api.day.app/myDeviceKey");
        assert_eq!(data["badge"], serde_json::json!(5));
        assert_eq!(data["isArchive"], serde_json::json!(0));
        assert_eq!(data["sound"], serde_json::json!("bell"));
        assert_eq!(data["url"], serde_json::json!("https://show.bilibili.com/order"));

        let (custom_url, _) = build_bark_request(
            "https://bark.example.com/customKey/?group=vip",
            "T",
            "M",
            &None,
        )
        .expect("custom server url should work");
        assert_eq!(custom_url, "https://bark.example.com/customKey");
    }

    #[tokio::test]
    async fn gotify_url_normalization_and_invalid_header_never_panics() {
        assert_eq!(
            normalize_gotify_url("gotify.example.com/").unwrap(),
            "https://gotify.example.com"
        );
        assert_eq!(
            normalize_gotify_url("gotify.example.com:443").unwrap(),
            "https://gotify.example.com:443"
        );
        assert_eq!(
            normalize_gotify_url("http://127.0.0.1:8080///").unwrap(),
            "http://127.0.0.1:8080"
        );
        assert_eq!(
            normalize_gotify_url("192.168.1.100:8080").unwrap(),
            "http://192.168.1.100:8080"
        );
        assert_eq!(
            normalize_gotify_url("127.0.0.1:8080/").unwrap(),
            "http://127.0.0.1:8080"
        );
        assert_eq!(
            normalize_gotify_url("192.168.1.100").unwrap(),
            "http://192.168.1.100"
        );
        assert_eq!(
            normalize_gotify_url("[::1]:8080").unwrap(),
            "http://[::1]:8080"
        );
        assert_eq!(
            normalize_gotify_url("localhost").unwrap(),
            "http://localhost"
        );
        assert_eq!(
            normalize_gotify_url("gotify.local:8080").unwrap(),
            "http://gotify.local:8080"
        );
        assert!(normalize_gotify_url("   ").is_err());

        let mut cfg = PushConfig::new();
        cfg.gotify_config.gotify_url = "https://gotify.example.com".to_string();
        cfg.gotify_config.gotify_token = "bad\r\ntoken".to_string();
        let (ok, msg) = cfg.push_gotify("t", "m", &None).await;
        assert!(!ok);
        assert!(msg.contains("无效字符"));
    }

    #[test]
    fn smtp_subject_folding_and_rfc5322_headers() {
        let long_cn = "恭喜您抢票成功！上海·星铁LAND2026五一漫展特别VIP前排豪华特典纪念票订单已生成请尽快付款🎉🎉";
        let encoded = encode_rfc2047_utf8_subject(long_cn);
        assert!(encoded.contains("\r\n "), "long subject should fold across lines");

        let mut decoded_combined = String::new();
        for (idx, line) in encoded.split("\r\n").enumerate() {
            assert!(
                line.len() <= 75,
                "RFC 2047 folded line {} exceeds 75 chars ({}): {}",
                idx,
                line.len(),
                line
            );
            let word = if idx == 0 {
                line
            } else {
                line.strip_prefix(' ')
                    .expect("continuation line must start with single space")
            };
            let b64 = word
                .strip_prefix("=?UTF-8?B?")
                .and_then(|s| s.strip_suffix("?="))
                .expect("each folded word must be valid UTF-8 Base64 encoded-word");
            let bytes = BASE64.decode(b64).expect("valid base64");
            let piece = String::from_utf8(bytes).expect("each chunk must be valid UTF-8");
            decoded_combined.push_str(&piece);
        }
        assert_eq!(decoded_combined, long_cn);

        let payload = format_smtp_data_payload(
            "sender@example.com",
            &["to@example.com"],
            long_cn,
            "正文内容",
        );
        let date_line = payload
            .lines()
            .find(|l| l.starts_with("Date: "))
            .expect("RFC 5322 Date header missing");
        chrono::DateTime::parse_from_rfc2822(date_line.trim_start_matches("Date: "))
            .expect("Date header must be valid RFC 2822");

        let msg_id_line = payload
            .lines()
            .find(|l| l.starts_with("Message-ID: "))
            .expect("RFC 5322 Message-ID header missing");
        assert!(
            msg_id_line.starts_with("Message-ID: <") && msg_id_line.ends_with("@example.com>"),
            "unexpected Message-ID: {}",
            msg_id_line
        );

        // 无 @ 域名时回退 localhost
        let fallback_payload = format_smtp_data_payload("localuser", &["to@example.com"], "T", "M");
        let fallback_id = fallback_payload
            .lines()
            .find(|l| l.starts_with("Message-ID: "))
            .expect("Message-ID missing");
        assert!(fallback_id.ends_with("@localhost>"));
    }

    #[tokio::test]
    async fn smtp_read_response_handles_gbk_bytes_and_detects_pipelined_buffer() {
        // 1. Legacy GBK error response must not abort with UTF-8 InvalidData
        let (mut server, client) = tokio::io::duplex(256);
        let mut conn = SmtpConn::new(client);
        server
            .write_all(b"550 \xd3\xc3\xbb\xa7\xb2\xbb\xb4\xe6\xd4\xda\r\n")
            .await
            .unwrap();
        let (code, lines) = conn
            .read_response()
            .await
            .expect("GBK response should decode lossily without error");
        assert_eq!(code, 550);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("550 "));

        // 2. Pipelined plaintext after 220 STARTTLS remains visible in BufReader buffer
        let (mut server2, client2) = tokio::io::duplex(256);
        let mut conn2 = SmtpConn::new(client2);
        server2
            .write_all(b"220 Ready to start TLS\r\n500 injected plaintext\r\n")
            .await
            .unwrap();
        let (tls_code, _) = conn2.read_response().await.unwrap();
        assert_eq!(tls_code, 220);
        assert!(
            !conn2.reader.buffer().is_empty(),
            "buffered trailing plaintext after STARTTLS must be detected"
        );
    }

    #[test]
    fn provider_json_error_code_parsers_detect_api_errors() {
        assert!(parse_bark_response(StatusCode::OK, r#"{"code":200,"message":"success"}"#).0);
        assert!(!parse_bark_response(StatusCode::OK, r#"{"code":400,"message":"bad key"}"#).0);

        assert!(parse_pushplus_response(StatusCode::OK, r#"{"code":200,"msg":"请求成功"}"#).0);
        let (ok, msg) =
            parse_pushplus_response(StatusCode::OK, r#"{"code":900,"msg":"用户token不存在"}"#);
        assert!(!ok);
        assert!(msg.contains("用户token不存在"));

        assert!(parse_fangtang_response(StatusCode::OK, r#"{"code":0,"message":""}"#).0);
        let (ok, msg) =
            parse_fangtang_response(StatusCode::OK, r#"{"code":40001,"info":"invalid sendkey"}"#);
        assert!(!ok);
        assert!(msg.contains("invalid sendkey"));

        assert!(parse_dingtalk_response(StatusCode::OK, r#"{"errcode":0,"errmsg":"ok"}"#).0);
        let (ok, msg) =
            parse_dingtalk_response(StatusCode::OK, r#"{"errcode":310000,"errmsg":"keywords not in content"}"#);
        assert!(!ok);
        assert!(msg.contains("keywords"));

        assert!(parse_wechat_response(StatusCode::OK, r#"{"errcode":0,"errmsg":"ok"}"#).0);
        let (ok, msg) =
            parse_wechat_response(StatusCode::OK, r#"{"errcode":93000,"errmsg":"invalid webhook"}"#);
        assert!(!ok);
        assert!(msg.contains("invalid webhook"));
    }

    #[test]
    fn push_config_debug_redacts_tokens_and_smtp_password() {
        let mut cfg = PushConfig::new();
        cfg.bark_token = "bark_secret".to_string();
        cfg.gotify_config.gotify_token = "gotify_secret".to_string();
        cfg.smtp_config.smtp_password = "smtp_secret_password".to_string();

        let dbg = format!("{:?}", cfg);
        assert!(!dbg.contains("bark_secret"));
        assert!(!dbg.contains("gotify_secret"));
        assert!(!dbg.contains("smtp_secret_password"));
        assert!(dbg.contains("[REDACTED]"));
    }

    #[tokio::test]
    async fn smtp_rejects_header_injection() {
        let mut cfg = PushConfig::new();
        cfg.smtp_config = SmtpConfig {
            smtp_server: "127.0.0.1".to_string(),
            smtp_port: "2525".to_string(),
            smtp_username: "user@example.com".to_string(),
            smtp_password: "pw".to_string(),
            smtp_from: "user@example.com".to_string(),
            smtp_to: "to@example.com".to_string(),
        };

        let (ok, msg) = cfg.push_smtp("Injected\r\nBcc: evil@example.com", "body").await;
        assert!(!ok);
        assert!(msg.contains("换行符"));
    }

    #[tokio::test]
    async fn push_smtp_end_to_end_against_local_mock_server() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind local mock smtp port");
        let port = listener.local_addr().unwrap().port();

        let server_task = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.expect("accept client");
            let mut reader = BufReader::new(socket);
            let mut transcript = Vec::new();
            let mut data_buf = String::new();

            reader
                .get_mut()
                .write_all(b"220 mock.smtp.local ESMTP Ready\r\n")
                .await
                .unwrap();

            let mut line = String::new();
            // EHLO
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            transcript.push(line.trim().to_string());
            reader
                .get_mut()
                .write_all(b"250-mock.smtp.local\r\n250-AUTH LOGIN PLAIN\r\n250 OK\r\n")
                .await
                .unwrap();

            // AUTH LOGIN
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            transcript.push(line.trim().to_string());
            reader.get_mut().write_all(b"334 VXNlcm5hbWU6\r\n").await.unwrap();

            // Username b64
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            transcript.push(line.trim().to_string());
            reader.get_mut().write_all(b"334 UGFzc3dvcmQ6\r\n").await.unwrap();

            // Password b64
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            transcript.push(line.trim().to_string());
            reader
                .get_mut()
                .write_all(b"235 2.7.0 Authentication successful\r\n")
                .await
                .unwrap();

            // MAIL FROM
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            transcript.push(line.trim().to_string());
            reader.get_mut().write_all(b"250 OK\r\n").await.unwrap();

            // RCPT TO #1
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            transcript.push(line.trim().to_string());
            reader.get_mut().write_all(b"250 OK\r\n").await.unwrap();

            // RCPT TO #2
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            transcript.push(line.trim().to_string());
            reader.get_mut().write_all(b"250 OK\r\n").await.unwrap();

            // DATA
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            transcript.push(line.trim().to_string());
            reader
                .get_mut()
                .write_all(b"354 End data with <CR><LF>.<CR><LF>\r\n")
                .await
                .unwrap();

            loop {
                line.clear();
                let n = reader.read_line(&mut line).await.unwrap();
                assert!(n > 0);
                if line == ".\r\n" {
                    break;
                }
                data_buf.push_str(&line);
            }
            reader.get_mut().write_all(b"250 Queued\r\n").await.unwrap();

            // QUIT
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            transcript.push(line.trim().to_string());
            reader.get_mut().write_all(b"221 Bye\r\n").await.unwrap();

            (transcript, data_buf)
        });

        let mut cfg = PushConfig::new();
        cfg.enabled = true;
        cfg.smtp_config = SmtpConfig {
            smtp_server: "127.0.0.1".to_string(),
            smtp_port: port.to_string(),
            smtp_username: "sender@example.com".to_string(),
            smtp_password: "secret_password".to_string(),
            smtp_from: String::new(), // 应回退到 smtp_username
            smtp_to: "a@example.com, b@example.com".to_string(),
        };

        let (ok, msg) = cfg
            .push_smtp("抢票成功通知", "第一行\n.以点开头的行\r\n第三行")
            .await;
        assert!(ok, "SMTP push should succeed, got: {}", msg);

        let (transcript, data_buf) = server_task.await.expect("mock server task should finish");
        assert_eq!(transcript[0], "EHLO localhost");
        assert_eq!(transcript[1], "AUTH LOGIN");
        assert_eq!(transcript[2], BASE64.encode(b"sender@example.com"));
        assert_eq!(transcript[3], BASE64.encode(b"secret_password"));
        assert_eq!(transcript[4], "MAIL FROM:<sender@example.com>");
        assert_eq!(transcript[5], "RCPT TO:<a@example.com>");
        assert_eq!(transcript[6], "RCPT TO:<b@example.com>");
        assert_eq!(transcript[7], "DATA");
        assert_eq!(transcript[8], "QUIT");

        let expected_subj = format!("Subject: =?UTF-8?B?{}?=", BASE64.encode("抢票成功通知".as_bytes()));
        assert!(data_buf.contains(&expected_subj));
        assert!(data_buf.contains("..以点开头的行\r\n"), "dot-stuffing missing: {}", data_buf);
    }
}
