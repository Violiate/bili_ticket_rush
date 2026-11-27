use common::taskmanager::{AddBuyerRequest, TaskRequest};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};

use crate::app::{normalize_fullwidth_ascii, Myapp};
use crate::windows::confirm_ticket::{
    draw_inline_status_icon, pill_badge, ConfirmPalette,
};
use crate::windows::{bounded_window_sizes, show_resizable_body};

#[derive(Clone, Debug)]
pub struct AddBuyerInput {
    pub name: String,
    pub phone: String,
    pub id_type: usize,
    pub id_number: String,
    pub as_default_buyer: bool,
    pub also_app_default: bool,
}

impl Default for AddBuyerInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            phone: String::new(),
            id_type: 0,
            id_number: String::new(),
            as_default_buyer: false,
            also_app_default: true,
        }
    }
}

const PREFILL_MEM_KEY: &str = "add_buyer_prefilled_uid";

pub fn show(app: &mut Myapp, ctx: &egui::Context, uid: &str) {
    let Some((account_uid, account_name, select_cookie_manager)) = app
        .account_manager
        .accounts
        .iter()
        .find(|account| account.uid.to_string() == uid)
        .map(|account| (account.uid, account.name.clone(), account.cookie_manager.clone()))
    else {
        log::warn!("未找到账号 {}，自动关闭添加购票人窗口", uid);
        reset_add_buyer_window(app, ctx);
        return;
    };

    let Some(select_cookie_manager) = select_cookie_manager else {
        log::error!("账号 {} 的客户端未初始化", account_uid);
        app.show_error(format!("账号 {} 的客户端未初始化，请重新登录", account_uid));
        reset_add_buyer_window(app, ctx);
        return;
    };

    // 首次打开（表单为空时）自动预填设置中的默认姓名与手机号
    let prefilled_uid =
        ctx.memory(|mem| mem.data.get_temp::<i64>(egui::Id::new(PREFILL_MEM_KEY)));
    if prefilled_uid != Some(account_uid)
        && app.add_buyer_input.name.is_empty()
        && app.add_buyer_input.phone.is_empty()
        && app.add_buyer_input.id_number.is_empty()
    {
        if let Some(preinput_name) = app.get_preinput_name_for_uid(account_uid) {
            app.add_buyer_input.name = preinput_name;
        }
        if let Some(preinput) = app.get_preinput_phone_for_uid(account_uid) {
            app.add_buyer_input.phone = preinput
                .chars()
                .filter(|c| c.is_ascii_digit())
                .take(15)
                .collect();
        }
        app.add_buyer_input.also_app_default = app
            .custom_config
            .default_buyers_for_uid(account_uid)
            .len()
            < common::utility::MAX_DEFAULT_BUYERS_PER_ACCOUNT;
        ctx.memory_mut(|mem| {
            mem.data
                .insert_temp(egui::Id::new(PREFILL_MEM_KEY), account_uid);
        });
    }

    let mut window_open = app.show_add_buyer_window.is_some();
    let mut submit_clicked = false;
    let mut cancel_clicked = false;

    let (default_sz, min_sz, max_sz) = bounded_window_sizes(ctx, 570.0, 515.0, 440.0, 380.0);
    let palette = ConfirmPalette::from_dark_mode(ctx.style().visuals.dark_mode);
    let current_default_count = app.custom_config.default_buyers_for_uid(account_uid).len();

    let validation_preview = validate_add_buyer_input(
        &app.add_buyer_input.name,
        &app.add_buyer_input.phone,
        app.add_buyer_input.id_type,
        &app.add_buyer_input.id_number,
    );
    let form_ready = validation_preview.is_ok();

    egui::Window::new("新增实名购票人")
        .id(egui::Id::new("add_buyer_window_v2"))
        .open(&mut window_open)
        .collapsible(false)
        .default_size(default_sz)
        .min_width(min_sz[0])
        .min_height(min_sz[1])
        .resize(|r| r.max_size(max_sz))
        .constrain(true)
        .resizable(true)
        .show(ctx, |ui| {
            let editing_enabled = !app.add_buyer_submitting;
            let ime_composing = app.is_ime_composing();

            let no_focus = ui.ctx().memory(|m| m.focus().is_none());
            if !ime_composing && editing_enabled {
                if no_focus && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    cancel_clicked = true;
                } else if no_focus && form_ready && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit_clicked = true;
                }
            }

            show_resizable_body(
                ui,
                "add_buyer_scroll_v2",
                |ui| {
                    // 顶部标题与目标账号徽章栏
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(
                                RichText::new("填写实名观演人资料")
                                    .size(16.0)
                                    .strong()
                                    .color(palette.text_primary),
                            );
                            ui.label(
                                RichText::new("信息仅直连加密提交至哔哩哔哩会员购官方接口")
                                    .size(11.8)
                                    .color(palette.text_muted),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            pill_badge(
                                ui,
                                &format!("账号: {} ({})", account_name, account_uid),
                                palette.blue_bg,
                                palette.blue,
                            );
                        });
                    });
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);
                },
                |ui| {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if app.add_buyer_submitting {
                            ui.spinner();
                            ui.label(
                                RichText::new("正在提交至哔哩哔哩...")
                                    .size(12.5)
                                    .color(palette.blue),
                            );
                        } else {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            draw_inline_status_icon(
                                ui,
                                form_ready,
                                if form_ready {
                                    palette.green
                                } else {
                                    palette.amber
                                },
                                Color32::WHITE,
                            );
                            ui.label(
                                RichText::new(if form_ready {
                                    "资料格式校验通过，点击右侧保存即可"
                                } else {
                                    "请补全真实姓名、手机号及有效证件号码"
                                })
                                .size(12.2)
                                .color(if form_ready {
                                    palette.green
                                } else {
                                    palette.text_muted
                                }),
                            );
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            let save_label = if app.add_buyer_submitting {
                                "提交中..."
                            } else {
                                "保存购票人"
                            };
                            let can_click_save = !app.add_buyer_submitting;
                            let save_btn = egui::Button::new(
                                RichText::new(save_label)
                                    .size(14.0)
                                    .strong()
                                    .color(if can_click_save {
                                        Color32::WHITE
                                    } else {
                                        palette.text_muted
                                    }),
                            )
                            .min_size(Vec2::new(112.0, 34.0))
                            .fill(if form_ready && can_click_save {
                                palette.accent
                            } else if can_click_save {
                                palette.accent.linear_multiply(0.75)
                            } else {
                                palette.badge_bg
                            })
                            .rounding(8.0);

                            if ui.add_enabled(can_click_save, save_btn).clicked() {
                                submit_clicked = true;
                            }

                            let cancel_btn = egui::Button::new(
                                RichText::new("取消")
                                    .size(13.5)
                                    .color(palette.text_primary),
                            )
                            .fill(palette.item_bg)
                            .stroke(Stroke::new(1.0, palette.item_border))
                            .min_size(Vec2::new(72.0, 34.0))
                            .rounding(8.0);
                            if ui.add(cancel_btn).clicked() {
                                cancel_clicked = true;
                            }
                        });
                    });
                },
                |ui| {
                    let card_outer_w = (ui.available_width() - 6.0).max(320.0);
                    let card_inner_w = (card_outer_w - 32.0).max(280.0);

                    // 内联错误提示条
                    if let Some(ref err_msg) = app.add_buyer_error {
                        egui::Frame::none()
                            .fill(palette.error_bg)
                            .stroke(Stroke::new(1.0, palette.red))
                            .rounding(8.0)
                            .inner_margin(egui::Margin::symmetric(12.0, 9.0))
                            .show(ui, |ui| {
                                ui.set_width((card_outer_w - 26.0).max(260.0));
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 7.0;
                                    draw_inline_status_icon(
                                        ui,
                                        false,
                                        palette.red,
                                        Color32::WHITE,
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(err_msg)
                                                .color(palette.red)
                                                .size(13.0),
                                        )
                                        .wrap(true),
                                    );
                                });
                            });
                        ui.add_space(10.0);
                    }

                    ui.add_enabled_ui(editing_enabled, |ui| {
                        // 卡片 ①：实名身份信息
                        egui::Frame::none()
                            .fill(palette.card_bg)
                            .stroke(Stroke::new(1.0, palette.card_border))
                            .rounding(10.0)
                            .inner_margin(egui::Margin::symmetric(16.0, 14.0))
                            .show(ui, |ui| {
                                ui.set_width(card_inner_w);

                                // 第一组：真实姓名 + 联系手机号（宽屏双列并排且严格顶底齐平，窄屏自动优雅衔接）
                                let gap = 12.0;
                                if card_inner_w >= 390.0 {
                                    let left_w = ((card_inner_w - gap) * 0.43).floor();
                                    let right_w = (card_inner_w - gap - left_w).max(160.0);
                                    ui.horizontal_top(|ui| {
                                        ui.spacing_mut().item_spacing.x = gap;
                                        ui.allocate_ui_with_layout(
                                            Vec2::new(left_w, 0.0),
                                            egui::Layout::top_down(egui::Align::Min),
                                            |ui| {
                                                render_field_header(
                                                    ui,
                                                    "真实姓名",
                                                    left_w,
                                                    None,
                                                    &palette,
                                                );
                                                ui.add_space(5.0);
                                                styled_form_input(
                                                    ui,
                                                    "add_buyer_name_box",
                                                    &mut app.add_buyer_input.name,
                                                    "请输入证件上的姓名",
                                                    left_w,
                                                    false,
                                                    &palette,
                                                );
                                            },
                                        );
                                        ui.allocate_ui_with_layout(
                                            Vec2::new(right_w, 0.0),
                                            egui::Layout::top_down(egui::Align::Min),
                                            |ui| {
                                                let phone_len = app.add_buyer_input.phone.len();
                                                let phone_badge = if phone_len == 11 {
                                                    Some(("11 位已就绪", palette.green_bg, palette.green))
                                                } else if phone_len > 0 {
                                                    Some(("7-15 位纯数字", palette.badge_bg, palette.text_muted))
                                                } else {
                                                    None
                                                };
                                                render_field_header(
                                                    ui,
                                                    "联系手机号",
                                                    right_w,
                                                    phone_badge,
                                                    &palette,
                                                );
                                                ui.add_space(5.0);
                                                let phone_resp = styled_form_input(
                                                    ui,
                                                    "add_buyer_phone_box",
                                                    &mut app.add_buyer_input.phone,
                                                    "接收出票短信的手机号",
                                                    right_w,
                                                    false,
                                                    &palette,
                                                );
                                                if (phone_resp.changed() || phone_resp.lost_focus())
                                                    && (!ime_composing || phone_resp.lost_focus())
                                                {
                                                    app.add_buyer_input.phone =
                                                        normalize_fullwidth_ascii(
                                                            &app.add_buyer_input.phone,
                                                        )
                                                        .chars()
                                                        .filter(|c| c.is_ascii_digit())
                                                        .take(15)
                                                        .collect();
                                                }
                                            },
                                        );
                                    });
                                } else {
                                    render_field_header(ui, "真实姓名", card_inner_w, None, &palette);
                                    ui.add_space(5.0);
                                    styled_form_input(
                                        ui,
                                        "add_buyer_name_box_narrow",
                                        &mut app.add_buyer_input.name,
                                        "请输入证件上的真实姓名",
                                        card_inner_w,
                                        false,
                                        &palette,
                                    );
                                    ui.add_space(10.0);
                                    render_field_header(ui, "联系手机号", card_inner_w, None, &palette);
                                    ui.add_space(5.0);
                                    let phone_resp = styled_form_input(
                                        ui,
                                        "add_buyer_phone_box_narrow",
                                        &mut app.add_buyer_input.phone,
                                        "请输入手机号（仅数字）",
                                        card_inner_w,
                                        false,
                                        &palette,
                                    );
                                    if (phone_resp.changed() || phone_resp.lost_focus())
                                        && (!ime_composing || phone_resp.lost_focus())
                                    {
                                        app.add_buyer_input.phone =
                                            normalize_fullwidth_ascii(&app.add_buyer_input.phone)
                                                .chars()
                                                .filter(|c| c.is_ascii_digit())
                                                .take(15)
                                                .collect();
                                    }
                                }

                                ui.add_space(14.0);

                                // 第二组：分段胶囊证件类型选择
                                render_field_header(ui, "证件类型", card_inner_w, None, &palette);
                                ui.add_space(6.0);
                                id_type_segmented_selector(
                                    ui,
                                    &mut app.add_buyer_input.id_type,
                                    card_inner_w,
                                    &palette,
                                );

                                ui.add_space(14.0);

                                // 第三组：证件号码（带实时位数与有效性提示）
                                let id_len = app.add_buyer_input.id_number.trim().len();
                                let id_badge = if app.add_buyer_input.id_type == 0 {
                                    if id_len == 18 || id_len == 15 {
                                        let ok = validate_add_buyer_input(
                                            "测试",
                                            "13800138000",
                                            0,
                                            &app.add_buyer_input.id_number,
                                        )
                                        .is_ok();
                                        if ok {
                                            Some((
                                                if id_len == 18 {
                                                    "18 位格式正确"
                                                } else {
                                                    "15 位格式正确"
                                                },
                                                palette.green_bg,
                                                palette.green,
                                            ))
                                        } else {
                                            Some(("身份证号格式有误", palette.amber_bg, palette.amber))
                                        }
                                    } else if id_len > 0 {
                                        Some(("需满 18 位（末位 X 自动大写）", palette.badge_bg, palette.text_muted))
                                    } else {
                                        None
                                    }
                                } else if id_len > 0 {
                                    Some(("已填写", palette.green_bg, palette.green))
                                } else {
                                    None
                                };
                                render_field_header(ui, "证件号码", card_inner_w, id_badge, &palette);
                                ui.add_space(5.0);
                                let id_resp = styled_form_input(
                                    ui,
                                    "add_buyer_id_number_box",
                                    &mut app.add_buyer_input.id_number,
                                    get_id_hint(app.add_buyer_input.id_type),
                                    card_inner_w,
                                    true,
                                    &palette,
                                );
                                if (id_resp.changed() || id_resp.lost_focus())
                                    && (!ime_composing || id_resp.lost_focus())
                                {
                                    let mut cleaned: String =
                                        normalize_fullwidth_ascii(&app.add_buyer_input.id_number)
                                            .chars()
                                            .filter(|c| {
                                                c.is_ascii_alphanumeric()
                                                    || matches!(*c, '(' | ')' | '-')
                                            })
                                            .take(24)
                                            .collect();
                                    if app.add_buyer_input.id_type == 0 {
                                        cleaned = cleaned.to_ascii_uppercase();
                                    }
                                    app.add_buyer_input.id_number = cleaned;
                                }
                            });

                        ui.add_space(10.0);

                        // 卡片 ②：默认偏好快捷开关
                        egui::Frame::none()
                            .fill(palette.card_bg)
                            .stroke(Stroke::new(1.0, palette.card_border))
                            .rounding(10.0)
                            .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                            .show(ui, |ui| {
                                ui.set_width(card_inner_w);
                                render_preference_toggle_row(
                                    ui,
                                    "add_buyer_auto_app_default",
                                    "加入本软件默认自动勾选名单",
                                    if current_default_count >= 2 {
                                        "当前账号已满 2 位默认购票人，可在「设置」中调整主次"
                                    } else {
                                        "保存成功后自动设为本账号默认购票人，打开选人页直接勾选"
                                    },
                                    &mut app.add_buyer_input.also_app_default,
                                    current_default_count < 2,
                                    card_inner_w,
                                    &palette,
                                );

                                ui.add_space(8.0);
                                ui.separator();
                                ui.add_space(8.0);

                                render_preference_toggle_row(
                                    ui,
                                    "add_buyer_bili_default",
                                    "同步设为 B 站会员购默认购票人",
                                    "在哔哩哔哩 App 会员购侧同步标记为默认观演人",
                                    &mut app.add_buyer_input.as_default_buyer,
                                    true,
                                    card_inner_w,
                                    &palette,
                                );
                            });
                    });
                },
            );
        });

    if submit_clicked {
        match validation_preview {
            Ok((name, phone, id_number)) => {
                app.add_buyer_error = None;
                app.add_buyer_submitting = true;
                if app.add_buyer_input.also_app_default && current_default_count < 2 {
                    app.pending_auto_default_buyer = Some((account_uid, name.clone()));
                } else {
                    app.pending_auto_default_buyer = None;
                }
                ctx.memory_mut(|mem| mem.data.remove::<i64>(egui::Id::new(PREFILL_MEM_KEY)));

                log::info!("正在为账号 {} 提交添加购票人请求...", account_uid);
                let req = TaskRequest::AddBuyerRequest(AddBuyerRequest {
                    uid: account_uid,
                    cookie_manager: select_cookie_manager,
                    name,
                    phone,
                    id_type: app.add_buyer_input.id_type as i32,
                    id_number,
                    as_default_buyer: app.add_buyer_input.as_default_buyer,
                });
                match app.task_manager.submit_task(req) {
                    Ok(task_id) => {
                        app.add_buyer_active_task_id = Some(task_id);
                    }
                    Err(e) => {
                        app.add_buyer_active_task_id = None;
                        app.add_buyer_submitting = false;
                        app.pending_auto_default_buyer = None;
                        app.add_buyer_error = Some(format!("提交添加购票人任务失败: {}", e));
                        log::error!("提交添加购票人任务失败: {}", e);
                    }
                }
            }
            Err(validation_err) => {
                app.add_buyer_error = Some(validation_err);
            }
        }
    }

    if !window_open || cancel_clicked {
        reset_add_buyer_window(app, ctx);
    }
}

