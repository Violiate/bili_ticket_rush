use eframe::egui;

use crate::app::{decode_image_to_color_image, AccountSwitch, ImageLoadState, Myapp};
use common::account::Account;
use common::utils::MAX_ACCOUNTS;

struct AccountPalette {
    card_fill: egui::Color32,
    card_border: egui::Color32,
    text_primary: egui::Color32,
    text_secondary: egui::Color32,
    avatar_bg: egui::Color32,
    avatar_border: egui::Color32,
    normal_member_bg: egui::Color32,
    normal_member_text: egui::Color32,
    accent: egui::Color32,
    danger: egui::Color32,
    success: egui::Color32,
    inactive_btn: egui::Color32,
    vip_pink: egui::Color32,
    init_badge: egui::Color32,
}

impl AccountPalette {
    fn from_dark_mode(dark_mode: bool) -> Self {
        if dark_mode {
            Self {
                card_fill: egui::Color32::from_rgb(36, 38, 46),
                card_border: egui::Color32::from_rgb(62, 66, 80),
                text_primary: egui::Color32::from_rgb(235, 238, 245),
                text_secondary: egui::Color32::from_rgb(165, 172, 188),
                avatar_bg: egui::Color32::from_rgb(52, 56, 68),
                avatar_border: egui::Color32::from_rgb(96, 104, 124),
                normal_member_bg: egui::Color32::from_rgb(54, 58, 72),
                normal_member_text: egui::Color32::from_rgb(210, 215, 228),
                accent: egui::Color32::from_rgb(45, 136, 245),
                danger: egui::Color32::from_rgb(220, 76, 76),
                success: egui::Color32::from_rgb(46, 160, 67),
                inactive_btn: egui::Color32::from_rgb(122, 112, 128),
                vip_pink: egui::Color32::from_rgb(251, 114, 153),
                init_badge: egui::Color32::from_rgb(218, 145, 38),
            }
        } else {
            Self {
                card_fill: egui::Color32::from_rgb(248, 249, 252),
                card_border: egui::Color32::from_rgb(214, 218, 230),
                text_primary: egui::Color32::from_rgb(35, 40, 52),
                text_secondary: egui::Color32::from_rgb(100, 108, 124),
                avatar_bg: egui::Color32::from_rgb(230, 234, 244),
                avatar_border: egui::Color32::from_rgb(180, 188, 206),
                normal_member_bg: egui::Color32::from_rgb(232, 236, 246),
                normal_member_text: egui::Color32::from_rgb(65, 72, 90),
                accent: egui::Color32::from_rgb(45, 136, 245),
                danger: egui::Color32::from_rgb(224, 78, 78),
                success: egui::Color32::from_rgb(46, 160, 67),
                inactive_btn: egui::Color32::from_rgb(142, 134, 148),
                vip_pink: egui::Color32::from_rgb(251, 114, 153),
                init_badge: egui::Color32::from_rgb(224, 146, 32),
            }
        }
    }
}

