use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::ops::{Index, IndexMut};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use aes::Aes128;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use block_modes::block_padding::Pkcs7;
use block_modes::{BlockMode, Cbc};
use once_cell::sync::Lazy;
use rand::Rng;
use reqwest::Client;
use serde_json::{json, Map, Value};
use single_instance::SingleInstance;

use crate::account::Account;
use crate::cookie_manager::CookieManager;
use crate::push::PushConfig;
use crate::utility::CustomConfig;

const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
/// 反黄牛限制：最多允许同时保存/登录 2 个账号
pub const MAX_ACCOUNTS: usize = 2; // 10MB

/// 共享纯 HTTP 图片客户端：不携带任何 B 站 Cookie，带连接与总超时
static PLAIN_IMAGE_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36")
        .build()
        .unwrap_or_else(|_| Client::new())
});

fn backup_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("data");
    path.with_file_name(format!("{}.bak", file_name))
}

fn read_string_with_backup_fallback(path: &Path) -> io::Result<String> {
    if !path.exists() {
        let bak_path = backup_path_for(path);
        if bak_path.exists() {
            log::warn!(
                "主文件 {:?} 缺失，已自动从备份文件 {:?} 恢复读取",
                path,
                bak_path
            );
            return fs::read_to_string(&bak_path);
        }
    }
    fs::read_to_string(path)
}

fn atomic_write_string(path: &Path, content: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("data");
    let tmp_path = path.with_file_name(format!("{}.{}.tmp", file_name, uuid::Uuid::new_v4()));
    let bak_path = backup_path_for(path);

    {
        let mut tmp_file = File::create(&tmp_path)?;
        tmp_file.write_all(content.as_bytes())?;
        tmp_file.sync_all()?;
    }

    match fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(rename_err) => {
            if path.exists() {
                let _ = fs::remove_file(&bak_path);
                if let Err(bak_err) = fs::rename(path, &bak_path) {
                    let _ = fs::remove_file(&tmp_path);
                    return Err(io::Error::new(
                        rename_err.kind(),
                        format!(
                            "原子写入备份原文件失败: {} (原错误: {})",
                            bak_err, rename_err
                        ),
                    ));
                }
                match fs::rename(&tmp_path, path) {
                    Ok(()) => {
                        let _ = fs::remove_file(&bak_path);
                        Ok(())
                    }
                    Err(second_err) => {
                        let _ = fs::rename(&bak_path, path);
                        let _ = fs::remove_file(&tmp_path);
                        Err(second_err)
                    }
                }
            } else {
                let _ = fs::remove_file(&tmp_path);
                Err(rename_err)
            }
        }
    }
}

#[derive(Clone)]
pub struct Config {
    data: Value,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let keys: Vec<&str> = self
            .data
            .as_object()
            .map(|m| m.keys().map(String::as_str).collect())
            .unwrap_or_default();
        f.debug_struct("Config").field("keys", &keys).finish()
    }
}

impl Config {
    pub fn delete_json_config() -> io::Result<()> {
        fs::remove_file("config.json")
    }

    pub fn load_config() -> io::Result<Self> {
        Self::load_config_from_path(Path::new("./config"))
    }