fn render_field_header(
    ui: &mut egui::Ui,
    label: &str,
    width: f32,
    badge: Option<(&str, Color32, Color32)>,
    palette: &ConfirmPalette,
) {
    // 严格锁定 22px 固定行高，确保无论右侧是否带有状态胶囊，双列输入框顶边均 0px 对齐
    let (row_rect, _) = ui.allocate_exact_size(Vec2::new(width, 22.0), egui::Sense::hover());
    if !ui.is_rect_visible(row_rect) {
        return;
    }
    let painter = ui.painter_at(row_rect.intersect(ui.clip_rect()));
    let label_galley = painter.layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(13.2),
        palette.text_secondary,
    );
    painter.galley(
        egui::pos2(row_rect.left(), row_rect.center().y - label_galley.size().y * 0.5),
        label_galley,
    );

    if let Some((txt, bg, fg)) = badge {
        let badge_galley = painter.layout_no_wrap(
            txt.to_string(),
            egui::FontId::proportional(11.2),
            fg,
        );
        let pill_w = badge_galley.size().x + 14.0;
        let pill_h = 19.0;
        let pill_rect = egui::Rect::from_min_size(
            egui::pos2(row_rect.right() - pill_w, row_rect.center().y - pill_h * 0.5),
            Vec2::new(pill_w, pill_h),
        );
        painter.rect_filled(pill_rect, 5.5, bg);
        painter.galley(
            egui::pos2(
                pill_rect.center().x - badge_galley.size().x * 0.5,
                pill_rect.center().y - badge_galley.size().y * 0.5,
            ),
            badge_galley,
        );
    }
}

