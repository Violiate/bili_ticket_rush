use eframe::egui;
use crate::{app::{AccountSwitch, Myapp}};
use common::{account::{signout_account, Account}, cookie_manager::{self, CookieManager}};
use common::utils::load_texture_from_url;
use std::sync::Arc;

pub fn render(app: &mut Myapp, ui: &mut egui::Ui){
    ui.heading("我的账户");
    ui.separator();
    let example_account = Account{
        uid: 0,
        name: "请登录账号".to_string(),
        vip_label: "未登录，请登录账号".to_string(),
        level: "未登录".to_string(),
        cookie: "".to_string(),
        csrf: "".to_string(),
        is_login: false,
        account_status: "未登录".to_string(),
        is_active: false,
        avatar_url: None,
        
        avatar_texture:None,
        
        cookie_manager: None,
    };


    // 加载默认头像
    load_default_avatar(ui.ctx(),app);

    if app.account_manager.accounts.is_empty() {
        if let Some(texture) = app.default_avatar_texture.clone() {
            show_user(
                ui,
                &texture,
                &example_account,
                &mut app.delete_account,
                &mut app.show_login_windows ,
                &mut app.config,
                &mut app.account_switch,
                &mut app.show_add_buyer_window,
                &mut app.show_orderlist_window,
                );
        }
    } else {
        let account_count = app.account_manager.accounts.len();
        for index in 0..account_count {
            let cookie_manager = app.account_manager.accounts[index].cookie_manager.clone();
            let avatar_texture = load_user_avatar(ui.ctx(), cookie_manager, &mut app.account_manager.accounts[index]);
            let texture = avatar_texture.or_else(|| app.default_avatar_texture.clone());
            if let Some(texture) = texture {
                show_user(
                    ui,
                    &texture,
                    &app.account_manager.accounts[index],
                    &mut app.delete_account,
                    &mut app.show_login_windows ,
                    &mut app.config,
                    &mut app.account_switch,
                    &mut app.show_add_buyer_window,
                    &mut app.show_orderlist_window,
                    );
            }
            ui.separator();
        }
        if account_count < 2 {
            if let Some(texture) = app.default_avatar_texture.clone() {
                show_user(
                    ui,
                    &texture,
                    &example_account,
                    &mut app.delete_account,
                    &mut app.show_login_windows ,
                    &mut app.config,
                    &mut app.account_switch,
                    &mut app.show_add_buyer_window,
                    &mut app.show_orderlist_window,
                    );
                ui.separator();
            }
        }
        let account_count = 0;
        for index in 0..account_count {
            let cookie_manager = app.account_manager.accounts[index].cookie_manager.clone();
            let avatar_texture = load_user_avatar(ui.ctx(), cookie_manager, &mut app.account_manager.accounts[index]);
            let texture = avatar_texture.or_else(|| app.default_avatar_texture.clone());
            if let Some(texture) = texture {
                show_user(
                    ui,
                    &texture,
                    &app.account_manager.accounts[index],
                    &mut app.delete_account,
                    &mut app.show_login_windows ,
                    &mut app.config,
                    &mut app.account_switch,
                    &mut app.show_add_buyer_window,
                    &mut app.show_orderlist_window,
                    );
            }
            ui.separator();
        }
    }
ui.separator();



}
/// 将任意图片显示为圆形
/// - texture: 要显示的图像纹理
/// - size: 圆形图片的直径大小
fn draw_user_avatar(ui: &mut egui::Ui, texture: &egui::TextureHandle, size: f32) -> egui::Response {
    // 分配正方形区域
    let (rect, response) = ui.allocate_exact_size(
        egui::Vec2::new(size, size),
        egui::Sense::click()
    );

    if ui.is_rect_visible(rect) {
        // 创建一个离屏渲染的自定义形状层
        let layer_id = egui::layers::LayerId::new(
            egui::layers::Order::Background,
            egui::Id::new("circular_image")
        );

        let painter = ui.ctx().layer_painter(layer_id);

        // 绘制圆形背景 (这一步可选)
        painter.circle_filled(
            rect.center(),
            size / 2.0,
            egui::Color32::from_rgb(220, 220, 240)
        );

        // 使用圆形纹理蒙版技术
        // 1. 创建一个与图像大小相同的圆形遮罩
        let circle_mask = egui::Shape::circle_filled(
            rect.center(),
            size / 2.0 - 1.0,
            egui::Color32::WHITE
        );

        // 2. 将图像绘制为自定义着色器，使用圆形遮罩
        let uv = egui::Rect::from_min_max(
            egui::pos2(0.0, 0.0),
            egui::pos2(1.0, 1.0)
        );

        // 使用裁剪圆绘制
        painter.add(circle_mask);

        // 以混合模式绘制图像，只在圆形区域内可见
        painter.image(
            texture.id(),
            rect,
            uv,
            egui::Color32::WHITE
        );

        // 添加边框
        painter.circle_stroke(
            rect.center(),
            size / 2.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(180, 180, 180, 180))
        );
    }

    response
}

