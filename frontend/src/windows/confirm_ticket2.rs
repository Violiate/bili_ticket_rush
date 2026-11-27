use std::sync::Arc;

use common::cookie_manager::CookieManager;
use common::taskmanager::{GrabTicketRequest, TaskRequest, TaskStatus};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};

use crate::app::{resolve_initial_buyers, Myapp};
use crate::windows::confirm_ticket::{
    draw_inline_status_icon, draw_vector_info, pill_badge, render_buyer_cards_grid,
    subtle_action_button, ConfirmPalette,
};
use crate::windows::{bounded_window_sizes, show_resizable_body};

/// 显示捡漏模式的确认窗口
/// 只需要选择购票人与可选过滤词，其他信息由后端动态匹配
pub fn show(app: &mut Myapp, ctx: &egui::Context, uid: &i64) {
    let mut open = app.confirm_ticket_info.is_some();
    if !open {
        return;
    }

    let biliticket_index = match app.bilibiliticket_list.iter().position(|bt| bt.uid == *uid) {
        Some(index) => index,
        None => {
            log::error!("没有找到uid为{}的抢票信息", uid);
            app.confirm_ticket_info = None;
            app.buyer_info_requesting = false;
            app.buyer_info_active_task_id = None;
            app.buyer_info_error = None;
            app.reset_buyer_selection();
            return;
        }
    };

    let (biliticket_uid, account_name, project_id, cookie_manager, buyers_loaded, buyers) = {
        let biliticket = &app.bilibiliticket_list[biliticket_index];
        let cm: Arc<CookieManager> = match &biliticket.account.cookie_manager {
            Some(cm) => cm.clone(),
            None => {
                let acc_uid = biliticket.uid;
                log::error!("账号 {} 未登录或cookie管理器未初始化", acc_uid);
                app.show_error(format!("账号 {} 的客户端未初始化，请重新登录", acc_uid));
                app.is_loading = false;
                app.confirm_ticket_info = None;
                app.buyer_info_requesting = false;
                app.buyer_info_active_task_id = None;
                app.buyer_info_error = None;
                app.reset_buyer_selection();
                return;
            }
        };
        let buyers_loaded = biliticket.all_buyer_info.is_some();
        let buyers = biliticket
            .all_buyer_info
            .as_ref()
            .map(|data| data.list.clone())
            .unwrap_or_default();

        (
            biliticket.uid,
            biliticket.account.name.clone(),
            biliticket.project_id.clone(),
            cm,
            buyers_loaded,
            buyers,
        )
    };

    let configured_default_ids = app.custom_config.default_buyers_for_uid(biliticket_uid);
    if buyers_loaded
        && !buyers.is_empty()
        && app.default_buyers_applied_uid != Some(biliticket_uid)
    {
        app.default_buyers_applied_uid = Some(biliticket_uid);
        if app.selected_buyer_list.is_none() {
            let auto_list = resolve_initial_buyers(
                app.custom_config.enable_default_buyers,
                &configured_default_ids,
                &buyers,
                true,
            );
            if !auto_list.is_empty() {
                app.selected_buyer_list = Some(auto_list);
                app.selected_buyer_uid = Some(biliticket_uid);
            }
        }
    }

    let mut cancel_clicked = false;
    let mut submit_clicked = false;
    let mut refresh_buyers_clicked = false;
    let mut add_buyer_clicked = false;
    let mut save_as_default_buyers: Option<Vec<i64>> = None;
    let mut skip_words_changed = false;

    let (default_sz, min_sz, max_sz) = bounded_window_sizes(ctx, 700.0, 590.0, 500.0, 380.0);

    let palette = ConfirmPalette::from_dark_mode(ctx.style().visuals.dark_mode);
    let ime_composing = app.is_ime_composing();

    egui::Window::new("捡漏模式 - 选择购票人")
        .id(egui::Id::new("confirm_ticket2_window_v2"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .constrain(true)
        .default_size(default_sz)
        .min_width(min_sz[0])
        .min_height(min_sz[1])
        .resize(|r| r.max_size(max_sz))
        .show(ctx, |ui| {
            let valid_selected_count = app.selected_buyer_list.as_ref().map_or(0, |list| {
                list.iter()
                    .filter(|sel| buyers.iter().any(|b| b.id == sel.id))
                    .count()
            });
            let has_buyers = valid_selected_count > 0;
            let (hint_text, hint_color) = if has_buyers {
                (
                    format!(
                        "已选 {} 位购票人，点击右侧立即开启捡漏监控",
                        valid_selected_count
                    ),
                    palette.green,
                )
            } else {
                ("请先选择至少 1 位实名购票人".to_string(), palette.amber)
            };

            let no_widget_focused = ui.ctx().memory(|m| m.focus().is_none());
            if no_widget_focused && !ime_composing {
                if has_buyers && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit_clicked = true;
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    cancel_clicked = true;
                }
            }

            show_resizable_body(
                ui,
                "confirm_ticket2_main_scroll",
                |ui| {
                    // 顶部目标账号、项目ID 与模式标签
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);
                        ui.label(
                            RichText::new(format!("账号: {} ({})", account_name, biliticket_uid))
                                .size(13.0)
                                .strong()
                                .color(palette.text_primary),
                        );
                        ui.label(RichText::new("·").color(palette.text_muted));
                        ui.label(
                            RichText::new(format!("项目ID: {}", project_id))
                                .size(13.0)
                                .color(palette.text_secondary),
                        );
                        ui.label(RichText::new("·").color(palette.text_muted));
                        pill_badge(ui, "捡漏回流模式", palette.blue_bg, palette.blue);
                    });

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(6.0);
                },
                |ui| {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(5.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        draw_inline_status_icon(
                            ui,
                            has_buyers,
                            if has_buyers {
                                palette.green
                            } else {
                                palette.amber
                            },
                            Color32::WHITE,
                        );
                        ui.add(
                            egui::Label::new(RichText::new(&hint_text).size(12.5).color(hint_color))
                                .wrap(true),
                        );
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let start_btn = egui::Button::new(
                                RichText::new("开始捡漏")
                                    .size(14.0)
                                    .strong()
                                    .color(if has_buyers {
                                        Color32::WHITE
                                    } else {
                                        palette.text_muted
                                    }),
                            )
                            .fill(if has_buyers {
                                palette.accent
                            } else {
                                palette.badge_bg
                            })
                            .rounding(7.0)
                            .min_size(Vec2::new(104.0, 34.0));

                            if ui.add_enabled(has_buyers, start_btn).clicked() {
                                submit_clicked = true;
                            }

                            if ui
                                .add(
                                    egui::Button::new("取消")
                                        .rounding(7.0)
                                        .min_size(Vec2::new(68.0, 34.0)),
                                )
                                .clicked()
                            {
                                cancel_clicked = true;
                            }
                        });
                    });
                },
                |ui| {
                    let card_outer_w = (ui.available_width() - 6.0).max(320.0);
                    let card_inner_w = (card_outer_w - 30.0).max(280.0);

                    // 精简模式提示条（矢量信息图标）
                    egui::Frame::none()
                        .fill(palette.blue_bg)
                        .stroke(Stroke::new(1.0, palette.blue.linear_multiply(0.35)))
                        .rounding(8.0)
                        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                        .show(ui, |ui| {
                            ui.set_width((card_outer_w - 26.0).max(260.0));
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 7.0;
                                let (ic_rect, _) =
                                    ui.allocate_exact_size(Vec2::new(16.0, 16.0), egui::Sense::hover());
                                ui.painter().circle_filled(ic_rect.center(), 7.5, palette.blue);
                                draw_vector_info(ui.painter(), ic_rect.center(), 6.2, Color32::WHITE);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(
                                            "系统将持续轮询监控所有可售场次与票种，发现回流余票立即尝试下单（仅支持实名制项目）",
                                        )
                                        .size(12.5)
                                        .color(palette.blue),
                                    )
                                    .wrap(true),
                                );
                            });
                        });

                    ui.add_space(10.0);

                    // 卡片 ①：选择实名购票人
                    let selected_names: Vec<String> = app
                        .selected_buyer_list
                        .as_ref()
                        .map(|list| {
                            list.iter()
                                .filter(|sel| buyers.iter().any(|b| b.id == sel.id))
                                .map(|b| b.name.clone())
                                .collect()
                        })
                        .unwrap_or_default();
                    let selected_count = selected_names.len();

                    egui::Frame::none()
                        .fill(palette.card_bg)
                        .stroke(Stroke::new(1.0, palette.card_border))
                        .rounding(10.0)
                        .inner_margin(egui::Margin::symmetric(14.0, 12.0))
                        .show(ui, |ui| {
                            ui.set_width(card_inner_w);

                            let selected_ids: Vec<i64> = app
                                .selected_buyer_list
                                .as_ref()
                                .map(|list| {
                                    list.iter()
                                        .filter(|sel| buyers.iter().any(|b| b.id == sel.id))
                                        .map(|b| b.id)
                                        .take(2)
                                        .collect()
                                })
                                .unwrap_or_default();
                            let can_save_default = !selected_ids.is_empty()
                                && selected_ids.len() <= 2
                                && (selected_ids != configured_default_ids
                                    || !app.custom_config.enable_default_buyers);

                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                ui.label(
                                    RichText::new("① 选择实名购票人")
                                        .size(15.0)
                                        .strong()
                                        .color(palette.text_primary),
                                );
                                let (bg, fg) = if selected_count > 0 {
                                    (palette.green_bg, palette.green)
                                } else {
                                    (palette.amber_bg, palette.amber)
                                };
                                pill_badge(
                                    ui,
                                    &format!("已选 {} 人", selected_count),
                                    bg,
                                    fg,
                                );

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    RichText::new("+ 新增")
                                                        .size(12.0)
                                                        .strong()
                                                        .color(Color32::WHITE),
                                                )
                                                .fill(palette.accent)
                                                .rounding(6.0)
                                                .min_size(Vec2::new(62.0, 26.0)),
                                            )
                                            .on_hover_text("为当前账号添加新的实名购票人")
                                            .clicked()
                                        {
                                            add_buyer_clicked = true;
                                        }
                                        if ui
                                            .add_enabled(
                                                !app.buyer_info_requesting,
                                                egui::Button::new(
                                                    RichText::new("刷新")
                                                        .size(12.0)
                                                        .color(palette.text_primary),
                                                )
                                                .fill(palette.item_bg)
                                                .stroke(Stroke::new(1.0, palette.item_border))
                                                .rounding(6.0)
                                                .min_size(Vec2::new(50.0, 26.0)),
                                            )
                                            .on_hover_text("重新拉取账号下的实名购票人列表")
                                            .clicked()
                                        {
                                            refresh_buyers_clicked = true;
                                        }
                                        if selected_count > 0
                                            && subtle_action_button(
                                                ui,
                                                "清空",
                                                palette.item_bg,
                                                palette.item_border,
                                                palette.text_secondary,
                                            )
                                            .on_hover_text("清空已选购票人")
                                            .clicked()
                                        {
                                            app.selected_buyer_list = None;
                                        }
                                        if can_save_default
                                            && subtle_action_button(
                                                ui,
                                                "设为默认",
                                                palette.blue_bg,
                                                palette.blue.linear_multiply(0.45),
                                                palette.blue,
                                            )
                                            .on_hover_text(
                                                "将当前已选购票人（最多2位）保存为本账号默认，下次自动勾选",
                                            )
                                            .clicked()
                                        {
                                            save_as_default_buyers = Some(selected_ids);
                                        }
                                    },
                                );
                            });

                            if selected_count > 1 {
                                ui.add_space(8.0);
                                egui::Frame::none()
                                    .fill(palette.amber_bg)
                                    .rounding(7.0)
                                    .inner_margin(egui::Margin::symmetric(10.0, 5.0))
                                    .show(ui, |ui| {
                                        ui.set_width((card_inner_w - 22.0).max(200.0));
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(
                                                    "提示：若目标项目仅限单人实名购票，多选购票人可能导致下单失败",
                                                )
                                                .size(12.0)
                                                .color(palette.amber),
                                            )
                                            .wrap(true),
                                        );
                                    });
                            }

                            ui.add_space(6.0);
                            if !selected_names.is_empty() {
                                ui.label(
                                    RichText::new(format!(
                                        "当前已选：{}   ·   点击下方卡片可切换（支持多选）",
                                        selected_names.join("、")
                                    ))
                                    .size(12.2)
                                    .color(palette.green),
                                );
                            } else {
                                ui.label(
                                    RichText::new("点击下方购票人卡片即可勾选（支持多选）")
                                        .size(12.2)
                                        .color(palette.text_muted),
                                );
                            }

                            ui.add_space(10.0);

                            if app.buyer_info_requesting {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(
                                        RichText::new("正在加载实名购票人...")
                                            .color(palette.text_secondary),
                                    );
                                });
                            } else if let Some(ref err_msg) = app.buyer_info_error {
                                egui::Frame::none()
                                    .fill(palette.error_bg)
                                    .stroke(Stroke::new(1.0, palette.red))
                                    .rounding(7.0)
                                    .inner_margin(10.0)
                                    .show(ui, |ui| {
                                        ui.set_width((card_inner_w - 22.0).max(200.0));
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(format!(
                                                    "获取购票人失败: {}",
                                                    err_msg
                                                ))
                                                .color(palette.red),
                                            )
                                            .wrap(true),
                                        );
                                        ui.add_space(6.0);
                                        if ui.button("重试获取购票人").clicked() {
                                            refresh_buyers_clicked = true;
                                        }
                                    });
                            } else if !buyers_loaded {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(
                                        RichText::new("正在加载实名购票人...")
                                            .color(palette.text_secondary),
                                    );
                                });
                            } else if buyers.is_empty() {
                                egui::Frame::none()
                                    .fill(palette.item_bg)
                                    .stroke(Stroke::new(1.0, palette.item_border))
                                    .rounding(8.0)
                                    .inner_margin(16.0)
                                    .show(ui, |ui| {
                                        ui.set_width((card_inner_w - 34.0).max(200.0));
                                        ui.vertical_centered(|ui| {
                                            ui.label(
                                                RichText::new("当前账号暂无实名购票人")
                                                    .size(14.0)
                                                    .strong()
                                                    .color(palette.amber),
                                            );
                                            ui.add_space(8.0);
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new("+ 立即添加购票人")
                                                            .color(Color32::WHITE)
                                                            .strong(),
                                                    )
                                                    .fill(palette.accent)
                                                    .rounding(7.0)
                                                    .min_size(Vec2::new(136.0, 32.0)),
                                                )
                                                .clicked()
                                            {
                                                add_buyer_clicked = true;
                                            }
                                        });
                                    });
                            } else {
                                let _ = render_buyer_cards_grid(
                                    ui,
                                    "confirm_mode2",
                                    &buyers,
                                    &configured_default_ids,
                                    &mut app.selected_buyer_list,
                                    true,
                                    card_inner_w,
                                    &palette,
                                );
                                if app.selected_buyer_list.is_some() {
                                    app.selected_buyer_uid = Some(biliticket_uid);
                                }
                            }
                        });

                    ui.add_space(10.0);

                    // 卡片 ②：跳过关键词过滤（可选）
                    egui::Frame::none()
                        .fill(palette.card_bg)
                        .stroke(Stroke::new(1.0, palette.card_border))
                        .rounding(10.0)
                        .inner_margin(egui::Margin::symmetric(14.0, 12.0))
                        .show(ui, |ui| {
                            ui.set_width(card_inner_w);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("② 跳过关键词过滤（可选）")
                                        .size(15.0)
                                        .strong()
                                        .color(palette.text_primary),
                                );
                                if app
                                    .skip_words
                                    .as_ref()
                                    .map_or(false, |words| !words.is_empty())
                                {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.small_button("清空全部").clicked() {
                                                app.skip_words = None;
                                                skip_words_changed = true;
                                            }
                                        },
                                    );
                                }
                            });
                            ui.add_space(3.0);
                            ui.label(
                                RichText::new(
                                    "命中关键词的票种将被自动跳过，多个词可用空格或逗号分隔",
                                )
                                .size(12.0)
                                .color(palette.text_muted),
                            );
                            ui.add_space(8.0);

                            let input_w = (card_inner_w - 86.0).clamp(160.0, 420.0);
                            ui.horizontal(|ui| {
                                let text_edit = ui.add(
                                    egui::TextEdit::singleline(&mut app.skip_words_input)
                                        .hint_text("例如：VIP、单日票、看台（回车或点击添加）")
                                        .desired_width(input_w)
                                        .min_size(Vec2::new(input_w, 30.0)),
                                );
                                let enter_pressed = text_edit.lost_focus()
                                    && !ime_composing
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                if ui
                                    .add(
                                        egui::Button::new("添加")
                                            .rounding(6.0)
                                            .min_size(Vec2::new(62.0, 30.0)),
                                    )
                                    .clicked()
                                    || enter_pressed
                                {
                                    merge_skip_words(&mut app.skip_words, &app.skip_words_input);
                                    app.skip_words_input.clear();
                                    skip_words_changed = true;
                                }
                            });

                            if let Some(words) = app.skip_words.clone() {
                                if !words.is_empty() {
                                    ui.add_space(8.0);
                                    let mut word_to_delete: Option<String> = None;

                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                                        for word in &words {
                                            let chip = egui::Button::new(
                                                RichText::new(format!("{}  ×", word))
                                                    .color(Color32::WHITE)
                                                    .size(12.5),
                                            )
                                            .fill(palette.accent)
                                            .rounding(12.0);

                                            if ui
                                                .add(chip)
                                                .on_hover_text("点击删除该跳过关键词")
                                                .clicked()
                                            {
                                                word_to_delete = Some(word.clone());
                                            }
                                        }
                                    });

                                    if let Some(word) = word_to_delete {
                                        if let Some(words_mut) = &mut app.skip_words {
                                            if let Some(pos) =
                                                words_mut.iter().position(|w| w == &word)
                                            {
                                                words_mut.remove(pos);
                                            }
                                            if words_mut.is_empty() {
                                                app.skip_words = None;
                                            }
                                            skip_words_changed = true;
                                        }
                                    }
                                }
                            }
                        });

                    ui.add_space(6.0);
                },
            );
        });

    if let Some(ids) = save_as_default_buyers {
        app.custom_config
            .set_default_buyers_for_uid(biliticket_uid, ids);
        app.custom_config.enable_default_buyers = true;
        if app.auto_save_settings() {
            app.show_success("已保存为当前账号默认购票人，下次打开将自动勾选");
        }
    }

    if skip_words_changed {
        app.persist_skip_words();
    }

    if add_buyer_clicked {
        app.add_buyer_input = crate::windows::add_buyer::AddBuyerInput::default();
        app.add_buyer_active_task_id = None;
        app.add_buyer_submitting = false;
        app.add_buyer_error = None;
        app.show_add_buyer_window = Some(uid.to_string());
    }

    if refresh_buyers_clicked {
        app.buyer_info_requesting = false;
        app.buyer_info_active_task_id = None;
        app.invalidate_buyer_cache(biliticket_uid);
        app.reset_buyer_selection();
    }

    if submit_clicked {
        // 若用户在输入框中键入了尚未点击"添加"的关键词，提交前自动合并并持久化
        if !app.skip_words_input.trim().is_empty() {
            merge_skip_words(&mut app.skip_words, &app.skip_words_input);
            app.skip_words_input.clear();
            app.persist_skip_words();
        }
        app.record_recent_project_and_save(&project_id, "");

        if let Some(buyer_list) = app.selected_buyer_list.clone() {
            let valid_list: Vec<_> = buyer_list
                .into_iter()
                .filter(|sel| buyers.iter().any(|b| b.id == sel.id))
                .collect();

            if !valid_list.is_empty() {
                let ids: Vec<i64> = valid_list.iter().map(|b| b.id).collect();
                log::info!("开始捡漏模式，选择的购票人IDs: {:?}", ids);

                let biliticket = &mut app.bilibiliticket_list[biliticket_index];
                biliticket.id_bind = 1;
                biliticket.buyer_info = Some(valid_list.clone());
                let biliticket_snapshot = biliticket.clone();

                let grab_ticket_request = GrabTicketRequest {
                    task_id: String::new(),
                    uid: biliticket_uid,
                    project_id,
                    screen_id: String::new(),
                    ticket_id: String::new(),
                    count: valid_list.len() as i16,
                    buyer_info: valid_list,
                    is_hot: true,
                    grab_mode: 2,
                    status: TaskStatus::Pending,
                    start_time: None,
                    cookie_manager,
                    biliticket: biliticket_snapshot,
                    local_captcha: app.local_captcha.clone(),
                    skip_words: app.skip_words.clone(),
                };

                log::debug!("提交捡漏模式任务: {:?}", grab_ticket_request);

                match app
                    .task_manager
                    .submit_task(TaskRequest::GrabTicketRequest(grab_ticket_request))
                {
                    Ok(task_id) => {
                        log::info!("提交捡漏模式任务成功，任务ID: {}", task_id);
                        app.show_success("捡漏任务已提交，请在「监视面板」查看进度");
                        app.confirm_ticket_info = None;
                        app.buyer_info_requesting = false;
                        app.buyer_info_active_task_id = None;
                        app.buyer_info_error = None;
                        app.reset_buyer_selection();
                    }
                    Err(e) => {
                        log::error!("提交捡漏模式任务失败: {}", e);
                        app.show_error(format!("提交捡漏模式任务失败: {}", e));
                    }
                }
            }
        }
    }

    if !open || cancel_clicked {
        app.confirm_ticket_info = None;
        app.buyer_info_requesting = false;
        app.buyer_info_active_task_id = None;
        app.buyer_info_error = None;
        app.reset_buyer_selection();
    }
}

