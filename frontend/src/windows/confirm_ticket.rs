use std::sync::Arc;

use common::cookie_manager::CookieManager;
use common::taskmanager::{GrabTicketRequest, TaskRequest, TaskStatus};
use common::ticket::{BuyerInfo, NoBindBuyerInfo};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};

use crate::app::{normalize_fullwidth_ascii, Myapp};
use crate::windows::{bounded_window_sizes, show_resizable_body};

#[derive(Clone, Copy)]
pub(crate) struct ConfirmPalette {
    pub card_bg: Color32,
    pub card_border: Color32,
    pub item_bg: Color32,
    pub item_hover_bg: Color32,
    pub item_border: Color32,
    pub selected_bg: Color32,
    pub selected_border: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub green: Color32,
    pub green_bg: Color32,
    pub blue: Color32,
    pub blue_bg: Color32,
    pub amber: Color32,
    pub amber_bg: Color32,
    pub red: Color32,
    pub error_bg: Color32,
    pub badge_bg: Color32,
}

impl ConfirmPalette {
    pub(crate) fn from_dark_mode(dark: bool) -> Self {
        if dark {
            Self {
                card_bg: Color32::from_rgb(30, 33, 41),
                card_border: Color32::from_rgb(56, 62, 76),
                item_bg: Color32::from_rgb(38, 42, 52),
                item_hover_bg: Color32::from_rgb(46, 51, 64),
                item_border: Color32::from_rgb(64, 70, 86),
                selected_bg: Color32::from_rgb(22, 54, 36),
                selected_border: Color32::from_rgb(34, 197, 94),
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
                red: Color32::from_rgb(248, 113, 113),
                error_bg: Color32::from_rgb(60, 24, 24),
                badge_bg: Color32::from_rgb(52, 58, 72),
            }
        } else {
            Self {
                card_bg: Color32::from_rgb(246, 248, 252),
                card_border: Color32::from_rgb(214, 220, 232),
                item_bg: Color32::from_rgb(255, 255, 255),
                item_hover_bg: Color32::from_rgb(244, 248, 255),
                item_border: Color32::from_rgb(220, 226, 238),
                selected_bg: Color32::from_rgb(234, 252, 241),
                selected_border: Color32::from_rgb(34, 197, 94),
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
                red: Color32::from_rgb(220, 56, 52),
                error_bg: Color32::from_rgb(254, 242, 242),
                badge_bg: Color32::from_rgb(232, 236, 246),
            }
        }
    }
}

pub(crate) fn pill_badge(ui: &mut egui::Ui, text: &str, fill: Color32, text_color: Color32) {
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

pub(crate) fn id_type_label(id_type: i64) -> &'static str {
    match id_type {
        0 => "身份证",
        1 => "护照",
        2 => "港澳通行证",
        3 => "台湾通行证",
        _ => "其他证件",
    }
}

/// 计算购票人网格的列数与严格有界的单卡内容宽度，杜绝任何水平撑宽。
pub(crate) fn compute_buyer_grid_layout(card_inner_w: f32) -> (usize, f32) {
    let spacing = 10.0;
    let cols: usize = if card_inner_w >= 520.0 { 2 } else { 1 };
    let total_gap = spacing * (cols.saturating_sub(1) as f32);
    // 单卡 Frame 含左右 margin(10*2=20) + stroke(2) + 额外余量(2) = 24
    let cell_outer_w = ((card_inner_w - total_gap - 2.0) / cols as f32).max(200.0);
    let cell_inner_w = (cell_outer_w - 24.0).max(176.0);
    (cols, cell_inner_w)
}

