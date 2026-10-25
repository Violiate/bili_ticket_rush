use std::{fs, process};
use std::fs::File;
use std::io;
use std::io::Write;
use std::ops::{Index, IndexMut};
use std::sync::Arc;
use serde_json::{Value, json, Map};
use crate::account::Account;
use crate::cookie_manager::CookieManager;
use crate::push::PushConfig;
use crate::utility::CustomConfig;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use block_modes::{BlockMode, Cbc};
use block_modes::block_padding::Pkcs7;
use aes::Aes128;

use rand::Rng;
use std::path::Path;
use reqwest::Client;

#[derive(Clone,Debug)]
pub struct Config{
    data: Value,
}

impl Config {
    pub fn delete_json_config() -> io::Result<()> {
        fs::remove_file("config.json")
    }
}

impl Config{
    pub fn load_config() -> io::Result<Self>{
        let raw_context = fs::read_to_string("./config")?;
        let content = raw_context.split("%").collect::<Vec<&str>>();
        if content.len() < 2 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "config 格式错误：缺少 IV 或密文"));
        }
        // base64解码后解密
        let iv = BASE64.decode(content[0].trim())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let decoded = BASE64.decode(content[1].trim())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let decrypted = decrypt_data(iv, &decoded)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let plain_text = String::from_utf8(decrypted)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let data = serde_json::from_str(&plain_text)?;
        Ok(Self{data})

    }
    pub fn load_json_config() -> io::Result<Self>{
        let content = fs::read_to_string("./config.json")?;
        let data = serde_json::from_str(&content)?;
        Ok(Self{data})

    }

    pub fn new() -> Self{
        let data = json!({});
        Self{data}
    }

    pub fn save_config(&self) -> io::Result<()> {   //后续上加密
        let json_str = serde_json::to_string_pretty(&self.data)?;
        // 加密后base64编码
        let (iv,encrypted) = encrypt_data(json_str.as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let encoded_iv = BASE64.encode(&iv);  
        let encoded_encrypted = BASE64.encode(&encrypted);
        fs::write("./config", encoded_iv+"%" + &*encoded_encrypted)
    }


    //添加账号
    pub fn add_account(&mut self, account: &Account) -> io::Result<()>{
        if !self["accounts"].is_array(){  //不存在则创建
            self["accounts"]= json!([]);
        }

        let account_json = serde_json::to_value(account)?;

        if let Value::Array(ref mut accounts)= self["accounts"]{
            accounts.push(account_json);
        }

        Ok(())
    }

    //加载账号
    pub fn load_accounts(&self) -> Result<Vec<Account>,serde_json::Error>{
        if self["accounts"].is_array(){
            let accounts_json = &self["accounts"];
            serde_json::from_value(accounts_json.clone())
        }
        else{
            Ok(Vec::new())
        }
    }

    //账号更新（Account更新后调用这个保存,uid唯一寻找标识）
    pub fn update_account(&mut self, account: &Account) ->io::Result<bool>{
        if !self["accounts"].is_array(){
            return Ok(false);
        }

        let account_json = serde_json::to_value(account)?;
        if let Value::Array(ref mut accounts) = self["accounts"]{
            for (index, acc) in accounts.iter_mut().enumerate() {
                if let Some(uid) = acc["uid"].as_i64(){
                    if uid == account.uid{
                        accounts[index] = account_json;
                        return Ok(true);
                    }
            }   }
        }
        Ok(false)

    }

    //删除账号，传uid
        pub fn delete_account(&mut self, uid: i64) ->bool{
        if !self["accounts"].is_array(){
            return false;
        }

        let mut remove_flag = false;
        if let Value::Array(ref mut accounts  )= self["accounts"]{
            let old_len = accounts.len();
            accounts.retain(|acc|{
                if let Some(account_uid) = acc["uid"].as_i64(){
                    account_uid != uid
                }
                else{
                    true
                }
            });
            remove_flag = accounts.len() != old_len;
        }
        match save_config(self, None, None, None){
            Ok(_) => {
                log::info!("删除账号成功");
            },
            Err(e) => {
                log::error!("删除账号失败: {}", e);
            }
        }
        remove_flag
    }

    pub fn load_all_accounts() -> Vec<Account> {
        match Self::load_config() {
            Ok(config) => {
                match config.load_accounts() {
                    Ok(accounts) => accounts,
                    Err(e) => {
                        log::error!("加载账号失败: {}", e);
                        Vec::new()
                    }
                }
            },
            Err(e) => {
                log::error!("加载配置文件失败: {}", e);
                Vec::new()
            }
        }
    }

}

impl Index<&str> for Config{
    type Output = Value;