    pub(crate) fn load_config_from_path(path: &Path) -> io::Result<Self> {
        let raw_context = read_string_with_backup_fallback(path)?;
        let content = raw_context.split('%').collect::<Vec<&str>>();
        if content.len() < 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "config 格式错误：缺少 IV 或密文",
            ));
        }
        // base64 解码后解密
        let iv = BASE64
            .decode(content[0].trim())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let decoded = BASE64
            .decode(content[1].trim())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let decrypted = decrypt_data(iv, &decoded)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let plain_text = String::from_utf8(decrypted)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let data = serde_json::from_str(&plain_text)?;
        Ok(Self { data })
    }

    pub fn load_json_config() -> io::Result<Self> {
        let content = fs::read_to_string("./config.json")?;
        let data = serde_json::from_str(&content)?;
        Ok(Self { data })
    }

    pub fn new() -> Self {
        let data = json!({});
        Self { data }
    }

    pub fn save_config(&self) -> io::Result<()> {
        self.save_config_to_path(Path::new("./config"))
    }

    pub(crate) fn save_config_to_path(&self, path: &Path) -> io::Result<()> {
        let json_str = serde_json::to_string_pretty(&self.data)?;
        let (iv, encrypted) = encrypt_data(json_str.as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let encoded_iv = BASE64.encode(&iv);
        let encoded_encrypted = BASE64.encode(&encrypted);
        let payload = format!("{}%{}", encoded_iv, encoded_encrypted);
        atomic_write_string(path, &payload)
    }

    /// 按 uid 原地更新或追加账号（自动去重，严格限制最多 MAX_ACCOUNTS 个不同账号）
    pub fn upsert_account(&mut self, account: Account) -> Result<(), String> {
        if !self["accounts"].is_array() {
            self["accounts"] = json!([]);
        }
        let account_json = serde_json::to_value(&account)
            .map_err(|e| format!("序列化账号失败: {}", e))?;

        if let Value::Array(ref mut accounts) = self["accounts"] {
            let mut updated = false;
            accounts.retain_mut(|acc| {
                if acc["uid"].as_i64() == Some(account.uid) {
                    if !updated {
                        *acc = account_json.clone();
                        updated = true;
                        true
                    } else {
                        false
                    }
                } else {
                    true
                }
            });
            if !updated {
                if accounts.len() >= MAX_ACCOUNTS {
                    return Err(format!(
                        "为防止滥用，最多仅支持同时登录 {} 个账号；如需添加新账号，请先登出已有账号",
                        MAX_ACCOUNTS
                    ));
                }
                accounts.push(account_json);
            }
        }
        Ok(())
    }

    // 添加账号（按 uid upsert，避免重复追加，并限制最多 MAX_ACCOUNTS 个账号）
    pub fn add_account(&mut self, account: &Account) -> io::Result<()> {
        self.upsert_account(account.clone())
            .map_err(|msg| io::Error::new(io::ErrorKind::PermissionDenied, msg))
    }

    // 直接从内存中的 self.data["accounts"] 读取账号列表（不重读磁盘；自动去重并限制上限）
    pub fn load_accounts(&self) -> Vec<Account> {
        let Some(Value::Array(arr)) = self.data.get("accounts") else {
            return Vec::new();
        };

        let mut accounts: Vec<Account> = Vec::with_capacity(arr.len().min(MAX_ACCOUNTS));
        for (idx, item) in arr.iter().enumerate() {
            match serde_json::from_value::<Account>(item.clone()) {
                Ok(acc) => {
                    if accounts.iter().any(|existing| existing.uid == acc.uid) {
                        continue;
                    }
                    if accounts.len() >= MAX_ACCOUNTS {
                        log::warn!(
                            "配置中账号数量超过上限 {}，已忽略多余账号 UID {}",
                            MAX_ACCOUNTS,
                            acc.uid
                        );
                        break;
                    }
                    accounts.push(acc);
                }
                Err(e) => {
                    log::warn!("反序列化第 {} 个账号配置失败，已跳过: {}", idx, e);
                }
            }
        }
        accounts
    }

    // 账号更新（Account 更新后调用这个保存，uid 唯一寻找标识）
    pub fn update_account(&mut self, account: &Account) -> io::Result<bool> {
        if !self["accounts"].is_array() {
            return Ok(false);
        }

        let account_json = serde_json::to_value(account)?;
        if let Value::Array(ref mut accounts) = self["accounts"] {
            for acc in accounts.iter_mut() {
                if acc["uid"].as_i64() == Some(account.uid) {
                    *acc = account_json;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    // 删除账号，传 uid；仅当确实存在且磁盘保存成功时返回 true
    pub fn delete_account(&mut self, uid: i64) -> bool {
        self.delete_account_with_path(uid, Path::new("./config"))
    }

    pub(crate) fn delete_account_with_path(&mut self, uid: i64, path: &Path) -> bool {
        if !self["accounts"].is_array() {
            return false;
        }

        let previous = self["accounts"].clone();
        let mut removed = false;
        if let Value::Array(ref mut accounts) = self["accounts"] {
            let old_len = accounts.len();
            accounts.retain(|acc| acc["uid"].as_i64() != Some(uid));
            removed = accounts.len() != old_len;
        }

        if !removed {
            return false;
        }

        match self.save_config_to_path(path) {
            Ok(()) => {
                log::info!("删除账号成功: UID {}", uid);
                true
            }
            Err(e) => {
                log::error!("删除账号后保存配置失败: {}", e);
                self["accounts"] = previous;
                false
            }
        }
    }

    pub fn load_all_accounts() -> Vec<Account> {
        Self::load_config()
            .map(|c| c.load_accounts())
            .unwrap_or_default()
    }
}

impl Index<&str> for Config {
    type Output = Value;

    fn index(&self, key: &str) -> &Self::Output {
        match self.data.get(key) {
            Some(value) => value,
            None => &Value::Null,
        }
    }
}

// 实现索引修改
impl IndexMut<&str> for Config {
    fn index_mut(&mut self, key: &str) -> &mut Self::Output {
        if let Value::Object(ref mut map) = self.data {
            map.entry(key.to_string()).or_insert(Value::Null)
        } else {
            let mut map = Map::new();
            map.insert(key.to_string(), Value::Null);
            self.data = Value::Object(map);

            if let Value::Object(ref mut map) = self.data {
                map.get_mut(key).unwrap()
            } else {
                unreachable!()
            }
        }
    }
}

pub fn save_config(
    config: &mut Config,
    push_config: Option<&PushConfig>,
    custom_config: Option<&CustomConfig>,
    account: Option<Account>,
) -> Result<bool, String> {
    if let Some(push_config) = push_config {
        config["push_config"] = serde_json::to_value(push_config)
            .map_err(|e| format!("序列化推送配置失败: {}", e))?;
    }
    if let Some(custom_config) = custom_config {
        config["custom_config"] = serde_json::to_value(custom_config)
            .map_err(|e| format!("序列化自定义配置失败: {}", e))?;
    }
    if let Some(account) = account {
        config
            .add_account(&account)
            .map_err(|e| format!("保存账号配置失败: {}", e))?;
    }

    match config.save_config() {
        Ok(()) => {
            log::info!("配置文件保存成功");
            Ok(true)
        }
        Err(e) => {
            log::error!("配置文件保存失败: {}", e);
            Err(e.to_string())
        }
    }
}

pub fn load_texture_from_path(
    ctx: &eframe::egui::Context,
    path: &str,
    name: &str,
) -> Option<eframe::egui::TextureHandle> {
    use std::io::Read;

    match File::open(path) {
        Ok(mut file) => {
            let mut bytes = Vec::new();
            if file.read_to_end(&mut bytes).is_ok() {
                decode_texture_from_bytes(ctx, &bytes, name)
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

fn decode_texture_from_bytes(
    ctx: &eframe::egui::Context,
    bytes: &[u8],
    name: &str,
) -> Option<eframe::egui::TextureHandle> {
    match image::load_from_memory(bytes) {
        Ok(image) => {
            let size = [image.width() as usize, image.height() as usize];
            let image_buffer = image.to_rgba8();
            let pixels = image_buffer.as_flat_samples();

            Some(ctx.load_texture(
                name,
                eframe::egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()),
                Default::default(),
            ))
        }
        Err(err) => {
            log::warn!("加载图片至内存失败: {}", err);
            None
        }
    }
}

pub(crate) fn normalize_image_url(url: &str) -> Result<String, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("图片 URL 为空".to_string());
    }

    let candidate = if trimmed.starts_with("//") {
        format!("https:{}", trimmed)
    } else {
        trimmed.to_string()
    };

    let mut parsed =
        reqwest::Url::parse(&candidate).map_err(|e| format!("无效的图片 URL: {}", e))?;

    match parsed.scheme() {
        "https" => Ok(parsed.to_string()),
        "http" => {
            if let Some(host) = parsed.host_str() {
                let host_lower = host.to_ascii_lowercase();
                if host_lower == "hdslb.com" || host_lower.ends_with(".hdslb.com") {
                    let _ = parsed.set_scheme("https");
                }
            }
            Ok(parsed.to_string())
        }
        scheme => Err(format!("不支持的图片 URL 协议: {}", scheme)),
    }
}

/// 使用无 Cookie 的共享客户端异步拉取图片字节（15 秒超时，最大 10MB）
pub async fn fetch_image_bytes(url: &str) -> Result<Vec<u8>, String> {
    let normalized = normalize_image_url(url)?;

    let mut resp = PLAIN_IMAGE_CLIENT
        .get(&normalized)
        .send()
        .await
        .map_err(|e| format!("图片 HTTP 请求失败: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("图片请求失败，HTTP 状态码: {}", status));
    }

    if let Some(content_len) = resp.content_length() {
        if content_len > MAX_IMAGE_BYTES as u64 {
            return Err(format!(
                "图片大小 ({} 字节) 超过 10MB 限制",
                content_len
            ));
        }
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("读取图片响应体失败: {}", e))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_IMAGE_BYTES {
            return Err("图片大小超过 10MB 限制".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }

    Ok(bytes)
}

pub fn load_texture_from_url(
    ctx: &eframe::egui::Context,
    _cookie_manager: Arc<CookieManager>,
    url: &String,
    name: &str,
) -> Option<eframe::egui::TextureHandle> {
    let bytes_result = match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(|| handle.block_on(fetch_image_bytes(url)))
        }
        Ok(_) => std::thread::scope(|s| {
            s.spawn(|| {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("创建图片加载运行时失败: {}", e))?;
                rt.block_on(fetch_image_bytes(url))
            })
            .join()
            .unwrap_or_else(|_| Err("图片加载线程异常退出".to_string()))
        }),
        Err(_) => match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt.block_on(fetch_image_bytes(url)),
            Err(e) => Err(format!("创建图片加载运行时失败: {}", e)),
        },
    };

    match bytes_result {
        Ok(bytes) => decode_texture_from_bytes(ctx, &bytes, name),
        Err(err) => {
            log::warn!("加载图片失败 ({}): {}", url, err);
            None
        }
    }
}

fn gen_machine_id_bytes_128b() -> Vec<u8> {
    let id: String = machine_uid::get().unwrap_or_else(|_| String::new());
    let mut key = id.as_bytes().to_vec();
    key.resize(16, 0u8);
    key
}

// 加密函数（保持 AES-128-CBC + PKCS7 格式以完全兼容现有用户配置文件）
fn encrypt_data(data: &[u8]) -> Result<(Vec<u8>, Vec<u8>), block_modes::BlockModeError> {
    type Aes128Cbc = Cbc<Aes128, Pkcs7>;
    let mut iv = [0u8; 16];
    rand::thread_rng().fill(&mut iv[..]);
    let cipher = Aes128Cbc::new_from_slices(&gen_machine_id_bytes_128b(), &iv)
        .map_err(|_| block_modes::BlockModeError)?;

    Ok((iv.to_vec(), cipher.encrypt_vec(data)))
}

fn decrypt_data(iv: Vec<u8>, encrypted: &[u8]) -> Result<Vec<u8>, block_modes::BlockModeError> {
    type Aes128Cbc = Cbc<Aes128, Pkcs7>;
    let cipher = Aes128Cbc::new_from_slices(&gen_machine_id_bytes_128b(), &iv)
        .map_err(|_| block_modes::BlockModeError)?;

    cipher.decrypt_vec(encrypted)
}

fn buyer_cache_path(uid: i64) -> String {
    format!("./buyer_cache_{}.dat", uid)
}

pub(crate) fn save_buyer_cache_to_path(
    path: &Path,
    buyers: &[crate::ticket::BuyerInfo],
) -> io::Result<()> {
    let json_str = serde_json::to_string(buyers)?;
    let (iv, encrypted) = encrypt_data(json_str.as_bytes())
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let content = format!("{}%{}", BASE64.encode(&iv), BASE64.encode(&encrypted));
    atomic_write_string(path, &content)
}

/// 保存某账号的购票人列表到本地（设备码加密，原子写入）。uid 用于区分不同账号。
pub fn save_buyer_cache(uid: i64, buyers: &[crate::ticket::BuyerInfo]) -> io::Result<()> {
    let path_str = buyer_cache_path(uid);
    save_buyer_cache_to_path(Path::new(&path_str), buyers)?;
    log::debug!("已缓存账号 {} 的购票人列表（{} 人）", uid, buyers.len());
    Ok(())
}

pub(crate) fn load_buyer_cache_from_path(path: &Path) -> io::Result<Vec<crate::ticket::BuyerInfo>> {
    let raw = read_string_with_backup_fallback(path)?;
    let parts = raw.split('%').collect::<Vec<&str>>();
    if parts.len() != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "购票人缓存格式错误",
        ));
    }
    let iv = BASE64
        .decode(parts[0].trim())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let encrypted = BASE64
        .decode(parts[1].trim())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let decrypted = decrypt_data(iv, &encrypted)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let json_str = String::from_utf8(decrypted)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let buyers = serde_json::from_str(&json_str)?;
    Ok(buyers)
}

