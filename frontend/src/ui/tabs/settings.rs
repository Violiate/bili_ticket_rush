use eframe::egui;

use crate::app::{normalize_fullwidth_ascii, Myapp};
use crate::windows::confirm_ticket::{
    compute_buyer_grid_layout, draw_buyer_card_item, draw_inline_status_icon, ConfirmPalette,
};

struct SettingsPalette {
    card_fill: egui::Color32,
    card_border: egui::Color32,
    text_primary: egui::Color32,
    text_secondary: egui::Color32,
    text_hint: egui::Color32,
    accent: egui::Color32,
    accent_border: egui::Color32,
    switch_off: egui::Color32,
    unselected_fill: egui::Color32,
    unselected_text: egui::Color32,
    unselected_border: egui::Color32,
    dirty_bg: egui::Color32,
    dirty_text: egui::Color32,
    saved_bg: egui::Color32,
    saved_text: egui::Color32,
    error_bg: egui::Color32,
    error_text: egui::Color32,
}

impl SettingsPalette {
    fn from_dark_mode(dark_mode: bool) -> Self {
        if dark_mode {
            Self {
                card_fill: egui::Color32::from_rgb(36, 38, 46),
                card_border: egui::Color32::from_rgb(62, 66, 80),
                text_primary: egui::Color32::from_rgb(235, 238, 245),
                text_secondary: egui::Color32::from_rgb(185, 190, 205),
                text_hint: egui::Color32::from_rgb(142, 148, 164),
                accent: egui::Color32::from_rgb(45, 136, 245),
                accent_border: egui::Color32::from_rgb(28, 108, 212),
                switch_off: egui::Color32::from_rgb(92, 96, 112),
                unselected_fill: egui::Color32::from_rgb(48, 52, 64),
                unselected_text: egui::Color32::from_rgb(218, 222, 232),
                unselected_border: egui::Color32::from_rgb(76, 82, 98),
                dirty_bg: egui::Color32::from_rgb(82, 58, 24),
                dirty_text: egui::Color32::from_rgb(255, 200, 110),
                saved_bg: egui::Color32::from_rgb(26, 66, 40),
                saved_text: egui::Color32::from_rgb(122, 230, 156),
                error_bg: egui::Color32::from_rgb(80, 28, 28),
                error_text: egui::Color32::from_rgb(255, 140, 140),
            }
        } else {
            Self {
                card_fill: egui::Color32::from_rgb(248, 249, 252),
                card_border: egui::Color32::from_rgb(214, 218, 230),
                text_primary: egui::Color32::from_rgb(32, 36, 48),
                text_secondary: egui::Color32::from_rgb(82, 88, 104),
                text_hint: egui::Color32::from_rgb(122, 128, 144),
                accent: egui::Color32::from_rgb(45, 136, 245),
                accent_border: egui::Color32::from_rgb(25, 106, 208),
                switch_off: egui::Color32::from_rgb(168, 174, 188),
                unselected_fill: egui::Color32::from_rgb(234, 237, 244),
                unselected_text: egui::Color32::from_rgb(48, 54, 68),
                unselected_border: egui::Color32::from_rgb(194, 200, 216),
                dirty_bg: egui::Color32::from_rgb(255, 243, 214),
                dirty_text: egui::Color32::from_rgb(176, 102, 12),
                saved_bg: egui::Color32::from_rgb(228, 248, 234),
                saved_text: egui::Color32::from_rgb(24, 128, 56),
                error_bg: egui::Color32::from_rgb(254, 232, 232),
                error_text: egui::Color32::from_rgb(196, 42, 42),
            }
        }
    }
}

#[derive(Default, Clone, Copy)]
struct FieldOutcome {
    changed: bool,
    lost_focus: bool,
}

fn on_switch(
    ui: &mut egui::Ui,
    label: &str,
    on: &mut bool,
    palette: &SettingsPalette,
) -> egui::Response {
    let mut changed = false;
    let resp = ui
        .horizontal(|ui| {
            if !label.is_empty() {
                ui.label(
                    egui::RichText::new(label)
                        .size(14.5)
                        .color(palette.text_primary)
                        .strong(),
                );
                ui.add_space(6.0);
            }

            let width = 46.0;
            let height = 24.0;
            let (rect, mut response) =
                ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

            if response.clicked() {
                *on = !*on;
                response.mark_changed();
                changed = true;
            }

            let animation_progress = ui.ctx().animate_bool(response.id, *on);
            let radius = height * 0.5;
            let circle_x = rect.left() + radius + animation_progress * (width - height);

            ui.painter().rect_filled(
                rect.expand(-1.0),
                radius,
                if *on {
                    palette.accent
                } else {
                    palette.switch_off
                },
            );

            ui.painter().circle_filled(
                egui::pos2(circle_x, rect.center().y),
                radius - 3.5,
                egui::Color32::WHITE,
            );

            response
        })
        .inner;

    let mut out = resp;
    if changed {
        out.mark_changed();
    }
    out
}