    fn index(&self, key: &str) -> &Self::Output{

        match self.data.get(key){
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
            // 如果当前不是对象，将其转换为对象
            let mut map = Map::new();
            map.insert(key.to_string(), Value::Null);
            self.data = Value::Object(map);

            if let Value::Object(ref mut map) = self.data {
                map.get_mut(key).unwrap()
            } else {
                unreachable!() // 理论上不可能到达这里
            }
        }
    }
}

pub fn save_config(config: &mut Config, push_config: Option<&PushConfig>, custon_config: Option<&CustomConfig>, account: Option<Account>) -> Result<bool, String> {
    if let Some(push_config) = push_config {
        config["push_config"] = serde_json::to_value(push_config).unwrap();
    }
    if let Some(custon_config) = custon_config {
        config["custom_config"] = serde_json::to_value(custon_config).unwrap();
    }
    if let Some(account) = account {
        config.add_account(&account).unwrap();
    }


    match config.save_config(){
        Ok(_) => {
            log::info!("配置文件保存成功");
            Ok(true)
        },
        Err(e) => {
            log::error!("配置文件保存失败: {}", e);
            Err(e.to_string())
        }
    }

}
pub fn load_texture_from_path(ctx: &eframe::egui::Context, path: &str, name: &str) -> Option<eframe::egui::TextureHandle> {
    use std::io::Read;


    match File::open(path) {

        Ok(mut file) => {
            let mut bytes = Vec::new();
            if file.read_to_end(&mut bytes).is_ok() {
                match image::load_from_memory(&bytes) {
                    Ok(image) => {
                        let size = [image.width() as usize, image.height() as usize];
                        let image_buffer = image.to_rgba8();
                        let pixels = image_buffer.as_flat_samples();

                        Some(ctx.load_texture(
                            name,
                            eframe::egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()),
                            Default::default()
                        ))
                    }
                    Err(_) => None,
                }
            } else {
                None
            }
        }
        Err(_) => None,
    }
}


fn write_bytes_to_file(file_path: &str, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(file_path)?; // 创建文件
    file.write_all(bytes)?; // 写入字节流
    file.flush()?; // 确保数据写入磁盘
    Ok(())
}

pub fn load_texture_from_url(ctx: &eframe::egui::Context, cookie_manager: Arc<CookieManager>, url: &String, name: &str) -> Option<eframe::egui::TextureHandle> {
    let rt = tokio::runtime::Runtime::new().unwrap();


    let bytes = rt.block_on(async {
        // 发送请求
        let resp = match cookie_manager.get(url).await.send().await {
            Ok(resp) => resp,
            Err(err) => {
                log::error!("HTTP请求失败: {}", err);
                return None;
            }
        };

        // 读取响应体
        match resp.bytes().await {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                log::error!("读取响应体失败: {}", err);
                None
            }
        }
    });


    let bytes = match bytes {
        Some(b) => b,
        None => return None,
    };

    // 处理图像数据
    match image::load_from_memory(&bytes) {
        Ok(image) => {
            let size = [image.width() as usize, image.height() as usize];
            let image_buffer = image.to_rgba8();
            let pixels = image_buffer.as_flat_samples();

            Some(ctx.load_texture(
                name,
                eframe::egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()),
                Default::default()
            ))
        }
        Err(err) => {
            log::warn!("加载图片至内存失败: {}，url:{}", err, url);
            None
        }
    }
}


fn gen_machine_id_bytes_128b()->Vec<u8> {
    let id: String = machine_uid::get().unwrap_or_else(|_| String::new());
    let mut key = id.as_bytes().to_vec();
    key.resize(16, 0u8);
    key

}
// 加密函数
fn encrypt_data(data: &[u8]) -> Result<(Vec<u8>,Vec<u8>), block_modes::BlockModeError> {
    type Aes128Cbc = Cbc<Aes128, Pkcs7>;
    let mut iv = [0u8; 16];
    rand::thread_rng()
        .fill(&mut iv[..]); // 填充 16 字节的随机数据
    let cipher = Aes128Cbc::new_from_slices(&gen_machine_id_bytes_128b(), &iv)
        .map_err(|_| block_modes::BlockModeError)?; // 将 InvalidKeyIvLength 转换为 BlockModeError

    Ok((iv.to_vec(), cipher.encrypt_vec(data)))
}

fn decrypt_data(iv:Vec<u8>,encrypted: &[u8]) -> Result<Vec<u8>, block_modes::BlockModeError> {
    type Aes128Cbc = Cbc<Aes128, Pkcs7>;
    let cipher = Aes128Cbc::new_from_slices(&gen_machine_id_bytes_128b(), &iv)
        .map_err(|_| block_modes::BlockModeError)?; // 将 InvalidKeyIvLength 转换为 BlockModeError

    cipher.decrypt_vec(encrypted)
}


