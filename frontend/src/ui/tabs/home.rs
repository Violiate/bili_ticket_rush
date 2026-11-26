use eframe::egui;

use crate::app::Myapp;
use common::taskmanager::TaskManager_debug;
use common::ticket::BilibiliTicket;

struct HomePalette {
    card_fill: egui::Color32,
    card_border: egui::Color32,
    input_fill: egui::Color32,
    input_border: egui::Color32,
    text_primary: egui::Color32,
    text_secondary: egui::Color32,
    mode_unselected_fill: egui::Color32,
    mode_unselected_text: egui::Color32,
    mode_unselected_border: egui::Color32,
    accent: egui::Color32,
    accent_border: egui::Color32,
    notice_fill: egui::Color32,
    notice_border: egui::Color32,
    notice_text: egui::Color32,
    warn_text: egui::Color32,
}

impl HomePalette {
    fn from_dark_mode(dark_mode: bool) -> Self {
        if dark_mode {
            Self {
                card_fill: egui::Color32::from_rgb(36, 38, 46),
                card_border: egui::Color32::from_rgb(62, 66, 80),
                input_fill: egui::Color32::from_rgb(26, 28, 35),
                input_border: egui::Color32::from_rgb(76, 82, 100),
                text_primary: egui::Color32::from_rgb(235, 238, 245),
                text_secondary: egui::Color32::from_rgb(168, 174, 190),
                mode_unselected_fill: egui::Color32::from_rgb(48, 52, 64),
                mode_unselected_text: egui::Color32::from_rgb(218, 222, 232),
                mode_unselected_border: egui::Color32::from_rgb(76, 82, 98),
                accent: egui::Color32::from_rgb(45, 136, 245),
                accent_border: egui::Color32::from_rgb(28, 108, 212),
                notice_fill: egui::Color32::from_rgb(56, 42, 30),
                notice_border: egui::Color32::from_rgb(160, 102, 48),
                notice_text: egui::Color32::from_rgb(255, 186, 118),
                warn_text: egui::Color32::from_rgb(255, 138, 128),
            }
        } else {
            Self {
                card_fill: egui::Color32::from_rgb(248, 249, 252),
                card_border: egui::Color32::from_rgb(212, 218, 230),
                input_fill: egui::Color32::from_rgb(255, 255, 255),
                input_border: egui::Color32::from_rgb(190, 198, 214),
                text_primary: egui::Color32::from_rgb(30, 35, 45),
                text_secondary: egui::Color32::from_rgb(100, 108, 124),
                mode_unselected_fill: egui::Color32::from_rgb(234, 237, 244),
                mode_unselected_text: egui::Color32::from_rgb(48, 54, 68),
                mode_unselected_border: egui::Color32::from_rgb(194, 200, 216),
                accent: egui::Color32::from_rgb(45, 136, 245),
                accent_border: egui::Color32::from_rgb(25, 106, 208),
                notice_fill: egui::Color32::from_rgb(255, 247, 235),
                notice_border: egui::Color32::from_rgb(242, 184, 120),
                notice_text: egui::Color32::from_rgb(196, 86, 18),
                warn_text: egui::Color32::from_rgb(212, 56, 48),
            }
        }
    }
}

