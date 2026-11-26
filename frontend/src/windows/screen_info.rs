use chrono::TimeZone;
use common::ticket::{ScreenInfo, ScreenTicketInfo, TicketInfo};
use eframe::egui::{self, Color32, RichText, Stroke};
use serde_json::Value;

use crate::app::Myapp;
use crate::windows::{
    bounded_window_sizes, show_resizable_body, show_resizable_frame, show_resizable_frame_with,
};

const TICKET_RIGHT_COL_W: f32 = 214.0;
const TICKET_ROW_SPACING: f32 = 8.0;
const TICKET_ROW_SLACK: f32 = 6.0;

struct ChosenTicket {
    screen_id: usize,
    ticket_id: usize,
    project_id: Option<usize>,
    ticket_desc: String,
    clickable: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum StatusTone {
    Available,
    Presale,
    Warning,
    Muted,
}

struct ScreenPalette {
    card_bg: Color32,
    card_border: Color32,
    row_bg: Color32,
    row_hover_bg: Color32,
    row_border: Color32,
    selected_bg: Color32,
    selected_border: Color32,
    text_primary: Color32,
    text_secondary: Color32,
    text_muted: Color32,
    accent: Color32,
    green: Color32,
    green_bg: Color32,
    blue: Color32,
    blue_bg: Color32,
    amber: Color32,
    amber_bg: Color32,
    muted_bg: Color32,
    price_red: Color32,
    hot_bg: Color32,
    hot_text: Color32,
    error_bg: Color32,
    error_text: Color32,
}

impl ScreenPalette {
    fn from_dark_mode(dark: bool) -> Self {
        if dark {
            Self {
                card_bg: Color32::from_rgb(30, 33, 41),
                card_border: Color32::from_rgb(56, 62, 76),
                row_bg: Color32::from_rgb(38, 42, 52),
                row_hover_bg: Color32::from_rgb(45, 50, 62),
                row_border: Color32::from_rgb(64, 70, 86),
                selected_bg: Color32::from_rgb(28, 52, 86),
                selected_border: Color32::from_rgb(59, 130, 246),
                text_primary: Color32::from_rgb(236, 239, 246),
                text_secondary: Color32::from_rgb(186, 192, 208),
                text_muted: Color32::from_rgb(138, 145, 162),
                accent: Color32::from_rgb(45, 136, 245),
                green: Color32::from_rgb(74, 222, 128),
                green_bg: Color32::from_rgb(22, 62, 38),
                blue: Color32::from_rgb(96, 165, 250),
                blue_bg: Color32::from_rgb(24, 48, 82),
                amber: Color32::from_rgb(251, 191, 36),
                amber_bg: Color32::from_rgb(66, 48, 20),
                muted_bg: Color32::from_rgb(50, 54, 66),
                price_red: Color32::from_rgb(248, 113, 113),
                hot_bg: Color32::from_rgb(82, 32, 42),
                hot_text: Color32::from_rgb(251, 114, 153),
                error_bg: Color32::from_rgb(60, 24, 24),
                error_text: Color32::from_rgb(248, 113, 113),
            }
        } else {
            Self {
                card_bg: Color32::from_rgb(246, 248, 252),
                card_border: Color32::from_rgb(214, 220, 232),
                row_bg: Color32::from_rgb(255, 255, 255),
                row_hover_bg: Color32::from_rgb(244, 248, 255),
                row_border: Color32::from_rgb(222, 227, 238),
                selected_bg: Color32::from_rgb(232, 242, 255),
                selected_border: Color32::from_rgb(45, 136, 245),
                text_primary: Color32::from_rgb(28, 33, 45),
                text_secondary: Color32::from_rgb(78, 86, 104),
                text_muted: Color32::from_rgb(122, 130, 148),
                accent: Color32::from_rgb(45, 136, 245),
                green: Color32::from_rgb(22, 163, 74),
                green_bg: Color32::from_rgb(228, 248, 235),
                blue: Color32::from_rgb(37, 99, 235),
                blue_bg: Color32::from_rgb(228, 239, 255),
                amber: Color32::from_rgb(194, 108, 12),
                amber_bg: Color32::from_rgb(255, 244, 218),
                muted_bg: Color32::from_rgb(234, 238, 246),
                price_red: Color32::from_rgb(225, 58, 58),
                hot_bg: Color32::from_rgb(254, 232, 240),
                hot_text: Color32::from_rgb(219, 39, 119),
                error_bg: Color32::from_rgb(254, 242, 242),
                error_text: Color32::from_rgb(210, 52, 46),
            }
        }
    }

    fn tone_colors(&self, tone: StatusTone) -> (Color32, Color32) {
        match tone {
            StatusTone::Available => (self.green_bg, self.green),
            StatusTone::Presale => (self.blue_bg, self.blue),
            StatusTone::Warning => (self.amber_bg, self.amber),
            StatusTone::Muted => (self.muted_bg, self.text_muted),
        }
    }
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
    let project_info_requesting = app.project_info_requesting;

    let mode_label = match ticket_method {
        0 => "定时自动抢票",
        1 => "直接抢票",
        2 => "捡漏回流票",
        _ => "未知模式",
    };

    let (default_sz, min_sz, max_sz) = bounded_window_sizes(ctx, 780.0, 600.0, 520.0, 380.0);