fn settings_card(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: Option<&str>,
    palette: &SettingsPalette,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::none()
        .fill(palette.card_fill)
        .rounding(12.0)
        .stroke(egui::Stroke::new(1.0, palette.card_border))
        .inner_margin(egui::Margin::symmetric(16.0, 14.0))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width().max(260.0));
            ui.label(
                egui::RichText::new(title)
                    .size(16.0)
                    .strong()
                    .color(palette.text_primary),
            );
            if let Some(sub) = subtitle {
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(sub)
                            .size(12.0)
                            .color(palette.text_hint),
                    )
                    .wrap(true),
                );
            }
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(10.0);
            add_contents(ui);
        });
}

fn grid_label(ui: &mut egui::Ui, text: &str, palette: &SettingsPalette) {
    ui.label(
        egui::RichText::new(text)
            .size(14.0)
            .color(palette.text_primary),
    );
}

fn text_input_row(
    ui: &mut egui::Ui,
    value: &mut String,
    hint: &str,
    width: f32,
    digits_only: bool,
    ime_composing: bool,
) -> FieldOutcome {
    let response = ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(width)
            .min_size(egui::vec2(width, 30.0)),
    );
    let lost_focus = response.lost_focus();
    let mut changed = response.changed();
    if digits_only && (changed || lost_focus) && (!ime_composing || lost_focus) {
        let filtered: String = normalize_fullwidth_ascii(value)
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect();
        if filtered != *value {
            *value = filtered;
            changed = true;
        }
    }
    FieldOutcome {
        changed,
        lost_focus,
    }
}

fn secret_input_row(
    ui: &mut egui::Ui,
    id_key: &str,
    value: &mut String,
    hint: &str,
    width: f32,
) -> FieldOutcome {
    let state_id = egui::Id::new(id_key);
    let mut visible = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(state_id).unwrap_or(false));

    let mut outcome = FieldOutcome::default();
    ui.horizontal(|ui| {
        let field_w = (width - 60.0).max(150.0);
        let response = ui.add(
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .password(!visible)
                .desired_width(field_w)
                .min_size(egui::vec2(field_w, 30.0)),
        );
        outcome.changed = response.changed();
        outcome.lost_focus = response.lost_focus();
        let toggle_label = if visible { "隐藏" } else { "显示" };
        if ui
            .add(egui::Button::new(toggle_label).min_size(egui::vec2(50.0, 28.0)))
            .clicked()
        {
            visible = !visible;
            ui.ctx().data_mut(|d| d.insert_temp(state_id, visible));
        }
    });
    outcome
}

fn custom_selection_control(
    ui: &mut egui::Ui,
    selected: &mut usize,
    options: &[&str],
    palette: &SettingsPalette,
) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
        for (idx, option) in options.iter().enumerate() {
            let is_selected = *selected == idx;
            let button = egui::Button::new(
                egui::RichText::new(*option)
                    .size(14.0)
                    .color(if is_selected {
                        egui::Color32::WHITE
                    } else {
                        palette.unselected_text
                    }),
            )
            .min_size(egui::vec2(96.0, 30.0))
            .fill(if is_selected {
                palette.accent
            } else {
                palette.unselected_fill
            })
            .stroke(egui::Stroke::new(
                1.0,
                if is_selected {
                    palette.accent_border
                } else {
                    palette.unselected_border
                },
            ))
            .rounding(7.0);

            if ui.add(button).clicked() && *selected != idx {
                *selected = idx;
                changed = true;
            }
        }
    });
    changed
}

