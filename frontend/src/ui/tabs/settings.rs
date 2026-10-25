use eframe::egui;
use crate::app::Myapp;
use common::utils::save_config;

fn on_switch(ui: &mut egui::Ui, output_char: &str, on: &mut bool) -> egui::Response {
    ui.label(
        egui::RichText::new(output_char)
              .size(15.0)                               
              .color(egui::Color32::from_rgb(0,0,0))  

              .strong()   
    );
    // 开关尺寸
    let width = 55.0;
    let height = 26.0;
    
    // 分配空间并获取响应
    let (rect, mut response) = ui.allocate_exact_size(
        egui::vec2(width, height), 
        egui::Sense::click()
    );
    
    // 处理点击
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    
    // 动画参数
    let animation_progress = ui.ctx().animate_bool(response.id, *on);
    let radius = height / 2.0;
    
    // 计算滑块位置
    let circle_x = rect.left() + radius + animation_progress * (width - height);
    
    // 绘制轨道
    ui.painter().rect_filled(
        rect.expand(-1.0), 
        radius, 
        if *on {
            egui::Color32::from_rgb(102,204,255)  // 启用状态颜色
        } else {
            egui::Color32::from_rgb(150, 150, 150)  // 禁用状态颜色
        }
    );
    
    // 绘制滑块
    ui.painter().circle_filled(
        egui::pos2(circle_x, rect.center().y),
        radius - 4.0,
        egui::Color32::WHITE
    );
    
    response
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
pub fn render(app: &mut Myapp, ui: &mut egui::Ui) {
    
    ui.horizontal(|ui|{
        ui.heading("设置");
        ui.add_space(20.0);
        let button = egui::Button::new(
            egui::RichText::new("保存设置").size(15.0).color(egui::Color32::WHITE)
            )
              .min_size(egui::vec2(100.0,35.0))
              .fill(egui::Color32::from_rgb(102,204,255))
              .rounding(15.0);//圆角成度
        let response = ui.add(button);
        if response.clicked(){
            match save_config(&mut app.config, Some(&app.push_config),Some(&app.custom_config), None){
                Ok(_) => {
                    log::info!("设置保存成功");
                },
                Err(e) => {
                    log::info!("设置保存失败: {}", e);
                }
            }
        }

    })   ; 
    
    ui.separator();
            //推送设置：
    // 创建圆角长方形框架  
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(245, 245, 250))  // 背景色
        .rounding(12.0)  // 圆角半径
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 200, 220)))  // 边框
        .inner_margin(egui::Margin { left: 10.0, right: 20.0, top: 15.0, bottom: 15.0 })  // 内边距
        .show(ui, |ui| {
            
        globle_setting(app,ui);
        ui.separator();


        });
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(245, 245, 250))  // 背景色
        .rounding(12.0)  // 圆角半径
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 200, 220)))  // 边框
        .inner_margin(egui::Margin { left: 10.0, right: 20.0, top: 15.0, bottom: 15.0 })  // 内边距
        .show(ui, |ui| {
            
            push_setting(app,ui);  // 调用推送设置
            ui.separator();

        });

        
   
}

pub fn globle_setting(app: &mut Myapp, ui: &mut egui::Ui){
    ui.horizontal(|ui| {
        
        common_input(ui, "请输入账号1预填手机号：", &mut app.custom_config.preinput_phone1, "请输入账号1绑定的手机号",true);
        common_input(ui, "请输入账号2预填手机号：", &mut app.custom_config.preinput_phone2, "请输入账号2绑定的手机号，没有可不填",true);
    });
    ui.separator();
    ui.horizontal(|ui|{
        ui.label("请选择验证码识别方式：");
         let options = ["本地识别", "ttocr识别", "选项3"];
         
        custom_selection_control(ui, &mut app.custom_config.captcha_mode, &options) ;
        match app.custom_config.captcha_mode{
            
            1 => {
                dynamic_caculate_space(ui, 300.0);
                common_input(ui, "请输入ttocr key：", &mut app.custom_config.ttocr_key, "请输入ttocr key",true);
                
                
            },
            _ => {
                
            }
        }

    });
    ui.separator();
    ui.horizontal(|ui| {
        on_switch(ui, "开启自定义UA", &mut app.custom_config.open_custom_ua);
        dynamic_caculate_space(ui, 180.0);
        common_input(ui, "", &mut app.custom_config.custom_ua, "请输入自定义UA",false);

    });
    
    
    
    

}

