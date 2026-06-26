use std::sync::Arc;
use std::fs::File;
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};
use eframe::egui;
use reqwest::{Client, header};
use serde_json::{json,Value};
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};


use crate::ui;
use crate::windows;
use crate::windows::login_windows::LoginTexture;
use crate::windows::add_buyer::AddBuyerInput;
use crate::ui::error_banner::render_error_banner;

use common::LOG_COLLECTOR;
use common::account::{Account,add_account};
use common::utils::{*};
use common::utility::CustomConfig;
use common::push::{*};
use common::login::LoginInput;
use common::taskmanager::{*};
use common::captcha::LocalCaptcha;
use common::show_orderlist::OrderResponse;
use common::cookie_manager::CookieManager;
use common::taskmanager::GetAllorderRequest;
use common::taskmanager::TaskRequest;
use common::ticket::{*};

use backend::taskmanager::TaskManagerImpl;


//UI
pub struct Myapp{  
    pub app: String,
    pub version: String,
    pub policy: Option<Value>,
    //ui
    pub left_panel_width: f32,  //左面板宽度
    pub selected_tab: usize,    //左侧已选中标签
    //加载动画
    pub loading_angle: f32,
    pub is_loading: bool,
    //运行状态（显示用）
    pub running_status: String,
    //自定义背景图  （未启用，效果不好，预留暂时不用）
    pub background_texture: Option<egui::TextureHandle>,
    //日志记录
    pub logs: Vec<String>,
    pub show_log_window: bool,
    //登录窗口
    pub show_login_windows: bool,
    //用户信息
    
    pub default_avatar_texture: Option<egui::TextureHandle>, // 默认头像
        
    //错误提醒横幅
    pub error_banner_active: bool,
    pub error_banner_text: String,
    pub error_banner_start_time: Option<std::time::Instant>,
    pub error_banner_opacity: f32,

    //成功提醒横幅
    pub success_banner_active: bool,
    pub success_banner_text: String,
    pub success_banner_start_time: Option<std::time::Instant>,
    pub success_banner_opacity: f32,

    //抢票id
    pub ticket_id: String,
   
   //任务管理
   pub task_manager: Box<dyn TaskManager>,
   pub account_manager: AccountManager,

   //推送设置
   pub push_config: PushConfig,

   //config
    pub config: Config,

    //自定义配置
    pub custom_config: CustomConfig,
    //登录背景
    pub login_texture: LoginTexture,

    //登录方式
    pub login_method: String,
    
    //用于登录的client，登录后存入account
    pub client: Client,

    //登录用，防止重复刷新二维码
    pub login_qrcode_url: Option<String>,

    //登录用异步回调taskid
    pub qrcode_polling_task_id: Option<String>,

    //登录用输入
    pub login_input: LoginInput,

    //登录用发送短信任务id
    pub pending_sms_task_id: Option<String>,

    //默认ua
    pub default_ua: String,

    //发送短信captcha_key
    pub sms_captcha_key: String,

    //删除账号
    pub delete_account: Option<String>,

    //cookie登录，暂存cookie
    pub cookie_login: Option<String>,

    //该账号开启抢票开关
    pub account_switch: Option<AccountSwitch>,

    //添加购票人的输入
    pub add_buyer_input: AddBuyerInput,

    //添加购票人窗口
    pub show_add_buyer_window: Option<String>, //如果是bool类型会导致无法对应申请添加的账号，
                                        //所以使用string表示要添加购票人的账号的uid

    pub show_orderlist_window: Option<String>, //订单列表窗口的账号uid

    pub total_order_data: Option<OrderData>, //订单数据缓存

    pub orderlist_need_reload: bool, //订单列表是否需要重新加载

    pub orderlist_last_request_time: Option<std::time::Instant>,  // 上次请求的时间
    pub orderlist_requesting: bool,  // 是否正在请求中

    //抢票相关
    pub status_delay: usize, //延迟时间

    pub grab_mode: u8,   // 0: 自动抢票, 1: 直接抢票, 2: 捡漏回流票
    pub selected_account_uid: Option<i64>, // 记录被选择账号的UID

    pub bilibiliticket_list: Vec<BilibiliTicket>, // 用于存储多个抢票实例

    pub ticket_info: Option<TicketInfo>,  //根据projectid获取的项目详情

    pub show_screen_info: Option<i64>, //开启显示场次窗口（获取到project信息后）

    pub selected_screen_index: Option<usize>,  // 当前选中的场次索引
    pub selected_screen_id: Option<i64>,       // 当前选中的场次ID
    pub selected_ticket_id: Option<i64>,       // 当前选中的票种ID

    pub ticket_info_last_request_time: Option<std::time::Instant>, // 上次请求的时间

    pub confirm_ticket_info: Option<String>, //确认抢票信息（购票人，预填手机号）

    pub selected_buyer_list: Option<Vec<BuyerInfo>>, // 选中的购票人ID

    pub local_captcha: LocalCaptcha, // 本地打码实例       

    pub  show_qr_windows: Option<String>, //扫码支付窗口  (传二维码数据)                   
    
    pub machine_id :String,
    
    pub announce1: Option<String>, //主公告
    pub announce2: Option<String>,
    pub announce3: Option<String>,//监视公告
    pub announce4: Option<String>, //退出公告
    

