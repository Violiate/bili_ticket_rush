use std::sync::Arc;
use std::time::Instant;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use once_cell::sync::Lazy;
use reqwest::Client;

use crate::account::Account;
use crate::captcha::LocalCaptcha;
use crate::cookie_manager::CookieManager;
use crate::push::PushConfig;
use crate::show_orderlist::OrderResponse;
use crate::ticket::*;
use crate::utility::CustomConfig;

// 任务状态枚举
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Cancelling,
    Cancelled,
    Completed(bool),
    Failed(String),
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
    GenerateQrCodeRequest(GenerateQrCodeRequest),
    AddAccountRequest(AddAccountRequest),
    SignoutAccountRequest(SignoutAccountRequest),
    AddBuyerRequest(AddBuyerRequest),
    FetchPolicyRequest(FetchPolicyRequest),
    FetchImageRequest(FetchImageRequest),
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
    GenerateQrCodeResult(GenerateQrCodeResult),
    AddAccountResult(AddAccountResult),
    SignoutAccountResult(SignoutAccountResult),
    AddBuyerResult(AddBuyerResult),
    FetchPolicyResult(FetchPolicyResult),
    FetchImageResult(FetchImageResult),
}

// 抢票请求
#[derive(Clone, Debug)]
pub struct GrabTicketRequest {
    pub task_id: String,
    pub uid: i64,
    pub project_id: String,
    pub screen_id: String,
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

#[derive(Clone, Debug)]
pub struct GrabTicketResult {
    pub task_id: String,
    pub uid: i64,
    pub project_id: String,
    pub account_name: String,
    pub success: bool,
    pub message: String,
    pub order_id: Option<String>,
    pub pay_token: Option<String>,
    pub confirm_result: Option<ConfirmTicketResult>,
    pub pay_result: Option<CheckFakeResultData>,
}

// 获取购票人信息
#[derive(Clone, Debug)]
pub struct GetBuyerInfoRequest {
    pub uid: i64,
    pub task_id: String,
    pub cookie_manager: Arc<CookieManager>,
}

#[derive(Clone, Debug)]
pub struct GetBuyerInfoResult {
    pub task_id: String,
    pub uid: i64,
    pub buyer_info: Option<BuyerInfoResponse>,
    pub success: bool,
    pub message: String,
}

// 请求 project_id 票详情
#[derive(Clone, Debug)]
pub struct GetTicketInfoRequest {
    pub uid: i64,
    pub task_id: String,
    pub project_id: String,
    pub cookie_manager: Arc<CookieManager>,
    pub referer_link: String,
}

#[derive(Clone, Debug)]
pub struct GetTicketInfoResult {
    pub task_id: String,
    pub uid: i64,
    pub ticket_info: Option<InfoResponse>,
    pub success: bool,
    pub message: String,
}

#[derive(Clone)]
pub struct PushRequest {
    pub title: String,
    pub message: String,
    pub jump_url: Option<String>,
    pub push_config: PushConfig,
    pub push_type: PushType,
}

// 推送类型
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PushType {
    All,
    Bark,
    PushPlus,
    Fangtang,
    Dingtalk,
    WeChat,
    Gotify,
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

// 获取全部订单信息
pub struct GetAllorderRequest {
    pub task_id: String,
    pub cookie_manager: Arc<CookieManager>,
    pub status: TaskStatus,
    pub cookies: String,
    pub account_id: String,
    pub start_time: Option<Instant>,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Clone)]
pub struct GetAllorderRequestResult {
    pub task_id: String,
    pub account_id: String,
    pub page: usize,
    pub success: bool,
    pub message: String,
    pub order_info: Option<OrderResponse>,
    pub timestamp: Instant,
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
    pub phone: String,
    pub code: String,
    pub captcha_key: String,
    pub client: Client,
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

// 异步生成登录二维码
pub struct GenerateQrCodeRequest {
    pub client: Client,
}

#[derive(Clone, Debug)]
pub struct GenerateQrCodeResult {
    pub task_id: String,
    pub success: bool,
    pub qrcode_key: Option<String>,
    pub qrcode_url: Option<String>,
    pub message: String,
}

// 异步添加账号
pub struct AddAccountRequest {
    pub cookie: String,
    pub client: Client,
    pub user_agent: String,
    pub source: String,
}

#[derive(Clone, Debug)]
pub struct AddAccountResult {
    pub task_id: String,
    pub success: bool,
    pub account: Option<Account>,
    pub source: String,
    pub message: String,
}

// 异步退出登录
pub struct SignoutAccountRequest {
    pub account: Account,
}

#[derive(Clone, Debug)]
pub struct SignoutAccountResult {
    pub task_id: String,
    pub uid: i64,
    pub remote_success: bool,
    pub message: String,
}

// 异步添加购票人
pub struct AddBuyerRequest {
    pub uid: i64,
    pub cookie_manager: Arc<CookieManager>,
    pub name: String,
    pub phone: String,
    pub id_type: i32,
    pub id_number: String,
    pub as_default_buyer: bool,
}

#[derive(Clone, Debug)]
pub struct AddBuyerResult {
    pub task_id: String,
    pub uid: i64,
    pub success: bool,
    pub message: String,
}

// 异步拉取策略配置
pub struct FetchPolicyRequest {
    pub client: Client,
    pub app: String,
    pub version: String,
    pub machine_id: String,
}

#[derive(Clone, Debug)]
pub struct FetchPolicyResult {
    pub task_id: String,
    pub success: bool,
    pub raw_response: Option<serde_json::Value>,
    pub error: Option<String>,
}

// 异步拉取图片字节（头像/封面等）
pub struct FetchImageRequest {
    pub key: String,
    pub url: String,
}

#[derive(Clone, Debug)]
pub struct FetchImageResult {
    pub task_id: String,
    pub key: String,
    pub url: String,
    pub bytes: Option<Vec<u8>>,
    pub error: Option<String>,
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

/// 已结束抢票任务的近期记录，供 UI 展示执行结果与历史
#[derive(Clone, Debug)]
pub struct RecentTaskInfo {
    pub task_id: String,
    pub uid: i64,
    pub account_name: String,
    pub project_id: String,
    pub project_name: String,
    pub grab_mode: u8,
    pub status: TaskStatus,
    pub message: String,
    pub started_at: std::time::Instant,
    pub finished_at: std::time::Instant,
}

pub trait TaskManager: Send + 'static {
    // 创建新的任务管理器
    fn new() -> Self
    where
        Self: Sized;

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