fn styled_form_input(
    ui: &mut egui::Ui,
    id_salt: &str,
    value: &mut String,
    hint: &str,
    width: f32,
    monospace: bool,
    palette: &ConfirmPalette,
) -> egui::Response {
    let edit_id = ui.id().with(id_salt);
    let is_focused = ui.ctx().memory(|m| m.has_focus(edit_id));
    let border = if is_focused {
        Stroke::new(1.5, palette.accent)
    } else {
        Stroke::new(1.0, palette.item_border)
    };

    // 固定 36px 外框高度并在内部垂直居中单行输入框，杜绝不同字体行高导致左右框体高低不一
    let box_h = 36.0;
    let (box_rect, _) = ui.allocate_exact_size(Vec2::new(width, box_h), egui::Sense::hover());
    ui.painter().rect(box_rect, 7.0, palette.item_bg, border);

    let inner_rect = box_rect.shrink2(Vec2::new(10.0, 6.0));
    let mut inner_ui = ui.child_ui(
        inner_rect,
        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
    );
    inner_ui.set_clip_rect(inner_rect.intersect(ui.clip_rect()));

    let font = if monospace {
        egui::FontId::new(14.5, egui::FontFamily::Monospace)
    } else {
        egui::FontId::new(14.5, egui::FontFamily::Proportional)
    };
    inner_ui.add(
        egui::TextEdit::singleline(value)
            .id(edit_id)
            .hint_text(hint)
            .font(font)
            .text_color(palette.text_primary)
            .desired_width(inner_rect.width())
            .frame(false),
    )
}