/// 将输入串按空白/逗号拆分为关键词，去首尾空格并忽略大小写去重后追加
pub(crate) fn merge_skip_words(existing: &mut Option<Vec<String>>, raw_input: &str) {
    let mut words = existing.take().unwrap_or_default();
    for token in raw_input.split(|c: char| c.is_whitespace() || matches!(c, ',' | '，' | '、')) {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_lowercase();
        if !words.iter().any(|w| w.to_lowercase() == lower) {
            words.push(trimmed.to_string());
        }
    }
    if words.is_empty() {
        *existing = None;
    } else {
        *existing = Some(words);
    }
}

#[cfg(test)]
mod tests {
    use super::merge_skip_words;

    #[test]
    fn merge_skip_words_trims_and_deduplicates_case_insensitively() {
        let mut words = None;
        merge_skip_words(&mut words, "  VIP  vip, 预售，VIP  测试 ");
        assert_eq!(
            words,
            Some(vec![
                "VIP".to_string(),
                "预售".to_string(),
                "测试".to_string()
            ])
        );

        merge_skip_words(&mut words, "vip 测试 新增");
        assert_eq!(
            words,
            Some(vec![
                "VIP".to_string(),
                "预售".to_string(),
                "测试".to_string(),
                "新增".to_string()
            ])
        );

        let mut empty = None;
        merge_skip_words(&mut empty, "   , ， ");
        assert_eq!(empty, None);
    }
}