    let dark_mode = ctx.style().visuals.dark_mode;
    let palette = ScreenPalette::from_dark_mode(dark_mode);

    egui::Window::new("选择场次与票种")
        .id(egui::Id::new("screen_info_window_v2"))
        .open(&mut window_open)
        .default_size(default_sz)
        .min_width(min_sz[0])
        .min_height(min_sz[1])
        .resize(|r| r.max_size(max_sz))
        .constrain(true)
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            let render_header = |ui: &mut egui::Ui, retry_clicked: &mut bool| {
                // 顶部账号 / 项目ID / 模式信息栏 + 刷新按钮
                ui.horizontal(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(8.0, 4.0);
                        ui.label(
                            RichText::new(format!("账号: {} ({})", account_name, uid))
                                .size(13.0)
                                .strong()
                                .color(palette.text_primary),
                        );
                        ui.label(RichText::new("·").color(palette.text_muted));
                        ui.label(
                            RichText::new(format!("项目ID: {}", draft_project_id))
                                .size(13.0)
                                .color(palette.text_secondary),
                        );
                        ui.label(RichText::new("·").color(palette.text_muted));
                        pill_badge(ui, mode_label, palette.blue_bg, palette.blue);
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let refresh_label = if project_info_requesting {
                            "刷新中..."
                        } else {
                            "刷新项目"
                        };
                        if ui
                            .add_enabled(
                                !project_info_requesting,
                                egui::Button::new(RichText::new(refresh_label).size(12.5))
                                    .rounding(6.0),
                            )
                            .on_hover_text("重新拉取该项目最新的场次与余票状态")
                            .clicked()
                        {
                            *retry_clicked = true;
                        }
                    });
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(6.0);
            };