pub fn render(app: &mut Myapp, ui: &mut egui::Ui) {
    let dark_mode = ui.ctx().style().visuals.dark_mode;
    let palette = SettingsPalette::from_dark_mode(dark_mode);
    let mut should_auto_save = false;

    // 修正历史上可能保存的非法 captcha_mode（仅支持 0: 本地识别 / 1: ttocr识别）
    if app.custom_config.captcha_mode >= 2 {
        app.custom_config.captcha_mode = 0;
        app.settings_dirty = true;
        should_auto_save = true;
    }

    // 顶部固定状态栏：标题 + 自动保存状态指示（输入框失焦/开关切换即自动保存，无需手动点击保存按钮）
    let mut retry_save_clicked = false;
    ui.horizontal(|ui| {
        ui.heading(
            egui::RichText::new("设置 / 微调")
                .size(22.0)
                .strong()
                .color(palette.text_primary),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(ref err_msg) = app.settings_save_error {
                let retry_btn = egui::Button::new(
                    egui::RichText::new("重试保存")
                        .size(13.0)
                        .strong()
                        .color(egui::Color32::WHITE),
                )
                .min_size(egui::vec2(82.0, 28.0))
                .fill(palette.accent)
                .rounding(7.0);
                if ui.add(retry_btn).clicked() {
                    retry_save_clicked = true;
                }
                ui.add_space(8.0);
                egui::Frame::none()
                    .fill(palette.error_bg)
                    .rounding(7.0)
                    .inner_margin(egui::Margin::symmetric(9.0, 4.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            draw_inline_status_icon(
                                ui,
                                false,
                                palette.error_text,
                                egui::Color32::WHITE,
                            );
                            ui.label(
                                egui::RichText::new("自动保存失败")
                                    .size(12.5)
                                    .strong()
                                    .color(palette.error_text),
                            )
                            .on_hover_text(err_msg);
                        });
                    });
            } else if app.settings_dirty {
                egui::Frame::none()
                    .fill(palette.dirty_bg)
                    .rounding(7.0)
                    .inner_margin(egui::Margin::symmetric(10.0, 4.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("编辑中 · 离开输入框后自动保存")
                                .size(12.5)
                                .strong()
                                .color(palette.dirty_text),
                        );
                    });
            } else if app.settings_last_saved_at.is_some() {
                egui::Frame::none()
                    .fill(palette.saved_bg)
                    .rounding(7.0)
                    .inner_margin(egui::Margin::symmetric(10.0, 4.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            draw_inline_status_icon(
                                ui,
                                true,
                                palette.saved_text,
                                egui::Color32::WHITE,
                            );
                            ui.label(
                                egui::RichText::new("已自动保存并生效")
                                    .size(12.5)
                                    .strong()
                                    .color(palette.saved_text),
                            );
                        });
                    });
            } else {
                ui.label(
                    egui::RichText::new("所有更改在离开输入框或切换开关后自动保存")
                        .size(12.5)
                        .color(palette.text_hint),
                );
            }
        });
    });

    if retry_save_clicked {
        should_auto_save = true;
    }

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(6.0);

    // 整页内容放入垂直 ScrollArea，避免 600px 高度下裁切任何推送/SMTP/Gotify 配置
    egui::ScrollArea::vertical()
        .id_source("settings_tab_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let input_width = (ui.available_width() - 210.0).clamp(220.0, 520.0);

            // 卡片 0：默认购票人设置（高频人性化配置）
            settings_card(
                ui,
                "默认购票人设置",
                Some("为每个账号预设最多 2 位常用购票人：单人实名项目自动勾选「默认1」，一票一证/捡漏模式自动勾选全部已设默认购票人（支持随时手动改选）"),
                &palette,
                |ui| {
                    render_default_buyers_settings(app, ui, &palette, &mut should_auto_save);
                },
            );

            ui.add_space(12.0);

            // 卡片 1：基础设置
            settings_card(
                ui,
                "基础设置",
                Some("预填联系人姓名与手机号将在非实名制项目确认页及添加购票人时自动填充；所有设置离开输入框后自动保存并立即生效"),
                &palette,
                |ui| {
                    render_global_settings(app, ui, input_width, &palette, &mut should_auto_save);
                },
            );

            ui.add_space(12.0);

            // 卡片 2：抢票参数微调
            settings_card(
                ui,
                "抢票参数微调",
                Some("控制捡漏轮询频率与下单重试节奏，合理配置可兼顾成功率与防风控"),
                &palette,
                |ui| {
                    render_grab_tuning_settings(app, ui, &palette, &mut should_auto_save);
                },
            );

            ui.add_space(12.0);

            // 卡片 3：推送通知设置
            settings_card(
                ui,
                "推送通知设置",
                Some("抢票成功生成订单后将自动向所有已填写的渠道发送通知"),
                &palette,
                |ui| {
                    render_push_settings(app, ui, input_width, &palette, &mut should_auto_save);
                },
            );

            ui.add_space(16.0);
        });

    if should_auto_save && (app.settings_dirty || app.settings_save_error.is_some()) {
        app.auto_save_settings();
    }
}

