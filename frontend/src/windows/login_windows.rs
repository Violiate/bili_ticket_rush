use std::time::Instant;

use common::taskmanager::{
    GenerateQrCodeRequest, LoginSmsRequest, SubmitLoginSmsRequest, TaskRequest,
};
use common::utils::MAX_ACCOUNTS;
use eframe::egui::{self, Color32, Image, RichText, Stroke, TextureHandle};
use image::Luma;
use qrcode::QrCode;

use crate::app::{normalize_fullwidth_ascii, Myapp};
use crate::windows::{bounded_window_sizes, show_resizable_body_with};

pub struct LoginTexture {
    pub left_conrner_texture: Option<egui::TextureHandle>,
    pub right_conrner_texture: Option<egui::TextureHandle>,
}

const CORNER_LOAD_ATTEMPTED_KEY: &str = "login_corner_textures_attempted";

pub fn show(app: &mut Myapp, ctx: &egui::Context) {
    let mut window_open = app.show_login_windows;
    if !window_open {
        return;
    }

    // 仅尝试解码一次装饰角标，避免解码失败时每帧重试
    let attempted = ctx.memory(|mem| {
        mem.data
            .get_temp::<bool>(egui::Id::new(CORNER_LOAD_ATTEMPTED_KEY))
            .unwrap_or(false)
    });
    if !attempted {
        ctx.memory_mut(|mem| {
            mem.data
                .insert_temp(egui::Id::new(CORNER_LOAD_ATTEMPTED_KEY), true);
        });
        if app.login_texture.left_conrner_texture.is_none() {
            app.login_texture.left_conrner_texture =
                load_image_from_base64(ctx, "left_corner", get_left_base64());
        }
        if app.login_texture.right_conrner_texture.is_none() {
            app.login_texture.right_conrner_texture =
                load_image_from_base64(ctx, "right_corner", get_right_base64());
        }
    }

    let (default_sz, min_sz, max_sz) = bounded_window_sizes(ctx, 620.0, 450.0, 480.0, 360.0);

    egui::Window::new("登录窗口")
        .open(&mut window_open)
        .default_size(default_sz)
        .min_width(min_sz[0])
        .min_height(min_sz[1])
        .resize(|r| r.max_size(max_sz))
        .constrain(true)
        .resizable(true)
        .show(ctx, |ui| {
            let window_rect = ui.available_rect_before_wrap();

            // 先在窗口底层绘制左下/右下装饰图，确保缩放窗口时绝不会遮挡正文、输入框和按钮
            let scale_factor = 0.55;
            if let Some(texture) = &app.login_texture.left_conrner_texture {
                let scaled_size = texture.size_vec2() * scale_factor;
                let image_pos = egui::pos2(window_rect.min.x, window_rect.max.y - scaled_size.y);
                let image_rect = egui::Rect::from_min_size(image_pos, scaled_size);
                ui.painter().image(
                    texture.id(),
                    image_rect,
                    egui::Rect::from_min_max([0.0, 0.0].into(), [1.0, 1.0].into()),
                    Color32::from_white_alpha(180),
                );
            }
            if let Some(texture) = &app.login_texture.right_conrner_texture {
                let scaled_size = texture.size_vec2() * scale_factor;
                let image_pos = egui::pos2(
                    window_rect.max.x - scaled_size.x,
                    window_rect.max.y - scaled_size.y,
                );
                let image_rect = egui::Rect::from_min_size(image_pos, scaled_size);
                ui.painter().image(
                    texture.id(),
                    image_rect,
                    egui::Rect::from_min_max([0.0, 0.0].into(), [1.0, 1.0].into()),
                    Color32::from_white_alpha(180),
                );
            }

            show_resizable_body_with(
                ui,
                "login_window_scroll",
                app,
                |ui, app| {
                    // 居中对称的登录方式切换栏（无硬编码偏移）
                    let tabs = [
                        ("扫码登录", "扫码登录"),
                        ("短信登录", "短信登录"),
                        ("Cookie登录", "ck登录"),
                        ("密码登录", "密码登录"),
                    ];
                    ui.columns(tabs.len(), |cols| {
                        for (i, (label, method_key)) in tabs.iter().enumerate() {
                            cols[i].vertical_centered(|ui| {
                                let is_selected = app.login_method == *method_key;
                                let mut text = RichText::new(*label).size(16.0);
                                if is_selected {
                                    text = text.strong().color(Color32::from_rgb(59, 130, 246));
                                }
                                if ui.selectable_label(is_selected, text).clicked() {
                                    if app.login_method != *method_key {
                                        if *method_key != "扫码登录" {
                                            stop_qr_login_tasks(app);
                                        }
                                        app.login_method = (*method_key).to_string();
                                    }
                                }
                            });
                        }
                    });

                    ui.separator();
                    if app.account_manager.accounts.len() >= MAX_ACCOUNTS {
                        ui.add_space(6.0);
                        egui::Frame::none()
                            .fill(if ui.visuals().dark_mode {
                                Color32::from_rgb(64, 48, 22)
                            } else {
                                Color32::from_rgb(255, 247, 230)
                            })
                            .stroke(Stroke::new(1.0, Color32::from_rgb(217, 119, 6)))
                            .rounding(6.0)
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(format!(
                                            "🛡 当前已达防黄牛上限（最多 {} 个账号）：仅允许重新登录已有账号刷新凭证，新增其他账号将被拦截。",
                                            MAX_ACCOUNTS
                                        ))
                                        .size(12.5)
                                        .color(if ui.visuals().dark_mode {
                                            Color32::from_rgb(255, 206, 120)
                                        } else {
                                            Color32::from_rgb(166, 90, 10)
                                        }),
                                    )
                                    .wrap(true),
                                );
                            });
                    }
                    ui.add_space(8.0);
                },
                |_, _| {},
                |ui, app| {
                    // 直接按当前 login_method 映射到轻量 Copy 枚举，避免借用冲突与 unreachable!()
                    #[derive(Clone, Copy)]
                    enum LoginTab {
                        QrCode,
                        Sms,
                        Cookie,
                        Password,
                    }
                    let active_tab = match app.login_method.as_str() {
                        "扫码登录" => LoginTab::QrCode,
                        "短信登录" => LoginTab::Sms,
                        "ck登录" | "Cookie登录" => LoginTab::Cookie,
                        "密码登录" => LoginTab::Password,
                        _ => {
                            app.login_method = "扫码登录".to_string();
                            LoginTab::QrCode
                        }
                    };
                    match active_tab {
                        LoginTab::QrCode => ui_qrcode_login(ui, app),
                        LoginTab::Sms => ui_sms_login(ui, app),
                        LoginTab::Cookie => ui_ck_login(ui, app),
                        LoginTab::Password => ui_password_login(ui, app),
                    }
                },
            );
        });

    if !window_open {
        app.show_login_windows = false;
        app.reset_login_state();
    }
}