fn load_user_avatar(ctx: &egui::Context, cookie_manager: Option<Arc<CookieManager>>, account: &mut Account) ->Option<egui::TextureHandle> {
    // 如果用户已登录且提供了头像路径，尝试加载
    if let Some(texture) = &account.avatar_texture {
        return Some(texture.clone());
    }
    if account.is_login && cookie_manager.is_some() {
        if let Some(avatar_url) = &account.avatar_url {
            // 尝试加载用户头像
            let texture_name = format!("user_avatar_{}", account.uid);
            let texture_option = load_texture_from_url(ctx, cookie_manager.unwrap(), avatar_url, &texture_name);

            account.avatar_texture = texture_option.clone();
            return texture_option;
        }
    }
    //未登录或加载失败
    None
}
// 加载默认头像
fn load_default_avatar(ctx: &egui::Context, app: &mut Myapp) {
    if app.default_avatar_texture.is_some() {
        return;
    }
    // 使用include_bytes!宏将图片直接嵌入到二进制文件中
    // 路径是相对于项目根目录的
    let default_avatar_bytes = include_bytes!("../../../assets/default_avatar.jpg");

    // 从内存中加载图片
    match image::load_from_memory(default_avatar_bytes) {
        Ok(image) => {
            let size = [image.width() as usize, image.height() as usize];
            let image_buffer = image.to_rgba8();
            let pixels = image_buffer.as_flat_samples();

            app.default_avatar_texture = Some(ctx.load_texture(
                "default_avatar",
                egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()),
                Default::default()
            ));
        },
        Err(_) => {
            // 图片加载失败，生成占位符头像
            app.default_avatar_texture = generate_placeholder_avatar(ctx);
        }
    }
}

// 生成一个占位符头像
fn generate_placeholder_avatar(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let size = 128; // 头像尺寸
    let mut image_data = vec![0; size * size * 4];

    // 生成一个简单的渐变图案
    for y in 0..size {
        for x in 0..size {
            let i = (y * size + x) * 4;
            // 浅蓝色调渐变
            image_data[i] = 180; // R
            image_data[i + 1] = 180 + (y as u8) / 2; // G
            image_data[i + 2] = 230; // B
            image_data[i + 3] = 255; // A
        }
    }

    Some(ctx.load_texture(
        "default_avatar",
        egui::ColorImage::from_rgba_unmultiplied([size, size], &image_data),
        Default::default()
    ))
}

