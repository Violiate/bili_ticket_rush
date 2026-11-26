use std::fmt;

use serde::{Deserialize, Serialize};

fn default_grab_interval_ms() -> u64 {
    2000
}

fn default_max_retry_count() -> u32 {
    60
}

fn default_order_retry_delay_ms() -> u64 {
    400
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomConfig {
    #[serde(default, alias = "enable_custom_ua")]
    pub open_custom_ua: bool, // 是否开启自定义UA
    #[serde(default)]
    pub custom_ua: String, // 自定义UA
    #[serde(default)]
    pub captcha_mode: usize, // 验证码模式 0:本地打码 1:ttocr
    #[serde(default)]
    pub ttocr_key: String, // ttocr key
    #[serde(default)]
    pub preinput_phone1: String, // 预填账号1手机号
    #[serde(default)]
    pub preinput_phone2: String, // 预填账号2手机号
    #[serde(default = "default_grab_interval_ms")]
    pub grab_interval_ms: u64, // 抢票/捡漏轮询间隔(ms)
    #[serde(default = "default_max_retry_count")]
    pub max_retry_count: u32, // 最大重试次数
    #[serde(default = "default_order_retry_delay_ms")]
    pub order_retry_delay_ms: u64, // 下单重试延迟(ms)
}

impl Default for CustomConfig {
    fn default() -> Self {
        Self {
            open_custom_ua: false,
            custom_ua: String::new(),
            captcha_mode: 0,
            ttocr_key: String::new(),
            preinput_phone1: String::new(),
            preinput_phone2: String::new(),
            grab_interval_ms: default_grab_interval_ms(),
            max_retry_count: default_max_retry_count(),
            order_retry_delay_ms: default_order_retry_delay_ms(),
        }
    }
}

impl CustomConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

fn redact_field(value: &str) -> &'static str {
    if value.is_empty() {
        ""
    } else {
        "[REDACTED]"
    }
}

impl fmt::Debug for CustomConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CustomConfig")
            .field("open_custom_ua", &self.open_custom_ua)
            .field("custom_ua", &self.custom_ua)
            .field("captcha_mode", &self.captcha_mode)
            .field("ttocr_key", &redact_field(&self.ttocr_key))
            .field("preinput_phone1", &redact_field(&self.preinput_phone1))
            .field("preinput_phone2", &redact_field(&self.preinput_phone2))
            .field("grab_interval_ms", &self.grab_interval_ms)
            .field("max_retry_count", &self.max_retry_count)
            .field("order_retry_delay_ms", &self.order_retry_delay_ms)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values_are_safe() {
        let cfg = CustomConfig::new();
        assert!(!cfg.open_custom_ua);
        assert!(cfg.custom_ua.is_empty());
        assert_eq!(cfg.captcha_mode, 0);
        assert_eq!(cfg.grab_interval_ms, 2000);
        assert_eq!(cfg.max_retry_count, 60);
        assert_eq!(cfg.order_retry_delay_ms, 400);
    }

    #[test]
    fn deserializes_legacy_partial_json_and_alias() {
        let legacy = r#"{"enable_custom_ua": true, "custom_ua": "MyUA/1.0", "captcha_mode": 1}"#;
        let cfg: CustomConfig = serde_json::from_str(legacy).expect("legacy json should parse");
        assert!(cfg.open_custom_ua);
        assert_eq!(cfg.custom_ua, "MyUA/1.0");
        assert_eq!(cfg.captcha_mode, 1);
        assert_eq!(cfg.grab_interval_ms, 2000);
        assert_eq!(cfg.max_retry_count, 60);
        assert_eq!(cfg.order_retry_delay_ms, 400);
    }

    #[test]
    fn debug_redacts_ttocr_key_and_phones() {
        let mut cfg = CustomConfig::new();
        cfg.ttocr_key = "super_secret_ttocr_token".to_string();
        cfg.preinput_phone1 = "13800138000".to_string();
        cfg.preinput_phone2 = "13900139000".to_string();

        let rendered = format!("{:?}", cfg);
        assert!(!rendered.contains("super_secret_ttocr_token"));
        assert!(!rendered.contains("13800138000"));
        assert!(!rendered.contains("13900139000"));
        assert!(rendered.contains("[REDACTED]"));
    }
}
