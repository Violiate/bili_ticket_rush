use std::sync::Arc;

use common::cookie_manager::CookieManager;
use common::taskmanager::{GrabTicketRequest, TaskRequest, TaskStatus};
use common::ticket::{BuyerInfo, NoBindBuyerInfo};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};

use crate::app::{normalize_fullwidth_ascii, resolve_initial_buyers, Myapp};
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
        2 => "港澳证",
        3 => "台湾证",
        _ => "其他证件",
    }
}

/// 纯矢量绘制抗锯齿对勾（完全不依赖系统字体字形，在任何平台上都不会出现方框乱码）
pub(crate) fn draw_vector_checkmark(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    stroke_width: f32,
    color: Color32,
) {
    let p1 = egui::pos2(center.x - radius * 0.48, center.y + radius * 0.04);
    let p2 = egui::pos2(center.x - radius * 0.10, center.y + radius * 0.40);
    let p3 = egui::pos2(center.x + radius * 0.54, center.y - radius * 0.34);
    let stroke = Stroke::new(stroke_width, color);
    painter.line_segment([p1, p2], stroke);
    painter.line_segment([p2, p3], stroke);
    painter.circle_filled(p2, stroke_width * 0.42, color);
}

/// 纯矢量绘制感叹号（用于错误/警告圆标）
pub(crate) fn draw_vector_exclamation(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    color: Color32,
) {
    let top = egui::pos2(center.x, center.y - radius * 0.48);
    let mid = egui::pos2(center.x, center.y + radius * 0.12);
    let dot = egui::pos2(center.x, center.y + radius * 0.48);
    painter.line_segment([top, mid], Stroke::new(1.9, color));
    painter.circle_filled(dot, 1.25, color);
}

/// 纯矢量绘制信息图标 i（用于提示圆标）
pub(crate) fn draw_vector_info(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: f32,
    color: Color32,
) {
    let dot = egui::pos2(center.x, center.y - radius * 0.44);
    let top = egui::pos2(center.x, center.y - radius * 0.12);
    let bot = egui::pos2(center.x, center.y + radius * 0.50);
    painter.circle_filled(dot, 1.25, color);
    painter.line_segment([top, bot], Stroke::new(1.9, color));
}

/// 绘制内联 16x16 圆形矢量状态图标（ok=true 画勾，ok=false 画感叹号）
pub(crate) fn draw_inline_status_icon(
    ui: &mut egui::Ui,
    ok: bool,
    circle_fill: Color32,
    glyph_color: Color32,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.circle_filled(rect.center(), 7.5, circle_fill);
    if ok {
        draw_vector_checkmark(painter, rect.center(), 6.2, 1.8, glyph_color);
    } else {
        draw_vector_exclamation(painter, rect.center(), 6.2, glyph_color);
    }
}

/// 精简胶囊按钮（替代简陋的默认 small_button，保持现代一致视觉）
pub(crate) fn subtle_action_button(
    ui: &mut egui::Ui,
    label: &str,
    fill: Color32,
    border: Color32,
    text_color: Color32,
) -> egui::Response {
    let btn = egui::Button::new(RichText::new(label).size(12.0).strong().color(text_color))
        .fill(fill)
        .stroke(Stroke::new(1.0, border))
        .rounding(6.0)
        .min_size(Vec2::new(0.0, 26.0));
    ui.add(btn)
}

/// 计算购票人网格的列数与严格有界的单卡外框宽度，杜绝任何水平撑宽。
pub(crate) fn compute_buyer_grid_layout(card_inner_w: f32) -> (usize, f32) {
    let spacing = 10.0;
    let cols: usize = if card_inner_w >= 500.0 { 2 } else { 1 };
    let total_gap = spacing * (cols.saturating_sub(1) as f32);
    let cell_outer_w = ((card_inner_w - total_gap - 2.0) / cols as f32).max(196.0);
    let cell_inner_w = (cell_outer_w - 24.0).max(172.0);
    (cols, cell_inner_w)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let prefix: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}…", prefix)
    } else {
        prefix
    }
}