fn custom_selection_control(ui: &mut egui::Ui, selected: &mut usize, options: &[&str]) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        for (idx, option) in options.iter().enumerate() {
            let is_selected = *selected == idx;
            let button = egui::Button::new(
                egui::RichText::new(*option)
                    .size(15.0)
                    .color(if is_selected { egui::Color32::WHITE } else { egui::Color32::BLACK })
            )
            .min_size(egui::vec2(80.0, 30.0))
            .fill(if is_selected { 
                egui::Color32::from_rgb(102, 204, 255) 
            } else { 
                egui::Color32::from_rgb(245, 245, 250)
            })
            .rounding(10.0);
                
            if ui.add(button).clicked() {
                *selected = idx;
                changed = true;
            }
        }
    });
    changed
}
pub fn push_setting(app: &mut Myapp, ui: &mut egui::Ui){
    //推送开关
            
            // 开关
            ui.horizontal(|ui| {
                
                
                on_switch(ui, "开启推送",&mut app.push_config.enabled);
                let available = ui.available_width();
                ui.add_space(available-100.0);
                
                let button = egui::Button::new(
                    egui::RichText::new("测试推送").size(15.0).color(egui::Color32::WHITE)
                    )
                      .min_size(egui::vec2(100.0,40.0))
                      .fill(egui::Color32::from_rgb(102,204,255))
                      .rounding(15.0);//圆角成度
                let response = ui.add(button);
                if response.clicked(){
                    app.push_config.push_all("biliticket推送测试", "这是一个推送测试", &None,&mut *app.task_manager);
                }
                  

            });
            if app.push_config.enabled{
            ui.separator();
            

            
            
            //推送设置
            ui.horizontal(|ui|{
                 
                common_input(ui, "bark推送：",&mut app.push_config.bark_token,"请输入推送地址，只填token，token后可用?附加参数",false);
                dynamic_caculate_space(ui, 180.0);
                common_input(ui, "pushplus推送：",&mut app.push_config.pushplus_token,"请输入推送地址，只填token",true);
                });
                //TODO补充每个推送方式使用方法

            ui.horizontal(|ui|{
                 
                common_input(ui, "方糖推送：",&mut app.push_config.fangtang_token,"请输入推送地址：SCTxxxxxxx",true);
                dynamic_caculate_space(ui, 180.0);
                common_input(ui, "钉钉机器人推送：",&mut app.push_config.dingtalk_token,"请输入钉钉机器人token，只填token",true);
                });

            ui.horizontal(|ui|{
                common_input(ui, "企业微信推送：",&mut app.push_config.wechat_token,"请输入企业微信机器人token",true);
                dynamic_caculate_space(ui, 180.0);

                });
            ui.horizontal(|ui|{
                common_input(ui, "smtp服务器地址：",&mut app.push_config.smtp_config.smtp_server,"请输入smtp服务器地址",true);
                dynamic_caculate_space(ui, 180.0);
                common_input(ui, "smtp服务器端口：",&mut app.push_config.smtp_config.smtp_port,"请输入smtp服务器端口",true);
                
            });
            ui.horizontal(|ui|{
                
                common_input(ui, "邮箱账号：",&mut app.push_config.smtp_config.smtp_username,"请输入发件人邮箱",true);
                dynamic_caculate_space(ui, 180.0);
                common_input(ui, "授权密码：",&mut app.push_config.smtp_config.smtp_password,"请输入授权密码",true);
                dynamic_caculate_space(ui, 180.0);
                
            });
            ui.horizontal(|ui|{
                
                
                
                common_input(ui, "发件人邮箱：",&mut app.push_config.smtp_config.smtp_from,"请输入发件人邮箱",true);
                dynamic_caculate_space(ui, 180.0);
                common_input(ui, "收件人邮箱：",&mut app.push_config.smtp_config.smtp_to,"请输入收件人邮箱",true);
                
            });
            ui.horizontal(|ui| {
                common_input(ui, "gotify地址：",&mut app.push_config.gotify_config.gotify_url,"请输入gotify服务器地址，只填写地址",false);
                dynamic_caculate_space(ui, 180.0);
                common_input(ui, "gotify的token", &mut app.push_config.gotify_config.gotify_token, "请输入gotify的token", true)
            });
        }
        
}
pub fn dynamic_caculate_space(ui :&mut egui::Ui, next_obj_space: f32) {
    let available_space = ui.available_width();
    let mut space = available_space - next_obj_space - 250.0;
    if space < 0.0 {
        space = 0.0;
    }
    ui.add_space(space);
}