fn show_user( //显示用户头像等信息
    ui: &mut egui::Ui,
    texture: &egui::TextureHandle,

    account: &Account,
    delete_account: &mut Option<String>,
    show_login_windows: &mut bool,
    config: &mut common::utils::Config,
    account_switch: &mut Option<AccountSwitch>,
    show_add_buyer_window: &mut Option<String>,
    show_orderlist_window: &mut Option<String>,


) {
    let mut user = account.clone();
    // 创建圆角长方形框架
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(245, 245, 250))  // 背景色
        .rounding(12.0)  // 圆角半径
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 200, 220)))  // 边框
        .inner_margin(egui::Margin { left: 10.0, right: 20.0, top: 15.0, bottom: 15.0 })  // 内边距
        .show(ui, |ui| {
            // 水平布局放置图片和文字
            ui.horizontal(|ui| {
                // 左侧图片区域，这里使用小尺寸的圆形图片
                let image_size = 84.0;
                draw_user_avatar(ui, texture, image_size);

                ui.add_space(12.0);  // 图片和文字之间的间距

                // 右侧文字区域
                ui.vertical(|ui| {
                    //第一行
                    //显示uid和昵称
                    ui.horizontal(|ui|{
                        ui.add(egui::widgets::Label::new(
                            egui::RichText::new(&user.name)
                                .size(30.0)
                                .strong()
                                .color(egui::Color32::from_rgb(60, 60, 80))
                        ));
                        ui.add_space(15.0);
                        ui.add(egui::widgets::Label::new(
                            egui::RichText::new(format!("UID: {}", user.uid))
                                .color(egui::Color32::from_rgb(100, 100, 120))
                                .size(16.0)
                        ));
                    });
                    //第二行
                    ui.add_space(10.0);
                    //显示大会员
                    ui.horizontal(|ui|{

                        match user.vip_label.as_str(){
                            "月度大会员"=> {
                                egui::Frame::none()
                                .fill(egui::Color32::from_rgb(251, 114, 153)) // 粉色背景 #FB7299
                                .rounding(10.0)  // 圆角

                                .inner_margin(egui::vec2(6.0, 3.0))  // 内边距
                                .show(ui, |ui| {
                                    // 白色文字 #FFFFFF
                                        ui.label(
                                              egui::RichText::new("月度大会员")
                                              .color(egui::Color32::from_rgb(255, 255, 255))
                                                // 白色文字
                                              .size(15.0)
                                                    );
                                 });
                            }
                            "年度大会员" =>{
                                egui::Frame::none()
                                .fill(egui::Color32::from_rgb(251, 114, 153))  // 粉色背景 #FB7299
                                .rounding(10.0)  // 圆角
                                .inner_margin(egui::vec2(6.0, 3.0))  // 内边距
                                .show(ui, |ui| {
                                    // 白色文字 #FFFFFF
                                        ui.label(
                                              egui::RichText::new("年度大会员")
                                              .color(egui::Color32::from_rgb(255, 255, 255))  // 白色文字
                                              .size(15.0)
                                                    );
                                 });
                            }
                            "十年大会员" =>{
                                egui::Frame::none()
                                .fill(egui::Color32::from_rgb(251, 114, 153))  // 粉色背景 #FB7299
                                .rounding(10.0)  // 圆角
                                .inner_margin(egui::vec2(6.0, 3.0))  // 内边距
                                .show(ui, |ui| {
                                    // 白色文字 #FFFFFF
                                        ui.label(
                                              egui::RichText::new("十年大会员")
                                              .color(egui::Color32::from_rgb(255, 255, 255))  // 白色文字
                                              .size(15.0)
                                                    );
                                 });
                            }
                            _ => {
                                egui::Frame::none()

                                .inner_margin(egui::vec2(6.0, 3.0))  // 内边距
                                .show(ui, |ui| {
                                    // 白色文字rgb(0, 0, 0)
                                        ui.label(
                                              egui::RichText::new("正式会员")
                                              .color(egui::Color32::from_rgb(0, 0, 0))  // 白色文字
                                              .size(15.0)
                                                    );
                                 });
                            }



                        }
                    })

                });


            });
            ui.separator();
            ui.vertical(|ui|{
                ui.add_space(15.0);
                ui.horizontal(|ui|{
                    ui.add_space(15.0);
                    if !user.is_login {


                    let button = egui::Button::new(
                      egui::RichText::new("登录").size(20.0).color(egui::Color32::WHITE)
                      )
                        .min_size(egui::vec2(120.0,50.0))
                        .fill(egui::Color32::from_rgb(102,204,255))
                        .rounding(15.0);//圆角成度
                    let response = ui.add(button);
                    if response.clicked(){
                        *show_login_windows = true;
                    }
                }else{
                    let button = egui::Button::new(
                      egui::RichText::new("登出").size(20.0).color(egui::Color32::WHITE)
                      )
                        .min_size(egui::vec2(120.0,50.0))
                        .fill(egui::Color32::from_rgb(255,174,201))
                        .rounding(15.0);//圆角成度
                    let response = ui.add(button);
                    if response.clicked(){
                        match signout_account(&user){
                            Ok(_) => {
                                *delete_account = Some(user.uid.to_string().clone());
                                log::info!("登出成功");

                            }
                            Err(e) => {
                                log::error!("登出失败: {}", e);
                            }
                        }

                    }
                }
                    dynamic_caculate_space(ui, 122.0, 3.0);
                    let button = egui::Button::new(
                        egui::RichText::new("查看全部订单").size(20.0).color(egui::Color32::WHITE)
                        )
                          .min_size(egui::vec2(130.0,50.0))
                          .fill(egui::Color32::from_rgb(102,204,255))
                          .rounding(15.0);//圆角成度
                    let response =   ui.add(button);
                    if response.clicked(){
                        *show_orderlist_window = Some(user.uid.to_string().clone());
                    }
                    dynamic_caculate_space(ui, 120.0, 2.0);
                    let button = egui::Button::new(
                        egui::RichText::new("添加购票人").size(18.0).color(egui::Color32::WHITE)
                        )
                          .min_size(egui::vec2(120.0,50.0))
                          .fill(egui::Color32::from_rgb(102,204,255))
                          .rounding(15.0);
                    let response = ui.add(button);
                    if response.clicked(){
                        *show_add_buyer_window = Some(user.uid.to_string().clone());
                    }
                    dynamic_caculate_space(ui, 120.0, 1.0);

                    if user.is_active == false{
                            let button = egui::Button::new(
                              egui::RichText::new("抢票关闭中").size(18.0).color(egui::Color32::WHITE)
                              )
                                .min_size(egui::vec2(120.0,50.0))
                                .fill(egui::Color32::from_rgb(255,174,201))
                                .rounding(15.0);
                            let response = ui.add(button);
                            if response.clicked(){
                                let switch = AccountSwitch{
                                    uid: user.uid.to_string(),
                                    switch: true,
                                };
                                *account_switch = Some(switch);
                            }
                    }else{
                            let button = egui::Button::new(
                              egui::RichText::new("抢票开启中").size(18.0).color(egui::Color32::WHITE)
                              )
                                .min_size(egui::vec2(120.0,50.0))
                                .fill(egui::Color32::from_rgb(102,204,255))
                                .rounding(15.0);
                            let response = ui.add(button);
                            if response.clicked(){
                                let switch = AccountSwitch{
                                    uid: user.uid.to_string(),
                                    switch: false,
                                };
                                *account_switch = Some(switch);
                            }
                        }

                    });



            })
        });
}



fn dynamic_caculate_space(
     ui :&mut egui::Ui,
     obj_space: f32, //如果有三个按钮，假设每个按钮尺寸x轴长度=120.0，那么就传入120.0
     number: f32 //按钮数量
    ) {
    let available_space = ui.available_width();
    let mut space = available_space/number - obj_space ;
    if space < 0.0 {
        space = 0.0;
    }
    ui.add_space(space);
}