fn render_default_buyers_settings(
    app: &mut Myapp,
    ui: &mut egui::Ui,
    palette: &SettingsPalette,
    should_auto_save: &mut bool,
) {
    let mut dirty = false;
    let mut refresh_uid: Option<i64> = None;
    let mut open_add_buyer_uid: Option<i64> = None;
    let mut open_login = false;
    let mut limit_hint: Option<&'static str> = None;

    ui.horizontal(|ui| {
        if on_switch(
            ui,
            "自动勾选默认购票人",
            &mut app.custom_config.enable_default_buyers,
            palette,
        )
        .changed()
        {
            dirty = true;
            *should_auto_save = true;
        }
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(if app.custom_config.enable_default_buyers {
                "已开启：打开购票确认页时自动勾选，免重复点击"
            } else {
                "已关闭：每次打开购票确认页时保持空白手动选择"
            })
            .size(12.5)
            .color(palette.text_hint),
        );
    });

    ui.add_space(10.0);

    let accounts_snapshot: Vec<(i64, String)> = app
        .account_manager
        .accounts
        .iter()
        .map(|a| (a.uid, a.name.clone()))
        .collect();

    if accounts_snapshot.is_empty() {
        egui::Frame::none()
            .fill(palette.unselected_fill)
            .stroke(egui::Stroke::new(1.0, palette.unselected_border))
            .rounding(8.0)
            .inner_margin(egui::Margin::symmetric(14.0, 12.0))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new("当前尚未登录账号，登录后即可为每个账号独立配置最多 2 位默认购票人。")
                            .size(13.0)
                            .color(palette.text_secondary),
                    );
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("去登录账号")
                                    .size(12.5)
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(palette.accent)
                            .rounding(6.0),
                        )
                        .clicked()
                    {
                        open_login = true;
                    }
                });
            });
    } else {
        for (idx, (uid, acc_name)) in accounts_snapshot.iter().enumerate() {
            if idx > 0 {
                ui.add_space(10.0);
            }
            let is_refreshing = app.settings_buyer_refresh_tasks.contains_key(uid);
            let cached_buyers = app
                .account_buyers_cache
                .get(uid)
                .cloned()
                .unwrap_or_default();
            let current_defaults = app.custom_config.default_buyers_for_uid(*uid);

            egui::Frame::none()
                .fill(if ui.visuals().dark_mode {
                    egui::Color32::from_rgb(30, 33, 41)
                } else {
                    egui::Color32::from_rgb(255, 255, 255)
                })
                .stroke(egui::Stroke::new(1.0, palette.card_border))
                .rounding(9.0)
                .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                .show(ui, |ui| {
                    ui.set_min_width((ui.available_width() - 4.0).max(240.0));
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(8.0, 4.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "账号 {}：{} ({})",
                                idx + 1,
                                acc_name,
                                uid
                            ))
                            .size(14.0)
                            .strong()
                            .color(palette.text_primary),
                        );

                        egui::Frame::none()
                            .fill(if current_defaults.is_empty() {
                                palette.unselected_fill
                            } else {
                                palette.saved_bg
                            })
                            .rounding(6.0)
                            .inner_margin(egui::Margin::symmetric(7.0, 2.0))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "默认 {}/2 人",
                                        current_defaults.len()
                                    ))
                                    .size(11.5)
                                    .strong()
                                    .color(if current_defaults.is_empty() {
                                        palette.text_hint
                                    } else {
                                        palette.saved_text
                                    }),
                                );
                            });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let sync_label = if is_refreshing {
                                "同步中..."
                            } else {
                                "同步购票人"
                            };
                            if ui
                                .add_enabled(
                                    !is_refreshing,
                                    egui::Button::new(egui::RichText::new(sync_label).size(12.0))
                                        .rounding(6.0),
                                )
                                .on_hover_text("从 B 站拉取该账号下最新的实名购票人列表")
                                .clicked()
                            {
                                refresh_uid = Some(*uid);
                            }

                            if ui
                                .add(
                                    egui::Button::new(egui::RichText::new("+ 新增").size(12.0))
                                        .rounding(6.0),
                                )
                                .on_hover_text("为该账号添加新的实名购票人")
                                .clicked()
                            {
                                open_add_buyer_uid = Some(*uid);
                            }

                            if current_defaults.len() == 2
                                && ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("互换主次").size(12.0),
                                        )
                                        .rounding(6.0),
                                    )
                                    .on_hover_text("互换「默认 1」与「默认 2」（单人实名项目优先勾选默认 1）")
                                    .clicked()
                            {
                                app.custom_config.swap_default_buyers_for_uid(*uid);
                                dirty = true;
                                *should_auto_save = true;
                            }

                            if !current_defaults.is_empty()
                                && ui
                                    .add(
                                        egui::Button::new(egui::RichText::new("清空").size(12.0))
                                            .rounding(6.0),
                                    )
                                    .on_hover_text("清空该账号的默认购票人")
                                    .clicked()
                            {
                                app.custom_config.set_default_buyers_for_uid(*uid, Vec::new());
                                dirty = true;
                                *should_auto_save = true;
                            }
                        });
                    });

                    ui.add_space(10.0);

                    if cached_buyers.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            if is_refreshing {
                                ui.spinner();
                                ui.label(
                                    egui::RichText::new("正在同步实名购票人列表...")
                                        .size(12.5)
                                        .color(palette.text_secondary),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new(
                                        "本地暂无该账号的购票人缓存，点击右上角「同步购票人」即可拉取并点选默认购票人。",
                                    )
                                    .size(12.5)
                                    .color(palette.text_hint),
                                );
                            }
                        });
                    } else {
                        let buyer_palette = ConfirmPalette::from_dark_mode(ui.visuals().dark_mode);
                        let grid_w = (ui.available_width() - 4.0).max(240.0);
                        let (cols, cell_inner_w) = compute_buyer_grid_layout(grid_w);
                        let cell_outer_w = cell_inner_w + 24.0;

                        for (row_idx, row_buyers) in cached_buyers.chunks(cols).enumerate() {
                            if row_idx > 0 {
                                ui.add_space(8.0);
                            }
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 10.0;
                                for buyer in row_buyers {
                                    let rank = current_defaults
                                        .iter()
                                        .position(|&id| id == buyer.id)
                                        .map(|p| p + 1);
                                    let is_sel = rank.is_some();
                                    let card_id =
                                        ui.id().with(("settings_default_buyer", *uid, buyer.id));
                                    let resp = draw_buyer_card_item(
                                        ui,
                                        card_id,
                                        buyer,
                                        is_sel,
                                        rank,
                                        true,
                                        cell_outer_w,
                                        &buyer_palette,
                                    )
                                    .on_hover_text(
                                        "点击设为默认或取消默认（每账号最多 2 位，按点击先后区分默认 1 / 默认 2）",
                                    );

                                    if resp.clicked() {
                                        match app
                                            .custom_config
                                            .toggle_default_buyer_for_uid(*uid, buyer.id)
                                        {
                                            Ok(_) => {
                                                dirty = true;
                                                *should_auto_save = true;
                                            }
                                            Err(msg) => {
                                                limit_hint = Some(msg);
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    }
                });
        }
    }

    if dirty {
        app.settings_dirty = true;
    }
    if let Some(msg) = limit_hint {
        app.show_info(msg);
    }
    if open_login {
        app.show_login_windows = true;
    }
    if let Some(uid) = refresh_uid {
        app.request_account_buyers_refresh(uid, false);
    }
    if let Some(uid) = open_add_buyer_uid {
        app.add_buyer_input = crate::windows::add_buyer::AddBuyerInput::default();
        app.add_buyer_active_task_id = None;
        app.add_buyer_submitting = false;
        app.add_buyer_error = None;
        app.show_add_buyer_window = Some(uid.to_string());
    }
}