pub fn render(app: &mut Myapp, ui: &mut egui::Ui) {
    let dark_mode = ui.ctx().style().visuals.dark_mode;
    let palette = HomePalette::from_dark_mode(dark_mode);

    // 清理失效的已选账号并默认选中首个已开启抢票的账号
    sync_selected_account(app);

    egui::ScrollArea::vertical()
        .id_source("home_tab_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let card_width = (ui.available_width() - 40.0).clamp(380.0, 560.0);

            ui.vertical_centered(|ui| {
                ui.add_space(18.0);
                ui.heading(
                    egui::RichText::new("仅供学习的小工具")
                        .size(30.0)
                        .strong()
                        .color(palette.text_primary),
                );
                ui.add_space(8.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(TaskManager_debug())
                            .size(13.5)
                            .color(palette.notice_text)
                            .strong(),
                    )
                    .wrap(true),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("请输入项目 ID 或粘贴票务链接，按回车或点击开始抢票")
                        .size(14.5)
                        .color(palette.text_secondary),
                );

                if let Some(announce) = app
                    .announce1
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                {
                    ui.add_space(10.0);
                    centered_block(ui, card_width, |ui, inner_w| {
                        egui::Frame::none()
                            .fill(palette.notice_fill)
                            .rounding(8.0)
                            .stroke(egui::Stroke::new(1.0, palette.notice_border))
                            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                            .show(ui, |ui| {
                                ui.set_width((inner_w - 24.0).max(200.0));
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(announce)
                                            .size(13.5)
                                            .color(palette.notice_text)
                                            .strong(),
                                    )
                                    .wrap(true),
                                );
                            });
                    });
                }

                ui.add_space(18.0);

                // 输入区域、账号和模式卡片、开始按钮
                ticket_input_area(ui, app, card_width, &palette);

                ui.add_space(24.0);
            });
        });
}

fn sync_selected_account(app: &mut Myapp) {
    if let Some(uid) = app.selected_account_uid {
        if !app.account_manager.accounts.iter().any(|a| a.uid == uid) {
            app.selected_account_uid = None;
        }
    }

    if app.selected_account_uid.is_none() && !app.account_manager.accounts.is_empty() {
        app.selected_account_uid = app
            .account_manager
            .accounts
            .iter()
            .find(|a| a.is_active)
            .or_else(|| app.account_manager.accounts.first())
            .map(|a| a.uid);
    }
}

fn centered_block<R>(
    ui: &mut egui::Ui,
    block_width: f32,
    add_contents: impl FnOnce(&mut egui::Ui, f32) -> R,
) -> R {
    let available = ui.available_width();
    let effective_width = block_width.min(available).max(240.0);
    let left_pad = ((available - effective_width) * 0.5).max(0.0);
    ui.horizontal(|ui| {
        ui.add_space(left_pad);
        add_contents(ui, effective_width)
    })
    .inner
}

fn ticket_input_area(
    ui: &mut egui::Ui,
    app: &mut Myapp,
    card_width: f32,
    palette: &HomePalette,
) {
    let input_response = styled_ticket_input(ui, &mut app.ticket_id, card_width, palette);
    let enter_pressed = input_response.lost_focus()
        && !app.is_ime_composing()
        && ui.input(|i| i.key_pressed(egui::Key::Enter));

    ui.add_space(14.0);
    styled_selection_area(ui, app, card_width, palette);
    ui.add_space(18.0);

    let grab_clicked = styled_grab_button(ui, palette).clicked();

    if enter_pressed || grab_clicked {
        trigger_start_grab(app);
    }
}

