use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::account::Account;
use crate::cookie_manager::CookieManager;
use crate::push::PushConfig;
use crate::utility::CustomConfig;

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + Deserialize<'de>,
    D: Deserializer<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

fn deserialize_non_negative_usize<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    let val = Option::<Value>::deserialize(deserializer)?;
    match val {
        Some(Value::Number(n)) => {
            if let Some(u) = n.as_u64() {
                Ok(u as usize)
            } else if let Some(i) = n.as_i64() {
                Ok(if i < 0 { 0 } else { i as usize })
            } else {
                Ok(0)
            }
        }
        Some(Value::String(s)) => Ok(s
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|&i| i >= 0)
            .unwrap_or(0) as usize),
        _ => Ok(0),
    }
}

fn deserialize_opt_non_negative_usize<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: Deserializer<'de>,
{
    let val = Option::<Value>::deserialize(deserializer)?;
    match val {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => {
            if let Some(u) = n.as_u64() {
                Ok(Some(u as usize))
            } else if let Some(i) = n.as_i64() {
                Ok(if i < 0 { None } else { Some(i as usize) })
            } else {
                Ok(None)
            }
        }
        Some(Value::String(s)) => match s.trim().parse::<i64>() {
            Ok(i) if i >= 0 => Ok(Some(i as usize)),
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

fn redact_pii(value: &str) -> &'static str {
    if value.is_empty() {
        ""
    } else {
        "[REDACTED]"
    }
}

// 成功下单结构体
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SubmitOrderResult {
    #[serde(rename = "orderId")]
    pub order_id: i128,
    #[serde(rename = "orderCreateTime")]
    pub order_create_time: i64,
    #[serde(rename = "token")]
    pub order_token: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct CheckFakeResult {
    pub errno: i32,
    pub code: i32,
    pub errtag: i32,
    pub msg: String,
    pub message: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub data: CheckFakeResultParam,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct CheckFakeResultParam {
    #[serde(rename = "payParam", default, deserialize_with = "deserialize_null_default")]
    pub pay_param: CheckFakeResultData,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct CheckFakeResultData {
    pub sign: String,
    pub code_url: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ConfirmTicketInfo {
    pub name: String,
    pub count: i32,
    pub price: i64,
}

// 确认订单结构体
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ConfirmTicketResult {
    pub count: i32,
    pub pay_money: i64,
    pub project_name: String,
    pub screen_name: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub ticket_info: ConfirmTicketInfo,
}

// 获取 token 响应结构体
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct TokenRiskParam {
    pub code: i32,
    pub message: String,
    pub mid: Option<String>,
    pub decision_type: Option<String>,
    pub buvid: Option<String>,
    pub ip: Option<String>,
    pub scene: Option<String>,
    pub ua: Option<String>,
    pub v_voucher: Option<String>,
    pub risk_param: Option<Value>,
}

#[derive(Clone)]
pub struct BilibiliTicket {
    pub uid: i64, // UID
    pub method: u8,
    pub ua: String,
    pub config: CustomConfig,
    pub account: Account,
    pub push_self: PushConfig,
    pub status_delay: usize,
    pub captcha_use_type: usize, // 选择的验证码方式
    pub cookie_manager: Option<Arc<CookieManager>>,

    // 抢票相关
    pub project_id: String,
    pub referer: String,
    pub screen_id: String,
    pub id_bind: usize, // 是否绑定

    pub project_info: Option<TicketInfo>,     // 项目详情
    pub all_buyer_info: Option<BuyerInfoData>, // 所有购票人信息
    pub buyer_info: Option<Vec<BuyerInfo>>,   // 购买人信息（实名票）

    pub no_bind_buyer_info: Option<NoBindBuyerInfo>, // 不实名制购票人信息

    pub select_ticket_id: Option<String>,

    pub pay_money: Option<i64>, // 支付金额
    pub count: Option<i32>,     // 购买数量
    pub device_id: String,      // 设备id
}

impl fmt::Debug for BilibiliTicket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BilibiliTicket")
            .field("uid", &self.uid)
            .field("method", &self.method)
            .field("ua", &self.ua)
            .field("config", &self.config)
            .field("account", &self.account)
            .field("push_self", &self.push_self)
            .field("status_delay", &self.status_delay)
            .field("captcha_use_type", &self.captcha_use_type)
            .field("cookie_manager", &self.cookie_manager)
            .field("project_id", &self.project_id)
            .field("referer", &self.referer)
            .field("screen_id", &self.screen_id)
            .field("id_bind", &self.id_bind)
            .field("project_info", &self.project_info.as_ref().and_then(|p| p.id))
            .field(
                "buyer_count",
                &self.buyer_info.as_ref().map(|v| v.len()).unwrap_or(0),
            )
            .field("no_bind_buyer_info", &self.no_bind_buyer_info)
            .field("select_ticket_id", &self.select_ticket_id)
            .field("pay_money", &self.pay_money)
            .field("count", &self.count)
            .field("device_id", &redact_pii(&self.device_id))
            .finish()
    }
}

impl BilibiliTicket {
    pub fn new(
        method: &u8,
        ua: &String,
        config: &CustomConfig,
        account: &Account,
        push_self: &PushConfig,
        status_delay: &usize,
        project_id: &str,
        referer: &str,
    ) -> Self {
        let finally_ua = if config.open_custom_ua && !config.custom_ua.trim().is_empty() {
            let custom = config.custom_ua.trim().to_string();
            log::info!("使用自定义UA：{}", custom);
            custom
        } else {
            log::info!("使用默认UA：{}", ua);
            ua.clone()
        };

        let captcha_type = config.captcha_mode;

        let new = Self {
            uid: account.uid,
            method: *method,
            ua: finally_ua,
            config: config.clone(),
            account: account.clone(),
            push_self: push_self.clone(),
            status_delay: *status_delay,
            captcha_use_type: captcha_type,
            cookie_manager: None,
            project_id: project_id.to_string(),
            referer: referer.to_string(),
            screen_id: String::new(),
            project_info: None,
            buyer_info: None,
            all_buyer_info: None,
            no_bind_buyer_info: None,
            select_ticket_id: None,
            pay_money: None,
            count: None,
            device_id: String::new(),
            id_bind: 999,
        };
        log::debug!("新建抢票对象：{:?}", new);
        new
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct TicketInfo {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub is_sale: Option<usize>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub pick_seat: Option<usize>,    // 0:不选座 1:选座
    pub project_type: Option<usize>, // 未知作用，bw2024 是 type1
    #[serde(default, deserialize_with = "deserialize_opt_non_negative_usize")]
    pub express_fee: Option<usize>, // 快递费（接口可能返回 -1，映射为 None）
    pub sale_begin: Option<i64>,     // 开售时间
    pub sale_end: Option<i64>,       // 截止时间
    pub count_down: Option<i64>,     // 倒计时（可能有负数）
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub screen_list: Vec<ScreenInfo>, // 场次列表
    pub sale_flag_number: Option<usize>, // 售票标志位
    pub sale_flag: String,           // 售票状态
    pub is_free: Option<bool>,
    pub performance_desc: Option<DescribeList>, // 基础信息
    pub id_bind: Option<usize>,                 // 是否绑定
    #[serde(rename = "hotProject")]
    pub hot_project: Option<bool>, // 是否热门项目
}

impl TicketInfo {
    /// 如果 sale_begin 缺失或 <= 0，仅从 screen.sale_start 与 ticket.saleStart 提取开售时间作为 fallback
    /// （不使用 screen.start_time，因为它是演出日期而非开售时间）
    pub fn fill_missing_sale_begin(&mut self) {
        if matches!(self.sale_begin, Some(v) if v > 0) {
            return;
        }

        let mut candidate_times: Vec<i64> = Vec::new();

        for screen in &self.screen_list {
            if screen.sale_start > 0 {
                candidate_times.push(screen.sale_start as i64);
            }

            for ticket in &screen.ticket_list {
                if let Some(t) = ticket.saleStart {
                    if t > 0 {
                        candidate_times.push(t as i64);
                    }
                }
            }
        }

        if let Some(&earliest) = candidate_times.iter().min() {
            self.sale_begin = Some(earliest);
            log::info!("sale_begin 缺失，已从开售时间字段提取最早时间: {}", earliest);
        } else {
            log::warn!("sale_begin 缺失且无法从 screen_list 开售字段提取有效时间");
        }
    }

    /// 校验项目信息中后续流程必需的字段是否为空
    pub fn validate(&self) -> Result<(), String> {
        let mut missing = Vec::new();

        if self.id.is_none() {
            missing.push("id(项目ID)");
        }
        if self.name.is_none() {
            missing.push("name(项目名称)");
        }
        if self.id_bind.is_none() {
            missing.push("id_bind(实名绑定标识)");
        }
        if self.screen_list.is_empty() {
            missing.push("screen_list(场次列表为空)");
        }

        let has_valid_sale_begin = matches!(self.sale_begin, Some(v) if v > 0);
        if !has_valid_sale_begin {
            let any_screen_launched = self
                .screen_list
                .iter()
                .any(|s| s.clickable || matches!(s.sale_flag_number, 2 | 4 | 8));
            let is_strictly_pre_sale = self.sale_flag_number == Some(1) && !any_screen_launched;
            if is_strictly_pre_sale {
                missing.push("sale_begin(开售时间)");
            } else {
                log::debug!(
                    "项目非纯未开售状态 (sale_flag_number={:?})，允许 sale_begin 为空或 <= 0",
                    self.sale_flag_number
                );
            }
        }

        if missing.is_empty() {
            Ok(())
        } else {
            let msg = format!(
                "项目信息校验失败，以下必需字段缺失或为空: [{}]",
                missing.join(", ")
            );
            log::error!("{}", msg);
            Err(msg)
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ScreenInfo {
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub sale_flag: SaleFlag,
    pub id: Option<usize>,
    #[serde(default, deserialize_with = "deserialize_opt_non_negative_usize")]
    pub start_time: Option<usize>,
    pub name: Option<String>,
    pub ticket_type: Option<usize>,
    pub screen_type: Option<usize>,
    pub delivery_type: Option<usize>,
    pub pick_seat: Option<usize>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub ticket_list: Vec<ScreenTicketInfo>, // 当日票种类列表
    pub clickable: bool, // 是否可点（可售）
    #[serde(default, deserialize_with = "deserialize_non_negative_usize")]
    pub sale_end: usize, // 截止时间
    #[serde(default, deserialize_with = "deserialize_non_negative_usize")]
    pub sale_start: usize, // 开售时间
    pub sale_flag_number: usize, // 售票标志位
    pub show_date: String, // 展示信息
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct SaleFlag {
    pub number: usize,
    pub display_name: String,
}

impl Default for SaleFlag {
    fn default() -> Self {
        Self {
            number: 0,
            display_name: "未知状态".to_string(),
        }
    }
}

#[allow(non_snake_case)]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ScreenTicketInfo {
    #[serde(default, deserialize_with = "deserialize_opt_non_negative_usize")]
    pub saleStart: Option<usize>, // 开售时间(时间戳) eg：1720260000
    #[serde(default, deserialize_with = "deserialize_opt_non_negative_usize")]
    pub saleEnd: Option<usize>, // 截止时间(时间戳)
    pub id: Option<usize>,         // 票种 id
    pub project_id: Option<usize>, // 项目 id
    pub price: Option<usize>,      // 票价(分)
    pub desc: Option<String>,      // 票种描述
    pub sale_start: Option<String>, // 开售时间（字符串） eg:2024-07-06 18:00:00
    pub sale_end: Option<String>,  // 截止时间（字符串）
    pub r#type: Option<usize>,     // 类型 关键词替换，对应 "type"
    pub sale_type: Option<usize>,  // 销售状态
    pub is_sale: Option<usize>,    // 是否销售？0是1否
    pub num: Option<usize>,        // 数量
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub sale_flag: SaleFlag, // 售票状态
    pub clickable: Option<bool>,   // 是否可点（可售）
    pub sale_flag_number: Option<usize>, // 售票标志位
    pub screen_name: Option<String>, // 场次名称
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct DescribeList {
    pub r#type: u8,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub list: Vec<ModuleItem>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModuleItem {
    pub module: String,
    // details 可能是字符串或数组，使用 serde_json::Value 处理多态
    pub details: Value,
    pub module_name: Option<String>,
}

// 为 base_info 模块中的详情项创建结构体
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BaseInfoItem {
    pub title: String,
    pub content: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InfoResponse {
    pub errno: i32,
    pub errtag: i32,
    pub msg: String,
    pub code: i32,
    pub message: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub data: TicketInfo,
}

#[allow(non_snake_case)]
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BuyerInfo {
    pub id: i64,
    pub uid: i64,
    pub personal_id: String,
    pub name: String,
    pub tel: String,
    pub id_type: i64,
    pub is_default: i64,
    pub id_card_front: String,
    pub id_card_back: String,
    pub verify_status: i64,
    pub isBuyerInfoVerified: bool,
    pub isBuyerValid: bool,
}

impl fmt::Debug for BuyerInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuyerInfo")
            .field("id", &self.id)
            .field("uid", &self.uid)
            .field("personal_id", &redact_pii(&self.personal_id))
            .field("name", &self.name)
            .field("tel", &redact_pii(&self.tel))
            .field("id_type", &self.id_type)
            .field("is_default", &self.is_default)
            .field("verify_status", &self.verify_status)
            .field("isBuyerInfoVerified", &self.isBuyerInfoVerified)
            .field("isBuyerValid", &self.isBuyerValid)
            .finish()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BuyerInfoResponse {
    pub errno: i32,
    pub errtag: i32,
    pub msg: String,
    pub code: i32,
    pub message: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub data: BuyerInfoData,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BuyerInfoData {
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub list: Vec<BuyerInfo>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NoBindBuyerInfo {
    pub name: String,
    pub tel: String,
    pub uid: i64,
}

impl fmt::Debug for NoBindBuyerInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NoBindBuyerInfo")
            .field("name", &self.name)
            .field("tel", &redact_pii(&self.tel))
            .field("uid", &self.uid)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_express_fee_and_sale_timestamps_deserialize_safely() {
        let json_str = r#"{
            "id": 10086,
            "name": "Test Show",
            "express_fee": -1,
            "id_bind": 1,
            "screen_list": [
                {
                    "id": 1,
                    "start_time": 1800000000,
                    "sale_start": -1,
                    "sale_end": -1,
                    "clickable": true,
                    "ticket_list": [
                        {
                            "id": 10,
                            "saleStart": -1,
                            "saleEnd": -1
                        }
                    ]
                }
            ]
        }"#;
        let info: TicketInfo = serde_json::from_str(json_str).expect("should deserialize");
        assert_eq!(info.express_fee, None);
        assert_eq!(info.screen_list.len(), 1);
        assert_eq!(info.screen_list[0].sale_start, 0);
        assert_eq!(info.screen_list[0].sale_end, 0);
        assert_eq!(info.screen_list[0].ticket_list[0].saleStart, None);
        assert_eq!(info.screen_list[0].ticket_list[0].saleEnd, None);
    }

    #[test]
    fn fill_missing_sale_begin_ignores_screen_start_time() {
        let mut info = TicketInfo {
            id: Some(1),
            name: Some("Show".to_string()),
            id_bind: Some(1),
            sale_begin: None,
            screen_list: vec![ScreenInfo {
                start_time: Some(1_700_000_000), // 演出日期，绝不应被当作开售时间
                sale_start: 0,
                ticket_list: vec![ScreenTicketInfo {
                    saleStart: Some(1_750_000_000),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };

        info.fill_missing_sale_begin();
        assert_eq!(info.sale_begin, Some(1_750_000_000));

        // 若无任何 sale_start / saleStart，则不应回退到 screen.start_time
        let mut info2 = TicketInfo {
            id: Some(2),
            name: Some("Show2".to_string()),
            id_bind: Some(1),
            sale_begin: None,
            screen_list: vec![ScreenInfo {
                start_time: Some(1_700_000_000),
                sale_start: 0,
                ticket_list: vec![],
                ..Default::default()
            }],
            ..Default::default()
        };
        info2.fill_missing_sale_begin();
        assert_eq!(info2.sale_begin, None);
    }

    #[test]
    fn validate_allows_missing_hot_project_and_on_sale_without_sale_begin() {
        let info = TicketInfo {
            id: Some(123),
            name: Some("OnSale".to_string()),
            id_bind: Some(1),
            hot_project: None,
            sale_begin: None,
            sale_flag_number: Some(2),
            screen_list: vec![ScreenInfo {
                clickable: true,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(info.validate().is_ok());

        // 未开售且无 sale_begin 仍应失败
        let pre_sale = TicketInfo {
            id: Some(123),
            name: Some("PreSale".to_string()),
            id_bind: Some(1),
            hot_project: None,
            sale_begin: None,
            sale_flag_number: Some(1),
            screen_list: vec![ScreenInfo {
                clickable: false,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(pre_sale.validate().is_err());
    }

    #[test]
    fn validate_allows_sold_out_projects_without_sale_begin() {
        let temp_sold_out = TicketInfo {
            id: Some(115413),
            name: Some("SoldOutProject".to_string()),
            id_bind: Some(2),
            sale_begin: None,
            sale_flag_number: Some(8),
            screen_list: vec![ScreenInfo {
                clickable: false,
                sale_start: 0,
                sale_flag_number: 8,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(temp_sold_out.validate().is_ok());

        let sold_out = TicketInfo {
            id: Some(115414),
            name: Some("SoldOutFlag4".to_string()),
            id_bind: Some(1),
            sale_begin: Some(0),
            sale_flag_number: Some(4),
            screen_list: vec![ScreenInfo {
                clickable: false,
                sale_start: 0,
                sale_flag_number: 4,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(sold_out.validate().is_ok());

        // 项目级 sale_flag_number=1 但存在已开售/售罄场次时，也不应误拒
        let mixed_screen = TicketInfo {
            id: Some(115415),
            name: Some("MixedScreens".to_string()),
            id_bind: Some(1),
            sale_begin: None,
            sale_flag_number: Some(1),
            screen_list: vec![ScreenInfo {
                clickable: false,
                sale_start: 0,
                sale_flag_number: 8,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(mixed_screen.validate().is_ok());
    }

    #[test]
    fn real_repo_json_captures_deserialize_cleanly() {
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");

        let cache_dir = workspace_root.join("cache");
        if cache_dir.is_dir() {
            for entry in std::fs::read_dir(&cache_dir).expect("read cache dir") {
                let path = entry.expect("cache dir entry").path();
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if name.starts_with("project_") && name.ends_with(".json") {
                    let content = std::fs::read_to_string(&path)
                        .unwrap_or_else(|e| panic!("failed to read {:?}: {}", path, e));
                    let mut resp: InfoResponse = serde_json::from_str(&content)
                        .unwrap_or_else(|e| panic!("failed to deserialize {:?}: {}", path, e));
                    resp.data.fill_missing_sale_begin();
                    resp.data
                        .validate()
                        .unwrap_or_else(|e| panic!("validate failed for {:?}: {}", path, e));
                }
            }
        }

        let sample_v2 = workspace_root.join("Log").join("SampleGetV2.json");
        if sample_v2.is_file() {
            let content = std::fs::read_to_string(&sample_v2).expect("read SampleGetV2.json");
            let mut resp: InfoResponse =
                serde_json::from_str(&content).expect("deserialize SampleGetV2.json");
            assert_eq!(resp.code, 0);
            assert!(resp.data.validate().is_ok());
            resp.data.fill_missing_sale_begin();
            assert!(resp.data.sale_begin.is_some_and(|v| v > 0));
            assert!(resp.data.validate().is_ok());
        }

        let buyer_capture = workspace_root.join("响应").join("apiticketbuyerlist.txt");
        if buyer_capture.is_file() {
            let content =
                std::fs::read_to_string(&buyer_capture).expect("read apiticketbuyerlist.txt");
            let resp: BuyerInfoResponse =
                serde_json::from_str(&content).expect("deserialize apiticketbuyerlist.txt");
            assert_eq!(resp.errno, 0);
            assert!(!resp.data.list.is_empty());
        }
    }

    #[test]
    fn buyer_info_debug_redacts_pii_and_preserves_json_field_names() {
        let buyer = BuyerInfo {
            id: 1,
            uid: 2,
            personal_id: "110101199001011234".to_string(),
            name: "张三".to_string(),
            tel: "13800138000".to_string(),
            isBuyerInfoVerified: true,
            isBuyerValid: true,
            ..Default::default()
        };

        let dbg = format!("{:?}", buyer);
        assert!(!dbg.contains("110101199001011234"));
        assert!(!dbg.contains("13800138000"));
        assert!(dbg.contains("[REDACTED]"));

        let serialized = serde_json::to_value(&buyer).expect("should serialize");
        assert_eq!(serialized["isBuyerInfoVerified"], Value::Bool(true));
        assert_eq!(serialized["isBuyerValid"], Value::Bool(true));
    }

    #[test]
    fn buyer_info_data_handles_null_list() {
        let resp: BuyerInfoResponse =
            serde_json::from_str(r#"{"errno": 0, "data": {"list": null}}"#)
                .expect("null buyer list should deserialize");
        assert!(resp.data.list.is_empty());
    }

    #[test]
    fn bilibiliticket_new_respects_open_custom_ua() {
        let account: Account = serde_json::from_str(r#"{"uid": 888}"#).unwrap();
        let default_ua = "DefaultUA/1.0".to_string();
        let push_cfg = PushConfig::new();

        let mut custom_off = CustomConfig::new();
        custom_off.open_custom_ua = false;
        custom_off.custom_ua = "CustomUA/2.0".to_string();
        let bt_off = BilibiliTicket::new(
            &0,
            &default_ua,
            &custom_off,
            &account,
            &push_cfg,
            &2,
            "10001",
            "https://show.bilibili.com/",
        );
        assert_eq!(bt_off.ua, "DefaultUA/1.0");

        let mut custom_on = CustomConfig::new();
        custom_on.open_custom_ua = true;
        custom_on.custom_ua = "CustomUA/2.0".to_string();
        let bt_on = BilibiliTicket::new(
            &0,
            &default_ua,
            &custom_on,
            &account,
            &push_cfg,
            &2,
            "10001",
            "https://show.bilibili.com/",
        );
        assert_eq!(bt_on.ua, "CustomUA/2.0");
    }
}
