use std::sync::Arc;

use reqwest::header::HeaderValue;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cookie_manager::CookieManager;
use crate::account::Account;
use crate::push::PushConfig;
use crate::utility::CustomConfig;

//成功下单结构体
#[derive(Clone,Debug,Deserialize,Serialize)]
pub struct SubmitOrderResult{
    #[serde(rename = "orderId")]
    pub order_id: i128,
    #[serde(rename = "orderCreateTime")]
    pub order_create_time: i64,
    #[serde(rename = "token")]
    pub order_token: String,
}

#[derive(Clone,Debug,Deserialize,Serialize)]
pub struct CheckFakeResult{
    #[serde(default)]
    pub errno: i32,
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub errtag: i32,
    #[serde(default)]
    pub msg: String,
    #[serde(default)]
    pub message: String,
    pub data: CheckFakeResultParam,
}
#[derive(Clone,Debug,Deserialize,Serialize)]
pub struct CheckFakeResultParam{
    #[serde(rename = "payParam")]
    pub pay_param: CheckFakeResultData,
}
#[derive(Clone,Debug,Deserialize,Serialize)]
pub struct CheckFakeResultData{
    pub sign : String,
    pub code_url : String,
}

#[derive(Clone,Debug,Deserialize,Serialize)]
pub struct ConfirmTicketInfo{
    pub name: String,
    pub count: i32,
    pub price: i64,
}

//确认订单结构体
#[derive(Clone,Debug,Deserialize,Serialize)]
pub struct ConfirmTicketResult{
    pub count : i32,
    pub pay_money: i64,
    pub project_name: String,
    pub screen_name: String,
    pub ticket_info: ConfirmTicketInfo,
    
}

//获取token响应结构体

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TokenRiskParam {
    #[serde(default)]
    pub code : i32,
    
    #[serde(default)]
    pub message: String,
    
    pub mid : Option<String>,
    pub decision_type : Option<String>,
    pub buvid : Option<String>,
    pub ip : Option<String>,
    pub scene: Option<String>,
    pub ua: Option<String>,
    pub v_voucher: Option<String>,
    pub risk_param: Option<Value>,
}

#[derive(Clone,Debug)]
pub struct BilibiliTicket{
    pub uid : i64, //UID
    pub method : u8,
    pub ua : String,
    pub config: CustomConfig,
    pub account: Account,
    pub push_self : PushConfig,
    pub status_delay : usize,
    pub captcha_use_type: usize,    //选择的验证码方式
    pub cookie_manager: Option<Arc<CookieManager>>,

    //抢票相关
    pub project_id: String,
    pub screen_id: String,
    pub id_bind: usize, //是否绑定

    pub project_info : Option<TicketInfo>, //项目详情
    pub all_buyer_info: Option<BuyerInfoData>, //所有购票人信息
    pub buyer_info: Option<Vec<BuyerInfo>>,  //购买人信息（实名票）

    pub no_bind_buyer_info: Option<NoBindBuyerInfo>, //不实名制购票人信息

    pub select_ticket_id : Option<String>,

    pub pay_money: Option<i64>, //支付金额
    pub count: Option<i32>, //购买数量
    pub device_id: String, //设备id

}

impl BilibiliTicket{
    pub fn new(
        
        method: &u8,
        ua: &String,
        config: &CustomConfig,
        account: &Account,
        push_self: &PushConfig,
        status_delay: &usize,
        project_id : &str,


    ) -> Self{
        let mut finally_ua = String::new();
        if config.custom_ua != "" {
            log::info!("使用自定义UA：{}",config.custom_ua);
            finally_ua.push_str(&config.custom_ua);
        }else{
            log::info!("使用默认UA：{}",ua);
            finally_ua.push_str(ua);
        }
        let mut headers = header::HeaderMap::new();
        match HeaderValue::from_str(&account.cookie){
            Ok(ck_value) => {
                headers.insert(header::COOKIE, ck_value);
                match HeaderValue::from_str(&finally_ua){
                    Ok(ua_value) => {
                        headers.insert(header::USER_AGENT,ua_value);
                    }
                    Err(e) => {
                        log::error!("client插入ua失败！原因：{}",e);
                    }
                }
                
            }
            Err(e) => {
                log::error!("cookie设置失败！原因：{:?}",e);
            }

        }
        

        let client = match Client::builder()
                                    .cookie_store(true)
                                    .user_agent(ua)
                                    .default_headers(headers)
                                    
                                    .build(){
                                        Ok(client) => client,
                                        Err(e) => {
                                            log::error!("初始化client失败！，原因：{:?}",e);
                                            Client::new()
                                        }
                                    };
        let captcha_type = config.captcha_mode;      

       
           
        let new = Self{
            uid: account.uid.clone(),
            method: method.clone(),
            ua: ua.clone(),
            config: config.clone(),
            account: account.clone(),
            push_self: push_self.clone(),
            status_delay: *status_delay,
            captcha_use_type: captcha_type,
            cookie_manager: None,
            project_id: project_id.to_string(),
            screen_id: String::new(),
            project_info: None,
            buyer_info: None,
            all_buyer_info: None,
            no_bind_buyer_info: None,
            select_ticket_id: None,
            pay_money: None,
            count: None,
            device_id: "".to_string(),
            id_bind: 999,

        };
        log::debug!("新建抢票对象：{:?}",new);
        new

    }

}

