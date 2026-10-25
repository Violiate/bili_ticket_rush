use crate::app::{Myapp, OrderData};
use eframe::egui::{self, RichText};
use egui::{Image, TextureHandle};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use common::{cookie_manager::CookieManager, utils::load_texture_from_url};

pub fn show(
    app: &mut Myapp,
    ctx: &egui::Context,

){
    let mut window_open = app.show_orderlist_window.is_some();

    let orders_data = match &app.total_order_data {
        Some(data) => {app.is_loading = false; data.clone()},
        None => {app.is_loading = true; return;},
    };


    // 显示窗口和订单数据
    egui::Window::new("订单列表")
        .open(&mut window_open)
        .default_height(600.0)
        .default_width(800.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("订单列表")
                    .size(20.0)
                    .color(egui::Color32::from_rgb(0, 0, 0))
                    .strong()
                );
            });

            // 添加滚动区域
            egui::ScrollArea::vertical().show(ui, |ui| {
                // 使用从内存中获取的orders_data
                if let Some(order_data) = &orders_data.data {
                    // 显示订单数据
                    for order in &order_data.data.list {
                        ui.add_space(12.0);

                        egui::Frame::none()
                            .fill(ui.style().visuals.widgets.noninteractive.bg_fill)
                            .rounding(8.0)
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(220, 220, 220)))
                            .shadow(egui::epaint::Shadow {
                                extrusion: 2.0,
                                color: egui::Color32::from_black_alpha(20),
                            })
                            .inner_margin(egui::vec2(12.0, 12.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // 图片处理
                                    let image_size = egui::vec2(80.0, 80.0);
                                    // 处理URL格式：如果以//开头，添加https:前缀
                                    let image_url = if order.img.url.starts_with("//") {
                                        format!("https:{}", order.img.url)
                                    } else {
                                        order.img.url.clone()
                                    };

                                    // 图片加载逻辑
                                    ui.add_sized(image_size, |ui: &mut egui::Ui| {
                                        if let Some(texture) = get_image_texture(ctx, &image_url) {
                                            ui.centered_and_justified(|ui| {
                                                ui.add(Image::new(&texture).fit_to_exact_size(image_size))
                                            }).inner
                                        } else {
                                            let inner_response = ui.centered_and_justified(|ui| {
                                                ui.label("图片加载中...")
                                            });
                                            // log::debug!("开始加载图片: {}", image_url);
                                            request_image_async(ctx.clone(), app,image_url);
                                            inner_response.inner
                                        }
                                    });

                                    ui.add_space(12.0);

                                    // 订单信息区域
                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            // 活动名称
                                            ui.label(RichText::new(&order.item_info.name).size(16.0).strong());

                                            // 订单状态
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                // 根据订单状态设置不同颜色
                                                let status_color = match order.status {
                                                    2 => egui::Color32::from_rgb(0, 150, 0),   // 已完成/已付款
                                                    4 => egui::Color32::from_rgb(200, 80, 0),  // 已取消
                                                    _ => egui::Color32::from_rgb(100, 100, 100),
                                                };
                                                ui.label(RichText::new(&order.sub_status_name)
                                                    .color(status_color)
                                                    .strong());
                                            });
                                        });

                                        ui.add_space(4.0);

                                        // 订单详细信息
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("订单号:").color(egui::Color32::GRAY));
                                            ui.monospace(&order.order_id);
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("场次:").color(egui::Color32::GRAY));
                                            ui.label(&order.item_info.screen_name);
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("下单时间:").color(egui::Color32::GRAY));
                                            ui.label(&order.ctime);
                                        });

                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("价格:").color(egui::Color32::GRAY));
                                            // 将分转换为元并格式化为价格
                                            let price_text = format!("¥{:.2}", order.pay_money as f64 / 100.0);
                                            ui.label(RichText::new(price_text).strong());

                                            // 显示支付方式（如果已支付）
                                            let pay_channel = match order.pay_channel {
                                                Some(ref channel) => channel.clone(),
                                                None => "".to_string(),
                                            };
                                            if !pay_channel.is_empty() {
                                                ui.add_space(8.0);
                                                ui.label(format!("(支付方式：{})", pay_channel));
                                            }

                                            // 操作按钮放在右侧
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                // 根据订单状态决定是否显示不同按钮
                                                if order.status == 2 {  // 已完成
                                                    /* let button = egui::Button::new(
                                                        egui::RichText::new("查看详情").size(16.0).color(egui::Color32::WHITE)
                                                    )
                                                    .min_size(egui::vec2(100.0, 36.0))
                                                    .fill(egui::Color32::from_rgb(102, 204, 255))
                                                    .rounding(18.0);

                                                    let response = ui.add(button);
                                                    if response.clicked() {
                                                        log::debug!("查看订单详情: {}", order.order_id);
                                                        // 处理点击事件
                                                    } */
                                                } else if order.status == 1 && order.sub_status == 1 {  // 待付款
                                                    let pay_button = egui::Button::new(
                                                        egui::RichText::new("未支付").size(16.0).color(egui::Color32::WHITE)
                                                    )
                                                    .min_size(egui::vec2(80.0, 36.0))
                                                    .fill(egui::Color32::from_rgb(250, 100, 0))
                                                    .rounding(18.0);

                                                    if ui.add(pay_button).clicked() {
                                                        log::info!("暂不支持支付订单: {}", order.order_id);
                                                        // 添加支付逻辑
                                                    }
                                                }
                                            });
                                        });
                                    });
                                });
                            });
                    }

                    // 如果没有订单
                    if order_data.data.list.is_empty() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(50.0);
                            ui.label(RichText::new("暂无订单记录").size(16.0).color(egui::Color32::GRAY));
                        });
                    }
                } else {
                    // 显示加载中状态
                    ui.vertical_centered(|ui| {
                        ui.add_space(50.0);
                        ui.label(RichText::new("加载中...").size(16.0).color(egui::Color32::GRAY));
                    });
                }

                ui.add_space(10.0); // 底部留白
            });

        });

    if !window_open {
        app.show_orderlist_window = None;
        app.orderlist_requesting = false;
        app.orderlist_need_reload = true;
    }
}