/// 统一的实名购票人卡片选择列表（供普通模式与捡漏模式共用）：
/// 整张卡片可点击，选中时绿色边框+勾选指示，展示默认购票人标签与脱敏证件号。
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_buyer_cards_grid(
    ui: &mut egui::Ui,
    id_salt: &str,
    buyers: &[BuyerInfo],
    selected_buyer_list: &mut Option<Vec<BuyerInfo>>,
    is_multi_select: bool,
    card_inner_w: f32,
    palette: &ConfirmPalette,
) -> Option<BuyerInfo> {
    let mut single_selected_buyer: Option<BuyerInfo> = None;
    let (cols, cell_inner_w) = compute_buyer_grid_layout(card_inner_w);
    let spacing = 10.0;

    for row_buyers in buyers.chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;
            for buyer in row_buyers {
                let is_selected = selected_buyer_list
                    .as_ref()
                    .map_or(false, |list| list.iter().any(|b| b.id == buyer.id));
                let card_id = ui.id().with((id_salt, "buyer_card", buyer.id));
                let hovered_prev = ui
                    .ctx()
                    .data(|d| d.get_temp::<bool>(card_id).unwrap_or(false));

                let fill = if is_selected {
                    palette.selected_bg
                } else if hovered_prev {
                    palette.item_hover_bg
                } else {
                    palette.item_bg
                };
                let stroke = if is_selected {
                    Stroke::new(1.5, palette.selected_border)
                } else if hovered_prev {
                    Stroke::new(1.0, palette.accent.linear_multiply(0.75))
                } else {
                    Stroke::new(1.0, palette.item_border)
                };

                let frame_resp = egui::Frame::none()
                    .fill(fill)
                    .stroke(stroke)
                    .rounding(8.0)
                    .inner_margin(egui::Margin::symmetric(10.0, 9.0))
                    .show(ui, |ui| {
                        ui.set_width(cell_inner_w);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;

                            // 左侧自定义复选/单选状态徽章
                            let indicator_size = Vec2::new(20.0, 20.0);
                            let (ind_rect, _) =
                                ui.allocate_exact_size(indicator_size, egui::Sense::hover());
                            let painter = ui.painter();
                            let rounding = if is_multi_select { 5.0 } else { 10.0 };
                            if is_selected {
                                painter.rect_filled(ind_rect, rounding, palette.selected_border);
                                painter.text(
                                    ind_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "✓",
                                    egui::FontId::proportional(13.0),
                                    Color32::WHITE,
                                );
                            } else {
                                painter.rect_stroke(
                                    ind_rect.shrink(1.0),
                                    rounding,
                                    Stroke::new(1.5, palette.text_muted),
                                );
                            }

                            // 右侧购票人详情
                            let info_w = (cell_inner_w - 32.0).max(120.0);
                            ui.allocate_ui_with_layout(
                                Vec2::new(info_w, 0.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_max_width(info_w);
                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 2.0);
                                        ui.label(
                                            RichText::new(&buyer.name)
                                                .size(14.5)
                                                .strong()
                                                .color(palette.text_primary),
                                        );
                                        pill_badge(
                                            ui,
                                            id_type_label(buyer.id_type),
                                            palette.badge_bg,
                                            palette.text_secondary,
                                        );
                                        if buyer.is_default == 1 {
                                            pill_badge(ui, "默认", palette.blue_bg, palette.blue);
                                        }
                                    });
                                    ui.add_space(3.0);
                                    ui.label(
                                        RichText::new(format!(
                                            "证件号: {}",
                                            mask_id(&buyer.personal_id)
                                        ))
                                        .size(12.0)
                                        .color(palette.text_secondary),
                                    );
                                    ui.label(
                                        RichText::new(format!("手机号: {}", buyer.tel))
                                            .size(12.0)
                                            .color(palette.text_muted),
                                    );
                                },
                            );
                        });
                    })
                    .response;

                let interact_resp = ui
                    .interact(frame_resp.rect, card_id, egui::Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                let now_hovered = interact_resp.hovered();
                if now_hovered != hovered_prev {
                    ui.ctx().data_mut(|d| d.insert_temp(card_id, now_hovered));
                }

                if interact_resp.clicked() {
                    if is_multi_select {
                        let buyer_list = selected_buyer_list.get_or_insert_with(Vec::new);
                        if let Some(pos) = buyer_list.iter().position(|b| b.id == buyer.id) {
                            buyer_list.remove(pos);
                        } else {
                            buyer_list.push(buyer.clone());
                        }
                    } else {
                        *selected_buyer_list = Some(vec![buyer.clone()]);
                        single_selected_buyer = Some(buyer.clone());
                    }
                }
            }
        });
        ui.add_space(8.0);
    }

    single_selected_buyer
}

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

    let cookie_manager: Arc<CookieManager> = match app.bilibiliticket_list[biliticket_index]
        .account
        .cookie_manager
        .clone()
    {
        Some(cm) => cm,
        None => {
            let acc_uid = app.bilibiliticket_list[biliticket_index].uid;
            log::error!("账号 {} 的客户端未初始化", acc_uid);
            app.show_error(format!("账号 {} 的客户端未初始化，请重新登录", acc_uid));
            app.confirm_ticket_info = None;
            app.buyer_info_requesting = false;
            app.buyer_info_active_task_id = None;
            app.buyer_info_error = None;
            app.reset_buyer_selection();
            return;
        }
    };

    // 仅提取轻量展示与提交所需字段，避免每帧深拷贝 BilibiliTicket / TicketInfo
    let (
        biliticket_uid,
        account_name,
        ticket_method,
        project_name,
        project_id_str,
        id_bind,
        is_hot,
        screen_summary,
        ticket_summary,
        buyers_loaded,
        buyers,
    ) = {
        let biliticket = &app.bilibiliticket_list[biliticket_index];
        let project_name = biliticket
            .project_info
            .as_ref()
            .and_then(|p| p.name.clone())
            .unwrap_or_else(|| "未知项目".to_string());
        let project_id_str = if !biliticket.project_id.trim().is_empty() {
            biliticket.project_id.clone()
        } else {
            biliticket
                .project_info
                .as_ref()
                .and_then(|p| p.id)
                .map(|id| id.to_string())
                .unwrap_or_default()
        };
        let id_bind = biliticket
            .project_info
            .as_ref()
            .and_then(|p| p.id_bind)
            .unwrap_or(9);
        let is_hot = biliticket
            .project_info
            .as_ref()
            .and_then(|p| p.hot_project)
            .unwrap_or(false);

        let (screen_summary, ticket_summary) = match &biliticket.project_info {
            Some(project) => {
                let screen = project.screen_list.iter().find(|s| {
                    s.id.map(|id| id.to_string()).as_deref() == Some(biliticket.screen_id.as_str())
                });
                if let Some(screen) = screen {
                    let ticket = screen.ticket_list.iter().find(|t| {
                        if let Some(tid) = app.selected_ticket_id {
                            t.id.map(|id| id as i64) == Some(tid)
                        } else if let Some(ref sel_str) = biliticket.select_ticket_id {
                            t.id.map(|id| id.to_string()).as_deref() == Some(sel_str.as_str())
                        } else {
                            false
                        }
                    });
                    (
                        Some((
                            screen.id.unwrap_or(0),
                            screen
                                .name
                                .clone()
                                .unwrap_or_else(|| "未知场次".to_string()),
                        )),
                        ticket.map(|t| {
                            (
                                t.id.unwrap_or(0),
                                t.desc.clone().unwrap_or_else(|| "未知票种".to_string()),
                                t.price.unwrap_or(0),
                            )
                        }),
                    )
                } else {
                    (None, None)
                }
            }
            None => (None, None),
        };

        let buyers_loaded = biliticket.all_buyer_info.is_some();
        let buyers: Vec<BuyerInfo> = biliticket
            .all_buyer_info
            .as_ref()
            .map(|d| d.list.clone())
            .unwrap_or_default();

        (
            biliticket.uid,
            biliticket.account.name.clone(),
            biliticket.method,
            project_name,
            project_id_str,
            id_bind,
            is_hot,
            screen_summary,
            ticket_summary,
            buyers_loaded,
            buyers,
        )
    };

    let grab_mode_text = match ticket_method {
        0 => "定时自动抢票",
        1 => "直接抢票",
        2 => "捡漏回流票",
        _ => "未知模式",
    };

    // 非实名制（id_bind == 0）首次打开时从设置预填手机号
    if id_bind == 0 && app.bilibiliticket_list[biliticket_index].no_bind_buyer_info.is_none() {
        let preinput_phone = app
            .get_preinput_phone_for_uid(*uid)
            .map(|p| {
                normalize_fullwidth_ascii(&p)
                    .chars()
                    .filter(|c| c.is_ascii_digit())
                    .collect::<String>()
            })
            .unwrap_or_default();
        app.bilibiliticket_list[biliticket_index].no_bind_buyer_info = Some(NoBindBuyerInfo {
            name: String::new(),
            tel: preinput_phone,
            uid: biliticket_uid,
        });
    }

    let mut cancel_clicked = false;
    let mut back_to_screen_clicked = false;
    let mut submit_clicked = false;
    let mut refresh_buyers_clicked = false;
    let mut add_buyer_clicked = false;

    let (default_sz, min_sz, max_sz) = bounded_window_sizes(ctx, 700.0, 590.0, 500.0, 380.0);

    let palette = ConfirmPalette::from_dark_mode(ctx.style().visuals.dark_mode);
    let ime_composing = app.is_ime_composing();

    egui::Window::new("确认购票信息")
        .id(egui::Id::new("confirm_ticket_window_v2"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .constrain(true)
        .default_size(default_sz)
        .min_width(min_sz[0])
        .min_height(min_sz[1])
        .resize(|r| r.max_size(max_sz))
        .show(ctx, |ui| {
            // 提前计算底部确认按钮状态，便于底栏在 bottom_up 中先渲染
            let screen_ticket_ready = screen_summary.is_some() && ticket_summary.is_some();
            let ticket_count_now = app.bilibiliticket_list[biliticket_index]
                .count
                .unwrap_or(1)
                .clamp(1, 10) as usize;

            let (button_enabled, status_hint, status_ok) = if !screen_ticket_ready {
                (false, "场次或票种信息缺失，请返回重选".to_string(), false)
            } else {
                match id_bind {
                    0 => {
                        let ok = app.bilibiliticket_list[biliticket_index]
                            .no_bind_buyer_info
                            .as_ref()
                            .map_or(false, |info| {
                                let tel_norm = normalize_fullwidth_ascii(&info.tel);
                                !info.name.trim().is_empty() && is_valid_phone(tel_norm.trim())
                            });
                        if ok {
                            (true, "✓ 联系人信息已就绪，点击右侧确认下单".to_string(), true)
                        } else {
                            (false, "请填写联系人姓名和有效手机号".to_string(), false)
                        }
                    }
                    1 => {
                        let ok = app.selected_buyer_list.as_ref().map_or(false, |list| {
                            list.len() == 1 && buyers.iter().any(|b| b.id == list[0].id)
                        });
                        if ok {
                            (true, "✓ 已选定购票人，点击右侧提交抢票任务".to_string(), true)
                        } else {
                            (false, "请先选择 1 位实名购票人".to_string(), false)
                        }
                    }
                    2 => {
                        let valid_sel = app.selected_buyer_list.as_ref().map_or(0, |list| {
                            list.iter()
                                .filter(|sel| buyers.iter().any(|b| b.id == sel.id))
                                .count()
                        });
                        if valid_sel > 0 && valid_sel == ticket_count_now {
                            (
                                true,
                                format!(
                                    "✓ 已选 {} 位购票人（{} 张票），可提交任务",
                                    valid_sel, ticket_count_now
                                ),
                                true,
                            )
                        } else {
                            (
                                false,
                                format!(
                                    "一票一证需选 {} 位购票人（当前已选 {} 人）",
                                    ticket_count_now, valid_sel
                                ),
                                false,
                            )
                        }
                    }
                    _ => (false, "当前项目实名类型不受支持".to_string(), false),
                }
            };

            show_resizable_body(
                ui,
                "confirm_ticket_main_scroll",
                |ui| {
                    // 顶部目标账号与模式上下文栏
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);
                        ui.label(
                            RichText::new(format!("账号: {} ({})", account_name, biliticket_uid))
                                .size(13.0)
                                .strong()
                                .color(palette.text_primary),
                        );
                        ui.label(RichText::new("·").color(palette.text_muted));
                        pill_badge(ui, grab_mode_text, palette.blue_bg, palette.blue);
                        let (bind_text, bind_bg, bind_fg) = match id_bind {
                            0 => ("非实名制", palette.green_bg, palette.green),
                            1 => ("单人实名", palette.blue_bg, palette.blue),
                            2 => ("一票一证实名", palette.amber_bg, palette.amber),
                            _ => ("未知实名类型", palette.badge_bg, palette.text_muted),
                        };
                        pill_badge(ui, bind_text, bind_bg, bind_fg);
                    });

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(6.0);
                },
                |ui| {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(&status_hint).size(12.5).color(if status_ok {
                                palette.green
                            } else {
                                palette.amber
                            }),
                        )
                        .wrap(true),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(RichText::new("← 返回重选票种").size(13.0))
                                    .rounding(6.0)
                                    .min_size(Vec2::new(112.0, 32.0)),
                            )
                            .on_hover_text("返回上一步重新选择场次或票种（无需重新拉取项目）")
                            .clicked()
                        {
                            back_to_screen_clicked = true;
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let confirm_btn = egui::Button::new(
                                RichText::new("确认购票")
                                    .size(14.0)
                                    .strong()
                                    .color(if button_enabled {
                                        Color32::WHITE
                                    } else {
                                        palette.text_muted
                                    }),
                            )
                            .fill(if button_enabled {
                                palette.accent
                            } else {
                                palette.badge_bg
                            })
                            .rounding(7.0)
                            .min_size(Vec2::new(104.0, 34.0));

                            if ui.add_enabled(button_enabled, confirm_btn).clicked() {
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

                    // 卡片 ①：已选场次、票种与购票数量
                    egui::Frame::none()
                        .fill(palette.card_bg)
                        .stroke(Stroke::new(1.0, palette.card_border))
                        .rounding(10.0)
                        .inner_margin(egui::Margin::symmetric(14.0, 12.0))
                        .show(ui, |ui| {
                            ui.set_width(card_inner_w);
                            ui.label(
                                RichText::new("① 确认所选场次与票种")
                                    .size(15.0)
                                    .strong()
                                    .color(palette.text_primary),
                            );
                            ui.add_space(6.0);

                            ui.add(
                                egui::Label::new(
                                    RichText::new(&project_name)
                                        .size(16.0)
                                        .strong()
                                        .color(palette.text_primary),
                                )
                                .wrap(true),
                            );

                            if let Some((_, ref screen_name)) = screen_summary {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!("场次：{}", screen_name))
                                        .size(13.5)
                                        .color(palette.text_secondary),
                                );

                                if let Some((_, ref ticket_desc, ticket_price)) = ticket_summary {
                                    ui.add_space(6.0);
                                    egui::Frame::none()
                                        .fill(palette.item_bg)
                                        .stroke(Stroke::new(1.0, palette.item_border))
                                        .rounding(8.0)
                                        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                                        .show(ui, |ui| {
                                            let ticket_box_w = (card_inner_w - 26.0).max(240.0);
                                            ui.set_width(ticket_box_w);
                                            ui.horizontal(|ui| {
                                                let desc_w = (ticket_box_w - 132.0).max(120.0);
                                                ui.allocate_ui_with_layout(
                                                    Vec2::new(desc_w, 0.0),
                                                    egui::Layout::top_down(egui::Align::Min),
                                                    |ui| {
                                                        ui.set_max_width(desc_w);
                                                        ui.add(
                                                            egui::Label::new(
                                                                RichText::new(format!(
                                                                    "票种：{}",
                                                                    ticket_desc
                                                                ))
                                                                .size(14.0)
                                                                .strong()
                                                                .color(palette.text_primary),
                                                            )
                                                            .wrap(true),
                                                        );
                                                    },
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        ui.label(
                                                            RichText::new(format!(
                                                                "单价 ¥{:.2}",
                                                                ticket_price as f64 / 100.0
                                                            ))
                                                            .size(14.0)
                                                            .strong()
                                                            .color(palette.red),
                                                        );
                                                    },
                                                );
                                            });

                                            ui.add_space(6.0);
                                            ui.separator();
                                            ui.add_space(6.0);

                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new("购票数量：")
                                                        .size(13.5)
                                                        .color(palette.text_secondary),
                                                );
                                                let mut count = app.bilibiliticket_list
                                                    [biliticket_index]
                                                    .count
                                                    .unwrap_or(1)
                                                    .clamp(1, 10);

                                                if ui
                                                    .add_enabled(
                                                        count > 1,
                                                        egui::Button::new(" − ")
                                                            .min_size(Vec2::new(28.0, 26.0))
                                                            .rounding(6.0),
                                                    )
                                                    .clicked()
                                                {
                                                    count -= 1;
                                                    app.bilibiliticket_list[biliticket_index]
                                                        .count = Some(count);
                                                }

                                                ui.label(
                                                    RichText::new(format!(" {} 张 ", count))
                                                        .size(14.5)
                                                        .strong()
                                                        .color(palette.text_primary),
                                                );

                                                if ui
                                                    .add_enabled(
                                                        count < 10,
                                                        egui::Button::new(" + ")
                                                            .min_size(Vec2::new(28.0, 26.0))
                                                            .rounding(6.0),
                                                    )
                                                    .clicked()
                                                {
                                                    count += 1;
                                                    app.bilibiliticket_list[biliticket_index]
                                                        .count = Some(count);
                                                }

                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        ui.label(
                                                            RichText::new(format!(
                                                                "¥{:.2}",
                                                                (ticket_price as f64 / 100.0)
                                                                    * count as f64
                                                            ))
                                                            .size(17.0)
                                                            .strong()
                                                            .color(palette.red),
                                                        );
                                                        ui.label(
                                                            RichText::new("预计总价：")
                                                                .size(13.0)
                                                                .color(palette.text_secondary),
                                                        );
                                                    },
                                                );
                                            });
                                        });
                                } else {
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new("未匹配到有效票种，请点击左下角「返回重选票种」")
                                            .color(palette.red),
                                    );
                                }
                            } else {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("未匹配到有效场次，请点击左下角「返回重选票种」")
                                        .color(palette.red),
                                );
                            }
                        });

                    ui.add_space(10.0);

                    // 卡片 ②：购票人 / 联系人信息
                    egui::Frame::none()
                        .fill(palette.card_bg)
                        .stroke(Stroke::new(1.0, palette.card_border))
                        .rounding(10.0)
                        .inner_margin(egui::Margin::symmetric(14.0, 12.0))
                        .show(ui, |ui| {
                            ui.set_width(card_inner_w);

                            match id_bind {
                                0 => {
                                    ui.label(
                                        RichText::new("② 填写联系人信息（非实名制项目）")
                                            .size(15.0)
                                            .strong()
                                            .color(palette.text_primary),
                                    );
                                    ui.add_space(8.0);
                                    if let Some(ref mut buyer_info) =
                                        app.bilibiliticket_list[biliticket_index].no_bind_buyer_info
                                    {
                                        let input_w = (card_inner_w - 140.0).clamp(180.0, 360.0);
                                        egui::Grid::new("no_bind_contact_grid_v2")
                                            .num_columns(2)
                                            .spacing([12.0, 10.0])
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new("联系人姓名：")
                                                        .size(14.0)
                                                        .color(palette.text_primary),
                                                );
                                                ui.add(
                                                    egui::TextEdit::singleline(&mut buyer_info.name)
                                                        .hint_text("请输入联系人姓名")
                                                        .desired_width(input_w)
                                                        .min_size(Vec2::new(input_w, 30.0)),
                                                );
                                                ui.end_row();

                                                ui.label(
                                                    RichText::new("联系人手机号：")
                                                        .size(14.0)
                                                        .color(palette.text_primary),
                                                );
                                                let tel_resp = ui.add(
                                                    egui::TextEdit::singleline(&mut buyer_info.tel)
                                                        .hint_text("请输入 7-15 位数字手机号")
                                                        .desired_width(input_w)
                                                        .min_size(Vec2::new(input_w, 30.0)),
                                                );
                                                if (tel_resp.changed() || tel_resp.lost_focus())
                                                    && (!ime_composing || tel_resp.lost_focus())
                                                {
                                                    buyer_info.tel =
                                                        normalize_fullwidth_ascii(&buyer_info.tel)
                                                            .chars()
                                                            .filter(|c| c.is_ascii_digit())
                                                            .take(15)
                                                            .collect();
                                                }
                                                ui.end_row();
                                            });

                                        let name_ok = !buyer_info.name.trim().is_empty();
                                        let tel_clean = normalize_fullwidth_ascii(&buyer_info.tel);
                                        let tel_ok = is_valid_phone(tel_clean.trim());
                                        if !name_ok || !tel_ok {
                                            ui.add_space(6.0);
                                            ui.label(
                                                RichText::new(
                                                    "提示：请填写非空联系人姓名及 7-15 位纯数字手机号",
                                                )
                                                .size(12.5)
                                                .color(palette.amber),
                                            );
                                        }
                                    }
                                }
                                1 | 2 => {
                                    let selected_names: Vec<String> = app
                                        .selected_buyer_list
                                        .as_ref()
                                        .map(|list| {
                                            list.iter()
                                                .filter(|sel| {
                                                    buyers.iter().any(|b| b.id == sel.id)
                                                })
                                                .map(|b| b.name.clone())
                                                .collect()
                                        })
                                        .unwrap_or_default();
                                    let selected_count = selected_names.len();
                                    let ticket_count = app.bilibiliticket_list[biliticket_index]
                                        .count
                                        .unwrap_or(1)
                                        .clamp(1, 10) as usize;
                                    let target_count = if id_bind == 1 { 1 } else { ticket_count };
                                    let count_matched = selected_count == target_count;

                                    // 顶部标题 + 进度徽章 + 快捷添加/刷新按钮
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("② 选择实名购票人")
                                                .size(15.0)
                                                .strong()
                                                .color(palette.text_primary),
                                        );
                                        let badge_text = if id_bind == 1 {
                                            format!("单选 · 已选 {}/1 人", selected_count)
                                        } else {
                                            format!(
                                                "一票一证 · 已选 {}/{} 人",
                                                selected_count, target_count
                                            )
                                        };
                                        let (bg, fg) = if count_matched {
                                            (palette.green_bg, palette.green)
                                        } else {
                                            (palette.amber_bg, palette.amber)
                                        };
                                        pill_badge(ui, &badge_text, bg, fg);

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .add_enabled(
                                                        !app.buyer_info_requesting,
                                                        egui::Button::new(
                                                            RichText::new("刷新").size(12.5),
                                                        )
                                                        .rounding(6.0),
                                                    )
                                                    .on_hover_text("重新拉取账号下的实名购票人列表")
                                                    .clicked()
                                                {
                                                    refresh_buyers_clicked = true;
                                                }
                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            RichText::new("+ 添加购票人")
                                                                .size(12.5)
                                                                .color(Color32::WHITE),
                                                        )
                                                        .fill(palette.accent)
                                                        .rounding(6.0),
                                                    )
                                                    .clicked()
                                                {
                                                    add_buyer_clicked = true;
                                                }
                                            },
                                        );
                                    });

                                    // 一票一证人数与票数不一致时提供一键同步票数快捷操作
                                    if id_bind == 2
                                        && selected_count > 0
                                        && selected_count != ticket_count
                                        && selected_count <= 10
                                    {
                                        ui.add_space(6.0);
                                        egui::Frame::none()
                                            .fill(palette.amber_bg)
                                            .rounding(7.0)
                                            .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                                            .show(ui, |ui| {
                                                ui.set_width((card_inner_w - 22.0).max(200.0));
                                                ui.horizontal_wrapped(|ui| {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "一票一证要求购票人数与票数一致（当前已选 {} 人，票数 {} 张）",
                                                            selected_count, ticket_count
                                                        ))
                                                        .size(12.5)
                                                        .color(palette.amber),
                                                    );
                                                    if ui
                                                        .add(
                                                            egui::Button::new(
                                                                RichText::new(format!(
                                                                    "将票数同步为 {} 张",
                                                                    selected_count
                                                                ))
                                                                .size(12.0)
                                                                .strong()
                                                                .color(Color32::WHITE),
                                                            )
                                                            .fill(palette.accent)
                                                            .rounding(5.0),
                                                        )
                                                        .clicked()
                                                    {
                                                        app.bilibiliticket_list[biliticket_index]
                                                            .count = Some(selected_count as i32);
                                                    }
                                                });
                                            });
                                    }

                                    // 已选人员摘要条
                                    if !selected_names.is_empty() {
                                        ui.add_space(6.0);
                                        ui.horizontal_wrapped(|ui| {
                                            ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                                            ui.label(
                                                RichText::new(format!(
                                                    "已选：{}",
                                                    selected_names.join("、")
                                                ))
                                                .size(12.5)
                                                .strong()
                                                .color(palette.green),
                                            );
                                            if id_bind == 2
                                                && ui
                                                    .small_button("清空")
                                                    .on_hover_text("清空当前已选购票人")
                                                    .clicked()
                                            {
                                                app.selected_buyer_list = None;
                                            }
                                        });
                                    }

                                    ui.add_space(8.0);

                                    // 加载态 / 错误态 / 空列表态 / 购票人卡片列表
                                    if app.buyer_info_requesting {
                                        ui.horizontal(|ui| {
                                            ui.spinner();
                                            ui.label(
                                                RichText::new("正在加载实名购票人信息...")
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
                                                RichText::new("正在准备获取购票人信息...")
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
                                                    ui.add_space(4.0);
                                                    ui.label(
                                                        RichText::new(
                                                            "点击下方按钮添加实名信息后即可继续下单",
                                                        )
                                                        .size(12.5)
                                                        .color(palette.text_muted),
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
                                        ui.label(
                                            RichText::new("点击任意购票人卡片即可选中或取消")
                                                .size(12.0)
                                                .color(palette.text_muted),
                                        );
                                        ui.add_space(4.0);

                                        let is_multi_select = id_bind == 2;
                                        if let Some(single_buyer) = render_buyer_cards_grid(
                                            ui,
                                            "confirm_mode01",
                                            &buyers,
                                            &mut app.selected_buyer_list,
                                            is_multi_select,
                                            card_inner_w,
                                            &palette,
                                        ) {
                                            app.selected_buyer_uid = Some(biliticket_uid);
                                            app.bilibiliticket_list[biliticket_index].buyer_info =
                                                Some(vec![single_buyer]);
                                        }
                                        if app.selected_buyer_list.is_some() {
                                            app.selected_buyer_uid = Some(biliticket_uid);
                                        }
                                    }
                                }
                                _ => {
                                    ui.label(
                                        RichText::new(format!(
                                            "当前项目返回了未知的实名绑定状态 (id_bind={})，无法确认购票人规则，请返回重新获取项目详情或改用捡漏模式。",
                                            id_bind
                                        ))
                                        .color(palette.red),
                                    );
                                }
                            }
                        });

                    ui.add_space(6.0);
                },
            );
        });

    if back_to_screen_clicked {
        app.confirm_ticket_info = None;
        app.buyer_info_requesting = false;
        app.buyer_info_active_task_id = None;
        app.buyer_info_error = None;
        app.reset_buyer_selection();
        app.show_screen_info = Some(biliticket_uid);
        return;
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
        if let (Some((screen_id, _)), Some((ticket_id, _, _))) = (screen_summary, ticket_summary) {
            let count = app.bilibiliticket_list[biliticket_index]
                .count
                .unwrap_or(1)
                .clamp(1, 10);
            app.bilibiliticket_list[biliticket_index].count = Some(count);

            let submitted_buyers: Option<Vec<BuyerInfo>> = match id_bind {
                0 => {
                    if let Some(ref mut info) =
                        app.bilibiliticket_list[biliticket_index].no_bind_buyer_info
                    {
                        info.name = info.name.trim().to_string();
                        info.tel = normalize_fullwidth_ascii(&info.tel).trim().to_string();
                        if !info.name.is_empty() && is_valid_phone(&info.tel) {
                            Some(Vec::new())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                1 => app.selected_buyer_list.as_ref().and_then(|list| {
                    if list.len() == 1 && buyers.iter().any(|b| b.id == list[0].id) {
                        Some(list.clone())
                    } else {
                        None
                    }
                }),
                2 => app.selected_buyer_list.as_ref().and_then(|list| {
                    if !list.is_empty()
                        && list.len() == count as usize
                        && list
                            .iter()
                            .all(|sel| buyers.iter().any(|b| b.id == sel.id))
                    {
                        Some(list.clone())
                    } else {
                        None
                    }
                }),
                _ => None,
            };

            if let Some(buyer_list) = submitted_buyers {
                if id_bind == 1 || id_bind == 2 {
                    app.bilibiliticket_list[biliticket_index].buyer_info = Some(buyer_list.clone());
                }
                let biliticket_snapshot = app.bilibiliticket_list[biliticket_index].clone();
                let grab_ticket_request = GrabTicketRequest {
                    task_id: String::new(),
                    uid: biliticket_uid,
                    project_id: project_id_str,
                    screen_id: screen_id.to_string(),
                    ticket_id: ticket_id.to_string(),
                    is_hot,
                    count: count as i16,
                    buyer_info: buyer_list,
                    grab_mode: ticket_method,
                    status: TaskStatus::Pending,
                    start_time: None,
                    cookie_manager,
                    biliticket: biliticket_snapshot,
                    local_captcha: app.local_captcha.clone(),
                    skip_words: app.skip_words.clone(),
                };
                log::debug!("提交抢票任务: {:?}", grab_ticket_request);
                match app
                    .task_manager
                    .submit_task(TaskRequest::GrabTicketRequest(grab_ticket_request))
                {
                    Ok(task_id) => {
                        log::info!("提交抢票任务成功，任务ID: {}", task_id);
                        app.show_success("抢票任务已提交，请在「监视面板」查看进度");
                        app.confirm_ticket_info = None;
                        app.buyer_info_requesting = false;
                        app.buyer_info_active_task_id = None;
                        app.buyer_info_error = None;
                        app.reset_buyer_selection();
                    }
                    Err(e) => {
                        log::error!("提交抢票任务失败: {}", e);
                        app.show_error(format!("提交抢票任务失败: {}", e));
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

fn is_valid_phone(phone: &str) -> bool {
    let trimmed = phone.trim();
    (7..=15).contains(&trimmed.len()) && trimmed.bytes().all(|b| b.is_ascii_digit())
}

/// UTF-8 安全地脱敏证件号码（按字符数而非字节切片）
pub(crate) fn mask_id(id: &str) -> String {
    let chars: Vec<char> = id.chars().collect();
    let len = chars.len();
    if len <= 6 {
        return id.to_string();
    }
    let prefix: String = chars.iter().take(3).collect();
    let suffix: String = chars[len - 3..].iter().collect();
    let mask = "*".repeat((len - 6).min(6));
    format!("{}{}{}", prefix, mask, suffix)
}

#[cfg(test)]
mod tests {
    use super::{
        compute_buyer_grid_layout, mask_id, render_buyer_cards_grid, ConfirmPalette,
    };
    use common::ticket::BuyerInfo;
    use eframe::egui;

    #[test]
    fn mask_id_handles_ascii_short_cjk_and_fullwidth_safely() {
        assert_eq!(mask_id("110101199001011234"), "110******234");
        assert_eq!(mask_id("123456"), "123456");
        assert_eq!(mask_id("abc"), "abc");
        assert_eq!(mask_id(""), "");
        assert_eq!(mask_id("张三李四王五赵六"), "张三李**五赵六");
        assert_eq!(mask_id("１２３４５６７８９０"), "１２３****８９０");
        assert_eq!(mask_id("京1234567890沪"), "京12******90沪");
    }

    #[test]
    fn buyer_grid_layout_stays_within_container_bounds_across_20_frames() {
        for width in [360.0_f32, 480.0, 560.0, 660.0, 900.0] {
            let (cols, cell_inner_w) = compute_buyer_grid_layout(width);
            let total = cols as f32 * (cell_inner_w + 24.0) + (cols - 1) as f32 * 10.0;
            assert!(
                total <= width + 0.5,
                "buyer grid total width {} exceeded container {}",
                total,
                width
            );
        }

        let buyers = vec![
            BuyerInfo {
                id: 1,
                name: "张三".to_string(),
                personal_id: "110101199001011234".to_string(),
                tel: "13800138000".to_string(),
                id_type: 0,
                is_default: 1,
                ..Default::default()
            },
            BuyerInfo {
                id: 2,
                name: "李四".to_string(),
                personal_id: "H12345678(0)".to_string(),
                tel: "13900139000".to_string(),
                id_type: 2,
                is_default: 0,
                ..Default::default()
            },
            BuyerInfo {
                id: 3,
                name: "王五长名字测试换行".to_string(),
                personal_id: "E12345678".to_string(),
                tel: "13700137000".to_string(),
                id_type: 1,
                is_default: 0,
                ..Default::default()
            },
        ];

        let ctx = egui::Context::default();
        let mut selected = Some(vec![buyers[0].clone()]);
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
                let palette = ConfirmPalette::from_dark_mode(false);
                let resp = egui::Window::new("确认购票信息")
                    .id(egui::Id::new("confirm_ticket_window_v2"))
                    .default_size([700.0, 590.0])
                    .min_width(520.0)
                    .min_height(420.0)
                    .resize(|r| r.max_size([1160.0, 640.0]))
                    .constrain(true)
                    .resizable(true)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical()
                            .id_source("confirm_ticket_test_scroll")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let card_outer_w = (ui.available_width() - 6.0).max(320.0);
                                let card_inner_w = (card_outer_w - 30.0).max(280.0);
                                let _ = render_buyer_cards_grid(
                                    ui,
                                    "test_grid",
                                    &buyers,
                                    &mut selected,
                                    true,
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

        assert!(
            (widths[19] - widths[2]).abs() < 1.0 && widths[19] <= 720.0,
            "confirm window width drifted: {:?}",
            widths
        );
    }
}
