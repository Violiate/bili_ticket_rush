use common::cookie_manager::CookieManager;
use common::http_utils::request_get;
use common::ticket::{*};
use common::gen_cp::CTokenGenerator;
use serde_json;
use common::login::QrCodeLoginStatus;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use serde_json::{json, Value};
use rand::{thread_rng, Rng};
use std::time::{SystemTime, UNIX_EPOCH};

fn cache_path(project_id: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new("cache");
    dir.join(format!("project_{}.json", project_id))
}

fn save_project_cache(project_id: &str, info: &InfoResponse) {
    let path = cache_path(project_id);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(info) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                log::warn!("保存项目缓存失败: {}", e);
            } else {
                log::info!("已缓存项目信息到 {:?}", path);
            }
        }
        Err(e) => log::warn!("序列化项目缓存失败: {}", e),
    }
}

fn load_project_cache(project_id: &str) -> Option<InfoResponse> {
    let path = cache_path(project_id);
    match std::fs::read_to_string(&path) {
        Ok(json) => {
            match serde_json::from_str::<InfoResponse>(&json) {
                Ok(info) => {
                    log::info!("已从本地缓存加载项目信息: {:?}", path);
                    Some(info)
                }
                Err(e) => {
                    log::warn!("解析本地缓存失败: {}", e);
                    None
                }
            }
        }
        Err(_) => None,
    }
}


pub async fn get_countdown(cookie_manager: Arc<CookieManager>, info: Option<TicketInfo>) -> Result<f64, String> {
    // 获取开始时间 (秒级)
    let sale_begin_sec = match info {
        Some(info) => match info.sale_begin {
            Some(v) => v,
            None => return Err("开售时间(sale_begin)为空".to_string()),
        },
        None => return Err("获取开始时间失败".to_string()),
    };
    log::debug!("获取开始时间(秒级)：{}", sale_begin_sec);
    
    // 获取网络时间 (秒级)
    let url = "https://api.bilibili.com/x/click-interface/click/now";
    let response = cookie_manager.get(url).await;
    let now_sec = match response.send().await {
        Ok(data) => {
            let text = data.text().await.unwrap_or_default();
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
        }
        Err(e) => {
            log::debug!("获取网络时间失败，原因：{}", e);
            0
        }
    };
    
    // 如果网络时间获取失败，使用本地时间 (转换为秒)
    let now_sec = if now_sec == 0 {
        log::debug!("使用本地时间");
        let local_sec = chrono::Utc::now().timestamp();
        log::debug!("本地时间(秒级)：{}", local_sec);
        local_sec
    } else {
        now_sec
    };
    
    // 计算倒计时(秒级)
    let countdown_sec = sale_begin_sec - now_sec;
    log::debug!("计算倒计时(秒)：开始时间[{}] - 当前时间[{}] = 倒计时[{}]秒", 
               sale_begin_sec, now_sec, countdown_sec);
    
    Ok(countdown_sec as f64)
}


pub async fn get_buyer_info(cookie_manager: Arc<CookieManager>) -> Result<BuyerInfoResponse,String>{
    let req = cookie_manager.get("https://show.bilibili.com/api/ticket/buyer/list").await;
    let response = req.send().await;
    match response {
        Ok(resp)=>{
            if resp.status().is_success(){
                match tokio::task::block_in_place(||{
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(resp.text())
                }){
                    Ok(text) => {
                        log::debug!("获取购票人信息：{}",text);
                        match serde_json::from_str::<BuyerInfoResponse>(&text){
                            Ok(buyer_info) => {
                                return Ok(buyer_info);
                            }
                            Err(e) => {
                                log::error!("获取购票人信息json解析失败：{}",e);
                                return Err(format!("获取购票人信息json解析失败：{}",e))
                            }

                        }
                    }
                    Err(e) => {
                        log::error!("获取购票人信息失败：{}",e);
                        return Err(format!("获取购票人信息失败：{}",e))
                    }

                }
            }
            else{
                
                log::debug!("请求响应失败: {:?}", resp);
                return Err(format!("请求响应失败: {}", resp.status()));
            }
        }
        Err(e) => {
            Err(format!("请求失败: {}", e))
        }
    }
}