            // 错误状态：保持全区域可缩放骨架，展示内联错误 + 重试 + 关闭
            if let Some(err_msg) = project_info_error_ref {
                show_resizable_frame_with(
                    ui,
                    &mut retry_clicked,
                    |ui, retry| render_header(ui, retry),
                    |_, _| {},
                    |ui, retry| {
                        ui.add_space(28.0);
                        ui.vertical_centered(|ui| {
                            egui::Frame::none()
                                .fill(palette.error_bg)
                                .stroke(Stroke::new(1.0, palette.error_text))
                                .rounding(8.0)
                                .inner_margin(16.0)
                                .show(ui, |ui| {
                                    ui.set_max_width((ui.available_width() - 32.0).max(260.0));
                                    ui.label(
                                        RichText::new("获取项目详情失败")
                                            .size(16.0)
                                            .strong()
                                            .color(palette.error_text),
                                    );
                                    ui.add_space(6.0);
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(err_msg).color(palette.text_primary),
                                        )
                                        .wrap(true),
                                    );
                                });

                            ui.add_space(16.0);
                            ui.horizontal(|ui| {
                                let btn_w = 96.0;
                                let total_w = btn_w * 2.0 + 12.0;
                                let pad = ((ui.available_width() - total_w) * 0.5).max(0.0);
                                ui.add_space(pad);
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("重试").color(Color32::WHITE).strong(),
                                        )
                                        .fill(palette.accent)
                                        .rounding(7.0)
                                        .min_size(egui::vec2(btn_w, 32.0)),
                                    )
                                    .clicked()
                                {
                                    *retry = true;
                                }
                                if ui
                                    .add(
                                        egui::Button::new("关闭")
                                            .rounding(7.0)
                                            .min_size(egui::vec2(btn_w, 32.0)),
                                    )
                                    .clicked()
                                {
                                    close_clicked = true;
                                }
                            });
                        });
                    },
                );
                return;
            }

            // 加载状态：保持全区域可缩放骨架，展示内联 spinner
            let Some(ticket_data) = project_info_ref else {
                show_resizable_frame(
                    ui,
                    |ui| render_header(ui, &mut retry_clicked),
                    |_| {},
                    |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(56.0);
                            ui.add(egui::Spinner::new().size(28.0));
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new("正在获取项目详情与场次列表，请稍候...")
                                    .size(14.5)
                                    .color(palette.text_secondary),
                            );
                            ui.add_space(18.0);
                            if ui
                                .add(
                                    egui::Button::new("取消")
                                        .rounding(7.0)
                                        .min_size(egui::vec2(88.0, 30.0)),
                                )
                                .clicked()
                            {
                                close_clicked = true;
                            }
                        });
                    },
                );
                return;
            };

            let footer_tip = match ticket_method {
                0 => "点击目标票种即可进入购票人确认，开售前将自动倒计时待命",
                1 => "直接抢票会立即尝试下单，未开售票种可能触发风控",
                _ => "选择票种后进入确认页即可提交任务",
            };

            show_resizable_body(
                ui,
                "screen_info_scroll",
                |ui| render_header(ui, &mut retry_clicked),
                |ui| {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let tip_w = (ui.available_width() - 88.0).max(120.0);
                        ui.allocate_ui_with_layout(
                            egui::vec2(tip_w, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_max_width(tip_w);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(footer_tip)
                                            .size(12.0)
                                            .color(palette.text_muted),
                                    )
                                    .wrap(true),
                                );
                            },
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new("关闭")
                                        .rounding(6.0)
                                        .min_size(egui::vec2(72.0, 28.0)),
                                )
                                .clicked()
                            {
                                close_clicked = true;
                            }
                        });
                    });
                },
                |ui| {
                    // 统一扣除安全余量，使全部卡片边缘齐平且严格小于滚动区宽度，彻底杜绝横向漂移
                    let card_outer_w = (ui.available_width() - 6.0).max(320.0);
                    let card_inner_w = (card_outer_w - 30.0).max(280.0);

                    // 1. 项目概览头部
                    render_project_header(ui, ticket_data, card_inner_w, &palette);
                    ui.add_space(10.0);

                    // 2. 步骤①：选择场次
                    render_screen_selector_card(
                        ui,
                        ticket_data,
                        selected_screen_index,
                        card_inner_w,
                        &palette,
                    );
                    ui.add_space(10.0);

                    // 3. 步骤②：选择票种
                    if let Some(idx) = *selected_screen_index {
                        if let Some(selected_screen) = ticket_data.screen_list.get(idx) {
                            if let Some(picked) = render_ticket_list_card(
                                ui,
                                selected_screen,
                                allow_force_select,
                                card_inner_w,
                                &palette,
                            ) {
                                chosen_ticket = Some(picked);
                            }
                        }
                    }

                    // 4. 折叠区：活动须知与基础信息
                    ui.add_space(10.0);
                    render_project_details_collapsing(ui, ticket_data, card_inner_w, &palette);
                    ui.add_space(6.0);
                },
            );
        });

    if retry_clicked {
        app.project_info_requesting = false;
        app.project_info_active_task_id = None;
        app.project_info_error = None;
        app.project_info_last_request_time = None;
        if let Some(bt) = app.bilibiliticket_list.get_mut(ticket_idx) {
            bt.project_info = None;
        }
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

fn render_project_header(
    ui: &mut egui::Ui,
    ticket_data: &TicketInfo,
    card_inner_w: f32,
    palette: &ScreenPalette,
) {
    let project_name = ticket_data.name.as_deref().unwrap_or("未知项目");
    let sale_flag_text = ticket_data.sale_flag.trim();
    let project_tone = project_status_tone(ticket_data.sale_flag_number, sale_flag_text);
    let (status_bg, status_fg) = palette.tone_colors(project_tone);

    egui::Frame::none()
        .fill(palette.card_bg)
        .stroke(Stroke::new(1.0, palette.card_border))
        .rounding(10.0)
        .inner_margin(egui::Margin::symmetric(14.0, 12.0))
        .show(ui, |ui| {
            ui.set_width(card_inner_w);
            ui.add(
                egui::Label::new(
                    RichText::new(project_name)
                        .size(18.0)
                        .strong()
                        .color(palette.text_primary),
                )
                .wrap(true),
            );
            ui.add_space(6.0);

            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 4.0);
                if !sale_flag_text.is_empty() {
                    pill_badge(ui, sale_flag_text, status_bg, status_fg);
                }

                if let Some((bind_label, bind_tone)) = id_bind_badge(ticket_data.id_bind) {
                    let (bg, fg) = palette.tone_colors(bind_tone);
                    pill_badge(ui, bind_label, bg, fg);
                }

                if ticket_data.hot_project == Some(true) {
                    pill_badge(ui, "热门项目", palette.hot_bg, palette.hot_text);
                }

                if let Some(begin) = ticket_data.sale_begin.filter(|&ts| ts > 0) {
                    ui.label(
                        RichText::new(format!("开售: {}", format_timestamp_i64(begin)))
                            .size(12.5)
                            .color(palette.text_secondary),
                    );
                }
            });
        });
}