fn stop_qr_login_tasks(app: &mut Myapp) {
    if let Some(task_id) = app.qrcode_polling_task_id.take() {
        let _ = app.task_manager.cancel_task(&task_id);
    }
    app.login_qr_gen_task_id = None;
    app.login_qr_requesting = false;
    app.login_qrcode_url = None;
    app.login_qr_texture_cache = None;
    app.login_qr_error = None;
    app.login_qr_status.clear();
}

fn trigger_qr_refresh(app: &mut Myapp) {
    if let Some(task_id) = app.qrcode_polling_task_id.take() {
        let _ = app.task_manager.cancel_task(&task_id);
    }
    app.login_qr_gen_task_id = None;
    app.login_qrcode_url = None;
    app.login_qr_texture_cache = None;
    app.login_qr_error = None;

    let req = TaskRequest::GenerateQrCodeRequest(GenerateQrCodeRequest {
        client: app.client.clone(),
    });
    match app.task_manager.submit_task(req) {
        Ok(task_id) => {
            app.login_qr_gen_task_id = Some(task_id);
            app.login_qr_requesting = true;
            app.login_qr_status = "正在获取登录二维码...".to_string();
        }
        Err(e) => {
            app.login_qr_gen_task_id = None;
            app.login_qr_requesting = false;
            app.login_qr_error = Some(format!("提交二维码生成任务失败: {}", e));
            app.login_qr_status = "获取二维码失败".to_string();
            log::error!("提交二维码生成任务失败: {}", e);
        }
    }
}