fn render_global_settings(
    app: &mut Myapp,
    ui: &mut egui::Ui,
    input_width: f32,
    palette: &SettingsPalette,
    should_auto_save: &mut bool,
) {
    let mut dirty = false;
    let ime_composing = app.is_ime_composing();
    let acc1_tag = app
        .account_manager
        .accounts
        .first()
        .map(|a| format!("账号 1 ({}) 预填：", a.name))
        .unwrap_or_else(|| "账号 1 预填联系人：".to_string());
    let acc2_tag = app
        .account_manager
        .accounts
        .get(1)
        .map(|a| format!("账号 2 ({}) 预填：", a.name))
        .unwrap_or_else(|| "账号 2 预填联系人：".to_string());

    let name_w = (input_width * 0.36).clamp(100.0, 170.0);
    let phone_w = (input_width - name_w - 10.0).max(130.0);

    egui::Grid::new("settings_global_grid")
        .num_columns(2)
        .spacing(egui::vec2(14.0, 10.0))
        .min_col_width(145.0)
        .show(ui, |ui| {
            grid_label(ui, &acc1_tag, palette);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let out_name = text_input_row(
                    ui,
                    &mut app.custom_config.preinput_name1,
                    "姓名（可选）",
                    name_w,
                    false,
                    ime_composing,
                );
                let out_phone = text_input_row(
                    ui,
                    &mut app.custom_config.preinput_phone1,
                    "手机号（仅数字，自动预填）",
                    phone_w,
                    true,
                    ime_composing,
                );
                if out_name.changed || out_phone.changed {
                    dirty = true;
                }
                if out_name.lost_focus || out_phone.lost_focus {
                    *should_auto_save = true;
                }
            });
            ui.end_row();

            grid_label(ui, &acc2_tag, palette);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let out_name = text_input_row(
                    ui,
                    &mut app.custom_config.preinput_name2,
                    "姓名（可选）",
                    name_w,
                    false,
                    ime_composing,
                );
                let out_phone = text_input_row(
                    ui,
                    &mut app.custom_config.preinput_phone2,
                    "手机号（仅数字，自动预填）",
                    phone_w,
                    true,
                    ime_composing,
                );
                if out_name.changed || out_phone.changed {
                    dirty = true;
                }
                if out_name.lost_focus || out_phone.lost_focus {
                    *should_auto_save = true;
                }
            });
            ui.end_row();

            grid_label(ui, "抢票成功声音提醒：", palette);
            ui.horizontal(|ui| {
                if on_switch(ui, "", &mut app.custom_config.alert_on_success, palette).changed() {
                    dirty = true;
                    *should_auto_save = true;
                }
                ui.label(
                    egui::RichText::new("挂机锁单成功弹出付款码时播放系统提示音")
                        .size(12.0)
                        .color(palette.text_hint),
                );
            });
            ui.end_row();

            grid_label(ui, "验证码识别方式：", palette);
            let options = ["本地识别", "ttocr识别"];
            if custom_selection_control(
                ui,
                &mut app.custom_config.captcha_mode,
                &options,
                palette,
            ) {
                dirty = true;
                *should_auto_save = true;
            }
            ui.end_row();

            if app.custom_config.captcha_mode == 1 {
                grid_label(ui, "ttocr AppKey：", palette);
                let out = secret_input_row(
                    ui,
                    "settings_show_ttocr_key",
                    &mut app.custom_config.ttocr_key,
                    "请输入 ttocr 平台提供的 AppKey",
                    input_width,
                );
                if out.changed {
                    dirty = true;
                }
                if out.lost_focus {
                    *should_auto_save = true;
                }
                ui.end_row();
            }

            grid_label(ui, "自定义 User-Agent：", palette);
            if on_switch(ui, "", &mut app.custom_config.open_custom_ua, palette).changed() {
                dirty = true;
                *should_auto_save = true;
            }
            ui.end_row();

            grid_label(ui, "UA 内容：", palette);
            ui.vertical(|ui| {
                let out = text_input_row(
                    ui,
                    &mut app.custom_config.custom_ua,
                    "开启上方开关后生效，留空则使用默认 UA",
                    input_width,
                    false,
                    ime_composing,
                );
                if out.changed {
                    dirty = true;
                }
                if out.lost_focus {
                    *should_auto_save = true;
                }
                if !app.custom_config.open_custom_ua {
                    ui.add_space(3.0);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("当前默认 UA：{}", app.base_default_ua))
                                .size(11.5)
                                .color(palette.text_hint),
                        )
                        .wrap(true),
                    );
                }
            });
            ui.end_row();
        });

    if dirty {
        app.settings_dirty = true;
    }
}