fn render_screen_selector_card(
    ui: &mut egui::Ui,
    ticket_data: &TicketInfo,
    selected_screen_index: &mut Option<usize>,
    card_inner_w: f32,
    palette: &ScreenPalette,
) {
    egui::Frame::none()
        .fill(palette.card_bg)
        .stroke(Stroke::new(1.0, palette.card_border))
        .rounding(10.0)
        .inner_margin(egui::Margin::symmetric(14.0, 12.0))
        .show(ui, |ui| {
            ui.set_width(card_inner_w);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("① 选择场次")
                        .size(15.5)
                        .strong()
                        .color(palette.text_primary),
                );
                ui.label(
                    RichText::new(format!("(共 {} 个场次)", ticket_data.screen_list.len()))
                        .size(12.5)
                        .color(palette.text_muted),
                );
            });
            ui.add_space(8.0);

            if ticket_data.screen_list.is_empty() {
                ui.label(
                    RichText::new("该项目当前暂无可选场次")
                        .size(13.5)
                        .color(palette.text_muted),
                );
                return;
            }

            let mut screen_list_ui = |ui: &mut egui::Ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    for (idx, screen) in ticket_data.screen_list.iter().enumerate() {
                        let is_selected = *selected_screen_index == Some(idx);
                        let screen_name = screen.name.as_deref().unwrap_or("未知场次");
                        let (status_label, status_tone) = resolve_screen_status(screen);
                        let (_, status_fg) = palette.tone_colors(status_tone);

                        let chip_id = ui.id().with(("screen_chip", idx));
                        let hovered_prev = ui
                            .ctx()
                            .data(|d| d.get_temp::<bool>(chip_id).unwrap_or(false));

                        let fill = if is_selected {
                            palette.selected_bg
                        } else if hovered_prev {
                            palette.row_hover_bg
                        } else {
                            palette.row_bg
                        };
                        let stroke = if is_selected {
                            Stroke::new(1.5, palette.selected_border)
                        } else if hovered_prev {
                            Stroke::new(1.0, palette.accent.linear_multiply(0.7))
                        } else {
                            Stroke::new(1.0, palette.row_border)
                        };

                        let frame_resp = egui::Frame::none()
                            .fill(fill)
                            .stroke(stroke)
                            .rounding(8.0)
                            .inner_margin(egui::Margin::symmetric(12.0, 7.0))
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    ui.label(
                                        RichText::new(screen_name)
                                            .size(13.5)
                                            .strong()
                                            .color(if is_selected {
                                                palette.accent
                                            } else {
                                                palette.text_primary
                                            }),
                                    );
                                    ui.label(
                                        RichText::new(format!("● {}", status_label))
                                            .size(11.5)
                                            .color(status_fg),
                                    );
                                });
                            })
                            .response;

                        let interact_resp = ui
                            .interact(frame_resp.rect, chip_id, egui::Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        let now_hovered = interact_resp.hovered();
                        if now_hovered != hovered_prev {
                            ui.ctx().data_mut(|d| d.insert_temp(chip_id, now_hovered));
                        }
                        if interact_resp.clicked() {
                            *selected_screen_index = Some(idx);
                        }
                    }
                });
            };

            // 场次较多时自动限高滚动，避免把下方的票种列表挤出首屏
            if ticket_data.screen_list.len() > 6 {
                egui::ScrollArea::vertical()
                    .id_source("screen_chips_scroll")
                    .max_height(140.0)
                    .show(ui, screen_list_ui);
            } else {
                screen_list_ui(ui);
            }

            // 选中场次的简要时间摘要条
            if let Some(idx) = *selected_screen_index {
                if let Some(selected_screen) = ticket_data.screen_list.get(idx) {
                    let mut meta_parts: Vec<String> = Vec::new();
                    if let Some(ts) = selected_screen.start_time.filter(|&v| v > 0) {
                        meta_parts.push(format!("演出开始: {}", format_timestamp(ts)));
                    }
                    if selected_screen.sale_start > 0 {
                        meta_parts.push(format!(
                            "开售时间: {}",
                            format_timestamp(selected_screen.sale_start)
                        ));
                    }
                    if selected_screen.sale_end > 0 {
                        meta_parts.push(format!(
                            "停售时间: {}",
                            format_timestamp(selected_screen.sale_end)
                        ));
                    }

                    if !meta_parts.is_empty() {
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(14.0, 4.0);
                            for part in meta_parts {
                                ui.label(
                                    RichText::new(part)
                                        .size(12.5)
                                        .color(palette.text_secondary),
                                );
                            }
                        });
                    }
                }
            }
        });
}

fn render_ticket_list_card(
    ui: &mut egui::Ui,
    selected_screen: &ScreenInfo,
    allow_force_select: &mut bool,
    card_inner_w: f32,
    palette: &ScreenPalette,
) -> Option<ChosenTicket> {
    let mut chosen: Option<ChosenTicket> = None;

    let total_tickets = selected_screen.ticket_list.len();
    let selectable_default_count = selected_screen
        .ticket_list
        .iter()
        .filter(|t| is_ticket_default_selectable(t))
        .count();

    egui::Frame::none()
        .fill(palette.card_bg)
        .stroke(Stroke::new(1.0, palette.card_border))
        .rounding(10.0)
        .inner_margin(egui::Margin::symmetric(14.0, 12.0))
        .show(ui, |ui| {
            ui.set_width(card_inner_w);
            // 卡片顶部：标题 + 可选统计 + 强选开关
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("② 选择票种")
                        .size(15.5)
                        .strong()
                        .color(palette.text_primary),
                );
                let count_summary = if total_tickets == 0 {
                    String::new()
                } else if selectable_default_count > 0 {
                    format!("(共 {} 档 · 可选 {} 档)", total_tickets, selectable_default_count)
                } else {
                    format!("(共 {} 档 · 当前均不可售)", total_tickets)
                };
                if !count_summary.is_empty() {
                    ui.label(
                        RichText::new(count_summary)
                            .size(12.5)
                            .color(palette.text_muted),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(allow_force_select, "允许强选不可售票种")
                        .on_hover_text(
                            "勾选后可强行选择已售罄/已停售/不可售票种（未开售与暂时售罄票种默认始终可选）",
                        );
                });
            });

            ui.add_space(6.0);

            if total_tickets == 0 {
                ui.label(
                    RichText::new("该场次暂无票种信息")
                        .size(13.5)
                        .color(palette.text_muted),
                );
                return;
            }

            if selectable_default_count == 0 && !*allow_force_select {
                egui::Frame::none()
                    .fill(palette.amber_bg)
                    .rounding(7.0)
                    .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                    .show(ui, |ui| {
                        ui.set_width((card_inner_w - 22.0).max(200.0));
                        ui.add(
                            egui::Label::new(
                                RichText::new(
                                    "提示：当前场次所有票种暂不可售；若用于提前配置或测试，请勾选右上角「允许强选不可售票种」。",
                                )
                                .size(12.5)
                                .color(palette.amber),
                            )
                            .wrap(true),
                        );
                    });
                ui.add_space(8.0);
            }

            // 逐张卡片渲染票种：使用严格有界的左右分栏（左列 + 间距 + 右列 < row_inner_w），彻底杜绝宽度膨胀
            let row_inner_w = (card_inner_w - 26.0).max(240.0);
            for (idx, ticket) in selected_screen.ticket_list.iter().enumerate() {
                if idx > 0 {
                    ui.add_space(7.0);
                }
                if let Some(picked) = render_ticket_row_card(
                    ui,
                    selected_screen.id.unwrap_or(0),
                    ticket,
                    *allow_force_select,
                    row_inner_w,
                    palette,
                ) {
                    chosen = Some(picked);
                }
            }
        });

    chosen
}