fn buyer_cache_path(uid: i64) -> String {
    format!("./buyer_cache_{}.dat", uid)
}

/// 保存某账号的购票人列表到本地（设备码加密）。uid 用于区分不同账号。
pub fn save_buyer_cache(uid: i64, buyers: &[crate::ticket::BuyerInfo]) -> io::Result<()> {
    let json_str = serde_json::to_string(buyers)?;
    let (iv, encrypted) = encrypt_data(json_str.as_bytes())
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let content = format!("{}%{}", BASE64.encode(&iv), BASE64.encode(&encrypted));
    fs::write(buyer_cache_path(uid), content)?;
    log::debug!("已缓存账号 {} 的购票人列表（{} 人）", uid, buyers.len());
    Ok(())
}

/// 从本地读取某账号缓存的购票人列表（设备码解密）。
pub fn load_buyer_cache(uid: i64) -> io::Result<Vec<crate::ticket::BuyerInfo>> {
    let raw = fs::read_to_string(buyer_cache_path(uid))?;
    let parts = raw.split('%').collect::<Vec<&str>>();
    if parts.len() != 2 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "购票人缓存格式错误"));
    }
    let iv = BASE64.decode(parts[0].trim())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let encrypted = BASE64.decode(parts[1].trim())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let decrypted = decrypt_data(iv, &encrypted)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let json_str = String::from_utf8(decrypted)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let buyers = serde_json::from_str(&json_str)?;
    Ok(buyers)
}

// 单例锁实现，防止程序多开
use single_instance::SingleInstance;

// 简化后的单例检查实现
pub fn ensure_single_instance() -> bool {
    // 使用应用程序唯一标识
    let app_id = "bili_ticket_rush_6BA7B79C-0E4F-4FCC-B7A2-4DA5E8D7E0F6"; // GUID 保证唯一性
    let instance = SingleInstance::new(app_id).unwrap();
    
    if !instance.is_single() {
        log::error!("程序已经在运行中，请勿重复启动！");
        eprintln!("程序已经在运行中，请勿重复启动！");
        std::thread::sleep(std::time::Duration::from_secs(2));
        false
    } else {
        // 保持实例在程序生命周期内
        Box::leak(Box::new(instance));
        true
    }
}

// 为不支持的平台提供默认实现
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn is_process_running(_pid: u32) -> bool {
    false // 不支持的平台，假设进程不存在
}

#[cfg(target_os = "windows")]
fn is_process_running(pid: u32) -> bool {
    use std::process::Command;
    
    // 使用 tasklist 命令检查进程
    let output = Command::new("tasklist")
        .args(&["/NH", "/FI", &format!("PID eq {}", pid)])
        .output();
        
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            !stdout.contains("信息: 没有运行的任务匹配指定标准") && 
            !stdout.contains("No tasks") && 
            stdout.contains(&format!("{}", pid))
        },
        Err(_) => false, // 执行命令失败，假设进程不存在
    }
}

#[cfg(target_os = "linux")]
fn is_process_running(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

#[cfg(target_os = "macos")]
fn is_process_running(pid: u32) -> bool {
    use std::process::Command;
    
    // 使用 ps 命令检查进程
    let output = Command::new("ps")
        .args(&["-p", &format!("{}", pid)])
        .output();
        
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.contains(&format!("{}", pid))
        },
        Err(_) => false, // 执行命令失败，假设进程不存在
    }
}

pub async fn get_now_time(client: &Client) -> i64 {
    // 获取网络时间 (秒级)
    let url = "https://api.bilibili.com/x/click-interface/click/now";
    
    let now_sec = match client.get(url).send().await {
        Ok(response) => {
            match response.text().await {
                Ok(text) => {
                    log::debug!("API原始响应：{}", text);
                    
                    let json_data: serde_json::Value = serde_json::from_str(&text).unwrap_or(
                        json!({
                            "code": 0,
                            "data": {
                                "now": 0
                            }
                        })
                    );
                    
                    let now_sec = json_data["data"]["now"].as_i64().unwrap_or(0);
                    log::debug!("解析出的网络时间(秒级)：{}", now_sec);
                    now_sec
                },
                Err(e) => {
                    log::debug!("解析网络时间响应失败：{}", e);
                    0
                }
            }
        },
        Err(e) => {
            log::debug!("获取网络时间失败，原因：{}", e);
            0
        }
    };
    
    // 如果网络时间获取失败，使用本地时间 (转换为秒)
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

    #[test]
    fn machine_key_is_16_bytes() {
        let key = gen_machine_id_bytes_128b();
        assert_eq!(key.len(), 16);
    }
}