fn ui_qrcode_login(ui: &mut egui::Ui, app: &mut Myapp) {
    // 首次进入扫码 tab 自动异步申请二维码（无错误、未请求、未正在登录提交时）
    if app.login_qrcode_url.is_none()
        && !app.login_qr_requesting
        && app.login_qr_error.is_none()
        && !app.login_submitting
    {
        trigger_qr_refresh(app);
    }

    ui.vertical_centered(|ui| {
        let refresh_btn = egui::Button::new(
            RichText::new("刷新二维码")
                .size(14.5)
                .color(Color32::WHITE),
        )
        .min_size(egui::vec2(140.0, 34.0))
        .fill(Color32::from_rgb(59, 130, 246))
        .rounding(10.0);

        if ui
            .add_enabled(
                !app.login_qr_requesting && !app.login_submitting,
                refresh_btn,
            )
            .clicked()
        {
            trigger_qr_refresh(app);
        }

        ui.add_space(12.0);

        // 状态提示
        if !app.login_qr_status.is_empty() {
            ui.label(RichText::new(&app.login_qr_status).size(14.0));
            ui.add_space(8.0);
        }

        // 错误态：内联展示 + 重试按钮，不触发离线无限重试循环
        if let Some(err_msg) = app.login_qr_error.clone() {
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
                    ui.add(
                        egui::Label::new(
                            RichText::new(err_msg).color(Color32::from_rgb(220, 68, 55)),
                        )
                        .wrap(true),
                    );
                    ui.add_space(8.0);
                    if ui.button("重试").clicked() {
                        trigger_qr_refresh(app);
                    }
                });
            return;
        }

        // 请求二维码或扫码确认后拉取账号中
        if app.login_qr_requesting || app.login_submitting {
            ui.add_space(28.0);
            ui.spinner();
            ui.add_space(10.0);
            let wait_text = if app.login_submitting {
                "正在验证登录并获取账号信息..."
            } else {
                "正在获取登录二维码..."
            };
            ui.label(wait_text);
            return;
        }

        // 使用纹理缓存渲染二维码（仅在 URL 变化时编码一次）
        if let Some(qr_url) = app.login_qrcode_url.clone() {
            let cache_hit = app
                .login_qr_texture_cache
                .as_ref()
                .map(|(cached_url, _)| cached_url == &qr_url)
                .unwrap_or(false);

            if !cache_hit {
                match create_qrcode_safe(ui.ctx(), "login_qrcode", &qr_url) {
                    Ok(tex) => {
                        app.login_qr_texture_cache = Some((qr_url, tex));
                    }
                    Err(e) => {
                        app.login_qr_error = Some(e);
                        app.login_qr_texture_cache = None;
                    }
                }
            }

            if let Some((_, ref texture)) = app.login_qr_texture_cache {
                egui::Frame::none()
                    .fill(Color32::WHITE)
                    .stroke(Stroke::new(1.0, Color32::from_gray(200)))
                    .rounding(8.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.add(Image::new(texture).fit_to_exact_size(egui::vec2(200.0, 200.0)));
                    });
            }
        }
    });
}

