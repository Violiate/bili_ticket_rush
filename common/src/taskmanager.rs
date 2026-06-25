use std::time::Instant;
use reqwest::Client;
use std::sync::Arc;
use crate::cookie_manager::CookieManager;
use crate::ticket::{*};
use crate::captcha::LocalCaptcha;
use crate::push::PushConfig;
use crate::utility::CustomConfig;
use crate::show_orderlist::OrderResponse;




// 任务状态枚举
#[derive(Clone,Debug)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed(bool),
    Failed(String),
}

// 票务结果
#[derive(Clone)]
pub struct TicketResult {
    pub success: bool,
    pub order_id: Option<String>,
    pub message: Option<String>,
    pub ticket_info: TicketInfo,
    pub timestamp: Instant,
}

// 任务信息
pub enum Task {
    
    QrCodeLoginTask(QrCodeLoginTask),
    LoginSmsRequestTask(LoginSmsRequestTask),
    PushTask(PushTask),
    SubmitLoginSmsRequestTask(SubmitLoginSmsRequestTask),
    GetAllorderRequestTask(GetAllorderRequest),
    GetTicketInfoTask(GetTicketInfoTask),
    GetBuyerInfoTask(GetBuyerInfoTask),
    GrabTicketTask(GrabTicketTask),    
}

// 任务请求枚举
pub enum TaskRequest {
    
    QrCodeLoginRequest(QrCodeLoginRequest),
    LoginSmsRequest(LoginSmsRequest),
    PushRequest(PushRequest),
    SubmitLoginSmsRequest(SubmitLoginSmsRequest),
    GetAllorderRequest(GetAllorderRequest),
    GetTicketInfoRequest(GetTicketInfoRequest),
    GetBuyerInfoRequest(GetBuyerInfoRequest),
    GrabTicketRequest(GrabTicketRequest),
}

// 任务结果枚举
#[derive(Clone)]
pub enum TaskResult {
    
    QrCodeLoginResult(TaskQrCodeLoginResult),
    LoginSmsResult(LoginSmsRequestResult),
    PushResult(PushRequestResult),
    SubmitSmsLoginResult(SubmitSmsLoginResult),
    GetAllorderRequestResult(GetAllorderRequestResult),
    GetTicketInfoResult(GetTicketInfoResult),
    GetBuyerInfoResult(GetBuyerInfoResult),
    GrabTicketResult(GrabTicketResult),
}
//抢票请求
#[derive(Clone,Debug)]
pub struct GrabTicketRequest {
    pub task_id: String,
    pub uid: i64,
    pub project_id : String,
    pub screen_id : String,
    pub ticket_id: String,
    pub count: i16,
    pub buyer_info: Vec<BuyerInfo>,
    pub cookie_manager: Arc<CookieManager>,
    pub biliticket: BilibiliTicket,
    pub grab_mode: u8,
    pub status: TaskStatus,
    pub start_time: Option<Instant>,
    pub is_hot: bool,
    pub local_captcha: LocalCaptcha,
    pub skip_words: Option<Vec<String>>,
    
}
#[derive(Clone,Debug)]
pub struct GrabTicketTask {
    pub task_id: String,
    pub biliticket: BilibiliTicket,
    pub status: TaskStatus,
    pub client: Arc<Client>,
    pub start_time: Option<Instant>,
    

}
#[derive(Clone,Debug)]
pub struct  GrabTicketResult {
    pub task_id: String,
    pub uid : i64,
    pub success: bool,
    pub message:String,
    pub order_id: Option<String>,
    pub pay_token: Option<String>,
    pub confirm_result: Option<ConfirmTicketResult>,
    pub pay_result: Option<CheckFakeResultData>,
}
//获取购票人信息
#[derive(Clone,Debug)]
pub struct GetBuyerInfoRequest {
    pub uid : i64,
    pub task_id : String,
    pub cookie_manager: Arc<CookieManager>,
}

#[derive(Clone,Debug)]
pub struct GetBuyerInfoResult{
    pub task_id : String,
    pub uid : i64,
    pub buyer_info: Option<BuyerInfoResponse>,
    pub success: bool,
    pub message : String,
}
#[derive(Clone,Debug)]
pub struct GetBuyerInfoTask {
    pub uid : i64,
    pub task_id : String,
    pub status: TaskStatus,
    pub start_time : Option<Instant>,
    pub cookie_manager: Arc<CookieManager>,
}
//请求project_id票详情
#[derive(Clone,Debug)]
pub struct GetTicketInfoRequest {
    pub uid : i64,
    pub task_id : String,
    pub project_id : String,
    pub cookie_manager: Arc<CookieManager>,
    pub referer_link : String,

}

#[derive(Clone,Debug)]
pub struct GetTicketInfoResult {
    pub task_id : String,
    pub uid : i64,
    pub ticket_info: Option<InfoResponse>,
    pub success: bool,
    pub message : String,
}

#[derive(Clone,Debug)]
pub struct GetTicketInfoTask {
    pub task_id : String,
    pub project_id : String,
    pub status: TaskStatus,
    pub start_time : Option<Instant>,
    pub referer_link : String,
    pub cookie_manager: Arc<CookieManager>,
}


#[derive(Clone)]
pub struct PushRequest{
    pub title: String,
    pub message: String,
    pub jump_url: Option<String>,
    pub push_config: PushConfig,
    pub push_type : PushType,
}

