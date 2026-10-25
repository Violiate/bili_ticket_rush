use crate::app::Myapp;
use eframe::egui::{self, RichText};
use serde_json::Value;

pub struct AddBuyerInput{
    pub name: String,
    pub phone: String,
    pub id_type: usize,
    pub id_number: String,
    pub as_default_buyer: bool,

}
pub fn show(app: &mut Myapp, ctx: &egui::Context, uid: &str) {
    let find_account = app.account_manager.accounts.iter().find(|account| account.uid.to_string() == uid);
    let select_account = match find_account {
        Some(account) => account,
        None => return,
    };
    let select_cookie_manager = match select_account.cookie_manager.clone() {
        Some(select_cookie_manager) => select_cookie_manager,
        None => {
            log::error!("账号 {} 的客户端未初始化", select_account.uid);
            app.error_banner_active = true;
            app.error_banner_text = format!("账号 {} 的客户端未初始化，请重新登录", select_account.uid);
            app.error_banner_start_time = Some(std::time::Instant::now());
            app.error_banner_opacity = 1.0;
            app.show_add_buyer_window = None;
            return;
        }
    };
    let mut window_open = app.show_add_buyer_window.is_some();
    
    egui::Window::new("添加购票人")
        .open(&mut window_open)
        .default_size([700.0, 400.0])
        .resizable(false)
        .show(ctx, |ui| {

            ui.vertical_centered(|ui|{            
                ui.label(RichText::new("添加购票人")
                .size(20.0)                               
                .color(egui::Color32::from_rgb(0,0,0))  
                .strong()   
            );
        });
            ui.add_space(20.0);
            ui.horizontal(|ui|{
                ui.add_space(8.0);
                common_input(ui, "姓名：", &mut app.add_buyer_input.name, "请输入你的真实姓名", false);
                
                
            });
            ui.add_space(20.0);
            ui.horizontal(|ui|{
                ui.add_space(8.0);
                common_input(ui, "手机号：", &mut app.add_buyer_input.phone, "请输入你的手机号", true);
                ui.add_space(20.0);
                
            });

            // 添加证件类型选择器
            ui.add_space(20.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("证件类型：")
                        .size(15.0)                               
                        .color(egui::Color32::from_rgb(0,0,0))  
                );
                ui.add_space(8.0);
                
                // 调用证件类型选择器
                id_type_selector(ui, &mut app.add_buyer_input.id_type);
            });
            
            // 添加证件号码输入
            ui.add_space(20.0);
            ui.horizontal(|ui|{
                ui.add_space(8.0);
                common_input(ui, "证件号码：", &mut app.add_buyer_input.id_number, get_id_hint(app.add_buyer_input.id_type), true);
            });
            
            // 添加默认购票人选项
            ui.add_space(20.0);
            ui.horizontal(|ui|{
                ui.add_space(8.0);
                ui.checkbox(&mut app.add_buyer_input.as_default_buyer, "设为默认购票人");
            });
            
            
            //确保空间大小合适
            ui.add_space(30.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    
                });
            });

            ui.vertical_centered(|ui|{
                // 创建按钮
                let button = egui::Button::new(
                    egui::RichText::new("保存").size(20.0).color(egui::Color32::WHITE)
                )
                .min_size(egui::vec2(120.0, 50.0))
                .fill(egui::Color32::from_rgb(102,204,255))
                .rounding(20.0);
                let response = ui.add(button);
                if response.clicked() {
                    let mut json_form = serde_json::Map::new();
                    json_form.insert("name".to_string(), serde_json::Value::String(app.add_buyer_input.name.clone()));
                    json_form.insert("tel".to_string(), serde_json::Value::String(app.add_buyer_input.phone.clone()));
                    json_form.insert("id_type".to_string(), serde_json::Value::Number(serde_json::Number::from(app.add_buyer_input.id_type)));
                    json_form.insert("personal_id".to_string(), serde_json::Value::String(app.add_buyer_input.id_number.clone()));
                    json_form.insert("is_default".to_string(), serde_json::Value::String(check_default(app.add_buyer_input.as_default_buyer).to_string()));
                    json_form.insert("src".to_string(), serde_json::Value::String("ticket".to_string()));
                    
                            

                    log::debug!("添加购票人数据: {:?}", json_form);
                    log::debug!("账号ck: {:?}", select_account.cookie.as_str());
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    let response = rt.block_on(async{
                        select_cookie_manager.post(
                            "https://show.bilibili.com/api/ticket/buyer/create",

                        ).await
                        .json(&json_form)
                        .send()
                        .await
                        .unwrap()
                    });
                    
                    if !response.status().is_success() {
                        log::error!("添加购票人失败: {:?}", response.status());
                        return;
                    }

                    let response_text = match rt.block_on(response.text()) {
                        Ok(text) => text,
                        Err(e) => {
                            log::error!("获取响应文本失败: {}", e);
                            return;
                        }
                    };

                    let json_value: Result<Value, _> = serde_json::from_str(&response_text);
                    let response_json_value = match json_value {
                        Ok(val) => val,
                        Err(e) => {
                            log::error!("解析JSON失败! 原因: {}, 响应原文: {}", e, response_text);
                            return;
                        }
                    };
                    let errno_value = response_json_value.get("errno").and_then(|v| v.as_i64()).unwrap_or(-1);
                    let code_value = response_json_value.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
                    let code = if errno_value != -1{
                        errno_value
                    } else {
                        code_value
                    };
                    if code == 0 {
                        log::info!("添加购票人成功: {:?}", response_text);
                        app.show_add_buyer_window = None;
                        // 重置表单
                        app.add_buyer_input = AddBuyerInput {
                            name: String::new(),
                            phone: String::new(),
                            id_type: 0,
                            id_number: String::new(),
                            as_default_buyer: false,
                        };
                    } else {
                        log::error!("添加购票人失败: {:?}", response_text);
                    }
                    

                    

                    
                }
            })

        });

        //更新窗口状态
        if !window_open {
            app.show_add_buyer_window = None;
        }
}