fn ui_password_login(ui: &mut egui::Ui, app: &mut Myapp) {
    ui.vertical_centered(|ui| {
        ui.add_space(18.0);
        egui::Frame::none()
            .fill(if ui.visuals().dark_mode {
                Color32::from_rgb(36, 40, 48)
            } else {
                Color32::from_rgb(245, 247, 252)
            })
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .rounding(8.0)
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.set_max_width(420.0);
                ui.label(RichText::new("暂不支持账号密码直接登录").size(16.0).strong());
                ui.add_space(8.0);
                ui.add(
                    egui::Label::new(
                        "由于哔哩哔哩 Web 端安全策略，账号密码登录暂不可用。为保护账号安全，请改用以下任一官方登录方式：",
                    )
                    .wrap(true),
                );
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui.button("扫码登录（推荐）").clicked() {
                        app.login_method = "扫码登录".to_string();
                    }
                    if ui.button("短信验证码登录").clicked() {
                        app.login_method = "短信登录".to_string();
                    }
                    if ui.button("Cookie 登录").clicked() {
                        app.login_method = "ck登录".to_string();
                    }
                });
            });
    });
}

fn ui_sms_login(ui: &mut egui::Ui, app: &mut Myapp) {
    let cooldown_secs = app.sms_cooldown_until.and_then(|until| {
        let now = Instant::now();
        if until > now {
            Some((until - now).as_secs() + 1)
        } else {
            None
        }
    });
    if cooldown_secs.is_none() {
        app.sms_cooldown_until = None;
    }

    let mut send_sms_clicked = false;
    let mut submit_sms_clicked = false;
    let ime_composing = app.is_ime_composing();

    ui.vertical_centered(|ui| {
        ui.add_space(8.0);

        egui::Grid::new("sms_login_form_grid")
            .num_columns(2)
            .spacing([12.0, 14.0])
            .show(ui, |ui| {
                ui.label(RichText::new("手机号:").size(15.0));
                ui.horizontal(|ui| {
                    let phone_resp = ui.add(
                        egui::TextEdit::singleline(&mut app.login_input.phone)
                            .hint_text("请输入 11 位手机号")
                            .desired_width(180.0),
                    );
                    if (phone_resp.changed() || phone_resp.lost_focus())
                        && (!ime_composing || phone_resp.lost_focus())
                    {
                        app.login_input.phone = normalize_fullwidth_ascii(&app.login_input.phone)
                            .chars()
                            .filter(|c| c.is_ascii_digit())
                            .take(15)
                            .collect();
                    }

                    let sms_btn_label = if app.sms_sending {
                        "发送中...".to_string()
                    } else if let Some(secs) = cooldown_secs {
                        format!("{}s 后重试", secs)
                    } else {
                        "获取验证码".to_string()
                    };
                    let can_send =
                        !app.sms_sending && cooldown_secs.is_none() && !app.login_submitting;
                    if ui
                        .add_enabled(can_send, egui::Button::new(sms_btn_label))
                        .clicked()
                    {
                        send_sms_clicked = true;
                    }
                });
                ui.end_row();

                ui.label(RichText::new("验证码:").size(15.0));
                let code_resp = ui.add(
                    egui::TextEdit::singleline(&mut app.login_input.sms_code)
                        .hint_text("请输入短信验证码")
                        .desired_width(180.0),
                );
                if (code_resp.changed() || code_resp.lost_focus())
                    && (!ime_composing || code_resp.lost_focus())
                {
                    app.login_input.sms_code = normalize_fullwidth_ascii(&app.login_input.sms_code)
                        .chars()
                        .filter(|c| c.is_ascii_digit())
                        .take(8)
                        .collect();
                }
                ui.end_row();
            });

        if app.sms_captcha_key.is_empty() {
            ui.add_space(4.0);
            ui.label(
                RichText::new("请先输入手机号并点击「获取验证码」")
                    .size(12.5)
                    .color(ui.visuals().weak_text_color()),
            );
        }

        ui.add_space(16.0);

        let login_label = if app.login_submitting {
            "登录中..."
        } else {
            "登录"
        };
        let login_btn = egui::Button::new(
            RichText::new(login_label).size(15.0).color(Color32::WHITE),
        )
        .min_size(egui::vec2(180.0, 38.0))
        .fill(Color32::from_rgb(0, 174, 236))
        .rounding(12.0);

        if ui
            .add_enabled(!app.login_submitting && !app.sms_sending, login_btn)
            .clicked()
        {
            submit_sms_clicked = true;
        }
    });

    if send_sms_clicked {
        let phone: String = normalize_fullwidth_ascii(&app.login_input.phone)
            .chars()
            .filter(|c| c.is_ascii_digit())
            .take(15)
            .collect();
        app.login_input.phone = phone.clone();
        if !is_valid_sms_phone(&phone) {
            app.show_error("请输入有效的数字手机号（11 位手机号）");
        } else {
            app.sms_sending = true;
            let sms_req = LoginSmsRequest {
                phone,
                client: app.client.clone(),
                custom_config: app.custom_config.clone(),
                local_captcha: app.local_captcha.clone(),
            };
            match app
                .task_manager
                .submit_task(TaskRequest::LoginSmsRequest(sms_req))
            {
                Ok(_) => {
                    log::info!("短信验证码发送中...");
                }
                Err(e) => {
                    app.sms_sending = false;
                    log::error!("提交短信任务失败: {}", e);
                    app.show_error(format!("提交短信任务失败: {}", e));
                }
            }
        }
    }

    if submit_sms_clicked {
        let phone: String = normalize_fullwidth_ascii(&app.login_input.phone)
            .chars()
            .filter(|c| c.is_ascii_digit())
            .take(15)
            .collect();
        let code: String = normalize_fullwidth_ascii(&app.login_input.sms_code)
            .chars()
            .filter(|c| c.is_ascii_digit())
            .take(8)
            .collect();
        app.login_input.phone = phone.clone();
        app.login_input.sms_code = code.clone();
        let captcha_key = app.sms_captcha_key.trim().to_string();

        if !is_valid_sms_phone(&phone) {
            app.show_error("请输入有效的数字手机号");
        } else if captcha_key.is_empty() {
            app.show_error("请先获取短信验证码后再登录");
        } else if code.len() < 4 || !code.bytes().all(|b| b.is_ascii_digit()) {
            app.show_error("请输入有效的数字短信验证码");
        } else {
            app.login_submitting = true;
            let req = SubmitLoginSmsRequest {
                phone,
                code,
                captcha_key,
                client: app.client.clone(),
            };
            match app
                .task_manager
                .submit_task(TaskRequest::SubmitLoginSmsRequest(req))
            {
                Ok(_) => {
                    log::info!("短信验证码登录中...");
                }
                Err(e) => {
                    app.login_submitting = false;
                    log::error!("提交短信登录任务失败: {}", e);
                    app.show_error(format!("提交短信登录任务失败: {}", e));
                }
            }
        }
    }
}