/// 根据行内可用宽度计算左列严格上限，确保 `left_w + spacing + right_col <= inner_w`
/// （当窗口极窄时由窗口 `min_width` 兜底），数学上杜绝每帧正向撑宽。
pub(crate) fn compute_ticket_left_col_width(inner_width: f32) -> f32 {
    (inner_width - TICKET_RIGHT_COL_W - TICKET_ROW_SPACING - TICKET_ROW_SLACK).max(120.0)
}

fn render_ticket_row_card(
    ui: &mut egui::Ui,
    screen_id: usize,
    ticket: &ScreenTicketInfo,
    allow_force_select: bool,
    row_inner_w: f32,
    palette: &ScreenPalette,
) -> Option<ChosenTicket> {
    let desc = ticket.desc.as_deref().unwrap_or("未知票种");
    let clickable = ticket.clickable.unwrap_or(false);
    let flag_num = ticket.sale_flag_number.unwrap_or(ticket.sale_flag.number);
    let (status_text, status_tone) = resolve_ticket_status(ticket);
    let (status_bg, status_fg) = palette.tone_colors(status_tone);

    let (button_text, button_enabled, button_fill, hover_hint) = if clickable {
        ("选择此票", true, palette.accent, None)
    } else if flag_num == 1 {
        (
            "定时预选",
            true,
            palette.blue,
            Some("该票种尚未开售，可预选用于定时自动抢票"),
        )
    } else if flag_num == 8 {
        (
            "预选捡漏",
            true,
            Color32::from_rgb(217, 119, 6),
            Some("该票种当前暂时售罄，预选后可尝试抢票或等待回流"),
        )
    } else if allow_force_select {
        (
            "强行选择",
            true,
            Color32::from_rgb(217, 119, 6),
            Some("当前票种不可售，强行选择后将继续进入购票人确认页"),
        )
    } else {
        (
            "不可选",
            false,
            palette.muted_bg,
            Some("该票种当前不可售；如需强行选择，请先勾选右上角「允许强选不可售票种」"),
        )
    };

    let price_str = format!("¥{:.2}", ticket.price.unwrap_or(0) as f64 / 100.0);
    let sale_start_text = ticket
        .sale_start
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| ticket.saleStart.filter(|&ts| ts > 0).map(format_timestamp));

    let mut chosen: Option<ChosenTicket> = None;

    let frame_resp = egui::Frame::none()
        .fill(palette.row_bg)
        .stroke(Stroke::new(1.0, palette.row_border))
        .rounding(8.0)
        .inner_margin(egui::Margin::symmetric(12.0, 9.0))
        .show(ui, |ui| {
            ui.set_width(row_inner_w);
            let left_w = compute_ticket_left_col_width(row_inner_w);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = TICKET_ROW_SPACING;

                // 左侧信息列（严格限定宽度，超长票名自动换行）
                ui.allocate_ui_with_layout(
                    egui::vec2(left_w, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_max_width(left_w);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(8.0, 4.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(desc)
                                        .size(14.5)
                                        .strong()
                                        .color(palette.text_primary),
                                )
                                .wrap(true),
                            );
                            pill_badge(ui, &status_text, status_bg, status_fg);
                        });

                        let mut sub_parts: Vec<String> = Vec::new();
                        if let Some(ref start_str) = sale_start_text {
                            sub_parts.push(format!("开售: {}", start_str));
                        }
                        if let Some(tid) = ticket.id.filter(|&id| id > 0) {
                            sub_parts.push(format!("票种ID: {}", tid));
                        }
                        if !sub_parts.is_empty() {
                            ui.add_space(2.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(sub_parts.join("  ·  "))
                                        .size(11.5)
                                        .color(palette.text_muted),
                                )
                                .wrap(true),
                            );
                        }
                    },
                );

                // 右侧价格与操作按钮列（固定有界宽度，右对齐）
                ui.allocate_ui_with_layout(
                    egui::vec2(TICKET_RIGHT_COL_W, 34.0),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;

                        let btn_text_color = if button_enabled {
                            Color32::WHITE
                        } else {
                            palette.text_muted
                        };
                        let btn = egui::Button::new(
                            RichText::new(button_text)
                                .size(13.0)
                                .strong()
                                .color(btn_text_color),
                        )
                        .fill(button_fill)
                        .rounding(7.0)
                        .min_size(egui::vec2(92.0, 30.0));

                        let mut btn_resp = ui.add_enabled(button_enabled, btn);
                        if let Some(hint) = hover_hint {
                            btn_resp = btn_resp.on_hover_text(hint).on_disabled_hover_text(hint);
                        }
                        if btn_resp.clicked() {
                            chosen = Some(ChosenTicket {
                                screen_id,
                                ticket_id: ticket.id.unwrap_or(0),
                                project_id: ticket.project_id,
                                ticket_desc: desc.to_string(),
                                clickable,
                            });
                        }

                        ui.label(
                            RichText::new(price_str)
                                .size(16.0)
                                .strong()
                                .color(palette.price_red),
                        );
                    },
                );
            });
        })
        .response;

    if frame_resp.hovered() && button_enabled {
        // 轻微悬停高亮边框提示可交互行
        ui.painter().rect_stroke(
            frame_resp.rect,
            8.0,
            Stroke::new(1.0, palette.accent.linear_multiply(0.55)),
        );
    }

    chosen
}