#[allow(clippy::too_many_arguments)]
fn render_preference_toggle_row(
    ui: &mut egui::Ui,
    id_salt: &str,
    title: &str,
    subtitle: &str,
    on: &mut bool,
    enabled: bool,
    row_w: f32,
    palette: &ConfirmPalette,
) {
    let effective_on = *on && enabled;
    ui.horizontal(|ui| {
        let text_w = (row_w - 58.0).max(140.0);
        ui.allocate_ui_with_layout(
            Vec2::new(text_w, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(
                    RichText::new(title)
                        .size(13.8)
                        .strong()
                        .color(if enabled {
                            palette.text_primary
                        } else {
                            palette.text_muted
                        }),
                );
                ui.add(
                    egui::Label::new(
                        RichText::new(subtitle)
                            .size(11.8)
                            .color(palette.text_muted),
                    )
                    .wrap(true),
                );
            },
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let sw_w = 44.0;
            let sw_h = 24.0;
            let sense = if enabled {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            };
            let sw_id = ui.id().with(id_salt);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(sw_w, sw_h), egui::Sense::hover());
            let resp = ui.interact(rect, sw_id, sense);
            if enabled {
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    *on = !*on;
                }
            }
            let anim = ui.ctx().animate_bool(sw_id, effective_on);
            let radius = sw_h * 0.5;
            let knob_x = rect.left() + radius + anim * (sw_w - sw_h);
            let track_fill = if effective_on {
                palette.accent
            } else {
                palette.badge_bg
            };
            ui.painter().rect_filled(rect.expand(-1.0), radius, track_fill);
            ui.painter().circle_filled(
                egui::pos2(knob_x, rect.center().y),
                radius - 3.5,
                if enabled {
                    Color32::WHITE
                } else {
                    palette.text_muted
                },
            );
        });
    });
}

