use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

pub const MAX_DEFAULT_BUYERS_PER_ACCOUNT: usize = 2;
pub const MAX_RECENT_PROJECTS: usize = 6;

fn default_grab_interval_ms() -> u64 {
    2000
}

fn default_max_retry_count() -> u32 {
    60
}

fn default_order_retry_delay_ms() -> u64 {
    400
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecentProjectEntry {
    pub project_id: String,
    pub name: String,
}

impl Default for RecentProjectEntry {
    fn default() -> Self {
        Self {
            project_id: String::new(),
            name: String::new(),
        }
    }
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
    pub preinput_name1: String, // 预填账号1联系人姓名（非实名制/添加购票人）
    #[serde(default)]
    pub preinput_phone1: String, // 预填账号1手机号
    #[serde(default)]
    pub preinput_name2: String, // 预填账号2联系人姓名（非实名制/添加购票人）
    #[serde(default)]
    pub preinput_phone2: String, // 预填账号2手机号
    #[serde(default = "default_true")]
    pub enable_default_buyers: bool, // 是否在选人页自动勾选账号默认购票人
    #[serde(default)]
    pub default_buyers_by_uid: BTreeMap<String, Vec<i64>>, // 每个账号最多2位默认购票人ID（按主次顺序）
    #[serde(default)]
    pub default_grab_mode: u8, // 上次选择的抢票模式 0:自动 1:直接 2:捡漏
    #[serde(default)]
    pub saved_skip_words: Vec<String>, // 持久化的捡漏跳过关键词
    #[serde(default)]
    pub recent_projects: Vec<RecentProjectEntry>, // 最近抢票项目快捷历史
    #[serde(default = "default_true")]
    pub alert_on_success: bool, // 抢票成功时发出系统提示音提醒
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
            preinput_name1: String::new(),
            preinput_phone1: String::new(),
            preinput_name2: String::new(),
            preinput_phone2: String::new(),
            enable_default_buyers: true,
            default_buyers_by_uid: BTreeMap::new(),
            default_grab_mode: 0,
            saved_skip_words: Vec::new(),
            recent_projects: Vec::new(),
            alert_on_success: true,
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

    /// 获取指定账号已配置的默认购票人 ID 列表（去重且最多返回 2 个，保持主次顺序）
    pub fn default_buyers_for_uid(&self, uid: i64) -> Vec<i64> {
        let Some(raw) = self.default_buyers_by_uid.get(&uid.to_string()) else {
            return Vec::new();
        };
        sanitize_default_buyer_ids(raw)
    }

    /// 设置指定账号的默认购票人 ID 列表（自动去重并截断至最多 2 个；为空则移除键）
    pub fn set_default_buyers_for_uid(&mut self, uid: i64, ids: Vec<i64>) {
        let cleaned = sanitize_default_buyer_ids(&ids);
        let key = uid.to_string();
        if cleaned.is_empty() {
            self.default_buyers_by_uid.remove(&key);
        } else {
            self.default_buyers_by_uid.insert(key, cleaned);
        }
    }

    /// 切换指定账号下某个购票人的默认状态：
    /// - 已在默认列表中 -> 移除并返回 `Ok(false)`；
    /// - 未在列表中且不足 2 人 -> 追加并返回 `Ok(true)`；
    /// - 已满 2 人 -> 返回 `Err` 提示先取消一位。
    pub fn toggle_default_buyer_for_uid(
        &mut self,
        uid: i64,
        buyer_id: i64,
    ) -> Result<bool, &'static str> {
        if buyer_id <= 0 {
            return Err("无效的购票人 ID");
        }
        let mut current = self.default_buyers_for_uid(uid);
        if let Some(pos) = current.iter().position(|&id| id == buyer_id) {
            current.remove(pos);
            self.set_default_buyers_for_uid(uid, current);
            Ok(false)
        } else if current.len() >= MAX_DEFAULT_BUYERS_PER_ACCOUNT {
            Err("每个账号最多设置 2 位默认购票人，请先点击已选购票人取消一位后再更换")
        } else {
            current.push(buyer_id);
            self.set_default_buyers_for_uid(uid, current);
            Ok(true)
        }
    }

    /// 互换指定账号两位默认购票人的主次顺序（单人票优先选第 1 位）
    pub fn swap_default_buyers_for_uid(&mut self, uid: i64) -> bool {
        let mut current = self.default_buyers_for_uid(uid);
        if current.len() == 2 {
            current.swap(0, 1);
            self.set_default_buyers_for_uid(uid, current);
            true
        } else {
            false
        }
    }

    /// 记录最近访问/抢票的项目（按最新使用置顶去重，最多保留 MAX_RECENT_PROJECTS 条）
    pub fn record_recent_project(&mut self, project_id: &str, name: &str) {
        let pid = project_id.trim();
        if pid.is_empty() {
            return;
        }
        let clean_name = name.trim();
        let prev_name = self
            .recent_projects
            .iter()
            .find(|item| item.project_id == pid)
            .map(|item| item.name.clone())
            .unwrap_or_default();
        let resolved_name = if !clean_name.is_empty() && clean_name != "未知项目" {
            clean_name.to_string()
        } else {
            prev_name
        };

        self.recent_projects.retain(|item| item.project_id != pid);
        self.recent_projects.insert(
            0,
            RecentProjectEntry {
                project_id: pid.to_string(),
                name: resolved_name,
            },
        );
        if self.recent_projects.len() > MAX_RECENT_PROJECTS {
            self.recent_projects.truncate(MAX_RECENT_PROJECTS);
        }
    }

    pub fn remove_recent_project(&mut self, project_id: &str) {
        let pid = project_id.trim();
        self.recent_projects.retain(|item| item.project_id != pid);
    }
}