pub fn render(app: &mut Myapp, ui: &mut egui::Ui) {
    let dark_mode = ui.ctx().style().visuals.dark_mode;
    let palette = AccountPalette::from_dark_mode(dark_mode);

    // 仅首次解码嵌入默认头像（带圆形抗锯齿遮罩）
    load_default_avatar(ui.ctx(), app);

    let total_accounts = app.account_manager.accounts.len();
    let active_accounts = app
        .account_manager
        .accounts
        .iter()
        .filter(|a| a.is_active)
        .count();

    // 顶部标题栏：标题 + 账号数量统计 + 防黄牛 2 账号上限控制的「+ 添加账号」按钮
    let can_add_account = total_accounts < MAX_ACCOUNTS;
    ui.horizontal(|ui| {
        ui.heading(
            egui::RichText::new("我的账户")
                .size(22.0)
                .strong()
                .color(palette.text_primary),
        );
        ui.add_space(10.0);
        egui::Frame::none()
            .fill(palette.normal_member_bg)
            .rounding(8.0)
            .inner_margin(egui::Margin::symmetric(8.0, 3.0))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "共 {} / {} 个账号 · 已开启 {}",
                        total_accounts, MAX_ACCOUNTS, active_accounts
                    ))
                    .size(12.5)
                    .color(palette.normal_member_text),
                );
            });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let add_btn = egui::Button::new(
                egui::RichText::new("+ 添加账号")
                    .size(14.0)
                    .strong()
                    .color(egui::Color32::WHITE),
            )
            .fill(palette.accent)
            .rounding(8.0)
            .min_size(egui::vec2(108.0, 32.0));
            let resp = ui
                .add_enabled(can_add_account, add_btn)
                .on_hover_text(format!(
                    "添加新的 B 站账号（防黄牛限制：最多同时保存 {} 个账号）",
                    MAX_ACCOUNTS
                ))
                .on_disabled_hover_text(format!(
                    "为防止项目被黄牛滥用，最多仅支持同时保存 {} 个账号；如需更换请先登出已有账号",
                    MAX_ACCOUNTS
                ));
            if resp.clicked() {
                app.show_login_windows = true;
            }
        });
    });

    if !can_add_account {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!(
                "🛡 防黄牛限制：当前已达最大账号上限（{} 个）。如需更换其他账号，请先登出已有账号。",
                MAX_ACCOUNTS
            ))
            .size(12.5)
            .color(palette.text_secondary),
        );
    }

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(8.0);

    let mut pending_avatar_requests: Vec<(String, String)> = Vec::new();
    let mut signout_uid: Option<i64> = None;
    let mut switch_action: Option<AccountSwitch> = None;
    let mut open_orders_uid: Option<i64> = None;
    let mut open_add_buyer_uid: Option<i64> = None;
    let mut open_login_window = false;

    egui::ScrollArea::vertical()
        .id_source("account_list_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if app.account_manager.accounts.is_empty() {
                render_empty_state(
                    ui,
                    app.default_avatar_texture.as_ref(),
                    &palette,
                    &mut open_login_window,
                );
            } else {
                for account in &app.account_manager.accounts {
                    let avatar_key = format!("avatar:{}", account.uid);
                    let (avatar_tex, avatar_loading) = match app.image_cache.get(&avatar_key) {
                        Some(ImageLoadState::Loaded(tex)) => (Some(tex), false),
                        Some(ImageLoadState::Loading) => {
                            (app.default_avatar_texture.as_ref(), true)
                        }
                        Some(ImageLoadState::Failed) => {
                            (app.default_avatar_texture.as_ref(), false)
                        }
                        None => {
                            if let Some(url) = account
                                .avatar_url
                                .as_deref()
                                .map(str::trim)
                                .filter(|u| !u.is_empty())
                            {
                                pending_avatar_requests
                                    .push((avatar_key.clone(), url.to_string()));
                            }
                            (app.default_avatar_texture.as_ref(), false)
                        }
                    };

                    let is_signing_out = app.signout_pending_uids.contains(&account.uid);
                    let is_initializing = app.initializing_account_uids.contains(&account.uid);

                    render_account_card(
                        ui,
                        account,
                        avatar_tex,
                        avatar_loading,
                        is_signing_out,
                        is_initializing,
                        &palette,
                        &mut signout_uid,
                        &mut switch_action,
                        &mut open_orders_uid,
                        &mut open_add_buyer_uid,
                    );
                    ui.add_space(10.0);
                }
            }
        });

    // 在不可变借用结束后统一执行状态变更和异步请求
    for (key, url) in pending_avatar_requests {
        app.request_image(&key, &url);
    }
    if open_login_window {
        app.show_login_windows = true;
    }
    if let Some(uid) = signout_uid {
        app.request_signout(uid);
    }
    if let Some(sw) = switch_action {
        app.account_switch = Some(sw);
    }
    if let Some(uid) = open_orders_uid {
        app.orderlist_page = 0;
        app.orderlist_error = None;
        app.orderlist_detail_id = None;
        app.orderlist_requesting = false;
        app.orderlist_active_task_id = None;
        app.orderlist_need_reload = true;
        app.show_orderlist_window = Some(uid.to_string());
    }
    if let Some(uid) = open_add_buyer_uid {
        app.add_buyer_input = crate::windows::add_buyer::AddBuyerInput::default();
        app.add_buyer_active_task_id = None;
        app.add_buyer_error = None;
        app.add_buyer_submitting = false;
        app.show_add_buyer_window = Some(uid.to_string());
    }
}