    pub public_key: String,
    pub skip_words: Option<Vec<String>>,
    pub skip_words_input: String, 
    
                                    }


//账号管理

pub struct AccountManager{
    pub accounts: Vec<Account>,
}

//获取全部订单结构体（便于区分）
pub struct OrderData {
    pub account_id: String,
    pub data : Option<OrderResponse>,
}




impl Myapp{
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self{
        
        //中文字体
        ui::fonts::configure_fonts(&cc.egui_ctx);
        let config = match Config::load_config() {
            Ok(load_config) => {
                log::info!("配置文件加载成功");
                load_config
            },
            Err(e) => {
                log::error!("配置文件加载失败: {}", e);
                log::info!("尝试迁移json配置");
                 match Config::load_json_config() {
                    Ok(load_config) => {
                        log::info!("配置文件加载成功");
                        match load_config.save_config() {
                            Ok(_) => {
                                log::info!("配置文件保存成功");
                                match Config::delete_json_config() {
                                    Ok(_) => {
                                        log::info!("旧配置文件删除成功");
                                    },
                                    Err(e) => {
                                        log::error!("旧配置文件删除失败: {}", e);
                                    }
                                }
                                log::info!("迁移成功");
                            },
                            Err(e) => {
                                log::error!("配置文件保存失败: {}", e);
                            }

                        }
                        load_config
                    }
                    Err(e) => {
                        log::error!("迁移失败: {}", e);
                        let cfg =Config::new();
                        match cfg.save_config() {
                            Ok(_) => {
                                log::info!("配置文件保存成功");
                            },
                            Err(e) => {
                                log::error!("配置文件保存失败: {}", e);
                            }

                        }
                        cfg
                    }
                
            }
            }
        };
        
        
        
        let mut app = Self {
            app: String::from("BRT"),
            version: String::from("6.6.1"),
            policy: None,
            public_key: String::from("-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEApTAS0RElXIs4Kr0bO4n8\nJB+eBFF/TwXUlvtOM9FNgHjK8m13EdwXaLy9zjGTSQr8tshSRr0dQ6iaCG19Zo2Y\nXfvJrwQLqdezMN+ayMKFy58/S9EGG3Np2eGgKHUPnCOAlRicqWvBdQ/cxzTDNCxa\nORMZdJRoBvya7JijLLIC3CoqmMc6Fxe5i8eIP0zwlyZ0L0C1PQ82BcWn58y7tlPY\nTCz12cWnuKwiQ9LSOfJ4odJJQK0k7rXxwBBsYxULRno0CJ3rKfApssW4cfITYVax\nFtdbu0IUsgEeXs3EzNw8yIYnsaoZlFwLS8SMVsiAFOy2y14lR9043PYAQHm1Cjaf\noQIDAQAB\n-----END PUBLIC KEY-----"),
            left_panel_width: 250.0,
            selected_tab: 0,
            is_loading: false,
            loading_angle: 0.0,
            background_texture: None,
            show_log_window: false,
            show_login_windows: false,
            logs: Vec::new(),
            client: Client::new(),
            default_avatar_texture: None,
            running_status: String::from("空闲ing"),
            ticket_id: String::from(""),
             // 初始化任务管理器
             task_manager: Box::new(TaskManagerImpl::new()),
             account_manager: AccountManager {
                 accounts: Config::load_all_accounts(),
             },
             
            push_config : match serde_json::from_value::<PushConfig>(config["push_config"].clone()) {
                Ok(config) => config,
                Err(e) => {
                    log::warn!("无法解析推送配置: {}, 使用默认值", e);
                    PushConfig::new()
                }
            },
        
            
               
            custom_config: match serde_json::from_value::<CustomConfig>(config["custom_config"].clone()) {
                Ok(config) => config,
                Err(e) => {
                    log::warn!("无法解析自定义配置: {}, 使用默认值", e);
                    CustomConfig::new()
                }
            },
            config: config.clone(),
            login_texture: LoginTexture { left_conrner_texture: None , right_conrner_texture: None},

                login_method: "扫码登录".to_string(),
              
                
                login_qrcode_url: None,
                qrcode_polling_task_id: None,
                login_input: LoginInput{
                    phone: String::new(),
                    account: String::new(),
                    password: String::new(),
                    cookie: String::new(),
                    sms_code: String::new(),
                },
            pending_sms_task_id: None,
            
            default_ua: String::from("Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Mobile Safari/537.36"),
            sms_captcha_key: String::new(),
            delete_account: None,
            cookie_login: None,
            account_switch: None,
            add_buyer_input: AddBuyerInput {
                name: String::new(),
                phone: String::new(),
                id_type: 0,
                id_number: String::new(),
                as_default_buyer: false,
            },
            show_add_buyer_window: None,
            show_orderlist_window: None,
            total_order_data: None,
            orderlist_need_reload: false,
            orderlist_last_request_time: None,
            orderlist_requesting: false,
            error_banner_active: false,
            error_banner_text: String::new(),
            error_banner_start_time: None,
            error_banner_opacity: 0.0,
            success_banner_active: false,
            success_banner_text: String::new(),
            success_banner_start_time: None,
            success_banner_opacity: 0.0,
            status_delay: 2,
            grab_mode: 0,
            selected_account_uid: None,
            bilibiliticket_list: Vec::new(),
            ticket_info: None,
            show_screen_info: None,
            selected_screen_index: None,
            selected_screen_id: None,
            selected_ticket_id: None,
            ticket_info_last_request_time: None,
            confirm_ticket_info: None,
            selected_buyer_list: None,
            local_captcha: LocalCaptcha::new(),
            show_qr_windows: None,
            announce1: None,
            announce2: None,
            announce3: None,
            announce4: None,
            machine_id: common::machine_id::get_machine_id_ob(),
            skip_words: None,
            skip_words_input: String::from(""), 

        };
        // 初始化每个账号的 client
        for account in &mut app.account_manager.accounts {
        account.ensure_client();
        
        log::debug!("为账号 {} 初始化了专属客户端", account.name);
        log::debug!("machine_id: {}", app.machine_id);

    }

    //初始化client和ua
    let random_value = generate_random_string(8);
    app.default_ua = format!(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36 Edg/134.0.0.0 {}", 
        random_value
    );
    if config["custom_config"]["enable_custom_ua"].as_bool().unwrap_or(false) && !config["custom_config"]["custom_ua"].is_null() {
        app.default_ua = config["custom_config"]["custom_ua"].as_str().unwrap_or(&app.default_ua).to_string();
        
    }
    let new_client = create_client(app.default_ua.clone());
    app.client = new_client;
        
      
    app
        
        
    }