fn trigger_start_grab(app: &mut Myapp) {
    let raw_input = app.ticket_id.clone();
    let project_id = match parse_project_id(&raw_input) {
        Ok(id) => id,
        Err(err) => {
            log::warn!("项目 ID / 链接校验失败: {}", err);
            app.show_error(err);
            return;
        }
    };
    let referer = build_referer(&raw_input, &project_id);

    if app.account_manager.accounts.is_empty() {
        log::info!("没有可用账号，请先登录账号");
        app.show_info("没有可用账号，请先登录账号");
        app.show_login_windows = true;
        return;
    }

    let Some(select_uid) = app.selected_account_uid else {
        app.show_error("请先选择一个抢票账号");
        return;
    };

    let Some(selected_account) = app
        .account_manager
        .accounts
        .iter()
        .find(|a| a.uid == select_uid)
        .cloned()
    else {
        app.selected_account_uid = None;
        app.show_error(format!("未找到 UID 为 {} 的账号，请重新选择", select_uid));
        return;
    };

    if !selected_account.is_active {
        app.show_error("该账号已关闭抢票开关，请先在「账号管理」中开启");
        return;
    }

    if selected_account.cookie_manager.is_none() {
        if app.initializing_account_uids.contains(&select_uid) {
            app.show_info("账号客户端正在初始化，请稍候再试");
        } else {
            app.show_error("账号客户端未初始化，请重新登录");
        }
        return;
    }

    // 重置上一次选票与购票人状态
    app.project_info_requesting = false;
    app.project_info_active_task_id = None;
    app.project_info_error = None;
    app.project_info_last_request_time = None;
    app.buyer_info_requesting = false;
    app.buyer_info_active_task_id = None;
    app.buyer_info_error = None;
    app.buyer_info_last_request_time = None;
    app.selected_screen_index = Some(0);
    app.selected_screen_id = None;
    app.selected_ticket_id = None;
    app.reset_buyer_selection();

    let bilibili_ticket = BilibiliTicket::new(
        &app.grab_mode,
        &app.default_ua,
        &app.custom_config,
        &selected_account,
        &app.push_config,
        &app.status_delay,
        &project_id,
        &referer,
    );
    app.upsert_ticket_draft(bilibili_ticket);
    log::debug!("已更新账号 {} 的抢票草稿 (项目 {})", select_uid, project_id);

    match app.grab_mode {
        0 | 1 => {
            app.show_screen_info = Some(selected_account.uid);
            app.confirm_ticket_info = None;
        }
        2 => {
            app.confirm_ticket_info = Some(selected_account.uid.to_string());
            app.show_screen_info = None;
        }
        other => {
            log::error!("当前抢票模式不受支持: {}", other);
            app.show_error("当前抢票模式不受支持，请重新选择");
        }
    }
}

fn styled_ticket_input(
    ui: &mut egui::Ui,
    text: &mut String,
    card_width: f32,
    palette: &HomePalette,
) -> egui::Response {
    centered_block(ui, card_width, |ui, effective_w| {
        egui::Frame::none()
            .fill(palette.input_fill)
            .rounding(10.0)
            .stroke(egui::Stroke::new(1.0, palette.input_border))
            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.set_width((effective_w - 24.0).max(200.0));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
                    ui.label(egui::RichText::new("🎫").size(18.0));

                    let input_w = (ui.available_width() - 4.0).max(160.0);
                    let input = egui::TextEdit::singleline(text)
                        .hint_text("输入项目 ID（如 100300）或粘贴完整票务链接")
                        .desired_width(input_w)
                        .text_color(palette.text_primary)
                        .font(egui::FontId::new(16.0, egui::FontFamily::Proportional))
                        .margin(egui::vec2(4.0, 6.0))
                        .frame(false);

                    ui.add(input)
                })
                .inner
            })
            .inner
    })
}

fn styled_selection_area(
    ui: &mut egui::Ui,
    app: &mut Myapp,
    card_width: f32,
    palette: &HomePalette,
) {
    centered_block(ui, card_width, |ui, effective_w| {
        egui::Frame::none()
            .fill(palette.card_fill)
            .rounding(10.0)
            .stroke(egui::Stroke::new(1.0, palette.card_border))
            .inner_margin(egui::Margin::symmetric(16.0, 14.0))
            .show(ui, |ui| {
                let inner_w = (effective_w - 32.0).max(200.0);
                ui.set_width(inner_w);

                ui.vertical(|ui| {
                    account_selection(ui, app, inner_w, palette);

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(12.0);

                    grab_mode_selection(ui, app, inner_w, palette);
                });
            });
    });
}