// 辅助函数：从缓存获取图片纹理
fn get_image_texture(ctx: &egui::Context, url: &str) -> Option<TextureHandle> {

    ctx.memory(|mem| {
        // log::debug!("{:?}", mem.data);
        mem.data.get_temp::<TextureHandle>(egui::Id::new(url))
    })
}

// 辅助函数：异步请求图片
fn request_image_async(ctx: egui::Context,app:&Myapp,url: String) {
    if ctx.memory(|mem| mem.data.get_temp::<TextureHandle>(egui::Id::new(&url)).is_some()){
        log::error!("图片已存在: {}", url);
        return;
    }
    // 避免重复请求
    if ctx.memory(|mem| mem.data.get_temp::<bool>(egui::Id::new(format!("loading_{}", url))).is_some()) {
        return;
    }

    // 标记为正在加载
    log::debug!("<正在加载图片>: {}", url);
    if app.account_manager.accounts.is_empty() {
        log::error!("没有可用账号，无法加载图片");
        return;
    }
    ctx.memory_mut(|mem| mem.data.insert_temp(egui::Id::new(format!("loading_{}", url)), true));
    
    // 启动异步加载线程
    let app_client =match  app.account_manager.accounts[0].cookie_manager.clone(){
        Some(client) => client,
        None => {
            log::error!("cookie_manager不存在");
            let rt = tokio::runtime::Runtime::new().unwrap();
           
            Arc::new(rt.block_on(CookieManager::new("", None, 0)))
        }
    };
    let app_ua = app.default_ua.clone();
    
        // 这里应该实现实际的图片加载逻辑
        // 示例：
    std::thread::spawn(move || {
        if let Some(texture)=load_texture_from_url(&ctx, app_client, &(url.clone()+"@74w_74h.jpeg"),  &url){
            ctx.memory_mut(|mem| {
                log::debug!("加载图片成功: {}", url);
                mem.data.insert_temp(egui::Id::new(&url), texture);
                mem.data.remove::<bool>(egui::Id::new(format!("loading_{}", url)));
            });
            ctx.request_repaint();
        }else{
            ctx.memory_mut(|mem| {
                log::warn!("加载图片失败_ui,retrying: {}", url);
                mem.data.remove::<bool>(egui::Id::new(format!("loading_{}", url)));
            });
        }

        });


}