    pub fn add_log(&mut self, message: &str) {
        self.logs.push(format!("{}", message));
        if self.logs.len() > 5000 {
            self.logs.drain(0..2500); // 删除前一半日志
        }
        // 横幅触发判定。
        
        let is_error = message.contains("] ERROR");
        
        let is_success = message.contains("成功") ;

        
        if is_error {
            self.error_banner_active = true;
            self.error_banner_text = message.to_string();
            self.error_banner_start_time = Some(std::time::Instant::now());
            self.error_banner_opacity = 1.0;
        }
        else if is_success {
            self.success_banner_active = true;
            self.success_banner_text = message.to_string();
            self.success_banner_start_time = Some(std::time::Instant::now());
            self.success_banner_opacity = 1.0;
        }
        // 普通消息不显示横幅
    }

    async fn get_policy(&mut self) -> Value {
        // 获取当前时间戳
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    
        // 构建请求数据
        let data = json!({
            "ts": timestamp,
            "machine_id": self.machine_id.clone(),
            
            
        });
    
        
        let url = format!("https://policy.nexaorion.cn/api/client/{}/{}/dispatch.json", 
                          self.app, self.version);

        match self.client.post(&url)
            .json(&data)
            .send()
            .await {
                Ok(response) => {
                    
                    match response.json::<Value>().await {
                        Ok(resp) => {
                             
                            if let Some(code) = resp["code"].as_i64() {
                                if code != 0 {
                                    log::error!("获取策略失败: {}", resp["message"]);
                                    return json!({"allow_run": true});
                                }
    
                                
                                match decode_policy(&resp["data"]["data"].as_str().unwrap_or(""), &self.public_key) {
                                    Ok(policy) => {
                                        
                                        if let Some(permission_token) = resp["data"]["permission"].as_str() {
                                            match decode_permissions(permission_token, &self.public_key) {
                                                Ok(permissions) => {
                                                    
                                                    if let Ok(mut file) = File::create("permissions") {
                                                        let _ = file.write_all(permission_token.as_bytes());
                                                    }
                                                    self.policy = Some(permissions);
                                                },
                                                Err(e) => {
                                                    log::error!("权限签名无效: {}", e);
                                                    self.policy = Some(load_local_permissions(self.public_key.clone().as_str()));
                                                }
                                            }
                                        }
                                        return policy;
                                    },
                                    Err(e) => {
                                        log::error!("策略签名无效: {}", e);
                                        return json!({"allow_run": false});
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            log::error!("解析响应失败: {}", e);
                            
                        }
                    }
                },
                Err(e) => {
                    log::error!("请求策略失败: {}", e);
                }
            }
    
        
        json!({"allow_run": true})
    }
    
    // 处理任务结果的方法
    fn process_task_results(&mut self) {
        // 获取所有可用结果
        let results = self.task_manager.get_results();
        
        // 存储需要记录的日志消息
        let mut pending_logs: Vec<String> = Vec::new();
        let mut account_updates: Vec<String> = Vec::new();
        
        for result in results {
            match result {
                
                
                //处理qrcode登录结果
                TaskResult::QrCodeLoginResult(qrcode_result) => {
                    // 二维码登录的处理逻辑
                    match qrcode_result.status {
                        common::login::QrCodeLoginStatus::Success(cookie) => {
                            log::info!("二维码登录成功!");
                            
                            
                            if let Some(cookie_str) = qrcode_result.cookie {
                                
                                self.handle_login_success(&cookie_str);
                            }
                        },
                        common::login::QrCodeLoginStatus::Failed(err) => {
                            log::error!("二维码登录失败: {}", err);
                        },
                        common::login::QrCodeLoginStatus::Expired => {
                            log::warn!("二维码已过期，请刷新");
                        },
                        _ => {
                            
                        }
                    }
                }
                TaskResult::LoginSmsResult(sms_result) => {
                    // 处理短信登录结果
                    if sms_result.success {
                        self.sms_captcha_key = sms_result.message.clone();
                        log::debug!("发送captchakey：{}",sms_result.message);
                        log::info!("短信发送成功 ");
                    } else {
                        log::error!("短信发送失败: {}", sms_result.message);
                    }
                }
                TaskResult::SubmitSmsLoginResult(submit_result) => {
                    if submit_result.success{
                        if let Some(cookie_str) = submit_result.cookie {
                            self.handle_login_success(&cookie_str);
                        }
                    } else {
                        log::error!("短信登录失败: {}", submit_result.message);
                    }
                }
                TaskResult::PushResult(push_result) => {
                    // 处理推送结果
                    if push_result.success {
                        log::info!("推送成功: {}", push_result.message);
                    } else {
                        log::error!("推送失败: {}", push_result.message);
                    }
                }
                TaskResult::GetAllorderRequestResult(order_result) => {
                    // 处理订单请求结果
                    if order_result.success {
                        self.total_order_data = Some(OrderData {
                            account_id: order_result.account_id.clone(),
                            data: order_result.order_info.clone(),
                        });
                        log::info!("账号 {} 订单请求成功", order_result.account_id);
                    } else {    
                        log::error!("账号 {} 订单请求失败", order_result.account_id);
                        
                    }
                }
                TaskResult::GetTicketInfoResult(order_result) => {
                    if order_result.success{
                        let inforesponse = match order_result.ticket_info {
                            Some(ref info) => info,
                            None => {
                                log::error!("获取project信息失败: {}", order_result.message);
                                self.is_loading = false;
                                // 保留时间戳，避免无限重试（遵循5秒延迟）
                                continue;
                            }
                        };

                        let project_info = inforesponse.data.clone();
                        let uid = order_result.uid.clone();
                        if let Some(bilibili_ticket) = self.bilibiliticket_list
                          .iter_mut()
                         .find(|ticket| ticket.uid == uid){
                            bilibili_ticket.project_info = Some(project_info.clone());
                            log::debug!("获取project信息成功: {:?}", project_info);
                         }else{
                            log::error!("未找到账号ID为 {} 的抢票对象，可能已被移除", uid);
                            self.show_screen_info = None;
                            continue;
                         }
                        
                    }else{
                        log::error!("获取project信息失败: {}", order_result.message);
                        self.is_loading = false;
                        self.error_banner_active = true;
                        self.error_banner_text = format!("获取项目信息失败: {}", order_result.message);
                        self.error_banner_start_time = Some(std::time::Instant::now());
                        self.error_banner_opacity = 1.0;
                        // 保留时间戳，避免无限重试（遵循5秒延迟）
                    }

                }
                TaskResult::GetBuyerInfoResult(get_buyerinfo_result)=>{
                    if get_buyerinfo_result.success{
                        let response = match get_buyerinfo_result.buyer_info {
                            Some(ref info) => info,
                            None => {
                                log::error!("获取购票人信息失败: {}", get_buyerinfo_result.message);
                                continue;
                            }
                        };
                        if response.errno != 0{
                            log::error!("获取购票人信息失败: {:?}", response);
                            continue;
                        }
                        let buyer_info = response.data.clone();
                        let uid = get_buyerinfo_result.uid.clone();
                        if let Some(bilibili_ticket) = self.bilibiliticket_list
                          .iter_mut()
                         .find(|ticket| ticket.uid == uid){
                            bilibili_ticket.all_buyer_info = Some(buyer_info.clone());
                            log::debug!("获取购票人信息成功: {:?}", buyer_info);
                         }else{
                            log::error!("未找到账号ID为 {} 的抢票对象，可能已被移除", uid);
                            self.show_screen_info = None;
                            continue;
                         }
                        
                    }else{
                        log::error!("获取购票人信息失败: {}", get_buyerinfo_result.message);
                        self.show_screen_info = None; 
                    }
                }
                TaskResult::GrabTicketResult(grab_ticket_result)=>{
                    if grab_ticket_result.success{
                        let pay_url = match grab_ticket_result.pay_result {
                            Some(ref data) => {
                                data.code_url.clone()
                            },
                            None => {
                                log::error!("获取支付链接失败: {}", grab_ticket_result.message);
                                continue;
                            }
                        };
                        self.show_qr_windows = Some(pay_url.clone());
                        let confirm_result = match grab_ticket_result.confirm_result{
                            Some(data) => data,
                            None => {
                                ConfirmTicketResult {
                                    project_name: "".to_string(),
                                    screen_name: "".to_string(),
                                    count: 0,
                                    pay_money: 0,
                                    ticket_info: ConfirmTicketInfo{
                                        name: "".to_string(),
                                        count: 0,
                                        price: 0,
                                    }
                                }
                            }
                        };
                        let jump_url = Some(format!("bilibili://mall/web?url=https://mall.bilibili.com/neul-next/ticket/orderDetail.html?order_id={}", grab_ticket_result.order_id.unwrap_or("".to_string())));
                        let title = format!("恭喜{}抢票成功！", confirm_result.project_name);
                        let message = format!("抢票成功！\n项目：{}\n场次：{}\n票类型：{}\n支付链接：{}\n请尽快支付{}元，以免支付超时导致票丢失\n如果觉得本项目好用，可前往https://github.com/biliticket/bili_ticket_rush 帮我们点个小星星star收藏本项目以防走丢\n本项目完全免费开源，仅供学习使用，开发组不承担使用本软件造成的一切后果",confirm_result.project_name, confirm_result.screen_name, confirm_result.ticket_info.name, pay_url ,confirm_result.ticket_info.price * confirm_result.count as i64/ 100);
                        log::info!("{}",title);
                        log::info!("{}",message);
                        //这里注释是因为推送任务已经在grab_ticket任务里提交了，由于挂后台不刷新不推送
                        if self.push_config.enabled{
                            let push_request = TaskRequest::PushRequest(PushRequest { 
                                title: title.clone(),
                                message: message.clone(),
                                push_type: PushType::All,
                                jump_url: jump_url.clone(),
                                push_config: self.push_config.clone(),

                            });
                            match self.task_manager.submit_task(push_request){
                                Ok(task_id) => {
                                    log::debug!("提交全渠道推送任务成功，任务ID: {}", task_id);
                                },
                                Err(e) => {
                                    log::error!("提交推送任务失败: {}", e);
                                }
                            }
                            
                            
                        }
                        //self.push_config.push_all(title.as_str(), message.as_str(), &jump_url,&mut *self.task_manager);
                    
                    }
                }
            }
        }
        
        // 更新账号状态
        for account_id in account_updates {
            if let Some(account) = self.account_manager.accounts.iter_mut()
                .find(|a| a.uid == account_id.parse::<i64>().unwrap_or(-1)) {
                account.account_status = "空闲".to_string();
            }
            
        }
        
        // 一次性添加所有日志，避免借用冲突
        for message in pending_logs {
            self.add_log(&message);
        }
    }

    pub fn add_log_windows(&mut self) { //从env_log添加日志进窗口
        if let Some(logs) = LOG_COLLECTOR.lock().unwrap().get_logs() {
            for log in logs {
                self.add_log(&log);
            }
        }
    }

    fn check_policy(&mut self) {
        if let Some(policy) = &self.policy {
            // 检查是否有公告信息
            if let Some(announcement) = policy.get("announcement1").and_then(|v| v.as_str()) {
                log::info!("公告: {}", announcement);
                // 可选：显示公告横幅
                self.success_banner_active = true;
                self.success_banner_text = format!("公告: {}", announcement);
                self.success_banner_start_time = Some(std::time::Instant::now());
                self.success_banner_opacity = 1.0;
                self.announce1 = Some(announcement.to_string());
            }

            if let Some(announcement) = policy.get("announcement2").and_then(|v| v.as_str()) {
                
                self.announce2 = Some(announcement.to_string());
            }

            if let Some(announcement) = policy.get("announcement3").and_then(|v| v.as_str()) {
                
                self.announce3 = Some(announcement.to_string());
            }

            if let Some(announcement) = policy.get("announcement4").and_then(|v| v.as_str()) {
                
                self.announce4 = Some(announcement.to_string());
            }
            
            // 检查是否允许运行
            let allow_run = policy.get("allow_run").and_then(|v| v.as_bool()).unwrap_or(false);
            if !allow_run {
                if let Some(accouncement) = self.announce4.clone(){
                    log::error!("公告: {}", accouncement);
                }
                log::error!("根据策略配置，当前版本不允许运行");
                // 显示错误横幅
                self.error_banner_active = true;
                self.error_banner_text = "根据策略配置，当前版本不允许运行".to_string();
                self.error_banner_start_time = Some(std::time::Instant::now());
                self.error_banner_opacity = 1.0;
                
                
                std::process::exit(1);
            }
        }
    }

    pub fn handle_login_success(&mut self, cookie: &str) {
    log::debug!("登录成功，cookie: {}", cookie);
    match add_account(cookie, &self.client,&self.default_ua){
        Ok(account) => {
            self.account_manager.accounts.push(account.clone());
            match save_config(&mut self.config, None, None, Some(account.clone())){
                Ok(_) => {
                    log::info!("登录成功，账号已添加");
                    self.show_login_windows = false;
                },
                Err(e) => {
                    log::error!("登录成功，但保存账号失败: {}", e);
                }
            }
            log::info!("登录成功，账号已添加");
        },
        Err(e) => {
            log::error!("登录成功，但添加账号失败: {}", e);
        }
    }

    }
}



impl eframe::App for Myapp{
    fn update(&mut self, ctx:&egui::Context, frame: &mut eframe::Frame){
        // egui 默认是“响应式”重绘：仅在有输入事件（鼠标/键盘）或显式 request_repaint 时才刷新。
        // 抢票任务运行在后台独立线程（TaskManagerImpl 的 worker 线程 + tokio），不受 UI 影响，
        // 即使界面静止也在正常抢票；但若不主动请求重绘，鼠标静止时界面（已运行计时、实时日志、
        // 任务列表、后台结果处理）会看起来“卡住”。这里请求一个稳定的基线重绘保证界面持续刷新。
        // 日志列表已虚拟化、任务卡片数量很少，此处低频重绘开销可忽略，不会重新引入卡顿。
        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        //侧栏
        ui::sidebar::render_sidebar(self,ctx);

        //主窗口
        egui::CentralPanel::default().show(ctx, |ui|{
            ui::tabs::render_tab_content(self, ui);
        } );


        //加载动画
        if self.is_loading{
            ui::loading::render_loading_overlay(self, ctx);
        }

        //日志
        if self.show_log_window{
            windows::log_windows::show(self, ctx);
        }

        //登录窗口
        if self.show_login_windows{
            
            windows::login_windows::show(self, ctx);
        }

        //处理异步任务结果
        self.process_task_results();

        static mut LAST_MONITOR_TIME: Option<std::time::Instant> = None;
    
        unsafe {
        let should_monitor = match LAST_MONITOR_TIME {
            Some(time) => time.elapsed() > std::time::Duration::from_secs(30),
            None => true,
        };
        
        if should_monitor {
            log::info!("资源监控 - 日志条数: {}, 任务数: {}", 
                self.logs.len(),
                self.task_manager.get_results().len());
            LAST_MONITOR_TIME = Some(std::time::Instant::now());
        }
        }

        //检查policy
        if self.policy.is_none(){
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
             let policy = self.get_policy().await;
             self.policy = Some(policy.clone());
             self.check_policy();
             self.ticket_id = policy["ticket_id"].as_str().unwrap_or("").to_string();
            });
           /*  let url = format!("https://policy.rakuyoudesu.com/api/client/{}/{}/dispatch.json",self.app,self.version);
            let rt= Runtime::new().unwrap();
            let timestamp = rt.block_on(get_now_time(&self.client));
            let data = serde_json::json!({
                "ts": timestamp,
                "machine_id": self.machine_id,
            });
            
            rt.block_on(async{
                match self.client.post(&url)
        .json(&data)
        .send()
        .await {
            Ok(response) => {
                match response.text().await {
                    Ok(text) => {
                        match serde_json::from_str::<Value>(&text) {
                            Ok(json) => {
                                log::debug!("获取policy成功: {}", json);
                                self.policy = Some(json);
                            },
                            Err(e) => {
                                log::error!("解析policy响应失败: {}", e);
                            }
                        }
                    },
                    Err(e) => {
                        log::error!("获取policy响应文本失败: {}", e);
                    }
                }
            },
            Err(e) => {
                log::error!("请求policy失败: {}", e);
            }
        }
                   

                
            })
 */
        }

        //从env_log添加日志进窗口
        self.add_log_windows();

        // 渲染错误横幅
        if self.error_banner_active {
            // 计算横幅显示时间和透明度
            if let Some(start_time) = self.error_banner_start_time {
                let elapsed = start_time.elapsed().as_secs_f32();
                
                // 横幅在屏幕上停留2秒，然后在0.5秒内淡出
                if elapsed < 4.5 {
                    // 如果超过2秒，开始淡出
                    if elapsed > 4.0 {
                        self.error_banner_opacity = 1.0 - (elapsed - 2.0) * 2.0; // 0.5秒内从1.0淡到0
                    }
                    
                    // 绘制横幅
                    render_error_banner(self, ctx);
                    
                    // 持续重绘以实现动画效果（限制为60fps）
                    ctx.request_repaint_after(std::time::Duration::from_millis(16));
                } else {
                    // 超过2.5秒，停用横幅
                    self.error_banner_active = false;
                    self.error_banner_start_time = None;
                }
            }
        }

        // 渲染成功横幅
        if self.success_banner_active {
            if let Some(start_time) = self.success_banner_start_time {
                let elapsed = start_time.elapsed().as_secs_f32();
                
                // 横幅在屏幕上停留3秒，然后在1秒内淡出
                if elapsed < 4.0 {
                    // 如果超过3秒，开始淡出
                    if elapsed > 3.0 {
                        self.success_banner_opacity = (1.0 - (elapsed - 3.0) / 1.0).max(0.0);
                    }
                    
                
                    render_error_banner(self, ctx);
                    
                    // 持续重绘以实现动画效果（限制为~60fps，避免GPU满载）
                    ctx.request_repaint_after(std::time::Duration::from_millis(16));
                } else {
                    // 超过4秒，停用横幅
                    self.success_banner_active = false;
                    self.success_banner_start_time = None;
                }
            }
        }
        

        //删除账号
        if let Some(account_id) = self.delete_account.take() {
            self.account_manager.accounts.retain(|account| account.uid != account_id.parse::<i64>().unwrap_or(-1));
            self.config.delete_account(account_id.parse::<i64>().unwrap_or(-1));
            log::info!("账号 {} 已删除", account_id);
        }

        //检测是否有cookie
        if let Some(cookie) = &self.cookie_login {
            log::info!("检测到cookie: {}", cookie);
            if let Ok(account) = add_account(cookie, &self.client,&self.default_ua) {
                self.account_manager.accounts.push(account.clone());
                match save_config(&mut self.config, None, None, Some(account.clone())){
                    Ok(_) => {
                        log::info!("cookie登录成功，账号已添加");
                    },
                    Err(e) => {
                        log::error!("cookie登录成功，但保存账号失败: {}", e);
                    }
                }
                log::info!("cookie登录成功，账号已添加");
                self.cookie_login = None; // 清空cookie
            } else {
                log::error!("cookie登录失败");
                self.cookie_login = None;
            }
        }
        

        //检测是否有更新账号开关
        if let Some(account_switch) = &self.account_switch {
            log::debug!("检测到账号开关: {}", account_switch.uid);
            if let Some(account) = self.account_manager.accounts.iter_mut().find(|a| a.uid == account_switch.uid.parse::<i64>().unwrap_or(-1)) {
                account.is_active = account_switch.switch;
                log::debug!("账号 {} 开关已更新", account_switch.uid);
            } else {
                log::error!("未找到账号 {}", account_switch.uid);
            }
            self.account_switch = None; // 清空开关
        }

        //开启添加购票人窗口？
        if let Some(account_id) = &self.show_add_buyer_window {
            if account_id == "0"{
                self.show_add_buyer_window = None;
                
            }
            else{
                windows::add_buyer::show(self, ctx, account_id.clone().as_str());
            }
            
        }

        //开启查看订单窗口？
        if let Some(uid) = &self.show_orderlist_window {
            let account_id = uid.clone().parse::<i64>().unwrap_or(0);
            if account_id == 0{
                self.show_orderlist_window = None;
                
            }
            else{
                
                let account = self.account_manager.accounts.iter_mut().find(|a| a.uid == account_id.clone()).unwrap();
                let cookie_manager = match account.cookie_manager.clone() {
                    Some(cookie_manager) => cookie_manager,
                    None => {
                        log::error!("账号 {} 的客户端未初始化", account.name);
                        self.show_orderlist_window = None;
                        return;
                    }
                };
                if self.total_order_data.is_none() {
                    self.orderlist_need_reload = true;
                   

                }else{
                    if self.total_order_data.as_ref().unwrap().account_id == uid.clone(){
                        
                    }else{
                        log::error!("账号不匹配，正在重新加载");
                        self.orderlist_need_reload = true;
                        
                        
                    }
                    
                }

                // 防止频繁请求的逻辑
                let should_request = self.orderlist_need_reload && !self.orderlist_requesting && 
                match self.orderlist_last_request_time {
                     Some(last_time) => last_time.elapsed() > std::time::Duration::from_secs(5), // 5秒冷却时间
                     None => true, // 从未请求过，允许请求
                        };
                if should_request {
                    log::debug!("提交订单请求 (冷却期已过)");
                     self.orderlist_requesting = true;  // 标记为正在请求中
                     self.orderlist_last_request_time = Some(std::time::Instant::now());
                     self.orderlist_need_reload = false;
                    submit_get_total_order(&mut self.task_manager, cookie_manager, account);
                    self.orderlist_need_reload = false;
                }
                windows::show_orderlist::show(self, ctx);
            }
            
        }


        //开启场次窗口
        if self.show_screen_info.is_some() {
            let account_id = self.show_screen_info.clone().unwrap();
            /* log::debug!("账号id:{}", account_id);
            
           
            log::debug!("当前列表长度: {}", self.bilibiliticket_list.len());
            for (i, ticket) in self.bilibiliticket_list.iter().enumerate() {
                log::debug!("列表项 #{}: uid={}", i, ticket.uid);
            } */
            
            
            if let Some(bilibili_ticket) = self.bilibiliticket_list
                .iter_mut()
                .find(|ticket| ticket.uid == account_id)
            {
                let should_request = bilibili_ticket.project_info.is_none() && match self.ticket_info_last_request_time{
                    Some(last_time) => last_time.elapsed() > std::time::Duration::from_secs(5),
                    None => true,
                };
                
                if should_request {
                    log::info!("提交获取{}project请求 ", self.ticket_id);
                    let cookie_manager = bilibili_ticket.account.cookie_manager.clone().unwrap();
                    {
                        let request = TaskRequest::GetTicketInfoRequest(GetTicketInfoRequest{
                            task_id: "".to_string(),
                            uid: bilibili_ticket.uid.clone(),
                            project_id: self.ticket_id.clone(),
                            cookie_manager: cookie_manager.clone(),
                            referer_link : bilibili_ticket.referer.clone(),
                        });
                        log::debug!("referer_link: {}", bilibili_ticket.referer.clone());
                        match self.task_manager.submit_task(request) {
                            Ok(task_id) => {
                                log::info!("提交获取project请求，任务ID: {}", task_id);
                                self.is_loading = true;
                                self.ticket_info_last_request_time = Some(std::time::Instant::now());
                                windows::screen_info::show(self, ctx, account_id);
                            },
                            Err(e) => {
                                log::error!("提交获取project请求失败: {}", e);
                            }
                        }
                    } 
                } else {
                    
                    windows::screen_info::show(self, ctx, account_id);
                }
            } else {
                
                log::error!("未找到账号ID为 {} 的抢票对象，可能已被移除", account_id);
                self.show_screen_info = None;
            }
        }


        //确认信息窗口
        if self.confirm_ticket_info.is_some() {
            let confirm_uid = match self.confirm_ticket_info.clone() {
                Some(uid) => {
                    uid.parse::<i64>().unwrap_or(0)
                }
                None => {
                    log::error!("确认信息窗口未找到账号ID，可能已被移除");
                    self.show_screen_info = None;
                    return;
                }
            };
            
            
            if let Some(bilibili_ticket) = self.bilibiliticket_list
                .iter_mut()
                .find(|ticket| ticket.uid == confirm_uid)
            {
                let mut should_request = bilibili_ticket.all_buyer_info.is_none() && match self.ticket_info_last_request_time{
                    Some(last_time) => last_time.elapsed() > std::time::Duration::from_secs(5),
                    None => true,
                };
                let mut id_bind = match bilibili_ticket.project_info.clone(){
                    Some(proj_info) => proj_info.id_bind.unwrap_or(0),
                    None => 0,
                };
                if bilibili_ticket.method == 2 {  //如果是捡漏模式，直接请求购票人信息
                    id_bind = 1;
                }
                if id_bind == 0{
                    self.is_loading = false;
                    should_request = false;
                }
                if should_request{
                    log::info!("提交获取购票人信息请求");
                    let cookie_manager = bilibili_ticket.account.cookie_manager.clone().unwrap();
                     {
                        let request = TaskRequest::GetBuyerInfoRequest(GetBuyerInfoRequest{
                            task_id: "".to_string(),
                            uid: bilibili_ticket.uid.clone(),
                            cookie_manager: cookie_manager.clone(),
                        });
                        match self.task_manager.submit_task(request) {
                            Ok(task_id) => {
                                log::info!("提交获取购票人信息请求，任务ID: {}", task_id);
                                self.is_loading = true;
                                self.ticket_info_last_request_time = Some(std::time::Instant::now());
                                
                            },
                            Err(e) => {
                                log::error!("提交获取购票人信息请求失败: {}", e);
                            }
                        }
                    } 
                }
                match bilibili_ticket.method {
                    0|1 => {
                        windows::confirm_ticket::show(self, ctx,  &confirm_uid.clone());
                    }
                    2 => {
                        windows::confirm_ticket2::show(self, ctx, &confirm_uid.clone());
                    }
                    _ => {
                        log::error!("未知的抢票方式: {}", bilibili_ticket.method);
                        self.show_screen_info = None;
                        return;
                    }
                }
                
            } else {
                log::error!("未找到账号ID为 {} 的抢票对象，可能已被移除", confirm_uid);
                self.show_screen_info = None;
            }
        }

        //扫码支付窗口
        if self.show_qr_windows.is_some() {
            windows::show_qrcode::show(self, ctx);
        }

        
    }
    

    
}


pub fn submit_get_total_order(task_manager: &mut Box<dyn TaskManager>,cookie_manager: Arc<CookieManager>, account: &Account){
    let request = TaskRequest::GetAllorderRequest(GetAllorderRequest{
        task_id: "".to_string(),
        account_id: account.uid.to_string().clone(),
        cookie_manager: cookie_manager.clone(),
        cookies: account.cookie.clone(),
        //ua: account.user_agent.clone(),
        status: TaskStatus::Pending,
        start_time: None,
    });

match task_manager.submit_task(request) {
    Ok(task_id) => {
        log::info!("订单请求提交成功，任务ID: {}", task_id);
    }
    Err(e) => {
        log::error!("查看全部订单请求提交失败：{}",e);
    }
}

}


pub fn create_client(user_agent: String) -> Client {
    let mut headers = header::HeaderMap::new();
    
    log::info!("客户端 User-Agent: {}", user_agent);
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_str(&user_agent).unwrap_or_else(|_| {
            header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        })
    );
    