/// 在指定 Painter 上绘制紧凑圆角胶囊徽章（返回胶囊总宽度），不创建任何子 Ui，严格遵守 Painter 裁剪矩形
fn paint_pill_on(
    painter: &egui::Painter,
    anchor_pos: egui::Pos2,
    right_aligned: bool,
    text: &str,
    fill: Color32,
    text_color: Color32,
) -> f32 {
    let galley = painter.layout_no_wrap(
        text.to_string(),
        egui::FontId::proportional(11.2),
        text_color,
    );
    let pad_x = 7.0;
    let pill_w = galley.size().x + pad_x * 2.0;
    let pill_h = 19.0;
    let rect = if right_aligned {
        egui::Rect::from_min_size(
            egui::pos2(anchor_pos.x - pill_w, anchor_pos.y - pill_h * 0.5),
            Vec2::new(pill_w, pill_h),
        )
    } else {
        egui::Rect::from_min_size(
            egui::pos2(anchor_pos.x, anchor_pos.y - pill_h * 0.5),
            Vec2::new(pill_w, pill_h),
        )
    };
    painter.rect_filled(rect, 5.5, fill);
    let text_pos = egui::pos2(
        rect.center().x - galley.size().x * 0.5,
        rect.center().y - galley.size().y * 0.5,
    );
    painter.galley(text_pos, galley);
    pill_w
}