//推送类型
#[derive(Clone,Debug)]
pub enum PushType {
    All,
    Bark,
    PushPlus,
    Fangtang,
    Dingtalk,
    WeChat,
    Smtp,
}

// 推送结果结构体
#[derive(Clone)]
pub struct PushRequestResult {
    pub task_id: String,
    pub success: bool,
    pub message: String,
    pub push_type: PushType,
}


#[derive(Clone)]
pub struct PushTask {
    pub task_id: String,
    pub title: String,
    pub message:String,
    pub push_type: PushType,
    pub status: TaskStatus,
    pub start_time: Option<Instant>,    
}

pub struct TicketTask {
    pub task_id: String,
    pub account_id: String,
    pub ticket_id: String,
    pub status: TaskStatus,
    pub start_time: Option<Instant>,
    pub result: Option<TicketResult>,
}

pub struct QrCodeLoginTask {
    pub task_id: String,
    pub qrcode_key: String,
    pub qrcode_url: String,
    pub status: TaskStatus,
    pub start_time: Option<Instant>,
    
}

pub struct LoginSmsRequestTask {
    pub task_id: String,
    pub phone : String,
    pub status: TaskStatus,
    pub start_time: Option<Instant>,
    
}

pub struct SubmitLoginSmsRequestTask {
    pub task_id: String,
    pub phone : String,
    pub code: String,
    pub captcha_key: String,
    pub status: TaskStatus,
    pub start_time: Option<Instant>,
}

//获取全部订单信息
pub struct GetAllorderRequest {
    pub task_id: String,
    pub cookie_manager: Arc<CookieManager>,
    pub status: TaskStatus,
    pub cookies: String,
    pub account_id: String,
    pub start_time: Option<Instant>,
   
}

#[derive(Clone)]
pub struct GetAllorderRequestResult {
    pub task_id: String,
    pub account_id: String,
    pub success: bool,
    pub message: String,
    pub order_info: Option<OrderResponse>,
    pub timestamp: Instant,
}

pub struct GetAllorderTask {
    pub task_id: String,
    pub account_id: String,
    pub status: TaskStatus,
    pub start_time: Option<Instant>,
}


pub struct TicketRequest {
    pub ticket_id: String,
    pub account_id: String,
    // 其他请求参数...
}

pub struct QrCodeLoginRequest {
    pub qrcode_key: String,
    pub qrcode_url: String,
    pub user_agent: Option<String>,
}

pub struct LoginSmsRequest {
    pub phone: String,
    pub client: Client,
    pub custom_config: CustomConfig,
    pub local_captcha: LocalCaptcha,
}

pub struct SubmitLoginSmsRequest {
    pub phone : String,
    pub code: String,
    pub captcha_key: String,
    pub client: Client,

}



#[derive(Clone)]
pub struct TaskTicketResult {
    pub task_id: String,
    pub account_id: String,
    pub result: Result<TicketResult, String>,
}

#[derive(Clone)]
pub struct TaskQrCodeLoginResult {
    pub task_id: String,
    pub status: crate::login::QrCodeLoginStatus,
    pub cookie: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct LoginSmsRequestResult {
    pub task_id: String,
    pub phone: String,
    pub success: bool,
    pub message: String,
}

#[derive(Clone)]
pub struct SubmitSmsLoginResult {
    pub task_id: String,
    pub phone: String,
    pub success: bool,
    pub message: String,
    pub cookie: Option<String>,
}
/// 运行中抢票任务的快照，供 UI 渲染（无 `Instant`，可直接跨线程 clone）
#[derive(Clone, Debug)]
pub struct RunningTaskInfo {
    pub task_id: String,
    /// 友好序号，#1 #2 …
    pub seq: u64,
    pub uid: i64,
    pub account_name: String,
    pub project_name: String,
    /// 0 定时 1 直接 2 捡漏
    pub grab_mode: u8,
    pub status: TaskStatus,
    /// 查询时即时算出的已运行秒数
    pub elapsed_secs: u64,
}

// 更新 TaskManager trait
pub trait TaskManager: Send + 'static {
    // 创建新的任务管理器
    fn new() -> Self where Self: Sized;

    // 提交任务
    fn submit_task(&mut self, request: TaskRequest) -> Result<String, String>;

    // 获取可用结果，返回 TaskResult 枚举
    fn get_results(&mut self) -> Vec<TaskResult>;

    // 取消任务
    fn cancel_task(&mut self, task_id: &str) -> Result<(), String>;

    // 获取任务状态
    fn get_task_status(&self, task_id: &str) -> Option<TaskStatus>;

    // 列出所有正在运行的抢票任务快照
    fn list_running_tasks(&self) -> Vec<RunningTaskInfo>;

     // 关闭任务管理器
    fn shutdown(&mut self);
}

pub const DISCLAIMER_TEXT_ENCODED: &str = "4p2X5pys6aG555uu5a6M5YWo5YWN6LS55byA5rqQ77yM56aB5q2i5ZWG55So5oiW5pS26LS577yM5LuF5L6b5a2m5Lmg5L2/55So77yM6K+35Zyo5LiL6L29MjTlsI/ml7blhoXliKDpmaTvvIzkvb/nlKjmnKzova/ku7bpgKDmiJDnmoTkuIDliIflkI7mnpzor7foh6rooYzmib/mi4U=";

pub fn TaskManager_debug() -> String {
    let bytes = base64::decode(DISCLAIMER_TEXT_ENCODED).unwrap_or_default();
    String::from_utf8(bytes).unwrap_or_else(|_| "本项目免费开源".to_string())
}