    Client::builder()
        .default_headers(headers)
        .cookie_store(true)
        .build()
        .unwrap_or_default()
}

#[derive(Debug, Serialize, Deserialize)]
struct PolicyPayload {
    policy: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct PermissionsPayload {
    permissions: Value,
}


// 解码策略JWT令牌
fn decode_policy(token: &str, public_key: &str) -> Result<Value, String> {
    let decoding_key = match DecodingKey::from_rsa_pem(public_key.as_bytes()) {
        Ok(key) => key,
        Err(e) => return Err(format!("无效的公钥: {}", e)),
    };
    
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = false;
    validation.required_spec_claims.clear();
    
    match decode::<PolicyPayload>(token, &decoding_key, &validation) {
        Ok(token_data) => Ok(token_data.claims.policy),
        Err(e) => Err(format!("解码JWT失败: {}", e)),
    }
}

// 解码权限JWT令牌
fn decode_permissions(token: &str, public_key: &str) -> Result<Value, String> {
    let decoding_key = match DecodingKey::from_rsa_pem(public_key.as_bytes()) {
        Ok(key) => key,
        Err(e) => return Err(format!("无效的公钥: {}", e)),
    };
    
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = false;
    validation.required_spec_claims.clear();
    
    match decode::<PermissionsPayload>(token, &decoding_key, &validation) {
        Ok(token_data) => Ok(token_data.claims.permissions),
        Err(e) => Err(format!("解码JWT失败: {}", e)),
    }
}

// 加载本地保存的权限
fn load_local_permissions(public_key: &str) -> Value {
    match File::open("permissions") {
        Ok(mut file) => {
            let mut contents = String::new();
            if file.read_to_string(&mut contents).is_ok() {
                if let Ok(decoded) = decode_permissions(&contents, &public_key) {
                    return decoded;
                }
            }
        },
        Err(_) => {}
    }
    json!({})
}
fn generate_random_string(length: usize) -> String {
    use rand::{thread_rng, Rng};
    use rand::distributions::Alphanumeric;
    
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(|c| c as char)
        .collect()
}

pub struct AccountSwitch {
    pub uid: String,
    pub switch: bool,
}