/// 单张高审美购票人卡片绘制（纯 Painter 几何排版 + 严格与父级 ScrollArea 裁剪区求交集，永不穿透底栏）
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_buyer_card_item(
    ui: &mut egui::Ui,
    card_id: egui::Id,
    buyer: &BuyerInfo,
    is_selected: bool,
    default_rank: Option<usize>,
    is_multi_select: bool,
    cell_outer_w: f32,
    palette: &ConfirmPalette,
) -> egui::Response {
    let card_h = 70.0;
    let (card_rect, resp) =
        ui.allocate_exact_size(Vec2::new(cell_outer_w, card_h), egui::Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let hovered = resp.hovered();
    let _ = card_id;

    let effective_clip = card_rect
        .expand(1.5)
        .intersect(ui.clip_rect());
    if !effective_clip.is_positive() {
        return resp;
    }

    let painter = ui.painter_at(effective_clip);

    let fill = if is_selected {
        palette.selected_bg
    } else if hovered {
        palette.item_hover_bg
    } else {
        palette.item_bg
    };
    let stroke = if is_selected {
        Stroke::new(1.6, palette.selected_border)
    } else if hovered {
        Stroke::new(1.2, palette.accent.linear_multiply(0.75))
    } else {
        Stroke::new(1.0, palette.item_border)
    };

    painter.rect(card_rect, 9.0, fill, stroke);

    // 选中态左侧精致高亮竖条
    if is_selected {
        let bar_rect = egui::Rect::from_min_max(
            egui::pos2(card_rect.left() + 3.5, card_rect.top() + 12.0),
            egui::pos2(card_rect.left() + 6.5, card_rect.bottom() - 12.0),
        );
        painter.rect_filled(bar_rect, 2.0, palette.selected_border);
    }

    // 左侧垂直居中的矢量单选/复选指示器（20x20）
    let check_center = egui::pos2(card_rect.left() + 23.0, card_rect.center().y);
    let check_rect = egui::Rect::from_center_size(check_center, Vec2::new(20.0, 20.0));
    let check_rounding = if is_multi_select { 6.0 } else { 10.0 };
    if is_selected {
        painter.rect_filled(check_rect, check_rounding, palette.selected_border);
        draw_vector_checkmark(&painter, check_center, 8.0, 2.1, Color32::WHITE);
    } else {
        let border_c = if hovered {
            palette.accent
        } else {
            palette.text_muted.linear_multiply(0.75)
        };
        painter.rect_stroke(
            check_rect.shrink(0.5),
            check_rounding,
            Stroke::new(1.5, border_c),
        );
    }

    // 右侧两行纯几何精准排版（全部绘制在已求交集的 painter 上，绝对不会越出卡片或底栏半像素）
    let content_left = card_rect.left() + 44.0;
    let content_right = card_rect.right() - 12.0;
    let content_w = (content_right - content_left).max(80.0);
    let row1_y = card_rect.top() + 23.5;
    let row2_y = card_rect.top() + 48.0;

    // 右上角：默认顺位胶囊（若存在）
    let right_tag_w = match default_rank {
        Some(1) => paint_pill_on(
            &painter,
            egui::pos2(content_right, row1_y),
            true,
            "默认 1",
            palette.green_bg,
            palette.green,
        ),
        Some(2) => paint_pill_on(
            &painter,
            egui::pos2(content_right, row1_y),
            true,
            "默认 2",
            palette.blue_bg,
            palette.blue,
        ),
        _ if buyer.is_default == 1 => paint_pill_on(
            &painter,
            egui::pos2(content_right, row1_y),
            true,
            "常用",
            palette.badge_bg,
            palette.text_muted,
        ),
        _ => 0.0,
    };

    // 左上角：姓名 + 证件类型胶囊
    let avail_for_name_and_type = (content_w - right_tag_w - 8.0).max(60.0);
    let max_name_chars = if avail_for_name_and_type < 130.0 { 4 } else { 7 };
    let display_name = truncate_chars(buyer.name.trim(), max_name_chars);
    let name_galley = painter.layout_no_wrap(
        display_name,
        egui::FontId::proportional(15.0),
        palette.text_primary,
    );
    let name_w = name_galley.size().x;
    painter.galley(
        egui::pos2(content_left, row1_y - name_galley.size().y * 0.5),
        name_galley,
    );

    let type_x = content_left + name_w + 7.0;
    if type_x + 42.0 <= content_right - right_tag_w {
        let _ = paint_pill_on(
            &painter,
            egui::pos2(type_x, row1_y),
            false,
            id_type_label(buyer.id_type),
            palette.badge_bg,
            palette.text_secondary,
        );
    }

    // 第二行：等宽脱敏证件号与手机号
    let masked_id = mask_id(&buyer.personal_id);
    let masked_tel = mask_phone(&buyer.tel);
    let meta_text = if masked_tel.is_empty() {
        masked_id
    } else {
        format!("{}  ·  {}", masked_id, masked_tel)
    };
    let meta_color = if is_selected {
        palette.text_primary.linear_multiply(0.88)
    } else {
        palette.text_secondary
    };
    let meta_galley = painter.layout_no_wrap(
        meta_text,
        egui::FontId::new(12.2, egui::FontFamily::Monospace),
        meta_color,
    );
    painter.galley(
        egui::pos2(content_left, row2_y - meta_galley.size().y * 0.5),
        meta_galley,
    );

    resp
}

/// 统一的实名购票人卡片选择列表（供普通模式与捡漏模式共用）：
/// 整张卡片固定 70px 等高网格对齐，纯矢量勾选动画，绝无乱码或参差错位。
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_buyer_cards_grid(
    ui: &mut egui::Ui,
    id_salt: &str,
    buyers: &[BuyerInfo],
    configured_default_ids: &[i64],
    selected_buyer_list: &mut Option<Vec<BuyerInfo>>,
    is_multi_select: bool,
    card_inner_w: f32,
    palette: &ConfirmPalette,
) -> Option<BuyerInfo> {
    let mut single_selected_buyer: Option<BuyerInfo> = None;
    let (cols, cell_inner_w) = compute_buyer_grid_layout(card_inner_w);
    let cell_outer_w = cell_inner_w + 24.0;
    let spacing = 10.0;

    for (row_idx, row_buyers) in buyers.chunks(cols).enumerate() {
        if row_idx > 0 {
            ui.add_space(9.0);
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;
            for buyer in row_buyers {
                let is_selected = selected_buyer_list
                    .as_ref()
                    .map_or(false, |list| list.iter().any(|b| b.id == buyer.id));
                let default_rank = configured_default_ids
                    .iter()
                    .position(|&id| id == buyer.id)
                    .map(|p| p + 1);
                let card_id = ui.id().with((id_salt, "buyer_card", buyer.id));

                let interact_resp = draw_buyer_card_item(
                    ui,
                    card_id,
                    buyer,
                    is_selected,
                    default_rank,
                    is_multi_select,
                    cell_outer_w,
                    palette,
                );

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

    // 非实名制（id_bind == 0）首次打开时从设置预填联系人姓名与手机号
    if id_bind == 0 && app.bilibiliticket_list[biliticket_index].no_bind_buyer_info.is_none() {
        let preinput_name = app.get_preinput_name_for_uid(*uid).unwrap_or_default();
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
            name: preinput_name,
            tel: preinput_phone,
            uid: biliticket_uid,
        });
    }

    let configured_default_ids = app.custom_config.default_buyers_for_uid(biliticket_uid);

    // 实名制项目（id_bind == 1 / 2）首次获取到购票人列表后，按设置单次自动勾选默认购票人（不覆盖后续手动操作）
    if (id_bind == 1 || id_bind == 2)
        && buyers_loaded
        && !buyers.is_empty()
        && app.default_buyers_applied_uid != Some(biliticket_uid)
    {
        app.default_buyers_applied_uid = Some(biliticket_uid);
        if app.selected_buyer_list.is_none() {
            let auto_list = resolve_initial_buyers(
                app.custom_config.enable_default_buyers,
                &configured_default_ids,
                &buyers,
                id_bind == 2,
            );
            if !auto_list.is_empty() {
                if id_bind == 2 {
                    app.bilibiliticket_list[biliticket_index].count =
                        Some((auto_list.len() as i32).clamp(1, 10));
                } else {
                    app.bilibiliticket_list[biliticket_index].buyer_info = Some(auto_list.clone());
                }
                app.selected_buyer_list = Some(auto_list);
                app.selected_buyer_uid = Some(biliticket_uid);
            }
        }
    }

    let mut cancel_clicked = false;
    let mut back_to_screen_clicked = false;
    let mut submit_clicked = false;
    let mut refresh_buyers_clicked = false;
    let mut add_buyer_clicked = false;
    let mut save_as_default_buyers: Option<Vec<i64>> = None;
    let mut save_preinput_contact: Option<(String, String)> = None;

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
                            (true, "联系人信息已就绪，点击右侧确认下单".to_string(), true)
                        } else {
                            (false, "请填写联系人姓名和有效手机号".to_string(), false)
                        }
                    }
                    1 => {
                        let ok = app.selected_buyer_list.as_ref().map_or(false, |list| {
                            list.len() == 1 && buyers.iter().any(|b| b.id == list[0].id)
                        });
                        if ok {
                            (true, "已选定购票人，点击右侧提交抢票任务".to_string(), true)
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
                                    "已选 {} 位购票人（{} 张票），可提交任务",
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

            let no_widget_focused = ui.ctx().memory(|m| m.focus().is_none());
            if no_widget_focused && !ime_composing {
                if button_enabled && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit_clicked = true;
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    cancel_clicked = true;
                }
            }

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
                    ui.add_space(5.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        draw_inline_status_icon(
                            ui,
                            status_ok,
                            if status_ok { palette.green } else { palette.amber },
                            Color32::WHITE,
                        );
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
                    });
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
                                        } else {
                                            let cur_name = buyer_info.name.trim().to_string();
                                            let cur_tel = tel_clean.trim().to_string();
                                            let saved_same = app
                                                .get_preinput_name_for_uid(biliticket_uid)
                                                .as_deref()
                                                == Some(cur_name.as_str())
                                                && app
                                                    .get_preinput_phone_for_uid(biliticket_uid)
                                                    .as_deref()
                                                    == Some(cur_tel.as_str());
                                            if !saved_same {
                                                ui.add_space(6.0);
                                                if subtle_action_button(
                                                    ui,
                                                    "保存为本号默认预填联系人",
                                                    palette.blue_bg,
                                                    palette.blue.linear_multiply(0.45),
                                                    palette.blue,
                                                )
                                                .on_hover_text("下次打开非实名制项目时将自动填入该姓名与手机号")
                                                .clicked()
                                                {
                                                    save_preinput_contact =
                                                        Some((cur_name, cur_tel));
                                                }
                                            }
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

                                    // 顶部统一标题与工具栏（单行清爽排布）
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0;
                                        ui.label(
                                            RichText::new("② 选择实名购票人")
                                                .size(15.0)
                                                .strong()
                                                .color(palette.text_primary),
                                        );
                                        let badge_text = if id_bind == 1 {
                                            format!("单选 {}/1", selected_count)
                                        } else {
                                            format!("一票一证 {}/{}", selected_count, target_count)
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
                                                if id_bind == 2
                                                    && selected_count > 0
                                                    && subtle_action_button(
                                                        ui,
                                                        "清空",
                                                        palette.item_bg,
                                                        palette.item_border,
                                                        palette.text_secondary,
                                                    )
                                                    .on_hover_text("清空当前已选购票人")
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

                                    // 一票一证人数与票数不一致时提供一键同步票数快捷操作
                                    if id_bind == 2
                                        && selected_count > 0
                                        && selected_count != ticket_count
                                        && selected_count <= 10
                                    {
                                        ui.add_space(8.0);
                                        egui::Frame::none()
                                            .fill(palette.amber_bg)
                                            .rounding(7.0)
                                            .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                                            .show(ui, |ui| {
                                                ui.set_width((card_inner_w - 22.0).max(200.0));
                                                ui.horizontal_wrapped(|ui| {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "当前已选 {} 人，票数为 {} 张（一票一证要求一致）",
                                                            selected_count, ticket_count
                                                        ))
                                                        .size(12.5)
                                                        .color(palette.amber),
                                                    );
                                                    if ui
                                                        .add(
                                                            egui::Button::new(
                                                                RichText::new(format!(
                                                                    "同步为 {} 张",
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

                                    // 单行精炼副标题（替代原先堆叠的多行杂乱文字）
                                    ui.add_space(6.0);
                                    if !selected_names.is_empty() {
                                        ui.label(
                                            RichText::new(format!(
                                                "当前已选：{}   ·   点击下方卡片可切换",
                                                selected_names.join("、")
                                            ))
                                            .size(12.2)
                                            .color(palette.green),
                                        );
                                    } else {
                                        ui.label(
                                            RichText::new("点击下方购票人卡片即可选中")
                                                .size(12.2)
                                                .color(palette.text_muted),
                                        );
                                    }

                                    ui.add_space(10.0);

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
                                        let is_multi_select = id_bind == 2;
                                        let prev_sel_len = app
                                            .selected_buyer_list
                                            .as_ref()
                                            .map_or(0, Vec::len);
                                        if let Some(single_buyer) = render_buyer_cards_grid(
                                            ui,
                                            "confirm_mode01",
                                            &buyers,
                                            &configured_default_ids,
                                            &mut app.selected_buyer_list,
                                            is_multi_select,
                                            card_inner_w,
                                            &palette,
                                        ) {
                                            app.selected_buyer_uid = Some(biliticket_uid);
                                            app.bilibiliticket_list[biliticket_index].buyer_info =
                                                Some(vec![single_buyer]);
                                        }
                                        let new_sel_len = app
                                            .selected_buyer_list
                                            .as_ref()
                                            .map_or(0, Vec::len);
                                        if id_bind == 2
                                            && new_sel_len != prev_sel_len
                                            && (1..=10).contains(&new_sel_len)
                                        {
                                            // 一票一证点击勾选购票人时自动联动同步票数，免去额外点击加减号
                                            app.bilibiliticket_list[biliticket_index].count =
                                                Some(new_sel_len as i32);
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

    if let Some(ids) = save_as_default_buyers {
        app.custom_config
            .set_default_buyers_for_uid(biliticket_uid, ids);
        app.custom_config.enable_default_buyers = true;
        if app.auto_save_settings() {
            app.show_success("已保存为当前账号默认购票人，下次打开将自动勾选");
        }
    }

    if let Some((name, tel)) = save_preinput_contact {
        if let Some(pos) = app
            .account_manager
            .accounts
            .iter()
            .position(|a| a.uid == biliticket_uid)
        {
            match pos {
                0 => {
                    app.custom_config.preinput_name1 = name;
                    app.custom_config.preinput_phone1 = tel;
                }
                1 => {
                    app.custom_config.preinput_name2 = name;
                    app.custom_config.preinput_phone2 = tel;
                }
                _ => {}
            }
            if app.auto_save_settings() {
                app.show_success("已保存为当前账号预填联系人信息");
            }
        }
    }

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

/// 手机号中间脱敏展示（如 138****8000）
pub(crate) fn mask_phone(tel: &str) -> String {
    let trimmed = tel.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    let len = chars.len();
    if len >= 11 {
        let p: String = chars.iter().take(3).collect();
        let s: String = chars[len - 4..].iter().collect();
        format!("{}****{}", p, s)
    } else {
        trimmed.to_string()
    }
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
                                    &[1],
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

    #[test]
    fn buyer_card_text_clip_rect_never_bleeds_outside_scroll_viewport() {
        let buyers: Vec<BuyerInfo> = (1..=6)
            .map(|i| BuyerInfo {
                id: i,
                name: format!("测试人{}", i),
                personal_id: "110101199001011234".to_string(),
                tel: "13800138000".to_string(),
                id_type: 0,
                ..Default::default()
            })
            .collect();

        let ctx = egui::Context::default();
        let mut selected = Some(vec![buyers[0].clone()]);
        let mut footer_top_y = 0.0_f32;
        let mut last_out = None;
        for _ in 0..3 {
            let out = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 500.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    let palette = ConfirmPalette::from_dark_mode(true);
                    egui::Window::new("裁剪验证弹窗")
                        .id(egui::Id::new("clip_verify_win"))
                        .default_size([620.0, 260.0])
                        .show(ctx, |ui| {
                            crate::windows::show_resizable_body(
                                ui,
                                "clip_check_scroll",
                                |ui| {
                                    ui.label("顶栏");
                                },
                                |ui| {
                                    footer_top_y = ui.max_rect().top();
                                    ui.label("底栏确认购票行");
                                },
                                |ui| {
                                    let _ = render_buyer_cards_grid(
                                        ui,
                                        "clip_grid",
                                        &buyers,
                                        &[1],
                                        &mut selected,
                                        true,
                                        520.0,
                                        &palette,
                                    );
                                },
                            );
                        });
                },
            );
            last_out = Some(out);
        }
        let out = last_out.unwrap();

        // 验证所有购票人卡片与文字图元（单卡宽度 ~255px < 400px）裁剪底边严格 <= 底栏顶边（0px 越界）
        let mut checked_card_shapes = 0usize;
        for clipped in &out.shapes {
            if clipped.clip_rect.width() < 400.0 && clipped.clip_rect.top() > 20.0 {
                checked_card_shapes += 1;
                assert!(
                    clipped.clip_rect.bottom() <= footer_top_y + 0.5,
                    "buyer card shape clip_rect {:?} bled into footer (footer_top_y = {})",
                    clipped.clip_rect,
                    footer_top_y
                );
            }
        }
        assert!(checked_card_shapes > 0, "expected buyer card shapes to be rendered");
    }
}
