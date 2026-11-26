use std::sync::Arc;

use common::cookie_manager::CookieManager;
use common::taskmanager::{GrabTicketRequest, TaskRequest, TaskStatus};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};

use crate::app::Myapp;
use crate::windows::confirm_ticket::mask_id;

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

    let mut cancel_clicked = false;
    let mut submit_clicked = false;
    let mut refresh_buyers_clicked = false;
    let mut add_buyer_clicked = false;

    egui::Window::new("捡漏模式 - 选择购票人")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(540.0)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 12.0);

            // 顶部目标账号与项目ID
            ui.vertical_centered(|ui| {
                ui.heading("捡漏模式 - 选择购票人");
                ui.add_space(2.0);
                ui.label(
                    RichText::new(format!(
                        "目标账号: {} (UID: {})  |  项目ID: {}",
                        account_name, biliticket_uid, project_id
                    ))
                    .color(ui.visuals().weak_text_color()),
                );
                ui.label(
                    RichText::new("系统会自动监控可用票种并尝试抢票")
                        .color(ui.visuals().weak_text_color()),
                );
            });
            ui.separator();

            // 主题适配的说明卡片（亮色/暗色均保持高对比度）
            let (tip_bg, tip_stroke, tip_text) = if ui.visuals().dark_mode {
                (
                    Color32::from_rgb(48, 40, 24),
                    Color32::from_rgb(138, 108, 48),
                    Color32::from_rgb(245, 224, 170),
                )
            } else {
                (
                    Color32::from_rgb(254, 249, 231),
                    Color32::from_rgb(235, 198, 106),
                    Color32::from_rgb(92, 68, 18),
                )
            };

            egui::Frame::none()
                .fill(tip_bg)
                .stroke(Stroke::new(1.0, tip_stroke))
                .rounding(8.0)
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.label(RichText::new("捡漏模式说明：").strong().color(tip_text));
                    ui.label(
                        RichText::new("1. 系统将持续监控所有可售场次和票种").color(tip_text),
                    );
                    ui.label(
                        RichText::new("2. 一旦发现有余票，会立即尝试创建订单").color(tip_text),
                    );
                    ui.label(
                        RichText::new("3. 仅支持实名制项目捡漏，请先选择至少一位购票人")
                            .color(tip_text),
                    );
                });

            let selected_count = app.selected_buyer_list.as_ref().map_or(0, |list| {
                list.iter()
                    .filter(|sel| buyers.iter().any(|b| b.id == sel.id))
                    .count()
            });

            ui.horizontal(|ui| {
                ui.heading("选择购票人");
                ui.label(
                    RichText::new(format!("(已选 {} 人)", selected_count)).color(
                        if selected_count > 0 {
                            Color32::from_rgb(34, 197, 94)
                        } else {
                            ui.visuals().weak_text_color()
                        },
                    ),
                );
            });

            if selected_count > 1 {
                ui.label(
                    RichText::new(
                        "提示：如果目标项目仅支持单人实名购票，多选购票人可能会导致下单失败",
                    )
                    .size(13.0)
                    .color(Color32::from_rgb(217, 119, 6)),
                );
            }

            // 购票人列表（区分加载中 / 加载失败 / 空列表 / 已加载）
            if app.buyer_info_requesting {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("正在加载购票人...");
                });
            } else if let Some(ref err_msg) = app.buyer_info_error {
                egui::Frame::none()
                    .fill(if ui.visuals().dark_mode {
                        Color32::from_rgb(60, 24, 24)
                    } else {
                        Color32::from_rgb(254, 242, 242)
                    })
                    .stroke(Stroke::new(1.0, Color32::from_rgb(220, 68, 55)))
                    .rounding(6.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!("获取购票人失败: {}", err_msg))
                                    .color(Color32::from_rgb(220, 68, 55)),
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
                    ui.label("正在加载购票人...");
                });
            } else if buyers.is_empty() {
                ui.label(
                    RichText::new("暂无购票人，请先添加").color(Color32::from_rgb(217, 119, 6)),
                );
            } else {
                egui::ScrollArea::vertical()
                    .id_source("confirm_ticket2_buyers_scroll")
                    .max_height(240.0)
                    .show(ui, |ui| {
                        let available_width = ui.available_width();
                        let card_width = 235.0;
                        let columns = (available_width / card_width).max(1.0).floor() as usize;

                        egui::Grid::new("buyers_grid_mode2")
                            .num_columns(columns)
                            .spacing([10.0, 10.0])
                            .show(ui, |ui| {
                                for (index, buyer) in buyers.iter().enumerate() {
                                    let is_selected = app
                                        .selected_buyer_list
                                        .as_ref()
                                        .map_or(false, |list| {
                                            list.iter().any(|b| b.id == buyer.id)
                                        });

                                    let card_color = if !ui.visuals().dark_mode {
                                        if is_selected {
                                            Color32::from_rgb(236, 252, 243)
                                        } else {
                                            Color32::from_rgb(245, 245, 250)
                                        }
                                    } else if is_selected {
                                        Color32::from_rgb(22, 52, 34)
                                    } else {
                                        Color32::from_rgb(32, 35, 42)
                                    };

                                    let stroke_color = if is_selected {
                                        Color32::from_rgb(34, 197, 94)
                                    } else {
                                        ui.visuals().widgets.noninteractive.bg_stroke.color
                                    };

                                    ui.scope(|ui| {
                                        ui.set_width(card_width - 10.0);

                                        egui::Frame::none()
                                            .fill(card_color)
                                            .stroke(Stroke::new(1.0, stroke_color))
                                            .rounding(8.0)
                                            .inner_margin(10.0)
                                            .show(ui, |ui| {
                                                let id_type_text = match buyer.id_type {
                                                    0 => "身份证",
                                                    1 => "护照",
                                                    2 => "港澳通行证",
                                                    3 => "台湾通行证",
                                                    _ => "其他证件",
                                                };

                                                ui.horizontal(|ui| {
                                                    let select_button = if is_selected {
                                                        ui.add(
                                                            egui::Button::new("☑").fill(
                                                                Color32::from_rgb(34, 197, 94),
                                                            ),
                                                        )
                                                    } else {
                                                        ui.add(
                                                            egui::Button::new("☐")
                                                                .fill(Color32::TRANSPARENT),
                                                        )
                                                    };

                                                    if select_button.clicked() {
                                                        app.selected_buyer_uid =
                                                            Some(biliticket_uid);
                                                        let buyer_list = app
                                                            .selected_buyer_list
                                                            .get_or_insert_with(Vec::new);
                                                        if let Some(pos) = buyer_list
                                                            .iter()
                                                            .position(|b| b.id == buyer.id)
                                                        {
                                                            buyer_list.remove(pos);
                                                        } else {
                                                            buyer_list.push(buyer.clone());
                                                        }
                                                    }

                                                    ui.vertical(|ui| {
                                                        ui.horizontal(|ui| {
                                                            ui.label(
                                                                RichText::new(&buyer.name)
                                                                    .strong()
                                                                    .size(15.0),
                                                            );
                                                            ui.label(
                                                                RichText::new(id_type_text)
                                                                    .weak()
                                                                    .size(12.5),
                                                            );
                                                        });
                                                        ui.label(format!(
                                                            "证件号: {}",
                                                            mask_id(&buyer.personal_id)
                                                        ));
                                                        ui.label(format!("手机号: {}", buyer.tel));
                                                    });
                                                });
                                            });
                                    });

                                    if (index + 1) % columns == 0 && index < buyers.len() - 1 {
                                        ui.end_row();
                                    }
                                }
                            });
                    });
            }

            // 无论购票人列表是否为空，始终提供添加与刷新按钮
            ui.horizontal(|ui| {
                if ui.button("添加新购票人").clicked() {
                    add_buyer_clicked = true;
                }
                if ui
                    .add_enabled(!app.buyer_info_requesting, egui::Button::new("刷新购票人"))
                    .clicked()
                {
                    refresh_buyers_clicked = true;
                }
            });

            ui.separator();

            // 关键词过滤
            ui.heading("关键词过滤");
            ui.label(
                RichText::new(
                    "输入需要跳过的关键词（多个词可用空格或逗号分隔），命中关键词的票种将自动跳过。",
                )
                .color(ui.visuals().weak_text_color()),
            );

            ui.horizontal(|ui| {
                ui.label("过滤关键词:");
                let text_edit = ui.add(
                    egui::TextEdit::singleline(&mut app.skip_words_input)
                        .hint_text("例如: VIP 预售（回车或点击添加）")
                        .desired_width(240.0),
                );
                let enter_pressed = text_edit.lost_focus()
                    && !app.is_ime_composing()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("添加").clicked() || enter_pressed {
                    merge_skip_words(&mut app.skip_words, &app.skip_words_input);
                    app.skip_words_input.clear();
                }
                if app
                    .skip_words
                    .as_ref()
                    .map_or(false, |words| !words.is_empty())
                    && ui.button("清空").clicked()
                {
                    app.skip_words = None;
                }
            });

            if let Some(words) = app.skip_words.clone() {
                if !words.is_empty() {
                    let mut word_to_delete: Option<String> = None;

                    ui.horizontal_wrapped(|ui| {
                        ui.label("当前过滤词:");
                        for word in &words {
                            let chip = egui::Button::new(
                                RichText::new(format!("{} ×", word))
                                    .color(Color32::WHITE)
                                    .size(13.0),
                            )
                            .fill(Color32::from_rgb(59, 130, 246))
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
                            if let Some(pos) = words_mut.iter().position(|w| w == &word) {
                                words_mut.remove(pos);
                            }
                            if words_mut.is_empty() {
                                app.skip_words = None;
                            }
                        }
                    }
                }
            }

            ui.separator();

            // 底部按钮
            let has_buyers = app.selected_buyer_list.as_ref().map_or(false, |list| {
                !list.is_empty()
                    && list
                        .iter()
                        .all(|sel| buyers.iter().any(|b| b.id == sel.id))
            });

            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            has_buyers,
                            egui::Button::new(
                                RichText::new("开始捡漏").color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(59, 130, 246))
                            .min_size(Vec2::new(100.0, 34.0)),
                        )
                        .clicked()
                    {
                        submit_clicked = true;
                    }

                    if ui.button("取消").clicked() {
                        cancel_clicked = true;
                    }
                });
            });
        });

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
        // 若用户在输入框中键入了尚未点击"添加"的关键词，提交前自动合并
        if !app.skip_words_input.trim().is_empty() {
            merge_skip_words(&mut app.skip_words, &app.skip_words_input);
            app.skip_words_input.clear();
        }

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