fn account_selection(
    ui: &mut egui::Ui,
    app: &mut Myapp,
    inner_w: f32,
    palette: &HomePalette,
) {
    ui.vertical(|ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
            ui.label(
                egui::RichText::new("选择账号：")
                    .color(palette.text_primary)
                    .size(15.0)
                    .strong(),
            );

            if app.account_manager.accounts.is_empty() {
                ui.label(
                    egui::RichText::new("未登录账号")
                        .color(palette.warn_text)
                        .size(14.0),
                );
                let login_btn = egui::Button::new(
                    egui::RichText::new("去登录")
                        .size(13.5)
                        .color(egui::Color32::WHITE),
                )
                .fill(palette.accent)
                .rounding(6.0)
                .min_size(egui::vec2(68.0, 26.0));
                if ui.add(login_btn).clicked() {
                    app.show_login_windows = true;
                }
            } else {
                let selected_account = app
                    .account_manager
                    .accounts
                    .iter()
                    .find(|a| Some(a.uid) == app.selected_account_uid)
                    .cloned();

                let selected_text = match &selected_account {
                    Some(account) => format_account_option(account.name.as_str(), account.uid, account.is_active),
                    None => "请选择账号".to_string(),
                };

                let combo_width = (inner_w - 110.0).clamp(180.0, 300.0);
                egui::ComboBox::from_id_source("account_selector")
                    .selected_text(selected_text)
                    .width(combo_width)
                    .show_ui(ui, |ui| {
                        for account in &app.account_manager.accounts {
                            let text = format_account_option(
                                account.name.as_str(),
                                account.uid,
                                account.is_active,
                            );
                            let is_selected = Some(account.uid) == app.selected_account_uid;
                            if ui.selectable_label(is_selected, text).clicked() {
                                app.selected_account_uid = Some(account.uid);
                            }
                        }
                    });

                if let Some(account) = &selected_account {
                    if !account.vip_label.trim().is_empty() {
                        ui.label(
                            egui::RichText::new(account.vip_label.trim())
                                .size(12.5)
                                .color(egui::Color32::from_rgb(251, 114, 153)),
                        );
                    }
                    if !account.level.trim().is_empty() {
                        ui.label(
                            egui::RichText::new(format!("LV{}", account.level.trim()))
                                .size(12.5)
                                .color(egui::Color32::from_rgb(0, 161, 214)),
                        );
                    }
                }
            }
        });

        if let Some(selected_uid) = app.selected_account_uid {
            if let Some(account) = app
                .account_manager
                .accounts
                .iter()
                .find(|a| a.uid == selected_uid)
            {
                if !account.is_active {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("⚠ 该账号已关闭抢票开关，请先在「账号管理」中开启")
                            .size(12.5)
                            .color(palette.warn_text),
                    );
                } else if account.cookie_manager.is_none()
                    && app.initializing_account_uids.contains(&account.uid)
                {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("⏳ 账号客户端正在后台初始化，请稍候...")
                            .size(12.5)
                            .color(palette.text_secondary),
                    );
                }
            }
        }
    });
}

fn format_account_option(name: &str, uid: i64, is_active: bool) -> String {
    if is_active {
        format!("{} ({})", name, uid)
    } else {
        format!("{} ({}) [已关闭抢票]", name, uid)
    }
}

fn grab_mode_selection(
    ui: &mut egui::Ui,
    app: &mut Myapp,
    inner_w: f32,
    palette: &HomePalette,
) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new("抢票模式：")
                .color(palette.text_primary)
                .size(15.0)
                .strong(),
        );
        ui.add_space(8.0);

        let spacing = 10.0;
        let btn_width = ((inner_w - spacing * 2.0) / 3.0).max(100.0);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;

            if mode_selection_button(
                ui,
                "🎫 自动抢票",
                "自动检测开票时间，到点立即抢票（推荐）",
                app.grab_mode == 0,
                btn_width,
                palette,
            )
            .clicked()
            {
                app.grab_mode = 0;
            }

            if mode_selection_button(
                ui,
                "⚡ 直接抢票",
                "跳过开票倒计时直接尝试下单（仅适用于已开票项目，未开票使用可能触发风控）",
                app.grab_mode == 1,
                btn_width,
                palette,
            )
            .clicked()
            {
                app.grab_mode = 1;
            }

            if mode_selection_button(
                ui,
                "🔄 捡漏模式",
                "针对已开票项目持续轮询监测回流余票并自动尝试下单",
                app.grab_mode == 2,
                btn_width,
                palette,
            )
            .clicked()
            {
                app.grab_mode = 2;
            }
        });

        ui.add_space(8.0);
        let mode_desc = match app.grab_mode {
            0 => "推荐模式：自动获取开售时间并在开票瞬间尝试下单。",
            1 => "直接下单：仅适合已开售项目，未开票项目使用可能触发账号风控。",
            2 => "回流捡漏：持续监测余票变化，发现回流票立即下单（间隔可在设置中调整）。",
            _ => "请选择有效的抢票模式。",
        };
        ui.add(
            egui::Label::new(
                egui::RichText::new(mode_desc)
                    .size(12.5)
                    .color(palette.text_secondary),
            )
            .wrap(true),
        );
    });
}