fn render_empty_state(
    ui: &mut egui::Ui,
    default_avatar: Option<&egui::TextureHandle>,
    palette: &AccountPalette,
    open_login_window: &mut bool,
) {
    egui::Frame::none()
        .fill(palette.card_fill)
        .rounding(12.0)
        .stroke(egui::Stroke::new(1.0, palette.card_border))
        .inner_margin(egui::Margin::symmetric(24.0, 36.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width().max(260.0));
            ui.vertical_centered(|ui| {
                draw_user_avatar(ui, default_avatar, 76.0, false, palette);
                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new("暂未登录任何账号")
                        .size(18.0)
                        .strong()
                        .color(palette.text_primary),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(format!(
                        "为防止被黄牛滥用，最多支持同时登录 {} 个 B 站账号；登录后可管理购票人、查看订单与发起抢票",
                        MAX_ACCOUNTS
                    ))
                        .size(13.5)
                        .color(palette.text_secondary),
                );
                ui.add_space(18.0);
                let login_btn = egui::Button::new(
                    egui::RichText::new("立即登录账号")
                        .size(16.0)
                        .strong()
                        .color(egui::Color32::WHITE),
                )
                .fill(palette.accent)
                .rounding(10.0)
                .min_size(egui::vec2(170.0, 44.0));
                if ui.add(login_btn).clicked() {
                    *open_login_window = true;
                }
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn render_account_card(
    ui: &mut egui::Ui,
    account: &Account,
    avatar_texture: Option<&egui::TextureHandle>,
    avatar_loading: bool,
    is_signing_out: bool,
    is_initializing: bool,
    palette: &AccountPalette,
    signout_uid: &mut Option<i64>,
    switch_action: &mut Option<AccountSwitch>,
    open_orders_uid: &mut Option<i64>,
    open_add_buyer_uid: &mut Option<i64>,
) {
    egui::Frame::none()
        .fill(palette.card_fill)
        .rounding(12.0)
        .stroke(egui::Stroke::new(1.0, palette.card_border))
        .inner_margin(egui::Margin::symmetric(16.0, 14.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width().max(260.0));

            // 上半部分：圆形头像 + 账号基础信息
            ui.horizontal(|ui| {
                draw_user_avatar(ui, avatar_texture, 72.0, avatar_loading, palette);
                ui.add_space(14.0);

                ui.vertical(|ui| {
                    // 昵称与 UID
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(10.0, 4.0);
                        ui.label(
                            egui::RichText::new(&account.name)
                                .size(22.0)
                                .strong()
                                .color(palette.text_primary),
                        );
                        ui.label(
                            egui::RichText::new(format!("UID: {}", account.uid))
                                .size(13.5)
                                .color(palette.text_secondary),
                        );
                    });

                    ui.add_space(8.0);

                    // 会员标签、等级标签、客户端初始化状态
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(8.0, 4.0);
                        render_vip_badge(ui, &account.vip_label, palette);

                        let level = account.level.trim();
                        if !level.is_empty() && level != "未登录" {
                            pill_badge(
                                ui,
                                &format!("LV{}", level),
                                egui::Color32::from_rgb(0, 161, 214),
                                egui::Color32::WHITE,
                            );
                        }

                        if is_initializing {
                            egui::Frame::none()
                                .fill(palette.init_badge)
                                .rounding(8.0)
                                .inner_margin(egui::Margin::symmetric(7.0, 2.5))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 4.0;
                                        ui.add(egui::Spinner::new().size(11.0));
                                        ui.label(
                                            egui::RichText::new("初始化中...")
                                                .size(12.0)
                                                .strong()
                                                .color(egui::Color32::WHITE),
                                        );
                                    });
                                });
                        }
                    });
                });
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            // 下半部分：响应式操作按钮行（绝不溢出）
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(10.0, 8.0);

                // 登出按钮（异步执行，登出中禁用）
                let signout_label = if is_signing_out { "登出中..." } else { "登出" };
                let signout_btn = egui::Button::new(
                    egui::RichText::new(signout_label)
                        .size(14.0)
                        .strong()
                        .color(egui::Color32::WHITE),
                )
                .min_size(egui::vec2(96.0, 36.0))
                .fill(palette.danger)
                .rounding(8.0);
                if ui
                    .add_enabled(!is_signing_out, signout_btn)
                    .on_hover_text("退出登录并移除本地保存的账号信息")
                    .clicked()
                {
                    *signout_uid = Some(account.uid);
                }

                // 查看全部订单
                let orders_btn = egui::Button::new(
                    egui::RichText::new("查看全部订单")
                        .size(14.0)
                        .color(egui::Color32::WHITE),
                )
                .min_size(egui::vec2(116.0, 36.0))
                .fill(palette.accent)
                .rounding(8.0);
                if ui.add_enabled(!is_signing_out, orders_btn).clicked() {
                    *open_orders_uid = Some(account.uid);
                }

                // 添加购票人
                let buyer_btn = egui::Button::new(
                    egui::RichText::new("添加购票人")
                        .size(14.0)
                        .color(egui::Color32::WHITE),
                )
                .min_size(egui::vec2(106.0, 36.0))
                .fill(palette.accent)
                .rounding(8.0);
                if ui.add_enabled(!is_signing_out, buyer_btn).clicked() {
                    *open_add_buyer_uid = Some(account.uid);
                }

                // 抢票开启/关闭开关
                let (toggle_text, toggle_fill, toggle_hint) = if account.is_active {
                    (
                        "抢票开启中",
                        palette.success,
                        "当前允许使用此账号抢票，点击切换为关闭",
                    )
                } else {
                    (
                        "抢票关闭中",
                        palette.inactive_btn,
                        "当前已禁止此账号抢票，点击切换为开启",
                    )
                };
                let toggle_btn = egui::Button::new(
                    egui::RichText::new(toggle_text)
                        .size(14.0)
                        .strong()
                        .color(egui::Color32::WHITE),
                )
                .min_size(egui::vec2(108.0, 36.0))
                .fill(toggle_fill)
                .rounding(8.0);
                if ui
                    .add_enabled(!is_signing_out, toggle_btn)
                    .on_hover_text(toggle_hint)
                    .clicked()
                {
                    *switch_action = Some(AccountSwitch {
                        uid: account.uid.to_string(),
                        switch: !account.is_active,
                    });
                }
            });
        });
}

