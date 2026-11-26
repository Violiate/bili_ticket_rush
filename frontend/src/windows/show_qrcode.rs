use eframe::egui::{self, Color32, Image, RichText, Stroke};

use crate::app::Myapp;
use crate::windows::login_windows::create_qrcode_safe;

pub fn show(app: &mut Myapp, ctx: &egui::Context) {
    let Some(item) = app.pending_pay_qrs.front().cloned() else {
        return;
    };

    let queue_len = app.pending_pay_qrs.len();
    let mut window_open = true;
    let mut close_current = false;
    let mut next_order = false;
    let mut copied_pay_url = false;
    let mut copied_order_id = false;

    // 仅在 pay_url 变化时重新编码二维码，避免每帧重复编码与上传纹理
    let cached_matches = app
        .pay_qr_texture_cache
        .as_ref()
        .map(|(url, _)| url == &item.pay_url)
        .unwrap_or(false);

    let qr_result = if cached_matches {
        Ok(app.pay_qr_texture_cache.as_ref().unwrap().1.clone())
    } else {
        match create_qrcode_safe(ctx, "pay_qrcode", item.pay_url.as_str()) {
            Ok(tex) => {
                app.pay_qr_texture_cache = Some((item.pay_url.clone(), tex.clone()));
                Ok(tex)
            }
            Err(e) => {
                app.pay_qr_texture_cache = None;
                Err(e)
            }
        }
    };

    let waited_secs = item.created_at.elapsed().as_secs();

    let screen_rect = ctx.screen_rect();
    let max_w = (screen_rect.width() - 40.0).max(420.0);
    let max_h = (screen_rect.height() - 40.0).max(460.0);

    egui::Window::new("扫码支付")
        .open(&mut window_open)
        .resizable(true)
        .constrain(true)
        .default_size([480.0_f32.min(max_w), 580.0_f32.min(max_h)])
        .resize(|r| r.max_size([max_w, max_h]))
        .show(ctx, |ui| {
            // 多成功订单队列徽章
            if queue_len > 1 {
                ui.horizontal(|ui| {
                    egui::Frame::none()
                        .fill(Color32::from_rgb(59, 130, 246))
                        .rounding(10.0)
                        .inner_margin(egui::Margin::symmetric(10.0, 3.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("待支付队列: 1 / {}", queue_len))
                                    .color(Color32::WHITE)
                                    .strong()
                                    .size(13.0),
                            );
                        });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("下一个待支付订单")
                            .on_hover_text("将当前订单移至队尾，查看队列中的下一笔待支付订单")
                            .clicked()
                        {
                            next_order = true;
                        }
                    });
                });
                ui.add_space(6.0);
            }

            // 订单摘要卡片
            let summary_bg = if ui.visuals().dark_mode {
                Color32::from_rgb(32, 35, 42)
            } else {
                Color32::from_rgb(245, 246, 250)
            };
            egui::Frame::none()
                .fill(summary_bg)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .rounding(8.0)
                .inner_margin(10.0)
                .show(ui, |ui| {
                    egui::Grid::new("pay_qr_order_summary")
                        .num_columns(2)
                        .spacing([12.0, 5.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new("账号:").color(ui.visuals().weak_text_color()));
                            ui.label(RichText::new(&item.account_name).strong());
                            ui.end_row();

                            ui.label(RichText::new("项目:").color(ui.visuals().weak_text_color()));
                            ui.add(egui::Label::new(&item.project_name).wrap(true));
                            ui.end_row();

                            ui.label(RichText::new("场次:").color(ui.visuals().weak_text_color()));
                            ui.add(egui::Label::new(&item.screen_name).wrap(true));
                            ui.end_row();

                            ui.label(RichText::new("票种:").color(ui.visuals().weak_text_color()));
                            ui.add(
                                egui::Label::new(format!("{} × {} 张", item.ticket_name, item.count))
                                    .wrap(true),
                            );
                            ui.end_row();

                            ui.label(
                                RichText::new("实付金额:").color(ui.visuals().weak_text_color()),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "¥{:.2}",
                                    item.pay_money_fen as f64 / 100.0
                                ))
                                .strong()
                                .size(16.0)
                                .color(Color32::from_rgb(239, 68, 68)),
                            );
                            ui.end_row();

                            ui.label(
                                RichText::new("订单号:").color(ui.visuals().weak_text_color()),
                            );
                            ui.horizontal(|ui| {
                                let oid_text = if item.order_id.is_empty() {
                                    "-"
                                } else {
                                    item.order_id.as_str()
                                };
                                ui.monospace(oid_text);
                                if !item.order_id.is_empty() && ui.small_button("复制").clicked() {
                                    ui.output_mut(|o| o.copied_text = item.order_id.clone());
                                    copied_order_id = true;
                                }
                            });
                            ui.end_row();

                            ui.label(
                                RichText::new("已等待支付:").color(ui.visuals().weak_text_color()),
                            );
                            ui.label(
                                RichText::new(format_waited_secs(waited_secs)).color(
                                    if waited_secs >= 300 {
                                        Color32::from_rgb(220, 68, 55)
                                    } else {
                                        Color32::from_rgb(217, 119, 6)
                                    },
                                ),
                            );
                            ui.end_row();
                        });
                });

            ui.add_space(10.0);

            // 二维码展示区（白底边框保证暗色模式下易于扫码）
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("请使用 支付宝 / 微信 扫描二维码尽快完成支付")
                        .size(15.5)
                        .strong()
                        .color(Color32::from_rgb(59, 130, 246)),
                );
                ui.add_space(8.0);

                match &qr_result {
                    Ok(texture) => {
                        egui::Frame::none()
                            .fill(Color32::WHITE)
                            .stroke(Stroke::new(1.0, Color32::from_gray(200)))
                            .rounding(8.0)
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.add(
                                    Image::new(texture)
                                        .fit_to_exact_size(egui::vec2(240.0, 240.0)),
                                );
                            });
                    }
                    Err(err_msg) => {
                        egui::Frame::none()
                            .fill(if ui.visuals().dark_mode {
                                Color32::from_rgb(60, 24, 24)
                            } else {
                                Color32::from_rgb(254, 242, 242)
                            })
                            .stroke(Stroke::new(1.0, Color32::from_rgb(220, 68, 55)))
                            .rounding(8.0)
                            .inner_margin(12.0)
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(format!("支付二维码生成失败: {}", err_msg))
                                        .color(Color32::from_rgb(220, 68, 55)),
                                );
                                ui.label("请使用下方按钮复制支付链接在浏览器或 App 中打开。");
                            });
                    }
                }
            });

            ui.add_space(10.0);

            // 可选择的只读支付链接 + 复制按钮
            ui.horizontal(|ui| {
                ui.label(RichText::new("支付链接:").color(ui.visuals().weak_text_color()));
                let mut url_str = item.pay_url.as_str();
                ui.add(
                    egui::TextEdit::singleline(&mut url_str)
                        .desired_width((ui.available_width() - 126.0).max(120.0)),
                );
                if ui.button("复制支付链接").clicked() {
                    ui.output_mut(|o| o.copied_text = item.pay_url.clone());
                    copied_pay_url = true;
                }
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("关闭").clicked() {
                        close_current = true;
                    }
                    if queue_len > 1 && ui.button("下一个待支付订单").clicked() {
                        next_order = true;
                    }
                });
            });
        });

    if copied_pay_url {
        app.show_success("已复制支付链接");
    } else if copied_order_id {
        app.show_success("已复制订单号");
    }

    if !window_open || close_current {
        app.pending_pay_qrs.pop_front();
        app.pay_qr_texture_cache = None;
    } else if next_order && app.pending_pay_qrs.len() > 1 {
        if let Some(front) = app.pending_pay_qrs.pop_front() {
            app.pending_pay_qrs.push_back(front);
        }
        app.pay_qr_texture_cache = None;
    }
}

fn format_waited_secs(secs: u64) -> String {
    if secs < 60 {
        format!("{} 秒", secs)
    } else {
        format!("{} 分 {} 秒", secs / 60, secs % 60)
    }
}
