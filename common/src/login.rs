use std::time::Duration;

use reqwest::Client;
use serde_json::json;

use crate::captcha::{captcha, LocalCaptcha};
use crate::http_utils::{request_get, request_post};
use crate::utility::CustomConfig;

const LOGIN_TIMEOUT: Duration = Duration::from_secs(15);

pub struct LoginInput {
    pub phone: String,
    pub account: String,
    pub password: String,
    pub sms_code: String,
    pub cookie: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum QrCodeLoginStatus {
    Pending,
    Scanning,
    Confirming,
    Success(String), // 成功时返回 cookie 信息
    Failed(String),  // 失败时返回错误信息
    Expired,
}

pub(crate) fn validate_phone(phone: &str) -> Result<String, String> {
    let trimmed = phone.trim();
    if trimmed.is_empty()
        || !trimmed.bytes().all(|b| b.is_ascii_digit())
        || trimmed.len() < 7
        || trimmed.len() > 15
    {
        return Err("手机号格式无效，请输入正确的数字手机号".to_string());
    }
    Ok(trimmed.to_string())
}

pub(crate) fn validate_sms_code(sms_code: &str) -> Result<String, String> {
    let trimmed = sms_code.trim();
    if trimmed.is_empty()
        || !trimmed.bytes().all(|b| b.is_ascii_digit())
        || trimmed.len() < 4
        || trimmed.len() > 8
    {
        return Err("短信验证码格式无效，请输入 4-8 位数字验证码".to_string());
    }
    Ok(trimmed.to_string())
}

pub async fn qrcode_login_async(client: &Client) -> Result<(String, String), String> {
    let response = client
        .get("https://passport.bilibili.com/x/passport-login/web/qrcode/generate")
        .timeout(LOGIN_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("获取登录二维码失败: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("获取登录二维码失败，HTTP 状态码: {}", status));
    }

    let json = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("解析二维码响应失败: {}", e))?;

    if json["code"].as_i64() != Some(0) {
        let msg = json["message"].as_str().unwrap_or("未知错误");
        return Err(format!("获取登录二维码失败: {}", msg));
    }

    let qrcode_key = json["data"]["qrcode_key"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "无法获取二维码 qrcode_key".to_string())?;

    let url = json["data"]["url"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "https://passport.bilibili.com/h5-app/passport/login/scan?navhide=1&qrcode_key={}&from=",
                qrcode_key
            )
        });

    Ok((qrcode_key.to_string(), url))
}

pub fn qrcode_login(client: &Client) -> Result<String, String> {
    let result = match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(|| handle.block_on(qrcode_login_async(client)))
        }
        Ok(_) => std::thread::scope(|s| {
            s.spawn(|| {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("创建异步运行时失败: {}", e))?;
                rt.block_on(qrcode_login_async(client))
            })
            .join()
            .unwrap_or_else(|_| Err("获取二维码执行线程异常退出".to_string()))
        }),
        Err(_) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("创建异步运行时失败: {}", e))?;
            rt.block_on(qrcode_login_async(client))
        }
    };

    result.map(|(qrcode_key, _url)| qrcode_key)
}

pub fn password_login(_username: &str, _password: &str) -> Result<String, String> {
    Err("暂不支持账号密码登录".to_string())
}