fn reset_add_buyer_window(app: &mut Myapp, ctx: &egui::Context) {
    app.show_add_buyer_window = None;
    app.add_buyer_input = AddBuyerInput::default();
    app.add_buyer_active_task_id = None;
    app.add_buyer_error = None;
    app.add_buyer_submitting = false;
    ctx.memory_mut(|mem| mem.data.remove::<i64>(egui::Id::new(PREFILL_MEM_KEY)));
}

pub(crate) fn validate_add_buyer_input(
    name: &str,
    phone: &str,
    id_type: usize,
    id_number: &str,
) -> Result<(String, String, String), String> {
    let clean_name = name.trim();
    if clean_name.is_empty() {
        return Err("请输入购票人真实姓名".to_string());
    }

    let normalized_phone = normalize_fullwidth_ascii(phone);
    let clean_phone = normalized_phone.trim();
    if clean_phone.is_empty()
        || !clean_phone.bytes().all(|b| b.is_ascii_digit())
        || !(7..=15).contains(&clean_phone.len())
    {
        return Err("请输入有效的手机号（7-15 位纯数字）".to_string());
    }

    let normalized_id = normalize_fullwidth_ascii(id_number);
    let clean_id = normalized_id.trim();
    if clean_id.is_empty() {
        return Err("请输入证件号码".to_string());
    }

    if !clean_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '(' | ')' | '-'))
    {
        return Err("证件号码包含非法字符".to_string());
    }

    if id_type == 0 {
        let upper = clean_id.to_ascii_uppercase();
        let bytes = upper.as_bytes();
        let valid_id_card = match bytes.len() {
            15 => bytes.iter().all(|b| b.is_ascii_digit()),
            18 => {
                bytes[..17].iter().all(|b| b.is_ascii_digit())
                    && (bytes[17].is_ascii_digit() || bytes[17] == b'X')
            }
            _ => false,
        };
        if !valid_id_card {
            return Err("居民身份证号码须为 15 位或 18 位（18位末位可为 X）".to_string());
        }
        return Ok((clean_name.to_string(), clean_phone.to_string(), upper));
    }

    Ok((
        clean_name.to_string(),
        clean_phone.to_string(),
        clean_id.to_string(),
    ))
}