fn ui_ck_login(ui: &mut egui::Ui, app: &mut Myapp) {
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("粘贴 B站完整 Cookie").size(15.0).strong());
        ui.add_space(6.0);

        let cookie_box_w = (ui.available_width() - 24.0).clamp(240.0, 460.0);
        egui::ScrollArea::vertical()
            .id_source("cookie_login_input_scroll")
            .max_height(120.0)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut app.login_input.cookie)
                        .hint_text("请粘贴完整 Cookie（需包含 SESSDATA、bili_jct 等字段）")
                        .desired_rows(4)
                        .desired_width(cookie_box_w),
                );
            });

        ui.add_space(16.0);

        let btn_label = if app.login_submitting {
            "验证登录中..."
        } else {
            "登录"
        };
        let button = egui::Button::new(RichText::new(btn_label).size(15.0).color(Color32::WHITE))
            .min_size(egui::vec2(180.0, 38.0))
            .fill(Color32::from_rgb(0, 174, 236))
            .rounding(12.0);

        if ui.add_enabled(!app.login_submitting, button).clicked() {
            let trimmed = app.login_input.cookie.trim().to_string();
            if trimmed.is_empty() {
                app.show_error("Cookie 不能为空，请粘贴有效 Cookie");
            } else {
                app.login_submitting = true;
                app.cookie_login = Some(trimmed);
            }
        }
    });
}