    // 列出近期结束的抢票任务记录
    fn list_recent_tasks(&self) -> Vec<RecentTaskInfo>;

    // 清空近期任务记录
    fn clear_recent_tasks(&self);

    // 动态更新全局推送配置
    fn update_push_config(&self, push_config: PushConfig);

    // 取消指定账号 UID 下的所有活动任务，返回取消数量
    fn cancel_tasks_for_uid(&self, uid: i64) -> usize;

    // 关闭任务管理器
    fn shutdown(&mut self);
}

pub const DISCLAIMER_TEXT_ENCODED: &str = "4p2X5pys6aG555uu5a6M5YWo5YWN6LS55byA5rqQ77yM56aB5q2i5ZWG55So5oiW5pS26LS577yM5LuF5L6b5a2m5Lmg5L2/55So77yM6K+35Zyo5LiL6L29MjTlsI/ml7blhoXliKDpmaTvvIzkvb/nlKjmnKzova/ku7bpgKDmiJDnmoTkuIDliIflkI7mnpzor7foh6rooYzmib/mi4U=";

static DISCLAIMER_TEXT: Lazy<String> = Lazy::new(|| {
    let bytes = STANDARD
        .decode(DISCLAIMER_TEXT_ENCODED)
        .unwrap_or_default();
    String::from_utf8(bytes).unwrap_or_else(|_| "本项目免费开源".to_string())
});

#[allow(non_snake_case)]
pub fn TaskManager_debug() -> String {
    DISCLAIMER_TEXT.clone()
}