fn render_project_details_collapsing(
    ui: &mut egui::Ui,
    ticket_data: &TicketInfo,
    card_inner_w: f32,
    palette: &ScreenPalette,
) {
    let mut base_items: Vec<(String, String)> = Vec::new();
    if let Some(desc) = &ticket_data.performance_desc {
        for item in &desc.list {
            if item.module == "base_info" {
                if let Some(array) = item.details.as_array() {
                    for info_item in array {
                        if let (Some(title), Some(content)) = (
                            info_item.get("title").and_then(Value::as_str),
                            info_item.get("content").and_then(Value::as_str),
                        ) {
                            let t = title.trim();
                            let c = content.trim();
                            if !t.is_empty() && !c.is_empty() {
                                base_items.push((t.to_string(), c.to_string()));
                            }
                        }
                    }
                }
            }
        }
    }

    egui::Frame::none()
        .fill(palette.card_bg)
        .stroke(Stroke::new(1.0, palette.card_border))
        .rounding(10.0)
        .inner_margin(egui::Margin::symmetric(14.0, 10.0))
        .show(ui, |ui| {
            ui.set_width(card_inner_w);
            egui::CollapsingHeader::new(
                RichText::new("活动须知与详细信息")
                    .size(14.0)
                    .strong()
                    .color(palette.text_primary),
            )
            .id_source("screen_info_base_details")
            .default_open(false)
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("项目ID: {}", ticket_data.id.unwrap_or(0)))
                        .size(12.5)
                        .color(palette.text_secondary),
                );
                if base_items.is_empty() {
                    ui.label(
                        RichText::new("暂无更多活动说明")
                            .size(12.5)
                            .color(palette.text_muted),
                    );
                } else {
                    for (title, content) in &base_items {
                        ui.add_space(4.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!("{}：{}", title, content))
                                    .size(12.5)
                                    .color(palette.text_secondary),
                            )
                            .wrap(true),
                        );
                    }
                }
            });
        });
}

fn pill_badge(ui: &mut egui::Ui, text: &str, fill: Color32, text_color: Color32) {
    egui::Frame::none()
        .fill(fill)
        .rounding(6.0)
        .inner_margin(egui::Margin::symmetric(7.0, 2.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .size(11.5)
                    .strong()
                    .color(text_color),
            );
        });
}

fn is_ticket_default_selectable(ticket: &ScreenTicketInfo) -> bool {
    let clickable = ticket.clickable.unwrap_or(false);
    let flag_num = ticket.sale_flag_number.unwrap_or(ticket.sale_flag.number);
    clickable || flag_num == 1 || flag_num == 8
}

fn id_bind_badge(id_bind: Option<usize>) -> Option<(&'static str, StatusTone)> {
    match id_bind {
        Some(0) => Some(("非实名制", StatusTone::Available)),
        Some(1) => Some(("单人实名", StatusTone::Presale)),
        Some(2) => Some(("一票一证实名", StatusTone::Warning)),
        _ => None,
    }
}

fn project_status_tone(flag_num: Option<usize>, display: &str) -> StatusTone {
    match flag_num {
        Some(2) => StatusTone::Available,
        Some(1) => StatusTone::Presale,
        Some(8) => StatusTone::Warning,
        _ => {
            if display.contains("售票中") || display.contains("预售中") {
                StatusTone::Available
            } else if display.contains("未开售") {
                StatusTone::Presale
            } else if display.contains("暂时") {
                StatusTone::Warning
            } else {
                StatusTone::Muted
            }
        }
    }
}