const ID_TYPES: [(&str, usize); 4] = [
    ("居民身份证", 0),
    ("护照", 1),
    ("港澳通行证", 2),
    ("台湾通行证", 3),
];

fn id_type_segmented_selector(
    ui: &mut egui::Ui,
    selected_type: &mut usize,
    total_w: f32,
    palette: &ConfirmPalette,
) {
    let gap = 8.0;
    let count = ID_TYPES.len() as f32;
    let pill_w = ((total_w - gap * (count - 1.0)) / count).max(76.0);

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(gap, 6.0);
        for (name, value) in ID_TYPES.iter() {
            let is_selected = *selected_type == *value;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(pill_w, 32.0), egui::Sense::click());
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() {
                *selected_type = *value;
            }
            if ui.is_rect_visible(rect) {
                let fill = if is_selected {
                    palette.accent
                } else if resp.hovered() {
                    palette.item_hover_bg
                } else {
                    palette.item_bg
                };
                let stroke = if is_selected {
                    Stroke::new(1.2, palette.accent)
                } else if resp.hovered() {
                    Stroke::new(1.0, palette.accent.linear_multiply(0.65))
                } else {
                    Stroke::new(1.0, palette.item_border)
                };
                let fg = if is_selected {
                    Color32::WHITE
                } else {
                    palette.text_primary
                };
                ui.painter().rect(rect, 7.0, fill, stroke);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    *name,
                    egui::FontId::proportional(13.2),
                    fg,
                );
            }
        }
    });
}