fn sanitize_default_buyer_ids(ids: &[i64]) -> Vec<i64> {
    let mut out = Vec::with_capacity(MAX_DEFAULT_BUYERS_PER_ACCOUNT);
    for &id in ids {
        if id > 0 && !out.contains(&id) {
            out.push(id);
            if out.len() >= MAX_DEFAULT_BUYERS_PER_ACCOUNT {
                break;
            }
        }
    }
    out
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
        let default_buyers_summary = if self.default_buyers_by_uid.is_empty() {
            "0 accounts".to_string()
        } else {
            format!("[REDACTED: {} accounts]", self.default_buyers_by_uid.len())
        };
        f.debug_struct("CustomConfig")
            .field("open_custom_ua", &self.open_custom_ua)
            .field("custom_ua", &self.custom_ua)
            .field("captcha_mode", &self.captcha_mode)
            .field("ttocr_key", &redact_field(&self.ttocr_key))
            .field("preinput_name1", &redact_field(&self.preinput_name1))
            .field("preinput_phone1", &redact_field(&self.preinput_phone1))
            .field("preinput_name2", &redact_field(&self.preinput_name2))
            .field("preinput_phone2", &redact_field(&self.preinput_phone2))
            .field("enable_default_buyers", &self.enable_default_buyers)
            .field("default_buyers_by_uid", &default_buyers_summary)
            .field("default_grab_mode", &self.default_grab_mode)
            .field("saved_skip_words_len", &self.saved_skip_words.len())
            .field("recent_projects_len", &self.recent_projects.len())
            .field("alert_on_success", &self.alert_on_success)
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
        assert!(cfg.enable_default_buyers);
        assert!(cfg.alert_on_success);
        assert!(cfg.default_buyers_by_uid.is_empty());
        assert_eq!(cfg.default_grab_mode, 0);
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
        assert!(cfg.enable_default_buyers);
        assert!(cfg.alert_on_success);
        assert_eq!(cfg.grab_interval_ms, 2000);
        assert_eq!(cfg.max_retry_count, 60);
        assert_eq!(cfg.order_retry_delay_ms, 400);
    }

    #[test]
    fn debug_redacts_ttocr_key_names_phones_and_default_buyers() {
        let mut cfg = CustomConfig::new();
        cfg.ttocr_key = "super_secret_ttocr_token".to_string();
        cfg.preinput_name1 = "张三丰".to_string();
        cfg.preinput_phone1 = "13800138000".to_string();
        cfg.preinput_name2 = "李四海".to_string();
        cfg.preinput_phone2 = "13900139000".to_string();
        cfg.set_default_buyers_for_uid(401924460, vec![998877, 665544]);

        let rendered = format!("{:?}", cfg);
        assert!(!rendered.contains("super_secret_ttocr_token"));
        assert!(!rendered.contains("张三丰"));
        assert!(!rendered.contains("李四海"));
        assert!(!rendered.contains("13800138000"));
        assert!(!rendered.contains("13900139000"));
        assert!(!rendered.contains("401924460"));
        assert!(!rendered.contains("998877"));
        assert!(rendered.contains("[REDACTED]"));
    }

    #[test]
    fn default_buyers_per_uid_caps_at_two_preserves_order_and_swaps() {
        let mut cfg = CustomConfig::new();
        assert_eq!(cfg.toggle_default_buyer_for_uid(1001, 11), Ok(true));
        assert_eq!(cfg.toggle_default_buyer_for_uid(1001, 22), Ok(true));
        assert!(cfg.toggle_default_buyer_for_uid(1001, 33).is_err());
        assert_eq!(cfg.default_buyers_for_uid(1001), vec![11, 22]);

        assert!(cfg.swap_default_buyers_for_uid(1001));
        assert_eq!(cfg.default_buyers_for_uid(1001), vec![22, 11]);

        // 取消第1位后第2位自动晋升为主选
        assert_eq!(cfg.toggle_default_buyer_for_uid(1001, 22), Ok(false));
        assert_eq!(cfg.default_buyers_for_uid(1001), vec![11]);

        // 批量写入自动去重且最多保留2位
        cfg.set_default_buyers_for_uid(1002, vec![5, 5, -1, 8, 9]);
        assert_eq!(cfg.default_buyers_for_uid(1002), vec![5, 8]);
    }

    #[test]
    fn recent_projects_deduplicates_mru_preserves_known_name_and_caps_len() {
        let mut cfg = CustomConfig::new();
        cfg.record_recent_project("10001", "上海漫展");
        cfg.record_recent_project("10002", "北京漫展");
        // Mode 2 可能只传入空项目名，不得覆盖已有项目名称，且应置顶
        cfg.record_recent_project("10001", "");
        assert_eq!(cfg.recent_projects.len(), 2);
        assert_eq!(cfg.recent_projects[0].project_id, "10001");
        assert_eq!(cfg.recent_projects[0].name, "上海漫展");

        for i in 3..=10 {
            cfg.record_recent_project(&format!("100{:02}", i), &format!("展{}", i));
        }
        assert_eq!(cfg.recent_projects.len(), MAX_RECENT_PROJECTS);
        assert_eq!(cfg.recent_projects[0].project_id, "10010");

        cfg.remove_recent_project("10010");
        assert_eq!(cfg.recent_projects.len(), MAX_RECENT_PROJECTS - 1);
    }
}
