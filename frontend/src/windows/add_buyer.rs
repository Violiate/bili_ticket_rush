use common::taskmanager::{AddBuyerRequest, TaskRequest};
use eframe::egui::{self, Color32, RichText, Stroke};

use crate::app::{normalize_fullwidth_ascii, Myapp};
use crate::windows::{bounded_window_sizes, show_resizable_body};

#[derive(Default, Clone, Debug)]
pub struct AddBuyerInput {
    pub name: String,
    pub phone: String,
    pub id_type: usize,
    pub id_number: String,
    pub as_default_buyer: bool,
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

    // 首次打开（表单为空时）自动预填设置中的默认手机号
    let prefilled_uid =
        ctx.memory(|mem| mem.data.get_temp::<i64>(egui::Id::new(PREFILL_MEM_KEY)));
    if prefilled_uid != Some(account_uid)
        && app.add_buyer_input.name.is_empty()
        && app.add_buyer_input.phone.is_empty()
        && app.add_buyer_input.id_number.is_empty()
    {
        if let Some(preinput) = app.get_preinput_phone_for_uid(account_uid) {
            app.add_buyer_input.phone = preinput
                .chars()
                .filter(|c| c.is_ascii_digit())
                .take(15)
                .collect();
        }
        ctx.memory_mut(|mem| {
            mem.data
                .insert_temp(egui::Id::new(PREFILL_MEM_KEY), account_uid);
        });
    }

    let mut window_open = app.show_add_buyer_window.is_some();
    let mut submit_clicked = false;
    let mut cancel_clicked = false;

    let (default_sz, min_sz, max_sz) = bounded_window_sizes(ctx, 560.0, 430.0, 440.0, 320.0);