fn render_vip_badge(ui: &mut egui::Ui, vip_label: &str, palette: &AccountPalette) {
    let trimmed = vip_label.trim();
    if !trimmed.is_empty() && !trimmed.starts_with("未登录") {
        let fill = if trimmed.contains("十年") || trimmed.contains("百年") {
            egui::Color32::from_rgb(230, 145, 38)
        } else {
            palette.vip_pink
        };
        pill_badge(ui, trimmed, fill, egui::Color32::WHITE);
    } else {
        pill_badge(
            ui,
            "正式会员",
            palette.normal_member_bg,
            palette.normal_member_text,
        );
    }
}

fn pill_badge(ui: &mut egui::Ui, text: &str, fill: egui::Color32, text_color: egui::Color32) {
    egui::Frame::none()
        .fill(fill)
        .rounding(8.0)
        .inner_margin(egui::Margin::symmetric(8.0, 2.5))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).color(text_color).size(12.5));
        });
}

/// 在常规 UI 图层中绘制已圆形 alpha 遮罩的头像，并叠加清晰圆形边框
fn draw_user_avatar(
    ui: &mut egui::Ui,
    texture: Option<&egui::TextureHandle>,
    size: f32,
    is_loading: bool,
    palette: &AccountPalette,
) {
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.circle_filled(rect.center(), size * 0.5, palette.avatar_bg);

        if let Some(tex) = texture {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            painter.image(tex.id(), rect, uv, egui::Color32::WHITE);
        }

        painter.circle_stroke(
            rect.center(),
            size * 0.5,
            egui::Stroke::new(1.5, palette.avatar_border),
        );

        if is_loading {
            let badge_size = 18.0;
            let spinner_rect = egui::Rect::from_min_size(
                egui::pos2(rect.right() - badge_size, rect.bottom() - badge_size),
                egui::vec2(badge_size, badge_size),
            );
            painter.circle_filled(spinner_rect.center(), badge_size * 0.5, palette.card_fill);
            ui.put(spinner_rect.shrink(2.0), egui::Spinner::new().size(12.0));
        }
    }
}

fn load_default_avatar(ctx: &egui::Context, app: &mut Myapp) {
    if app.default_avatar_texture.is_some() {
        return;
    }

    let default_avatar_bytes = include_bytes!("../../../assets/default_avatar.jpg");
    match decode_image_to_color_image(default_avatar_bytes, true) {
        Ok(color_image) => {
            app.default_avatar_texture = Some(ctx.load_texture(
                "default_avatar",
                color_image,
                egui::TextureOptions::LINEAR,
            ));
        }
        Err(_) => {
            app.default_avatar_texture = generate_placeholder_avatar(ctx);
        }
    }
}

fn generate_placeholder_avatar(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let size = 128usize;
    let mut image_data = vec![0u8; size * size * 4];
    let center = (size as f32 - 1.0) * 0.5;
    let radius = size as f32 * 0.5;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let alpha = if dist > radius {
                0
            } else if dist > radius - 1.5 {
                (((radius - dist) / 1.5).clamp(0.0, 1.0) * 255.0) as u8
            } else {
                255
            };

            let i = (y * size + x) * 4;
            image_data[i] = 140;
            image_data[i + 1] = 175 + ((y * 45) / size) as u8;
            image_data[i + 2] = 235;
            image_data[i + 3] = alpha;
        }
    }

    Some(ctx.load_texture(
        "default_avatar",
        egui::ColorImage::from_rgba_unmultiplied([size, size], &image_data),
        egui::TextureOptions::LINEAR,
    ))
}