#[derive(Clone,Debug,Deserialize,Serialize)]
pub struct TicketInfo {
    pub id: i32,
    pub name: String,
    pub is_sale: usize,
    pub start_time: i64,
    pub end_time: i64,
    pub pick_seat: usize, //0:不选座 1:选座
    pub project_type: usize, //未知作用，bw2024是type1
    #[serde(default)]
    pub express_fee: Option<usize>, //快递费
    pub sale_begin: i64, //开售时间
    pub sale_end: i64, //截止时间
    pub count_down: i64, //倒计时（可能有负数）
    pub screen_list: Vec<ScreenInfo>, //场次列表
    pub sale_flag_number: usize, //售票标志位
    #[serde(default)]
    pub sale_flag: String, //售票状态
    pub is_free: bool,
    pub performance_desc: Option<DescribeList>, //基础信息
    pub id_bind: usize, //是否绑定
    #[serde(rename = "hotProject")]
    pub hot_project: bool, //是否热门项目

    


}

#[derive(Clone,Debug,Deserialize,Serialize)]
pub struct ScreenInfo {
    #[serde(default)]
    pub sale_flag: SaleFlag,
    pub id: usize,
    pub start_time: usize,
    pub name: String,
    pub ticket_type: usize,
    pub screen_type: usize,
    pub delivery_type: usize,
    pub pick_seat: usize,
    pub ticket_list: Vec<ScreenTicketInfo>, //当日票种类列表
    pub clickable: bool, //是否可点（可售）
    pub sale_end: usize, //截止时间
    pub sale_start: usize, //开售时间
    pub sale_flag_number: usize, //售票标志位
    pub show_date: String, //展示信息
    
}

#[derive(Clone,Debug,Deserialize,Serialize)]
pub struct SaleFlag {
    #[serde(default)]
    pub number: usize,
    #[serde(default)]
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

#[derive(Clone,Debug,Deserialize,Serialize)]
pub struct ScreenTicketInfo{
    pub saleStart : usize, //开售时间(时间戳)   eg：1720260000
    pub saleEnd : usize, //截止时间(时间戳)
    pub id: usize, //票种id
    pub project_id: usize, //项目id
    pub price: usize, //票价(分)
    pub desc: String, //票种描述
    pub sale_start: String, //开售时间（字符串）    eg:2024-07-06 18:00:00
    pub sale_end: String, //截止时间（字符串）
    pub r#type: usize, //类型 关键词替换，对应”type“
    pub sale_type: usize, //销售状态
    pub is_sale: usize, //是否销售？0是1否
    pub num: usize, //数量
    pub sale_flag: SaleFlag, //售票状态
    pub clickable: bool, //是否可点（可售）
    pub sale_flag_number: usize, //售票标志位
    pub screen_name: String, //场次名称


}

#[derive(Clone,Debug,Deserialize,Serialize)]
pub struct DescribeList{
    pub r#type: u8,  // 使用 r# 前缀处理 Rust 关键字
    pub list: Vec<ModuleItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModuleItem {
    pub module: String,
    
    // details 可能是字符串或数组，使用 serde_json::Value 处理多态
    #[serde(default)]
    pub details: Value,
    
    // 可选字段
    #[serde(default)]
    pub module_name: Option<String>,
}

// 为 base_info 模块中的详情项创建结构体
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BaseInfoItem {
    pub title: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InfoResponse{
    #[serde(default)]
    pub errno: i32,
    #[serde(default)]
    pub errtag: i32,
    #[serde(default)]
    pub msg: String,
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub message: String,
    pub data: TicketInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuyerInfo{
    pub id: i64,
    pub uid: i64,
    pub personal_id: String,
    pub name: String,
    pub tel: String,
    pub id_type: i64,
    pub is_default: i64,
    #[serde(default)]
    pub id_card_front: String,
    #[serde(default)]
    pub id_card_back: String,
    #[serde(default)]
    pub verify_status: i64,
    #[serde(default)]
    pub isBuyerInfoVerified: bool,
    #[serde(default)]
    pub isBuyerValid: bool,
    

}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuyerInfoResponse{
    #[serde(default)]
    pub errno: i32,
    #[serde(default)]
    pub errtag: i32,
    #[serde(default)]
    pub msg: String,
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub message: String,
    pub data: BuyerInfoData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuyerInfoData{
    pub list: Vec<BuyerInfo>,
    
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoBindBuyerInfo {
    pub name: String,
    pub tel: String,
    pub uid: i64,
}