pub async fn send_loginsms(
    phone: &str,
    client: &Client,
    custom_config: CustomConfig,
    local_captcha: LocalCaptcha,
) -> Result<String, String> {
    let clean_phone = validate_phone(phone)?;

    // 预热主站以获取基础会话 Cookie（不记录原始 Cookie）
    if let Err(e) = request_get(client, "https://www.bilibili.com/", None).await {
        log::warn!("预热主站请求失败（将继续尝试发送短信）: {}", e);
    }

    // 获取极验验证码参数
    let response = request_get(
        client,
        "https://passport.bilibili.com/x/passport-login/captcha",
        None,
    )
    .await
    .map_err(|e| format!("获取人机验证参数失败: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("获取人机验证参数失败，HTTP 状态码: {}", status));
    }

    let json = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("解析人机验证参数失败: {}", e))?;

    if json["code"].as_i64() != Some(0) {
        let msg = json["message"].as_str().unwrap_or("未知错误");
        return Err(format!("获取人机验证参数失败: {}", msg));
    }

    let gt = json["data"]["geetest"]["gt"].as_str().unwrap_or("").trim();
    let challenge = json["data"]["geetest"]["challenge"]
        .as_str()
        .unwrap_or("")
        .trim();
    let token = json["data"]["token"].as_str().unwrap_or("").trim();

    if gt.is_empty() || challenge.is_empty() || token.is_empty() {
        return Err("人机验证参数不完整（缺少 gt/challenge/token）".to_string());
    }

    let referer = "https://passport.bilibili.com/x/passport-login/captcha";
    let result_str = captcha(custom_config, gt, challenge, referer, 33, local_captcha)
        .await
        .map_err(|e| {
            log::error!("验证码识别失败: {}", e);
            format!("验证码识别失败: {}", e)
        })?;

    log::info!("人机验证识别成功");
    let result: serde_json::Value = serde_json::from_str(&result_str)
        .map_err(|e| format!("解析人机验证结果失败: {}", e))?;

    let json_data = json!({
        "cid": 86,
        "tel": clean_phone,
        "token": token,
        "source": "main_mini",
        "challenge": result["challenge"],
        "validate": result["validate"],
        "seccode": result["seccode"],
    });

    let send_sms = request_post(
        client,
        "https://passport.bilibili.com/x/passport-login/web/sms/send",
        None,
        Some(&json_data),
    )
    .await
    .map_err(|e| format!("发送短信验证码请求失败: {}", e))?;

    let send_status = send_sms.status();
    if !send_status.is_success() {
        return Err(format!("发送短信验证码失败，HTTP 状态码: {}", send_status));
    }

    let json_response = send_sms
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("解析短信发送响应失败: {}", e))?;

    if json_response["code"].as_i64() == Some(0) {
        let captcha_key = json_response["data"]["captcha_key"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "短信接口未返回有效的 captcha_key".to_string())?;
        log::info!("短信验证码发送成功");
        Ok(captcha_key.to_string())
    } else {
        let msg = json_response["message"].as_str().unwrap_or("未知错误");
        log::error!("短信验证码发送失败: {}", msg);
        Err(format!("验证码发送失败: {}", msg))
    }
}

pub async fn sms_login(
    phone: &str,
    sms_code: &str,
    captcha_key: &str,
    client: &Client,
) -> Result<String, String> {
    let clean_phone = validate_phone(phone)?;
    let clean_code = validate_sms_code(sms_code)?;
    let clean_captcha_key = captcha_key.trim();

    if clean_captcha_key.is_empty() {
        return Err("captcha_key 为空，请先获取短信验证码".to_string());
    }

    // 必须以字符串发送 tel 与 code，防止 "012345" 类前导零验证码被截断
    let data = json!({
        "cid": 86,
        "tel": clean_phone,
        "code": clean_code,
        "source": "main_mini",
        "captcha_key": clean_captcha_key,
    });

    let login_response = request_post(
        client,
        "https://passport.bilibili.com/x/passport-login/web/login/sms",
        None,
        Some(&data),
    )
    .await
    .map_err(|e| format!("短信登录请求失败: {}", e))?;

    let status = login_response.status();
    if !status.is_success() {
        return Err(format!("短信登录失败，HTTP 状态码: {}", status));
    }

    let mut all_cookies = Vec::new();
    for value in login_response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
    {
        if let Ok(cookie_str) = value.to_str() {
            let pair = cookie_str.split(';').next().unwrap_or("").trim();
            if pair.contains('=') {
                all_cookies.push(pair.to_string());
            }
        }
    }

    let json_response = login_response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("解析短信登录响应失败: {}", e))?;

    if json_response["code"].as_i64() == Some(0) {
        if all_cookies.is_empty() {
            return Err("短信登录成功但未返回有效 Cookie".to_string());
        }
        log::info!("短信登录成功！");
        return Ok(all_cookies.join("; "));
    }

    let msg = json_response["message"].as_str().unwrap_or("未知错误");
    log::error!("短信登录失败: {}", msg);
    Err(format!("短信登录失败: {}", msg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_sms_code_preserves_leading_zeros() {
        assert_eq!(validate_sms_code("012345").unwrap(), "012345");
        assert_eq!(validate_sms_code("  000123  ").unwrap(), "000123");
        assert!(validate_sms_code("").is_err());
        assert!(validate_sms_code("12ab56").is_err());
        assert!(validate_sms_code("123").is_err());
    }

    #[test]
    fn validate_phone_rejects_invalid_inputs() {
        assert_eq!(validate_phone("13800138000").unwrap(), "13800138000");
        assert_eq!(validate_phone(" 13800138000 ").unwrap(), "13800138000");
        assert!(validate_phone("").is_err());
        assert!(validate_phone("abc").is_err());
        assert!(validate_phone("138-0013-8000").is_err());
        assert!(validate_phone("12345").is_err());
    }
}