fn is_valid_sms_phone(phone: &str) -> bool {
    (7..=15).contains(&phone.len()) && phone.bytes().all(|b| b.is_ascii_digit())
}

/// 将内容编码为二维码 ColorImage（无 panic，可供离线单元测试直接验证）
pub(crate) fn render_qrcode_color_image(content: &str) -> Result<egui::ColorImage, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("二维码内容不能为空".to_string());
    }

    let qr_code =
        QrCode::new(trimmed.as_bytes()).map_err(|e| format!("生成二维码失败: {}", e))?;
    let qr_image = qr_code.render::<Luma<u8>>().build();

    let width = qr_image.width() as usize;
    let height = qr_image.height() as usize;
    let mut color_image = egui::ColorImage::new([width, height], Color32::WHITE);

    for y in 0..height {
        for x in 0..width {
            let pixel = qr_image.get_pixel(x as u32, y as u32);
            color_image.pixels[y * width + x] = if pixel[0] == 0 {
                Color32::BLACK
            } else {
                Color32::WHITE
            };
        }
    }

    Ok(color_image)
}

/// 安全生成二维码纹理（不 panic，使用 NEAREST 采样保持缩放清晰）
pub fn create_qrcode_safe(
    ctx: &egui::Context,
    texture_name: &str,
    content: &str,
) -> Result<TextureHandle, String> {
    let color_image = render_qrcode_color_image(content)?;
    Ok(ctx.load_texture(
        texture_name,
        color_image,
        egui::TextureOptions::NEAREST,
    ))
}

fn load_image_from_base64(
    ctx: &egui::Context,
    name: &str,
    base64_data: &str,
) -> Option<egui::TextureHandle> {
    let base64_decoded = match base64::decode(base64_data) {
        Ok(data) => data,
        Err(err) => {
            log::warn!("解码内置装饰图失败 ({}): {}", name, err);
            return None;
        }
    };

    let image = match image::load_from_memory(&base64_decoded) {
        Ok(img) => img,
        Err(err) => {
            log::warn!("加载内置装饰图失败 ({}): {}", name, err);
            return None;
        }
    };

    let image_buffer = image.to_rgba8();
    let size = [image_buffer.width() as _, image_buffer.height() as _];
    let pixels = image_buffer.into_vec();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);

    Some(ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR))
}

#[cfg(test)]
mod tests {
    use super::render_qrcode_color_image;

    #[test]
    fn qrcode_rendering_succeeds_for_url_and_fails_gracefully_for_empty_or_oversized() {
        let ok_img = render_qrcode_color_image("https://passport.bilibili.com/h5-app/passport/login/scan?qrcode_key=test123")
            .expect("valid URL should encode to QR image");
        assert!(ok_img.size[0] > 0 && ok_img.size[1] > 0);
        assert_eq!(ok_img.pixels.len(), ok_img.size[0] * ok_img.size[1]);

        assert!(render_qrcode_color_image("").is_err());
        assert!(render_qrcode_color_image("   ").is_err());

        // 超出 QR 最大容量的内容应返回 Err 而绝不 panic
        let oversized = "A".repeat(10_000);
        assert!(render_qrcode_color_image(&oversized).is_err());
    }
}

fn get_left_base64() -> &'static str{
    "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"
}

fn get_right_base64() -> &'static str{
    "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"
}