/// 智能推导场次展示状态：B站接口中 `screen.sale_flag.display_name` 常返回 `"未知状态"`，
/// 此时优先依据 `clickable`、`sale_flag_number` 与该场次下 `ticket_list` 的真实状态推导可读文案。
pub(crate) fn resolve_screen_status(screen: &ScreenInfo) -> (String, StatusTone) {
    let clickable_count = screen
        .ticket_list
        .iter()
        .filter(|t| t.clickable.unwrap_or(false))
        .count();
    let total = screen.ticket_list.len();
    if clickable_count > 0 {
        return (
            format!("可售 {}/{}", clickable_count, total),
            StatusTone::Available,
        );
    }

    let presale_count = screen
        .ticket_list
        .iter()
        .filter(|t| t.sale_flag_number.unwrap_or(t.sale_flag.number) == 1)
        .count();
    if presale_count > 0 || screen.sale_flag_number == 1 {
        return ("未开售".to_string(), StatusTone::Presale);
    }

    let temp_sold_out = screen
        .ticket_list
        .iter()
        .any(|t| t.sale_flag_number.unwrap_or(t.sale_flag.number) == 8);
    if temp_sold_out || screen.sale_flag_number == 8 {
        return ("暂时售罄".to_string(), StatusTone::Warning);
    }

    let raw_name = screen.sale_flag.display_name.trim();
    if !raw_name.is_empty() && raw_name != "未知状态" {
        let tone = project_status_tone(Some(screen.sale_flag_number), raw_name);
        return (raw_name.to_string(), tone);
    }

    match screen.sale_flag_number {
        2 => ("预售中".to_string(), StatusTone::Available),
        4 => ("已售罄".to_string(), StatusTone::Muted),
        5 => ("不可售".to_string(), StatusTone::Muted),
        6 => ("已停售".to_string(), StatusTone::Muted),
        _ => {
            if let Some(first_flag) = screen
                .ticket_list
                .first()
                .map(|t| t.sale_flag.display_name.trim())
                .filter(|s| !s.is_empty() && *s != "未知状态")
            {
                (first_flag.to_string(), StatusTone::Muted)
            } else {
                ("暂不可售".to_string(), StatusTone::Muted)
            }
        }
    }
}

fn resolve_ticket_status(ticket: &ScreenTicketInfo) -> (String, StatusTone) {
    let clickable = ticket.clickable.unwrap_or(false);
    let flag_num = ticket.sale_flag_number.unwrap_or(ticket.sale_flag.number);
    let raw = ticket.sale_flag.display_name.trim();
    let label = if !raw.is_empty() && raw != "未知状态" {
        raw.to_string()
    } else if clickable {
        "可售".to_string()
    } else {
        match flag_num {
            1 => "未开售".to_string(),
            2 => "预售中".to_string(),
            4 => "已售罄".to_string(),
            5 => "不可售".to_string(),
            8 => "暂时售罄".to_string(),
            _ => "暂不可售".to_string(),
        }
    };

    let tone = if clickable {
        StatusTone::Available
    } else if flag_num == 1 {
        StatusTone::Presale
    } else if flag_num == 8 {
        StatusTone::Warning
    } else {
        StatusTone::Muted
    };
    (label, tone)
}

// 将时间戳转换为可读时间
fn format_timestamp(timestamp: usize) -> String {
    if timestamp == 0 {
        return "未设置".to_string();
    }

    match i64::try_from(timestamp) {
        Ok(ts) => format_timestamp_i64(ts),
        Err(_) => "时间戳溢出".to_string(),
    }
}