fn render_grab_tuning_settings(
    app: &mut Myapp,
    ui: &mut egui::Ui,
    palette: &SettingsPalette,
    should_auto_save: &mut bool,
) {
    let mut dirty = false;

    egui::Grid::new("settings_tuning_grid")
        .num_columns(2)
        .spacing(egui::vec2(14.0, 10.0))
        .min_col_width(145.0)
        .show(ui, |ui| {
            grid_label(ui, "捡漏轮询间隔：", palette);
            ui.vertical(|ui| {
                let resp = ui.add(
                    egui::Slider::new(&mut app.custom_config.grab_interval_ms, 500..=10000)
                        .step_by(100.0)
                        .suffix(" ms"),
                );
                if resp.changed() {
                    dirty = true;
                }
                if resp.drag_released() || resp.lost_focus() || (resp.changed() && !resp.dragged())
                {
                    *should_auto_save = true;
                }
                ui.label(
                    egui::RichText::new("默认 2000 ms；过短的间隔更容易触发 B 站接口频控")
                        .size(11.5)
                        .color(palette.text_hint),
                );
            });
            ui.end_row();

            grid_label(ui, "下单最大重试次数：", palette);
            ui.vertical(|ui| {
                let resp = ui.add(
                    egui::Slider::new(&mut app.custom_config.max_retry_count, 10..=200)
                        .suffix(" 次"),
                );
                if resp.changed() {
                    dirty = true;
                }
                if resp.drag_released() || resp.lost_focus() || (resp.changed() && !resp.dragged())
                {
                    *should_auto_save = true;
                }
                ui.label(
                    egui::RichText::new("默认 60 次；单次抢票任务中遇到余票冲突时的最大重试上限")
                        .size(11.5)
                        .color(palette.text_hint),
                );
            });
            ui.end_row();

            grid_label(ui, "下单重试间隔：", palette);
            ui.vertical(|ui| {
                let resp = ui.add(
                    egui::Slider::new(&mut app.custom_config.order_retry_delay_ms, 200..=3000)
                        .step_by(50.0)
                        .suffix(" ms"),
                );
                if resp.changed() {
                    dirty = true;
                }
                if resp.drag_released() || resp.lost_focus() || (resp.changed() && !resp.dragged())
                {
                    *should_auto_save = true;
                }
                ui.label(
                    egui::RichText::new("默认 400 ms；每次下单失败后等待多久再发起下一次重试")
                        .size(11.5)
                        .color(palette.text_hint),
                );
            });
            ui.end_row();
        });

    ui.add_space(6.0);
    if ui
        .add(
            egui::Button::new(
                egui::RichText::new("恢复默认微调参数")
                    .size(12.5)
                    .color(palette.text_secondary),
            )
            .rounding(6.0),
        )
        .clicked()
    {
        app.custom_config.grab_interval_ms = 2000;
        app.custom_config.max_retry_count = 60;
        app.custom_config.order_retry_delay_ms = 400;
        dirty = true;
        *should_auto_save = true;
    }

    if dirty {
        app.settings_dirty = true;
    }
}