    egui::Window::new("添加购票人")
        .open(&mut window_open)
        .default_size(default_sz)
        .min_width(min_sz[0])
        .min_height(min_sz[1])
        .resize(|r| r.max_size(max_sz))
        .constrain(true)
        .resizable(true)
        .show(ctx, |ui| {
            let editing_enabled = !app.add_buyer_submitting;
            let ime_composing = app.is_ime_composing();

            show_resizable_body(
                ui,
                "add_buyer_scroll",
                |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("添加实名购票人");
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(format!(
                                "目标账号: {} (UID: {})",
                                account_name, account_uid
                            ))
                            .color(ui.visuals().weak_text_color()),
                        );
                    });
                    ui.separator();
                    ui.add_space(4.0);
                },
                |ui| {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if app.add_buyer_submitting {
                            ui.spinner();
                            ui.label("正在提交购票人信息...");
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let save_label = if app.add_buyer_submitting {
                                "提交中..."
                            } else {
                                "保存"
                            };
                            let save_btn = egui::Button::new(
                                RichText::new(save_label)
                                    .size(15.0)
                                    .color(Color32::WHITE),
                            )
                            .min_size(egui::vec2(108.0, 36.0))
                            .fill(Color32::from_rgb(59, 130, 246))
                            .rounding(8.0);

                            if ui.add_enabled(!app.add_buyer_submitting, save_btn).clicked() {
                                submit_clicked = true;
                            }

                            if ui.button("取消").clicked() {
                                cancel_clicked = true;
                            }
                        });
                    });
                },
                |ui| {
                    // 内联错误提示条
                    if let Some(ref err_msg) = app.add_buyer_error {
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
                                        RichText::new(err_msg)
                                            .color(Color32::from_rgb(220, 68, 55))
                                            .size(13.5),
                                    )
                                    .wrap(true),
                                );
                            });
                        ui.add_space(8.0);
                    }

                    let input_w = (ui.available_width() - 120.0).clamp(180.0, 360.0);
                    ui.add_enabled_ui(editing_enabled, |ui| {
                        egui::Grid::new("add_buyer_form_grid")
                            .num_columns(2)
                            .spacing([12.0, 14.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("姓名:").size(15.0));
                                ui.add(
                                    egui::TextEdit::singleline(&mut app.add_buyer_input.name)
                                        .hint_text("请输入真实姓名")
                                        .desired_width(input_w),
                                );
                                ui.end_row();

                                ui.label(RichText::new("手机号:").size(15.0));
                                let phone_resp = ui.add(
                                    egui::TextEdit::singleline(&mut app.add_buyer_input.phone)
                                        .hint_text("请输入手机号（仅数字）")
                                        .desired_width(input_w),
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
                                ui.end_row();

                                ui.label(RichText::new("证件类型:").size(15.0));
                                id_type_selector(ui, &mut app.add_buyer_input.id_type);
                                ui.end_row();

                                ui.label(RichText::new("证件号码:").size(15.0));
                                let id_resp = ui.add(
                                    egui::TextEdit::singleline(&mut app.add_buyer_input.id_number)
                                        .hint_text(get_id_hint(app.add_buyer_input.id_type))
                                        .desired_width(input_w),
                                );
                                if (id_resp.changed() || id_resp.lost_focus())
                                    && (!ime_composing || id_resp.lost_focus())
                                {
                                    app.add_buyer_input.id_number =
                                        normalize_fullwidth_ascii(&app.add_buyer_input.id_number)
                                            .chars()
                                            .filter(|c| {
                                                c.is_ascii_alphanumeric()
                                                    || matches!(*c, '(' | ')' | '-')
                                            })
                                            .collect();
                                }
                                ui.end_row();

                                ui.label("");
                                ui.checkbox(
                                    &mut app.add_buyer_input.as_default_buyer,
                                    "设为默认购票人",
                                );
                                ui.end_row();
                            });
                    });
                },
            );
        });

    if submit_clicked {
        match validate_add_buyer_input(
            &app.add_buyer_input.name,
            &app.add_buyer_input.phone,
            app.add_buyer_input.id_type,
            &app.add_buyer_input.id_number,
        ) {
            Ok((name, phone, id_number)) => {
                app.add_buyer_error = None;
                app.add_buyer_submitting = true;
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
    ("身份证", 0),
    ("护照", 1),
    ("港澳居民往来内地通行证", 2),
    ("台湾居民往来大陆通行证", 3),
];

fn id_type_selector(ui: &mut egui::Ui, selected_type: &mut usize) {
    let dark_mode = ui.visuals().dark_mode;
    ui.horizontal_wrapped(|ui| {
        for (name, value) in ID_TYPES.iter() {
            let is_selected = *selected_type == *value;
            let (fill, text_color) = if is_selected {
                (Color32::from_rgb(59, 130, 246), Color32::WHITE)
            } else if dark_mode {
                (Color32::from_rgb(44, 48, 56), ui.visuals().text_color())
            } else {
                (Color32::from_rgb(238, 240, 245), ui.visuals().text_color())
            };

            let button = egui::Button::new(RichText::new(*name).size(13.5).color(text_color))
                .min_size(egui::vec2(0.0, 28.0))
                .fill(fill)
                .rounding(6.0);

            if ui.add(button).clicked() {
                *selected_type = *value;
            }
        }
    });
}

fn get_id_hint(id_type: usize) -> &'static str {
    match id_type {
        0 => "请输入 15 位或 18 位身份证号码",
        1 => "请输入护照号码",
        2 => "请输入港澳通行证号码（支持括号）",
        3 => "请输入台湾通行证号码",
        _ => "请输入证件号码",
    }
}

#[cfg(test)]
mod tests {
    use super::validate_add_buyer_input;

    #[test]
    fn validates_id_card_and_permits_with_parentheses() {
        assert!(validate_add_buyer_input("张三", "13800138000", 0, "11010119900101123x").is_ok());
        assert!(validate_add_buyer_input("张三", "13800138000", 0, "12345").is_err());
        assert!(validate_add_buyer_input("李四", "13800138000", 2, "H12345678(0)").is_ok());
        assert!(validate_add_buyer_input("   ", "13800138000", 1, "E12345678").is_err());
        assert!(validate_add_buyer_input("张三", "123", 1, "E12345678").is_err());
    }
}