/// 从本地读取某账号缓存的购票人列表（设备码解密）。
pub fn load_buyer_cache(uid: i64) -> io::Result<Vec<crate::ticket::BuyerInfo>> {
    let path_str = buyer_cache_path(uid);
    load_buyer_cache_from_path(Path::new(&path_str))
}

// 单例检查实现（失败时优雅降级不 panic，无额外 sleep）
pub fn ensure_single_instance() -> bool {
    const APP_LOCK_NAME: &str = "bili_ticket_rush_6BA7B79C-0E4F-4FCC-B7A2-4DA5E8D7E0F6";
    // macOS 上 single-instance 使用文件路径加 flock 锁；使用系统临时目录避免在只读 .app 工作目录失败或残留锁文件
    #[cfg(target_os = "macos")]
    let app_id_owned = std::env::temp_dir()
        .join(format!("{}.lock", APP_LOCK_NAME))
        .to_string_lossy()
        .into_owned();
    #[cfg(target_os = "macos")]
    let app_id = app_id_owned.as_str();
    #[cfg(not(target_os = "macos"))]
    let app_id = APP_LOCK_NAME;

    let instance = match SingleInstance::new(app_id) {
        Ok(inst) => inst,
        Err(e) => {
            log::warn!("创建单实例锁失败: {}，跳过多开检测", e);
            return true;
        }
    };

    if !instance.is_single() {
        log::error!("程序已经在运行中，请勿重复启动！");
        let _ = writeln!(std::io::stderr(), "程序已经在运行中，请勿重复启动！");
        false
    } else {
        // 保持实例在程序生命周期内
        Box::leak(Box::new(instance));
        true
    }
}