fn render_push_settings(
    app: &mut Myapp,
    ui: &mut egui::Ui,
    input_width: f32,
    palette: &SettingsPalette,
    should_auto_save: &mut bool,
) {
    let mut dirty = false;
    let mut send_test_push = false;
    let ime_composing = app.is_ime_composing();

    // 推送总开关 + 右对齐测试推送按钮（禁用时悬停提示）
    ui.horizontal(|ui| {
        if on_switch(ui, "开启推送", &mut app.push_config.enabled, palette).changed() {
            dirty = true;
            *should_auto_save = true;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let test_btn = egui::Button::new(
                egui::RichText::new("测试推送")
                    .size(13.5)
                    .strong()
                    .color(egui::Color32::WHITE),
            )
            .min_size(egui::vec2(94.0, 32.0))
            .fill(palette.accent)
            .rounding(8.0);

            let resp = ui
                .add_enabled(app.push_config.enabled, test_btn)
                .on_hover_text("向所有已配置的推送渠道发送一条测试消息")
                .on_disabled_hover_text("请先开启推送开关");
            if resp.clicked() {
                send_test_push = true;
            }
        });
    });

    if app.push_config.enabled {
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.label(
            egui::RichText::new("即时消息渠道")
                .size(14.0)
                .strong()
                .color(palette.text_secondary),
        );
        ui.add_space(6.0);

        egui::Grid::new("settings_push_channels_grid")
            .num_columns(2)
            .spacing(egui::vec2(14.0, 10.0))
            .min_col_width(145.0)
            .show(ui, |ui| {
                grid_label(ui, "Bark 推送：", palette);
                let out = text_input_row(
                    ui,
                    &mut app.push_config.bark_token,
                    "填写设备 Key 或完整 URL，可在末尾用 ? 附加参数",
                    input_width,
                    false,
                    ime_composing,
                );
                if out.changed {
                    dirty = true;
                }
                if out.lost_focus {
                    *should_auto_save = true;
                }
                ui.end_row();

                grid_label(ui, "PushPlus 推送：", palette);
                let out = text_input_row(
                    ui,
                    &mut app.push_config.pushplus_token,
                    "填写 PushPlus Token",
                    input_width,
                    false,
                    ime_composing,
                );
                if out.changed {
                    dirty = true;
                }
                if out.lost_focus {
                    *should_auto_save = true;
                }
                ui.end_row();

                grid_label(ui, "方糖推送：", palette);
                let out = text_input_row(
                    ui,
                    &mut app.push_config.fangtang_token,
                    "填写 Server酱 SendKey（如 SCTxxxxxxx）",
                    input_width,
                    false,
                    ime_composing,
                );
                if out.changed {
                    dirty = true;
                }
                if out.lost_focus {
                    *should_auto_save = true;
                }
                ui.end_row();

                grid_label(ui, "钉钉机器人：", palette);
                let out = text_input_row(
                    ui,
                    &mut app.push_config.dingtalk_token,
                    "填写钉钉机器人 Webhook 的 access_token",
                    input_width,
                    false,
                    ime_composing,
                );
                if out.changed {
                    dirty = true;
                }
                if out.lost_focus {
                    *should_auto_save = true;
                }
                ui.end_row();

                grid_label(ui, "企业微信机器人：", palette);
                let out = text_input_row(
                    ui,
                    &mut app.push_config.wechat_token,
                    "填写企业微信机器人 Webhook 的 key",
                    input_width,
                    false,
                    ime_composing,
                );
                if out.changed {
                    dirty = true;
                }
                if out.lost_focus {
                    *should_auto_save = true;
                }
                ui.end_row();

                grid_label(ui, "Gotify 地址：", palette);
                let out = text_input_row(
                    ui,
                    &mut app.push_config.gotify_config.gotify_url,
                    "填写服务器地址，如 https://gotify.example.com",
                    input_width,
                    false,
                    ime_composing,
                );
                if out.changed {
                    dirty = true;
                }
                if out.lost_focus {
                    *should_auto_save = true;
                }
                ui.end_row();

                grid_label(ui, "Gotify Token：", palette);
                let out = secret_input_row(
                    ui,
                    "settings_show_gotify_token",
                    &mut app.push_config.gotify_config.gotify_token,
                    "填写 Gotify 应用 Token",
                    input_width,
                );
                if out.changed {
                    dirty = true;
                }
                if out.lost_focus {
                    *should_auto_save = true;
                }
                ui.end_row();
            });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(10.0);

        ui.label(
            egui::RichText::new("SMTP 邮件通知")
                .size(14.0)
                .strong()
                .color(palette.text_secondary),
        );
        ui.add_space(2.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new(
                    "支持 465 端口（隐式 TLS）与 587 端口（STARTTLS）；多个收件人可用英文逗号分隔",
                )
                .size(11.5)
                .color(palette.text_hint),
            )
            .wrap(true),
        );
        ui.add_space(8.0);

        egui::Grid::new("settings_smtp_grid")
            .num_columns(2)
            .spacing(egui::vec2(14.0, 10.0))
            .min_col_width(145.0)
            .show(ui, |ui| {
                grid_label(ui, "SMTP 服务器地址：", palette);
                let out = text_input_row(
                    ui,
                    &mut app.push_config.smtp_config.smtp_server,
                    "如 smtp.qq.com 或 smtp.163.com",
                    input_width,
                    false,
                    ime_composing,
                );
                if out.changed {
                    dirty = true;
                }
                if out.lost_focus {
                    *should_auto_save = true;
                }
                ui.end_row();

                grid_label(ui, "SMTP 端口：", palette);
                let out = text_input_row(
                    ui,
                    &mut app.push_config.smtp_config.smtp_port,
                    "仅数字，推荐 465（TLS）或 587（STARTTLS），留空默认 465",
                    input_width,
                    true,
                    ime_composing,
                );
                if out.changed {
                    dirty = true;
                }
                if out.lost_focus {
                    *should_auto_save = true;
                }
                ui.end_row();

                grid_label(ui, "邮箱账号：", palette);
                let out = text_input_row(
                    ui,
                    &mut app.push_config.smtp_config.smtp_username,
                    "登录 SMTP 服务器使用的邮箱账号",
                    input_width,
                    false,
                    ime_composing,
                );
                if out.changed {
                    dirty = true;
                }
                if out.lost_focus {
                    *should_auto_save = true;
                }
                ui.end_row();

                grid_label(ui, "授权密码：", palette);
                let out = secret_input_row(
                    ui,
                    "settings_show_smtp_password",
                    &mut app.push_config.smtp_config.smtp_password,
                    "请输入邮箱 SMTP 授权码或密码",
                    input_width,
                );
                if out.changed {
                    dirty = true;
                }
                if out.lost_focus {
                    *should_auto_save = true;
                }
                ui.end_row();

                grid_label(ui, "发件人邮箱：", palette);
                let out = text_input_row(
                    ui,
                    &mut app.push_config.smtp_config.smtp_from,
                    "发件人邮箱地址（留空默认与邮箱账号一致）",
                    input_width,
                    false,
                    ime_composing,
                );
                if out.changed {
                    dirty = true;
                }
                if out.lost_focus {
                    *should_auto_save = true;
                }
                ui.end_row();

                grid_label(ui, "收件人邮箱：", palette);
                let out = text_input_row(
                    ui,
                    &mut app.push_config.smtp_config.smtp_to,
                    "接收通知的邮箱地址，多个地址可用逗号分隔",
                    input_width,
                    false,
                    ime_composing,
                );
                if out.changed {
                    dirty = true;
                }
                if out.lost_focus {
                    *should_auto_save = true;
                }
                ui.end_row();
            });
    } else {
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("开启上方开关后可配置 Bark、PushPlus、方糖、钉钉、企业微信、Gotify 及 SMTP 邮件通知。")
                .size(12.5)
                .color(palette.text_hint),
        );
    }

    if dirty {
        app.settings_dirty = true;
    }

    if send_test_push && app.push_config.enabled {
        if *should_auto_save || app.settings_dirty {
            app.auto_save_settings();
            *should_auto_save = false;
        }
        app.task_manager.update_push_config(app.push_config.clone());
        app.push_config.push_all(
            "BiliTicket 推送测试",
            "这是一条测试推送通知，收到此消息说明推送配置正常。",
            &None,
            &mut *app.task_manager,
        );
        app.show_info("正在发送测试推送...");
    }
}