fn mode_selection_button(
    ui: &mut egui::Ui,
    title: &str,
    tooltip: &str,
    selected: bool,
    width: f32,
    palette: &HomePalette,
) -> egui::Response {
    let btn = egui::Button::new(egui::RichText::new(title).size(14.0).color(if selected {
        egui::Color32::WHITE
    } else {
        palette.mode_unselected_text
    }))
    .min_size(egui::vec2(width, 36.0))
    .fill(if selected {
        palette.accent
    } else {
        palette.mode_unselected_fill
    })
    .rounding(7.0)
    .stroke(egui::Stroke::new(
        1.0,
        if selected {
            palette.accent_border
        } else {
            palette.mode_unselected_border
        },
    ));

    ui.add(btn).on_hover_text(tooltip)
}

fn styled_grab_button(ui: &mut egui::Ui, palette: &HomePalette) -> egui::Response {
    let button_width = 210.0;
    let button_height = 52.0;

    centered_block(ui, button_width, |ui, effective_w| {
        let button = egui::Button::new(
            egui::RichText::new("开始抢票")
                .size(21.0)
                .strong()
                .color(egui::Color32::WHITE),
        )
        .min_size(egui::vec2(effective_w, button_height))
        .fill(palette.accent)
        .stroke(egui::Stroke::new(1.0, palette.accent_border))
        .rounding(10.0);

        ui.add(button)
    })
}

fn is_valid_project_id(candidate: &str) -> bool {
    (5..=10).contains(&candidate.len())
        && candidate.bytes().all(|b| b.is_ascii_digit())
        && !candidate.starts_with('0')
        && candidate.parse::<u64>().map(|v| v > 0).unwrap_or(false)
}

/// 解析用户输入的项目 ID 或 B 站票务链接，提取合法的 5~10 位非零项目 ID
pub(crate) fn parse_project_id(input: &str) -> Result<String, String> {
    let normalized = crate::app::normalize_fullwidth_ascii(input);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return Err("请输入有效的项目 ID 或票务链接".to_string());
    }

    // 纯数字输入
    if trimmed.bytes().all(|b| b.is_ascii_digit()) {
        if is_valid_project_id(trimmed) {
            return Ok(trimmed.to_string());
        }
        return Err("输入的项目 ID 不合法，应为 5~10 位有效数字（不能以 0 开头）".to_string());
    }

    // 拒绝 b23 短链并给出明确提示
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("b23.tv") {
        return Err(
            "不支持 b23.tv 短链接，请先在浏览器打开后复制完整票务链接或直接输入项目 ID"
                .to_string(),
        );
    }

    // 先剥离 #fragment，再解析 query 参数
    let without_fragment = trimmed.split('#').next().unwrap_or(trimmed).trim();

    if let Some((_, query)) = without_fragment.split_once('?') {
        for pair in query.split('&') {
            if let Some((raw_key, raw_val)) = pair.split_once('=') {
                let key = raw_key.trim();
                let val = raw_val.trim();
                if key == "id" || key == "project_id" {
                    if is_valid_project_id(val) {
                        return Ok(val.to_string());
                    }
                    return Err(format!(
                        "链接中的项目 ID \"{}\" 不合法（应为 5~10 位有效数字）",
                        val
                    ));
                }
            }
        }
        return Err(
            "链接中未找到 id 或 project_id 参数，请确认链接正确或直接输入项目 ID".to_string(),
        );
    }

    if trimmed.contains("://") || lower.contains("bilibili.com") || trimmed.contains('/') {
        return Err(
            "链接缺少项目 ID 参数，请粘贴包含 id=... 的完整票务链接或直接输入项目 ID".to_string(),
        );
    }

    Err("输入格式无效，请输入 5~10 位数字项目 ID 或完整的 B 站票务链接".to_string())
}

