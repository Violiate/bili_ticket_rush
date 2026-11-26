use crate::app::Myapp;
use chrono::TimeZone;
use eframe::egui::{self, Color32, RichText};
use serde_json::Value;

struct ChosenTicket {
    screen_id: usize,
    ticket_id: usize,
    project_id: Option<usize>,
    ticket_desc: String,
    clickable: bool,
}

pub fn show(app: &mut Myapp, ctx: &egui::Context, uid: i64) {
    let Some(ticket_idx) = app.bilibiliticket_list.iter().position(|t| t.uid == uid) else {
        log::error!("未找到uid为{}的抢票信息", uid);
        app.show_screen_info = None;
        app.project_info_requesting = false;
        app.project_info_active_task_id = None;
        app.project_info_error = None;
        app.is_loading = false;
        return;
    };

    let mut window_open = app.show_screen_info.is_some();
    if !window_open {
        return;
    }

    // 同步 id_bind 并在项目切换或索引越界时重置/钳位 selected_screen_index
    if let Some(td) = &app.bilibiliticket_list[ticket_idx].project_info {
        let id_bind = td.id_bind.unwrap_or(9);
        let screen_len = td.screen_list.len();
        app.bilibiliticket_list[ticket_idx].id_bind = id_bind;
        app.is_loading = false;
        if screen_len == 0 {
            app.selected_screen_index = None;
        } else if app
            .selected_screen_index
            .map_or(true, |idx| idx >= screen_len)
        {
            app.selected_screen_index = Some(0);
        }
    }

    let mut chosen_ticket: Option<ChosenTicket> = None;
    let mut close_clicked = false;
    let mut retry_clicked = false;

    // 使用不相交字段引用，避免每帧克隆庞大的 TicketInfo
    let project_info_ref = app.bilibiliticket_list[ticket_idx].project_info.as_ref();
    let account_name = app.bilibiliticket_list[ticket_idx].account.name.as_str();
    let ticket_method = app.bilibiliticket_list[ticket_idx].method;
    let draft_project_id = app.bilibiliticket_list[ticket_idx].project_id.as_str();
    let selected_screen_index = &mut app.selected_screen_index;
    let allow_force_select = &mut app.allow_force_select_unavailable;
    let project_info_error_ref = app.project_info_error.as_deref();

    let mode_label = match ticket_method {
        0 => "定时自动抢票",
        1 => "直接抢票",
        2 => "捡漏回流票",
        _ => "未知模式",
    };

    egui::Window::new("项目详情")
        .open(&mut window_open)
        .default_height(600.0)
        .default_width(820.0)
        .resizable(true)
        .show(ctx, |ui| {
            // 顶部目标账号与模式信息
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!("账号: {} ({})", account_name, uid))
                        .strong(),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!("项目ID: {}", draft_project_id))
                        .color(ui.visuals().weak_text_color()),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!("抢票模式: {}", mode_label))
                        .color(Color32::from_rgb(59, 130, 246)),
                );
            });
            ui.separator();

            // 错误状态：展示内联错误 + 重试 + 关闭，不卡全局遮罩、不触发死循环
            if let Some(err_msg) = project_info_error_ref {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    egui::Frame::none()
                        .fill(if ui.visuals().dark_mode {
                            Color32::from_rgb(60, 24, 24)
                        } else {
                            Color32::from_rgb(254, 242, 242)
                        })
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(220, 68, 55)))
                        .rounding(8.0)
                        .inner_margin(16.0)
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("获取项目详情失败")
                                    .size(16.0)
                                    .strong()
                                    .color(Color32::from_rgb(220, 68, 55)),
                            );
                            ui.add_space(6.0);
                            ui.add(egui::Label::new(err_msg).wrap(true));
                        });

                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        if ui.button("重试").clicked() {
                            retry_clicked = true;
                        }
                        if ui.button("关闭").clicked() {
                            close_clicked = true;
                        }
                    });
                });
                ui.add_space(24.0);
                return;
            }

            // 加载状态：窗口内展示内联 spinner
            let Some(ticket_data) = project_info_ref else {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.spinner();
                    ui.add_space(12.0);
                    ui.label("正在获取项目信息，请稍候...");
                    ui.add_space(16.0);
                    if ui.button("取消").clicked() {
                        close_clicked = true;
                    }
                    ui.add_space(30.0);
                });
                return;
            };

            egui::ScrollArea::vertical()
                .id_source("screen_info_scroll")
                .show(ui, |ui| {
                    // 项目标题区
                    ui.vertical_centered(|ui| {
                        ui.heading(ticket_data.name.as_deref().unwrap_or("未知项目"));
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("售票状态: {}", ticket_data.sale_flag))
                                .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(8.0);
                    });

                    ui.separator();

                    // 场次选择区
                    ui.heading("选择场次");
                    ui.add_space(5.0);

                    if ticket_data.screen_list.is_empty() {
                        ui.label(
                            RichText::new("该项目当前暂无可选场次")
                                .color(ui.visuals().weak_text_color()),
                        );
                    } else {
                        egui::ScrollArea::horizontal()
                            .id_source("screen_tabs")
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    for (idx, screen) in ticket_data.screen_list.iter().enumerate()
                                    {
                                        let is_selected = *selected_screen_index == Some(idx);
                                        let label = format!(
                                            "{} ({})",
                                            screen.name.as_deref().unwrap_or("未知场次"),
                                            &screen.sale_flag.display_name
                                        );
                                        if ui
                                            .add(egui::SelectableLabel::new(is_selected, label))
                                            .clicked()
                                        {
                                            *selected_screen_index = Some(idx);
                                        }
                                    }
                                });
                            });
                    }

                    ui.add_space(10.0);

                    // 显示选中场次的票种信息
                    if let Some(idx) = *selected_screen_index {
                        if let Some(selected_screen) = ticket_data.screen_list.get(idx) {
                            let bg_color = if ui.visuals().dark_mode {
                                Color32::from_rgb(32, 35, 42)
                            } else {
                                Color32::from_rgb(245, 246, 250)
                            };
                            egui::Frame::none()
                                .fill(bg_color)
                                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                                .rounding(8.0)
                                .inner_margin(12.0)
                                .show(ui, |ui| {
                                    // 场次基本信息
                                    egui::Grid::new("screen_meta_grid")
                                        .num_columns(2)
                                        .spacing([24.0, 4.0])
                                        .show(ui, |ui| {
                                            ui.label(format!(
                                                "开始时间: {}",
                                                format_timestamp(
                                                    selected_screen.start_time.unwrap_or(0)
                                                )
                                            ));
                                            ui.label(format!(
                                                "售票状态: {}",
                                                selected_screen.sale_flag.display_name
                                            ));
                                            ui.end_row();
                                            ui.label(format!(
                                                "售票开始: {}",
                                                format_timestamp(selected_screen.sale_start)
                                            ));
                                            ui.label(format!(
                                                "售票结束: {}",
                                                format_timestamp(selected_screen.sale_end)
                                            ));
                                            ui.end_row();
                                        });

                                    ui.add_space(8.0);
                                    ui.separator();
                                    ui.add_space(8.0);

                                    // 票种标题与强选开关
                                    ui.horizontal(|ui| {
                                        ui.heading("票种列表");
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.checkbox(
                                                    allow_force_select,
                                                    "允许强选不可售票种",
                                                )
                                                .on_hover_text(
                                                    "勾选后可强制选择已售罄/已停售/不可售的票种（未开售与暂时售罄票种默认始终可选）",
                                                );
                                            },
                                        );
                                    });

                                    ui.add_space(6.0);

                                    if selected_screen.ticket_list.is_empty() {
                                        ui.label(
                                            RichText::new("该场次暂无票种信息")
                                                .color(ui.visuals().weak_text_color()),
                                        );
                                    } else {
                                        let name_col_width =
                                            (ui.available_width() - 320.0).max(180.0);

                                        egui::Grid::new("ticket_list_grid")
                                            .num_columns(4)
                                            .striped(true)
                                            .spacing([14.0, 8.0])
                                            .show(ui, |ui| {
                                                // 表头
                                                ui.add_sized(
                                                    [name_col_width, 20.0],
                                                    egui::Label::new(
                                                        RichText::new("票种名称").strong(),
                                                    ),
                                                );
                                                ui.add_sized(
                                                    [85.0, 20.0],
                                                    egui::Label::new(
                                                        RichText::new("价格").strong(),
                                                    ),
                                                );
                                                ui.add_sized(
                                                    [95.0, 20.0],
                                                    egui::Label::new(
                                                        RichText::new("状态").strong(),
                                                    ),
                                                );
                                                ui.add_sized(
                                                    [105.0, 20.0],
                                                    egui::Label::new(
                                                        RichText::new("操作").strong(),
                                                    ),
                                                );
                                                ui.end_row();

                                                for ticket in &selected_screen.ticket_list {
                                                    let desc = ticket
                                                        .desc
                                                        .as_deref()
                                                        .unwrap_or("未知票种");
                                                    ui.allocate_ui_with_layout(
                                                        egui::vec2(name_col_width, 26.0),
                                                        egui::Layout::left_to_right(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            ui.add(
                                                                egui::Label::new(desc).wrap(true),
                                                            );
                                                        },
                                                    );

                                                    // 价格
                                                    let price = format!(
                                                        "¥{:.2}",
                                                        ticket.price.unwrap_or(0) as f64 / 100.0
                                                    );
                                                    ui.add_sized(
                                                        [85.0, 26.0],
                                                        egui::Label::new(
                                                            RichText::new(price).strong().color(
                                                                Color32::from_rgb(245, 108, 108),
                                                            ),
                                                        ),
                                                    );

                                                    // 状态
                                                    let flag_num =
                                                        ticket.sale_flag_number.unwrap_or(
                                                            ticket.sale_flag.number,
                                                        );
                                                    let status_color = if ticket
                                                        .clickable
                                                        .unwrap_or(false)
                                                    {
                                                        Color32::from_rgb(34, 197, 94)
                                                    } else if flag_num == 1 {
                                                        Color32::from_rgb(59, 130, 246)
                                                    } else if flag_num == 8 {
                                                        Color32::from_rgb(245, 158, 11)
                                                    } else {
                                                        ui.visuals().weak_text_color()
                                                    };
                                                    ui.add_sized(
                                                        [95.0, 26.0],
                                                        egui::Label::new(
                                                            RichText::new(
                                                                &ticket.sale_flag.display_name,
                                                            )
                                                            .color(status_color),
                                                        ),
                                                    );

                                                    // 操作按钮（智能状态处理，保证未开售零回归）
                                                    let clickable =
                                                        ticket.clickable.unwrap_or(false);
                                                    let (button_text, button_enabled, hover_hint) =
                                                        if clickable {
                                                            ("选择", true, None)
                                                        } else if flag_num == 1 {
                                                            (
                                                                "定时预选",
                                                                true,
                                                                Some(
                                                                    "该票种尚未开售，可预选用于定时自动抢票",
                                                                ),
                                                            )
                                                        } else if flag_num == 8 {
                                                            (
                                                                "预选(暂售罄)",
                                                                true,
                                                                Some(
                                                                    "警告：该票种当前暂时售罄，预选后将等待回流或尝试抢票",
                                                                ),
                                                            )
                                                        } else {
                                                            (
                                                                "不可选",
                                                                *allow_force_select,
                                                                Some(
                                                                    "该票种当前不可售；如需强行选择，请先勾选上方「允许强选不可售票种」",
                                                                ),
                                                            )
                                                        };

                                                    let mut btn_resp = ui.add_enabled(
                                                        button_enabled,
                                                        egui::Button::new(button_text)
                                                            .min_size(egui::vec2(96.0, 26.0)),
                                                    );
                                                    if let Some(hint) = hover_hint {
                                                        btn_resp = btn_resp.on_hover_text(hint);
                                                    }

                                                    if btn_resp.clicked() {
                                                        chosen_ticket = Some(ChosenTicket {
                                                            screen_id: selected_screen
                                                                .id
                                                                .unwrap_or(0),
                                                            ticket_id: ticket.id.unwrap_or(0),
                                                            project_id: ticket.project_id,
                                                            ticket_desc: desc.to_string(),
                                                            clickable,
                                                        });
                                                    }
                                                    ui.end_row();
                                                }
                                            });
                                    }
                                });
                        }
                    }

                    // 项目详细信息区
                    ui.add_space(10.0);
                    ui.collapsing("查看详细信息", |ui| {
                        ui.label("基本信息:");
                        ui.indent("basic_info", |ui| {
                            ui.label(format!("项目ID: {}", ticket_data.id.unwrap_or(0)));

                            if let Some(desc) = &ticket_data.performance_desc {
                                for item in &desc.list {
                                    if item.module == "base_info" {
                                        if let Some(array) = item.details.as_array() {
                                            for info_item in array {
                                                if let (Some(title), Some(content)) = (
                                                    info_item
                                                        .get("title")
                                                        .and_then(Value::as_str),
                                                    info_item
                                                        .get("content")
                                                        .and_then(Value::as_str),
                                                ) {
                                                    ui.horizontal_wrapped(|ui| {
                                                        ui.label(
                                                            RichText::new(format!("{}:", title))
                                                                .strong(),
                                                        );
                                                        ui.label(content);
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        });
                    });
                });
        });

    if retry_clicked {
        app.project_info_requesting = false;
        app.project_info_active_task_id = None;
        app.project_info_error = None;
        app.project_info_last_request_time = None;
        return;
    }

    if let Some(chosen) = chosen_ticket {
        if !chosen.clickable && ticket_method == 1 {
            log::warn!(
                "请注意！该票种目前不可售，直接抢票模式会持续尝试下单，可能触发风控限制"
            );
            app.show_info("提示：当前所选票种暂不可售，直接抢票模式会持续尝试下单");
        }

        let bilibili_ticket = &mut app.bilibiliticket_list[ticket_idx];
        bilibili_ticket.screen_id = chosen.screen_id.to_string();
        bilibili_ticket.select_ticket_id = Some(chosen.ticket_id.to_string());
        // 保留 bilibili_ticket.project_id，不覆写全局 app.ticket_id
        if bilibili_ticket.project_id.trim().is_empty() {
            if let Some(pid) = chosen.project_id.filter(|&id| id > 0) {
                bilibili_ticket.project_id = pid.to_string();
            }
        }

        log::debug!(
            "选择场次/票种: screen_id={}, ticket_id={}, project_id={}",
            chosen.screen_id,
            chosen.ticket_id,
            bilibili_ticket.project_id
        );
        log::info!("已选择: {} [{}]", chosen.ticket_desc, chosen.ticket_id);

        app.selected_screen_id = Some(chosen.screen_id as i64);
        app.selected_ticket_id = Some(chosen.ticket_id as i64);
        app.project_info_requesting = false;
        app.project_info_active_task_id = None;
        app.buyer_info_requesting = false;
        app.buyer_info_active_task_id = None;
        app.buyer_info_error = None;
        app.buyer_info_last_request_time = None;
        app.reset_buyer_selection();
        app.show_screen_info = None;
        app.confirm_ticket_info = Some(uid.to_string());
        return;
    }

    if !window_open || close_clicked {
        app.show_screen_info = None;
        app.project_info_requesting = false;
        app.project_info_active_task_id = None;
        app.project_info_error = None;
        app.is_loading = false;
        if let Some(bt) = app.bilibiliticket_list.get_mut(ticket_idx) {
            bt.project_info = None;
        }
    }
}

// 将时间戳转换为可读时间
fn format_timestamp(timestamp: usize) -> String {
    if timestamp == 0 {
        return "未设置".to_string();
    }

    let timestamp_i64 = match i64::try_from(timestamp) {
        Ok(ts) => ts,
        Err(_) => return "时间戳溢出".to_string(),
    };

    match chrono::Local.timestamp_opt(timestamp_i64, 0) {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        _ => "无效时间".to_string(),
    }
}