fn format_timestamp_i64(timestamp: i64) -> String {
    if timestamp <= 0 {
        return "未设置".to_string();
    }
    match chrono::Local.timestamp_opt(timestamp, 0) {
        chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        _ => "无效时间".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        compute_ticket_left_col_width, format_timestamp, render_project_details_collapsing,
        render_project_header, render_screen_selector_card, render_ticket_list_card,
        resolve_screen_status, ScreenPalette, StatusTone, TICKET_RIGHT_COL_W, TICKET_ROW_SPACING,
    };
    use common::ticket::{SaleFlag, ScreenInfo, ScreenTicketInfo, TicketInfo};
    use eframe::egui;

    #[test]
    fn ticket_row_width_never_exceeds_available_width_for_valid_window_sizes() {
        for inner_w in [460.0_f32, 540.0, 680.0, 780.0, 1200.0, 1920.0] {
            let left_w = compute_ticket_left_col_width(inner_w);
            assert!(
                left_w + TICKET_ROW_SPACING + TICKET_RIGHT_COL_W < inner_w,
                "row width must stay strictly below inner_w={} to avoid egui window horizontal drift",
                inner_w
            );
        }
    }

    #[test]
    fn screen_window_width_stays_stable_across_20_frames_without_horizontal_drift() {
        // 构造与真实项目 102626 同结构的 3 场次 × 6 票种数据，在真实 egui 布局引擎中连续跑 20 帧
        let make_ticket = |id: usize, desc: &str, price: usize| ScreenTicketInfo {
            id: Some(id),
            project_id: Some(102626),
            price: Some(price),
            desc: Some(desc.to_string()),
            sale_start: Some("2025-07-09 14:11:52".to_string()),
            clickable: Some(false),
            sale_flag_number: Some(5),
            sale_flag: SaleFlag {
                number: 5,
                display_name: "不可售".to_string(),
            },
            ..Default::default()
        };
        let make_screen = |id: usize, name: &str| ScreenInfo {
            id: Some(id),
            name: Some(name.to_string()),
            start_time: Some(1752228000),
            sale_start: 1752041512,
            sale_end: 1752163200,
            sale_flag: SaleFlag::default(),
            sale_flag_number: 5,
            ticket_list: vec![
                make_ticket(690018, "S级", 138000),
                make_ticket(690159, "A级", 118000),
                make_ticket(690132, "B级（内场）", 98000),
                make_ticket(690109, "B级（看台）", 98000),
                make_ticket(690015, "C级", 68000),
                make_ticket(690003, "D级超长票种名称测试自动换行不会撑宽窗口", 38000),
            ],
            ..Default::default()
        };
        let ticket_data = TicketInfo {
            id: Some(102626),
            name: Some("上海·BILIBILI MACRO LINK 2025".to_string()),
            sale_flag: "已结束".to_string(),
            sale_flag_number: Some(102),
            sale_begin: Some(1752041512),
            id_bind: Some(2),
            hot_project: Some(true),
            screen_list: vec![
                make_screen(262672, "2025-07-11 18:00 - 20:30"),
                make_screen(262673, "2025-07-12 18:00 - 21:00"),
                make_screen(262674, "2025-07-13 18:00 - 21:00"),
            ],
            ..Default::default()
        };

        let ctx = egui::Context::default();
        let mut selected_screen_index = Some(0usize);
        let mut allow_force_select = false;
        let mut widths = Vec::new();

        for _ in 0..20 {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 680.0),
                )),
                ..Default::default()
            };
            let mut measured_w = 0.0_f32;
            let _ = ctx.run(raw, |ctx| {
                let palette = ScreenPalette::from_dark_mode(false);
                let screen_rect = ctx.screen_rect();
                let max_w = (screen_rect.width() - 40.0).max(540.0);
                let max_h = (screen_rect.height() - 40.0).max(420.0);
                let resp = egui::Window::new("选择场次与票种")
                    .id(egui::Id::new("screen_info_window_v2"))
                    .default_size([780.0_f32.min(max_w), 600.0_f32.min(max_h)])
                    .min_width(540.0)
                    .min_height(400.0)
                    .resize(|r| r.max_size([max_w, max_h]))
                    .constrain(true)
                    .resizable(true)
                    .show(ctx, |ui| {
                        let scroll_max_height = (ui.available_height() - 44.0).max(200.0);
                        egui::ScrollArea::vertical()
                            .id_source("screen_info_scroll")
                            .auto_shrink([false, false])
                            .max_height(scroll_max_height)
                            .show(ui, |ui| {
                                let card_outer_w = (ui.available_width() - 6.0).max(320.0);
                                let card_inner_w = (card_outer_w - 30.0).max(280.0);
                                render_project_header(ui, &ticket_data, card_inner_w, &palette);
                                render_screen_selector_card(
                                    ui,
                                    &ticket_data,
                                    &mut selected_screen_index,
                                    card_inner_w,
                                    &palette,
                                );
                                if let Some(screen) = ticket_data.screen_list.first() {
                                    let _ = render_ticket_list_card(
                                        ui,
                                        screen,
                                        &mut allow_force_select,
                                        card_inner_w,
                                        &palette,
                                    );
                                }
                                render_project_details_collapsing(
                                    ui,
                                    &ticket_data,
                                    card_inner_w,
                                    &palette,
                                );
                            });
                    });
                if let Some(inner) = resp {
                    measured_w = inner.response.rect.width();
                }
            });
            widths.push(measured_w);
        }

        let w_early = widths[2];
        let w_final = *widths.last().unwrap();
        assert!(
            (w_final - w_early).abs() < 1.0,
            "window width drifted across frames: frame2={}, frame19={}, all={:?}",
            w_early,
            w_final,
            widths
        );
        assert!(
            w_final <= 800.0,
            "window width unexpectedly exceeded default bounds: {} (all={:?})",
            w_final,
            widths
        );
    }

    #[test]
    fn resolve_screen_status_derives_readable_label_when_api_returns_unknown() {
        // 类似 project_102626.json：screen.sale_flag 为"未知状态"，sale_flag_number=5，所有票不可售
        let unavailable_screen = ScreenInfo {
            sale_flag: SaleFlag::default(),
            sale_flag_number: 5,
            ticket_list: vec![ScreenTicketInfo {
                clickable: Some(false),
                sale_flag_number: Some(5),
                sale_flag: SaleFlag {
                    number: 5,
                    display_name: "不可售".to_string(),
                },
                ..Default::default()
            }],
            ..Default::default()
        };
        let (label, tone) = resolve_screen_status(&unavailable_screen);
        assert_eq!(label, "不可售");
        assert_eq!(tone, StatusTone::Muted);

        // 类似 project_1003000.json：screen.sale_flag 为"未知状态"，1/2 张票可买
        let available_screen = ScreenInfo {
            sale_flag: SaleFlag::default(),
            sale_flag_number: 2,
            clickable: true,
            ticket_list: vec![
                ScreenTicketInfo {
                    clickable: Some(true),
                    sale_flag_number: Some(2),
                    ..Default::default()
                },
                ScreenTicketInfo {
                    clickable: Some(false),
                    sale_flag_number: Some(1),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let (label, tone) = resolve_screen_status(&available_screen);
        assert_eq!(label, "可售 1/2");
        assert_eq!(tone, StatusTone::Available);
    }

    #[test]
    fn format_timestamp_handles_zero_and_valid_epoch() {
        assert_eq!(format_timestamp(0), "未设置");
        assert!(!format_timestamp(1752041512).contains("无效"));
    }
}