pub fn common_input(
    ui: &mut egui::Ui, 
    title: &str,
    text: &mut String,
    hint: &str,
    open_filter: bool,


) -> bool{
    ui.label(
        egui::RichText::new(title)
              .size(15.0)                               
              .color(egui::Color32::from_rgb(0,0,0))  

              
    );
    ui.add_space(8.0);
    let input = egui::TextEdit::singleline( text)
                .hint_text(hint)//提示
                .desired_rows(1)//限制1行       
                .min_size(egui::vec2(120.0, 35.0));
                
                
    let response = ui.add(input);
    if response.changed(){
        if open_filter{
            *text = text.chars()//过滤非法字符
            .filter(|c| c.is_ascii_alphanumeric() || *c == '@' || *c == '.' || *c == '-' || *c == '_')
            .collect();
        }
        else{
            *text = text.chars()//过滤非法字符
            .collect();
        };
            
    }
    response.changed()

}

// 证件类型的名称和值
const ID_TYPES: [(&str, usize); 4] = [
    ("身份证", 0),
    ("护照", 1),
    ("港澳居民往来内地通行证", 2),
    ("台湾居民往来大陆通行证", 3),
];

fn check_default(is_default: bool) -> &'static str {
    if is_default {
        "1"
    } else {
        "0"
    }
}
fn id_type_selector(ui: &mut egui::Ui, selected_type: &mut usize) {
    ui.horizontal(|ui| {
        for (name, value) in ID_TYPES.iter() {
            let is_selected = *selected_type == *value;
            
            // 创建更美观的选择按钮
            let button = egui::Button::new(
                RichText::new(*name)
                    .size(14.0)
                    .color(
                        if is_selected { 
                            egui::Color32::WHITE 
                        } else { 
                            egui::Color32::from_rgb(60, 60, 60) 
                        }
                    )
                )
                .min_size(egui::vec2(0.0, 32.0))
                .fill(
                    if is_selected { 
                        egui::Color32::from_rgb(102, 204, 255) 
                    } else { 
                        egui::Color32::from_rgb(240, 240, 240) 
                    }
                )
                .rounding(5.0);
            
            if ui.add(button).clicked() {
                *selected_type = *value;
            }
            
            ui.add_space(8.0); // 按钮之间的间距
        }
    });
}

// 根据证件类型获取不同的提示文字
fn get_id_hint(id_type: usize) -> &'static str {
    match id_type {
        0 => "请输入18位身份证号码",
        1 => "请输入护照号码",
        2 => "请输入港澳通行证号码",
        3 => "请输入台湾通行证号码",
        _ => "请输入证件号码",
    }
}