fn build_referer(raw_input: &str, project_id: &str) -> String {
    let normalized = crate::app::normalize_fullwidth_ascii(raw_input);
    let without_fragment = normalized
        .trim()
        .split('#')
        .next()
        .unwrap_or("")
        .trim();
    if (without_fragment.starts_with("https://") || without_fragment.starts_with("http://"))
        && !without_fragment.chars().any(char::is_control)
    {
        without_fragment.to_string()
    } else {
        format!(
            "https://show.bilibili.com/platform/detail.html?id={}",
            project_id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::parse_project_id;

    #[test]
    fn parses_raw_numeric_ids_with_whitespace() {
        assert_eq!(parse_project_id("12345").unwrap(), "12345");
        assert_eq!(parse_project_id("  1003000 \r\n").unwrap(), "1003000");
        assert_eq!(parse_project_id("1234567890").unwrap(), "1234567890");
    }

    #[test]
    fn parses_http_https_and_schemeless_urls() {
        assert_eq!(
            parse_project_id("http://show.bilibili.com/platform/detail.html?id=98765").unwrap(),
            "98765"
        );
        assert_eq!(
            parse_project_id("https://show.bilibili.com/platform/detail.html?id=123456").unwrap(),
            "123456"
        );
        assert_eq!(
            parse_project_id("show.bilibili.com/platform/detail.html?id=12345#hash").unwrap(),
            "12345"
        );
    }

    #[test]
    fn matches_exact_id_and_project_id_ignoring_decoy_params_and_fragments() {
        assert_eq!(
            parse_project_id(
                "https://show.bilibili.com/platform/detail.html?uid=99999&spmid=88888&from_id=99&id=123456#top"
            )
            .unwrap(),
            "123456"
        );
        assert_eq!(
            parse_project_id(
                "https://mall.bilibili.com/neul-next/ticket/detail.html?from_id=55555&project_id=1003000#anchor"
            )
            .unwrap(),
            "1003000"
        );
        // 仅含 from_id / uid / spmid 时绝不能误匹配
        assert!(parse_project_id(
            "https://show.bilibili.com/platform/detail.html?uid=99999&from_id=123456&spmid=77777"
        )
        .is_err());
    }

    #[test]
    fn rejects_invalid_empty_short_zero_and_non_numeric_inputs() {
        assert!(parse_project_id("").is_err());
        assert!(parse_project_id("   \n\t  ").is_err());
        assert!(parse_project_id("0").is_err());
        assert!(parse_project_id("00000").is_err());
        assert!(parse_project_id("012345").is_err());
        assert!(parse_project_id("123").is_err());
        assert!(parse_project_id("1234").is_err());
        assert!(parse_project_id("12345678901").is_err());
        assert!(parse_project_id("abc12345").is_err());
        assert!(parse_project_id("https://b23.tv/abcdef").is_err());
    }

    #[test]
    fn parses_fullwidth_chinese_ime_digits_and_urls() {
        assert_eq!(parse_project_id("１００３００").unwrap(), "100300");
        assert_eq!(
            parse_project_id("ｈｔｔｐｓ：／／ｓｈｏｗ．ｂｉｌｉｂｉｌｉ．ｃｏｍ／ｄｅｔａｉｌ？ｉｄ＝９８７６５")
                .unwrap(),
            "98765"
        );
    }
}