pub async fn get_project(cookie_manager: Arc<CookieManager>, project_id : &str, referer_link: &str) -> Result<InfoResponse,String>{
    let replace_referer: HashMap<&str, &str> = HashMap::from([
        ("Referer", referer_link),
       
    ]);
    let req = cookie_manager.get_with_headers(format!("https://show.bilibili.com/api/ticket/project/getV2?id={}&project_id={}",project_id, project_id).as_str(),replace_referer).await;
    let response = req.send().await;
    let result = match response {
        Ok(resp)=>{
            if resp.status().is_success(){
                match tokio::task::block_in_place(||{
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(resp.text())
                }){
                    Ok(text) => {
                        log::debug!("获取项目详情：{}",text);
                        // 尝试常规解析
                        match serde_json::from_str::<InfoResponse>(&text){
                            Ok(mut ticket_info) => {
                                // 补全缺失的 sale_begin（从 screen_list 提取）
                                ticket_info.data.fill_missing_sale_begin();
                                
                                // 校验关键字段是否缺失
                                if let Err(e) = ticket_info.data.validate() {
                                    Err(e)
                                } else {
                                    Ok(ticket_info)
                                }
                            }
                            Err(e) => {
                                log::error!("获取项目详情json解析失败：{}", e);
                                Err(format!("获取项目详情json解析失败：{}", e))
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("获取项目详情失败：{}", e);
                        Err(format!("获取项目详情失败：{}", e))
                    }
                }
            }
            else{
                log::debug!("请求响应失败: {:?}", resp);
                Err(format!("请求响应失败: {}", resp.status()))
            }
        }
        Err(e) => {
            Err(format!("请求失败: {}", e))
        }
    };

    // 成功时缓存到本地，失败时尝试从缓存加载
    match result {
        Ok(info) => {
            save_project_cache(project_id, &info);
            Ok(info)
        }
        Err(e) => {
            log::warn!("在线获取项目信息失败({}), 尝试从本地缓存加载...", e);
            match load_project_cache(project_id) {
                Some(cached) => {
                    log::info!("已降级使用本地缓存的项目信息");
                    Ok(cached)
                }
                None => Err(e),
            }
        }
    }
}


//轮询登录状态
pub async fn poll_qrcode_login(qrcode_key: &str,user_agent: Option<&str>) ->QrCodeLoginStatus {
    
    
    let client_builder = Client::builder();
    let client = if let Some(ua) = user_agent {
        client_builder.user_agent(ua)
    } else {
        client_builder.user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/110.0.0.0 Safari/537.36")
    }.build()
    .unwrap_or_default();
    
    let max_attempts = 60;
    
    for attempt in 1..max_attempts{

    
    //轮询
    let response = match request_get(
        &client,
        &format!("https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={}", qrcode_key),
       
        None,
    ).await {
        Ok(resp) => resp,
        Err(e) => return QrCodeLoginStatus::Failed(e.to_string()),
    };

    let mut all_cookies = Vec::new();
    let cookie_headers = response.headers().get_all(reqwest::header::SET_COOKIE);
    for value in cookie_headers {
     if let Ok(cookie_str) = value.to_str() {
        
        if let Some(end_pos) = cookie_str.find(';') {
            all_cookies.push(cookie_str[0..end_pos].to_string());
        } else {
            all_cookies.push(cookie_str.to_string());
        }
    }
    }
    
    let json = match response.json::<serde_json::Value>().await {
        Ok(j) => j,
        Err(e) => return QrCodeLoginStatus::Failed(e.to_string()),
    };
    
    
    let code = json["data"]["code"]
        .as_i64()
        .or_else(|| json["code"].as_i64().filter(|&c| c != 0))
        .unwrap_or(-1);
    match code {
        0 => {
            //json获取cookie
            
            if let Some(cookie_info) = json["data"]["cookie_info"].as_object() {
                for (key, value) in cookie_info {
                    if let Some(val_str) = value["value"].as_str() {
                        all_cookies.push(format!("{}={}", key, val_str));
                    }
                }
            }
            
            
            if !all_cookies.is_empty() {
                return QrCodeLoginStatus::Success(all_cookies.join("; "));
            } else {
                return QrCodeLoginStatus::Failed("无法获取Cookie信息".to_string());
            }
        },
        86038 => return QrCodeLoginStatus::Expired,
        86090 => {
            log::info!("二维码已扫描，等待确认 (尝试 {} / {} 次)", attempt, max_attempts);
            //return QrCodeLoginStatus::Scanning;
        },
        86101 => {
            log::info!("二维码已生成，等待扫描 (尝试 {} / {} 次)", attempt, max_attempts);
            //return QrCodeLoginStatus::Pending
        },
        _ => {
            let message = json["data"]["message"]
                .as_str()
                .or_else(|| json["message"].as_str())
                .unwrap_or("未知错误");

            return QrCodeLoginStatus::Failed(message.to_string())
        }
    }
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
}
QrCodeLoginStatus::Expired
}


pub async fn get_ticket_token(cookie_manager:Arc<CookieManager>, 
    cpdd: Arc<Mutex<CTokenGenerator>>,
    project_id : &str , screen_id: &str, ticket_id: &str, count: i16,is_hot: bool) 
    -> Result<(String,String),TokenRiskParam>{
    
    

    let params = if is_hot {
        json!({
            "count": count,
            "screen_id": screen_id.parse::<i64>().unwrap_or(0),
            "order_type": 1,
            "project_id": project_id.parse::<i64>().unwrap_or(0),
            "sku_id": ticket_id.parse::<i64>().unwrap_or(0),
            
            
            "token": cpdd.lock().unwrap().generate_ctoken(false),
            "newRisk": true,
            "requestSource": "neul-next",
        })
    } else {
        json!({
            "count": count,
            "screen_id": screen_id.parse::<i64>().unwrap_or(0),
            "order_type": 1,
            "project_id": project_id.parse::<i64>().unwrap_or(0),
            "sku_id": ticket_id.parse::<i64>().unwrap_or(0),
            
            "token": "",
            "newRisk": true,
            "requestSource": "neul-next",
        })
    };
    log::debug!("获取票token参数：{:?}", params);
    let url = format!("https://show.bilibili.com/api/ticket/order/prepare?project_id={}",project_id);
    // 热门项目prepare 走 HTTP/2（与 createV2 同一个 h2 客户端），用桌面 Chrome 身份（默认 UA），
    let response = if is_hot {
        cookie_manager
            .post_with_headers_h2(&url, HashMap::new()).await
            .json(&params)
            .send()
            .await
    } else {
        cookie_manager
            .post(&url).await
            .json(&params)
            .send()
            .await
    };
    match response {
        Ok(resp) => {
            if resp.status().is_success(){
                match tokio::task::block_in_place(||{
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(resp.json::<serde_json::Value>())
                }){
                    Ok(json) => {
                        log::debug!("获取票token：{}",json);
                        let errno_value = json.get("errno").and_then(|v| v.as_i64()).unwrap_or(-1);
                        let code_value = json.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
                        let code = if errno_value != -1 { errno_value } else { code_value };
                        let msg = json["msg"].as_str().unwrap_or("未知错误");
                        
                        match code {
                            0 => {
                                let token = json["data"]["token"].as_str().unwrap_or("");
                                if is_hot {
                                    let ptoken = json["data"]["ptoken"].as_str().unwrap_or("");
                                    return Ok((token.to_string(), ptoken.to_string()));
                                }
                                return Ok((token.to_string(), String::new()));
                            }
                            -401 | 401 => {
                                log::info!("需要进行人机验证");
                                let mid = json["data"]["ga_data"]["riskParams"]["mid"].as_str().unwrap_or("");
                                let decision_type = json["data"]["ga_data"]["riskParams"]["decision_type"].as_str().unwrap_or("");
                                let buvid = json["data"]["ga_data"]["riskParams"]["buvid"].as_str().unwrap_or("");
                                let ip = json["data"]["ga_data"]["riskParams"]["ip"].as_str().unwrap_or("");
                                let scene = json["data"]["ga_data"]["riskParams"]["scene"].as_str().unwrap_or("");
                                let ua = json["data"]["ga_data"]["riskParams"]["ua"].as_str().unwrap_or("");
                                let v_voucher = json["data"]["ga_data"]["riskParams"]["v_voucher"].as_str().unwrap_or("");
                                let risk_param = json["data"]["ga_data"]["riskParams"].clone();
                                let token_risk_param = TokenRiskParam {
                                    code: code as i32,
                                    
                                    message: msg.to_string(),
                                    mid: Some(mid.to_string()),
                                    decision_type: Some(decision_type.to_string()),
                                    buvid: Some(buvid.to_string()),
                                    ip: Some(ip.to_string()),
                                    scene: Some(scene.to_string()),
                                    ua: Some(ua.to_string()),
                                    v_voucher: Some(v_voucher.to_string()),
                                    risk_param: Some(risk_param.clone()),
                                };
                                log::debug!("{:?}", token_risk_param);
                                return Err(token_risk_param);
                            }
                            _ => {
                                log::error!("获取token失败，未知错误码：{}，错误信息：{}，可提issue收集此问题", code, msg);
                                log::error!("{:?}", json);
                                return Err(TokenRiskParam {
                                    code: code as i32,
                                   
                                    message: msg.to_string(),
                                    mid: None,
                                    decision_type: None,
                                    buvid: None,
                                    ip: None,
                                    scene: None,
                                    ua: None,
                                    v_voucher: None,
                                    risk_param: None,
                                });
                            }
                        }
                },
                Err(e) => {
                    log::error!("解析票务token响应失败: {}", e);
                    return Err(TokenRiskParam{

                        code: 999 as i32,
                        
                        message: e.to_string(),
                        
                        mid: None,
                        decision_type: None,
                        buvid: None,
                        ip: None,
                        scene: None,
                        ua: None,
                        v_voucher: None,
                        risk_param: None,
                    })
                }
            }
            }else{
                log::error!("获取票token失败，服务器不期待响应，响应状态码：{}",resp.status());
                return Err(TokenRiskParam{
                    code: 999 as i32,
                    
                    message: resp.status().to_string(),
                    
                    mid: None,
                    decision_type: None,
                    buvid: None,
                    ip: None,
                    scene: None,
                    ua: None,
                    v_voucher: None,
                    risk_param: None,
                });
            }
        }
        Err(e) => {
            log::error!("获取票token失败，错误信息：{}",e);
            return Err(TokenRiskParam{
                code: 999 as i32,
                
                message: e.to_string(),
                
                mid: None,
                decision_type: None,
                buvid: None,
                ip: None,
                scene: None,
                ua: None,
                v_voucher: None,
                risk_param: None,
            });
        }
    }

}

pub async fn confirm_ticket_order(cookie_manager:Arc<CookieManager>,project_id : &str,token: &str) -> Result<ConfirmTicketResult, String> {
    let url = format!("https://show.bilibili.com/api/ticket/order/confirmInfo?token={}&voucher=&project_id={}&requestSource=neul-next",token,project_id);
    let response = cookie_manager.get(&url)
        .await
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;
        
    if !response.status().is_success() {
        return Err(format!("请求失败: {}", response.status()));
    }
    let text = response.text()
        .await
        .map_err(|e| format!("获取响应文本失败: {}", e))?;
    log::debug!("确认订单响应：{}", text);
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("解析响应文本失败: {}", e))?;
    if json["errno"]!=0 {
        return Err(format!("确认订单失败: {}", json["msg"].as_str().unwrap_or("未知错误")));
    }
    let confirm_result = serde_json::from_value(json["data"].clone())
        .map_err(|e| format!("解析确认订单结果失败: {}", e))?;
    Ok(confirm_result)
}

pub async fn create_order(
    cookie_manager: Arc<CookieManager>,
    cpdd: Arc<Mutex<CTokenGenerator>>,
    project_id: &str,
    token: &str,
    ptoken: &str,
    confirm_result: &ConfirmTicketResult,
    is_hot: bool,
    biliticket: &BilibiliTicket,
    buyer_info: &Vec<BuyerInfo>,
    is_mobile: bool,
    need_retry: bool,
    fast_mode: bool,
    screen_size: Option<(u32, u32)> // 可选参数：(宽度,高度)
) -> Result<Value, i32> {
    
    let ptoken = ptoken.replace('=', "");
    let url = if !is_hot {
        format!("https://show.bilibili.com/api/ticket/order/createV2?project_id={}", project_id)
    }else{
        format!("https://show.bilibili.com/api/ticket/order/createV2?project_id={}&ptoken={}", project_id, ptoken)
    };
    
    // 选择适当的位置类型
    let position_type = if need_retry && is_mobile {
        ClickPositionType::RetryButton
    } else if is_mobile {
        ClickPositionType::MobileConfirm
    } else {
        ClickPositionType::PcConfirm
    };
    
    let risk_header = format!("platform/{} uid/{} deviceId/{}"
    ,"h5"
    ,cookie_manager.get_cookie("DedeUserID").unwrap_or("".to_string())
    ,cookie_manager.get_cookie("buvid3").unwrap_or("".to_string())
    );
    let mut input_risk_header = HashMap::new();
    input_risk_header.insert("X-Risk-Header", risk_header.as_str());
    
    // 提取屏幕尺寸（如果提供）
    let (width, height) = screen_size.unwrap_or((1080, 2400));
    
    // 生成点击位置
    let click_position = random_click_position(
        position_type,
        fast_mode,
        Some(width),
        Some(height)
    ).await;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let count = confirm_result.count.clone();
    let pay_money = confirm_result.pay_money.clone();

    let ticket_id = match biliticket.select_ticket_id.clone() {
        Some(id) => id,
        None => return Err(999), 
    };
    let ticket_id_int = ticket_id.parse::<i64>().map_err(|_| 999)?;

    
    let data = match biliticket.id_bind {
        0 => {
            // 不实名制购票人信息
            let no_bind_buyer_info = match biliticket.no_bind_buyer_info.clone() {
                Some(info) => info,
                None => {
                    log::error!("非实名购票人信息缺失，无法下单");
                    return Err(999);
                }
            };
                
            let data = json!({
                "project_id": project_id.parse::<i64>().unwrap_or(0),
                "screen_id": biliticket.screen_id.parse::<i64>().unwrap_or(0),
                "sku_id": ticket_id_int, 
                "token": token,
                "buyer": no_bind_buyer_info.name,
                "tel": no_bind_buyer_info.tel,
                "clickPosition": click_position,
                "newRisk": true,
                "requestSource": if is_mobile { "neul-next" } else { "pc-new" }, 
                "deviceId": cookie_manager.get_cookie("deviceFingerprint"),
                "pay_money": pay_money,
                "count": count,
                "timestamp": timestamp,
                "order_type": 1, 
            });
            data
        }
        1 | 2 => {
            let data = if is_hot {
                json!({
                    "project_id": project_id.parse::<i64>().unwrap_or(0),
                    "screen_id": biliticket.screen_id.parse::<i64>().unwrap_or(0),
                    "sku_id": ticket_id_int,
                    "token": token,
                    "ctoken": cpdd.lock().unwrap().generate_ctoken(true),
                    "ptoken":ptoken,
                    "buyer_info": serde_json::to_string(buyer_info).unwrap_or_default(),
                    "clickPosition": click_position,
                    "newRisk": true,
                    "requestSource": if is_mobile { "neul-next" } else { "pc-new" }, 
                    "deviceId": cookie_manager.get_cookie("deviceFingerprint"),
                    "pay_money": pay_money,
                    "count": count,
                    "timestamp": timestamp,
                    "order_type": 1,
                })
            }else{
                json!({
                    "project_id": project_id.parse::<i64>().unwrap_or(0),
                    "screen_id": biliticket.screen_id.parse::<i64>().unwrap_or(0),
                    "sku_id": ticket_id_int, 
                    "token": token,
                    "buyer_info": serde_json::to_string(buyer_info).unwrap_or_default(),
                    "clickPosition": click_position,
                    "newRisk": true,
                    "requestSource": if is_mobile { "neul-next" } else { "pc-new" }, 
                    "deviceId": cookie_manager.get_cookie("deviceFingerprint"),
                    "pay_money": pay_money,
                    "count": count,
                    "timestamp": timestamp,
                    "order_type": 1, 
                })
            };
            data
        }
        _ => {
            log::error!("购票人信息错误，id_bind: {}", biliticket.id_bind);
            return Err(919); // 错误的购票人信息
        }
    };

    log::debug!("抢票data ：{:?}", data);
    let response = cookie_manager.post_with_headers_h2(&url, input_risk_header).await
        .json(&data)
        .send()
        .await
        .map_err(|e| {
            log::error!("请求失败: {}", e);
            412
        })?;
    log::debug!("createV2 实际协议版本: {:?}", response.version());
    if response.status() != 200 {
        log::error!("请求失败: {}", response.status());
        return Err(response.status().as_u16() as i32);
    };
    let text = response
        .text()
        .await
        .map_err(|e| {
            log::error!("获取响应文本失败: {}", e);
            412
        })?;
    log::info!("{}",text);
    let value: Value = serde_json::from_str(&text)
        .map_err(|e| {
            log::error!("解析响应文本失败: {}", e);
            412
        })?;
    
    let errno_value = value.get("errno").and_then(|v| v.as_i64()).unwrap_or(-1);
    let code_value = value.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    
    // 只要有一个错误码不是0，就认为有错误
    if (errno_value != 0 && errno_value != -1) || (errno_value == -1 && code_value != 0) {
        return Err(if errno_value != -1 { 
            errno_value as i32 
        } else { 
            code_value as i32 
        });
    }
    
    Ok(value)
}    

pub async fn check_fake_ticket(cookie_manager: Arc<CookieManager>, project_id: &str, pay_token: &str, order_id: i64) -> Result<Value,String>{
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32;
    let mut url = format!("https://show.bilibili.com/api/ticket/order/createstatus?project_id={}&token={}&timestamp={}",project_id, pay_token, timestamp);
    if order_id != 0{
        url = format!("{}&orderId={}",url, order_id);
    } 
    log::debug!("check_fake_ticket_url: {}", url);
    let response = cookie_manager.get(&url)
        .await
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;
    log::debug!("check_fake_ticket: {:?}", response);
    let data = serde_json::from_str::<Value>(&response.text().await.unwrap_or_default())
        .map_err(|e| format!("解析响应文本失败: {}", e))?;
    Ok(data)
}

/// 点击位置类型枚举
#[derive(Debug, Clone, Copy)]
pub enum ClickPositionType {
    /// PC端确认下单按钮位置
    PcConfirm,
    /// 手机端确认下单按钮位置
    MobileConfirm,
    /// "再试一次"按钮位置（屏幕中间）
    RetryButton,
}


/// 生成随机点击位置
/// 
/// # 参数
/// * `position_type` - 位置类型：PC/手机/重试按钮
/// * `fast_mode` - 时间间隔模式：true为快模式(0.8-4.6秒)，false为慢模式(4-12秒)
/// * `screen_width` - (可选)手机屏幕宽度，用于计算比例坐标，默认1080
/// * `screen_height` - (可选)手机屏幕高度，用于计算比例坐标，默认2400
/// 
pub async fn random_click_position(
    position_type: ClickPositionType, 
    fast_mode: bool,
    screen_width: Option<u32>,
    screen_height: Option<u32>
) -> Value {
    let mut rng = thread_rng();
    
    // 获取手机屏幕尺寸（默认使用常见尺寸1080x2400）
    let mobile_width = screen_width.unwrap_or(1080);
    let mobile_height = screen_height.unwrap_or(2400);
    
    // 根据不同设备/按钮类型确定基准坐标和偏移范围
    let (base_x, base_y, offset_range) = match position_type {
        ClickPositionType::PcConfirm => {
            // PC端确认下单按钮位置(右侧中下部)
            (1131, 636, 10)
        },
        ClickPositionType::MobileConfirm => {
            // 手机端确认下单按钮位置(右下角)
            // 使用比例计算：x在屏幕宽度的0.55-0.9之间，y在屏幕底部附近
            let x_ratio = rng.gen_range(0.55..0.9);
            let y_ratio = rng.gen_range(0.9..0.95);
            
            let x = (mobile_width as f32 * x_ratio) as i32;
            let y = (mobile_height as f32 * y_ratio) as i32;
            
            (x, y, mobile_width.min(20) as i32 / 4) // 偏移范围根据屏幕宽度按比例缩放
        },
        ClickPositionType::RetryButton => {
            // 手机版"再试一次"按钮位置(屏幕中间靠下)
            // x坐标在屏幕宽度的1/3到2/3之间，y坐标在屏幕高度的2/3左右
            let x_ratio = rng.gen_range(0.33..0.67); // 屏幕宽度的2/6到4/6之间
            let y_ratio = rng.gen_range(0.6..0.7);   // 屏幕高度的2/3左右
            
            let x = (mobile_width as f32 * x_ratio) as i32;
            let y = (mobile_height as f32 * y_ratio) as i32;
            
            (x, y, mobile_width.min(30) as i32 / 4) // 偏移较大，因为这是个大按钮
        }
    };
    
    // 生成随机偏移
    let offset_x = rng.gen_range(-offset_range..=offset_range);
    let offset_y = rng.gen_range(-offset_range..=offset_range);
    
    // 计算最终坐标
    let final_x = base_x + offset_x;
    let final_y = base_y + offset_y;
    
    // 获取当前时间戳
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    
    // 根据模式生成不同的延迟时间
    let random_delay = if fast_mode {
        // 快模式：0.8-4.6秒
        rng.gen_range(800..4600)
    } else {
        // 慢模式：4-12秒
        rng.gen_range(4000..12000)
    };
    
    // 计算起始时间
    let origin = now - random_delay;
    
    // 构建JSON对象
    json!({
        "x": final_x,
        "y": final_y,
        "origin": origin,
        "now": now
    })
}


fn extract_qrcode_poll_code(json: &Value) -> i64 {
    json["data"]["code"]
        .as_i64()
        .or_else(|| json["code"].as_i64().filter(|&c| c != 0))
        .unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qrcode_poll_uses_nested_data_code() {
        let unscanned = json!({
            "code": 0,
            "message": "0",
            "ttl": 1,
            "data": {
                "url": "",
                "refresh_token": "",
                "timestamp": 0,
                "code": 86101,
                "message": "未扫码"
            }
        });
        assert_eq!(extract_qrcode_poll_code(&unscanned), 86101);

        let scanned = json!({
            "code": 0,
            "data": { "code": 86090, "message": "二维码已扫码未确认" }
        });
        assert_eq!(extract_qrcode_poll_code(&scanned), 86090);

        let success = json!({
            "code": 0,
            "data": { "code": 0, "message": "" }
        });
        assert_eq!(extract_qrcode_poll_code(&success), 0);

        let api_err = json!({
            "code": -400,
            "message": "请求错误"
        });
        assert_eq!(extract_qrcode_poll_code(&api_err), -400);
    }
}