fn get_id_hint(id_type: usize) -> &'static str {
    match id_type {
        0 => "请输入 15 位或 18 位身份证号码",
        1 => "请输入护照号码",
        2 => "请输入港澳居民来往内地通行证号码（支持括号）",
        3 => "请输入台湾居民来往大陆通行证号码",
        _ => "请输入证件号码",
    }
}

#[cfg(test)]
mod tests {
    use super::{render_field_header, styled_form_input, validate_add_buyer_input};
    use crate::windows::confirm_ticket::ConfirmPalette;
    use eframe::egui::{self, Vec2};

    #[test]
    fn validates_id_card_and_permits_with_parentheses() {
        assert!(validate_add_buyer_input("张三", "13800138000", 0, "11010119900101123x").is_ok());
        assert!(validate_add_buyer_input("张三", "13800138000", 0, "12345").is_err());
        assert!(validate_add_buyer_input("李四", "13800138000", 2, "H12345678(0)").is_ok());
        assert!(validate_add_buyer_input("   ", "13800138000", 1, "E12345678").is_err());
        assert!(validate_add_buyer_input("张三", "123", 1, "E12345678").is_err());
    }

    #[test]
    fn name_and_phone_fields_are_strictly_level_with_and_without_badge() {
        let ctx = egui::Context::default();
        let mut name = "张三".to_string();
        let mut phone = "13800138000".to_string();

        for with_badge in [false, true] {
            let mut left_box = egui::Rect::NOTHING;
            let mut right_box = egui::Rect::NOTHING;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    let palette = ConfirmPalette::from_dark_mode(false);
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let card_inner_w = 500.0_f32;
                        let gap = 12.0_f32;
                        let left_w = ((card_inner_w - gap) * 0.43).floor();
                        let right_w = card_inner_w - gap - left_w;
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            ui.allocate_ui_with_layout(
                                Vec2::new(left_w, 0.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    render_field_header(ui, "真实姓名", left_w, None, &palette);
                                    ui.add_space(5.0);
                                    left_box = styled_form_input(
                                        ui,
                                        "test_name",
                                        &mut name,
                                        "姓名",
                                        left_w,
                                        false,
                                        &palette,
                                    )
                                    .rect;
                                },
                            );
                            ui.allocate_ui_with_layout(
                                Vec2::new(right_w, 0.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    let badge = if with_badge {
                                        Some(("11 位已就绪", palette.green_bg, palette.green))
                                    } else {
                                        None
                                    };
                                    render_field_header(ui, "联系手机号", right_w, badge, &palette);
                                    ui.add_space(5.0);
                                    right_box = styled_form_input(
                                        ui,
                                        "test_phone",
                                        &mut phone,
                                        "手机号",
                                        right_w,
                                        false,
                                        &palette,
                                    )
                                    .rect;
                                },
                            );
                        });
                    });
                },
            );

            assert!(
                (left_box.top() - right_box.top()).abs() < 0.1
                    && (left_box.bottom() - right_box.bottom()).abs() < 0.1,
                "name and phone boxes not level (with_badge={}): left={:?}, right={:?}",
                with_badge,
                left_box,
                right_box
            );
        }
    }
}