pub async fn get_now_time(client: &Client) -> i64 {
    let url = "https://api.bilibili.com/x/click-interface/click/now";

    let now_sec = match client.get(url).timeout(Duration::from_secs(5)).send().await {
        Ok(response) => match response.text().await {
            Ok(text) => {
                log::debug!("API原始响应：{}", text);

                let json_data: Value = serde_json::from_str(&text).unwrap_or_else(|_| {
                    json!({
                        "code": 0,
                        "data": {
                            "now": 0
                        }
                    })
                });

                let now_sec = json_data["data"]["now"].as_i64().unwrap_or(0);
                log::debug!("解析出的网络时间(秒级)：{}", now_sec);
                now_sec
            }
            Err(e) => {
                log::debug!("解析网络时间响应失败：{}", e);
                0
            }
        },
        Err(e) => {
            log::debug!("获取网络时间失败，原因：{}", e);
            0
        }
    };

    if now_sec == 0 {
        log::debug!("使用本地时间");
        let local_sec = chrono::Utc::now().timestamp();
        log::debug!("本地时间(秒级)：{}", local_sec);
        local_sec
    } else {
        now_sec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_account(uid: i64, name: &str) -> Account {
        Account {
            uid,
            name: name.to_string(),
            level: "6".to_string(),
            cookie: "SESSDATA=abc; bili_jct=def".to_string(),
            csrf: "def".to_string(),
            is_login: true,
            account_status: "空闲".to_string(),
            vip_label: "年度大会员".to_string(),
            is_active: true,
            avatar_url: None,
            avatar_texture: None,
            cookie_manager: None,
        }
    }

    #[test]
    fn machine_key_is_16_bytes() {
        let key = gen_machine_id_bytes_128b();
        assert_eq!(key.len(), 16);
    }

    #[test]
    fn config_atomic_save_and_reload_roundtrip() {
        let mut temp_path = std::env::temp_dir();
        temp_path.push(format!(
            "biliticket_config_test_{}_{}.dat",
            std::process::id(),
            rand::random::<u64>()
        ));

        let mut cfg = Config::new();
        cfg.upsert_account(sample_account(1001, "Alice"))
            .expect("upsert first account");
        cfg["custom_config"] = serde_json::to_value(CustomConfig::new()).unwrap();
        cfg["push_config"] = serde_json::to_value(PushConfig::new()).unwrap();

        // 首次写入 + 覆盖写入两次以验证原子替换路径
        cfg.save_config_to_path(&temp_path).expect("first atomic save");
        cfg.upsert_account(sample_account(1002, "Bob")).expect("upsert second account");
        cfg.save_config_to_path(&temp_path).expect("second atomic save");

        let loaded = Config::load_config_from_path(&temp_path).expect("reload config");
        let accounts = loaded.load_accounts();
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].uid, 1001);
        assert_eq!(accounts[1].uid, 1002);

        let _ = fs::remove_file(&temp_path);
    }

    #[test]
    fn upsert_account_deduplicates_by_uid_and_enforces_max_two_accounts() {
        let mut cfg = Config::new();
        cfg.upsert_account(sample_account(42, "OldName")).expect("first account");
        cfg.upsert_account(sample_account(99, "Other")).expect("second account");
        // 已满 2 个账号时，更新已有 UID 仍然允许
        cfg.upsert_account(sample_account(42, "NewName")).expect("update existing uid");
        // 新增第 3 个不同 UID 必须被反黄牛上限拦截
        assert!(cfg.upsert_account(sample_account(100, "Third")).is_err());

        let accounts = cfg.load_accounts();
        assert_eq!(accounts.len(), MAX_ACCOUNTS);
        assert_eq!(accounts[0].uid, 42);
        assert_eq!(accounts[0].name, "NewName");
        assert_eq!(accounts[1].uid, 99);
    }

    #[test]
    fn delete_account_returns_false_for_missing_uid_and_true_on_saved_delete() {
        let mut temp_path = std::env::temp_dir();
        temp_path.push(format!(
            "biliticket_delete_test_{}_{}.dat",
            std::process::id(),
            rand::random::<u64>()
        ));

        let mut cfg = Config::new();
        cfg.upsert_account(sample_account(7, "User7")).expect("add user7");

        // 不存在的 UID 直接返回 false 且不创建文件
        assert!(!cfg.delete_account_with_path(999, &temp_path));
        assert!(!temp_path.exists());

        // 存在的 UID 删除成功并写入文件
        assert!(cfg.delete_account_with_path(7, &temp_path));
        assert!(cfg.load_accounts().is_empty());
        assert!(temp_path.exists());

        let _ = fs::remove_file(&temp_path);
    }

    #[test]
    fn legacy_partial_custom_and_push_configs_deserialize_cleanly() {
        let legacy_custom: CustomConfig =
            serde_json::from_str(r#"{"enable_custom_ua": true}"#).expect("legacy custom config");
        assert!(legacy_custom.open_custom_ua);
        assert_eq!(legacy_custom.grab_interval_ms, 2000);
        assert_eq!(legacy_custom.max_retry_count, 60);
        assert_eq!(legacy_custom.order_retry_delay_ms, 400);

        let legacy_push: PushConfig =
            serde_json::from_str(r#"{"enabled": true, "bark_token": "abc"}"#)
                .expect("legacy push config");
        assert!(legacy_push.enabled);
        assert_eq!(legacy_push.bark_token, "abc");
        assert!(legacy_push.gotify_config.gotify_url.is_empty());
        assert!(legacy_push.smtp_config.smtp_server.is_empty());
    }

    #[test]
    fn normalize_image_url_upgrades_hdslb_http() {
        assert_eq!(
            normalize_image_url("http://i0.hdslb.com/bfs/face/a.jpg").unwrap(),
            "https://i0.hdslb.com/bfs/face/a.jpg"
        );
        assert_eq!(
            normalize_image_url("//i1.hdslb.com/bfs/face/b.jpg").unwrap(),
            "https://i1.hdslb.com/bfs/face/b.jpg"
        );
        assert!(normalize_image_url("").is_err());
        assert!(normalize_image_url("ftp://example.com/a.jpg").is_err());
    }

    #[test]
    fn config_recovers_from_bak_when_primary_missing() {
        let unique = uuid::Uuid::new_v4();
        let temp_path = std::env::temp_dir().join(format!(
            "biliticket_bak_recover_{}_{}.dat",
            std::process::id(),
            unique
        ));
        let bak_path = backup_path_for(&temp_path);

        let mut cfg = Config::new();
        cfg.upsert_account(sample_account(2026, "RecoveredUser")).expect("upsert");
        cfg.save_config_to_path(&temp_path)
            .expect("save initial config");
        fs::rename(&temp_path, &bak_path).expect("move primary config to .bak");

        assert!(!temp_path.exists());
        assert!(bak_path.exists());

        let recovered =
            Config::load_config_from_path(&temp_path).expect("recover config from .bak");
        let accounts = recovered.load_accounts();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].uid, 2026);
        assert_eq!(accounts[0].name, "RecoveredUser");

        let _ = fs::remove_file(&temp_path);
        let _ = fs::remove_file(&bak_path);

        // 同样验证购票人缓存在主文件缺失时可从 .bak 恢复
        let buyer_path = std::env::temp_dir().join(format!(
            "biliticket_buyer_bak_recover_{}_{}.dat",
            std::process::id(),
            unique
        ));
        let buyer_bak = backup_path_for(&buyer_path);
        let buyers = vec![crate::ticket::BuyerInfo {
            id: 101,
            uid: 2026,
            name: "测试购票人".to_string(),
            personal_id: "110101199001011234".to_string(),
            tel: "13800138000".to_string(),
            ..Default::default()
        }];
        save_buyer_cache_to_path(&buyer_path, &buyers).expect("save buyer cache");
        fs::rename(&buyer_path, &buyer_bak).expect("move buyer cache to .bak");
        assert!(!buyer_path.exists());

        let recovered_buyers =
            load_buyer_cache_from_path(&buyer_path).expect("recover buyer cache from .bak");
        assert_eq!(recovered_buyers.len(), 1);
        assert_eq!(recovered_buyers[0].id, 101);
        assert_eq!(recovered_buyers[0].name, "测试购票人");

        let _ = fs::remove_file(&buyer_path);
        let _ = fs::remove_file(&buyer_bak);
    }
}
