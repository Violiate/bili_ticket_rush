use std::u32;

use eframe::egui;
use eframe::egui::Widget;
use crate::app::Myapp;
use common::account::{Account};
use common::taskmanager::{TaskStatus, TicketRequest, TaskManager_debug};
use common::ticket::BilibiliTicket;


pub fn render(app: &mut Myapp, ui: &mut egui::Ui) {
    //页面标题
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);
        ui.heading(egui::RichText::new("仅供学习的小工具").size(32.0).strong());
        ui.add_space(10.0);
        ui.label(egui::RichText::new(TaskManager_debug())
            .size(14.0)
            .color(egui::Color32::from_rgb(255, 120, 50))
            .strong());
        ui.add_space(10.0);
        ui.label(egui::RichText::new("请输入项目ID或粘贴票务链接，点击开始抢票").size(16.0).color(egui::Color32::GRAY));
        ui.add_space(10.0);
        if let Some(accounce) = app.announce1.clone() {
            ui.label(egui::RichText::new(accounce)
            .size(14.0)
            .color(egui::Color32::from_rgb(255, 120, 50))
            .strong());
        } 
        ui.add_space(25.0);

        //输入区域
        ticket_input_area(ui, app);
    });
}

fn ticket_input_area(ui: &mut egui::Ui, app: &mut Myapp) {
    //居中布局的输入框和按钮组合
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 20.0);

        //输入框布局
        let response = styled_ticket_input(ui, &mut app.ticket_id);

        // 新增：账号和抢票模式选择区域
        ui.add_space(15.0);
        styled_selection_area(ui, app);
        ui.add_space(15.0);

        //抢票按钮
        if styled_grab_button(ui).clicked() {
            let (project_id, referer) = match check_input_ticket(&app.ticket_id) {
                Some(res) => {
                    app.ticket_id = res.0.clone();
                    
                    res
                },
                None => {
                    app.show_log_window = true;
                    return;
                }
            };
            if app.account_manager.accounts.is_empty() {
                log::info!("没有可用账号，请登录账号");
                app.show_login_windows = true;
                return
            }
            let select_uid = match app.selected_account_uid {
                Some(uid) => uid,
                None => {
                    log::error!("没有选择账号，请选择账号！");
                    return
                }
            };
            let selected_account = match app.account_manager.accounts
                .iter()
                .find(|a| a.uid == select_uid)
            {
                Some(account) => account,
                None => {
                    log::error!("未找到uid为{}的账号", select_uid);
                    app.error_banner_active = true;
                    app.error_banner_text = format!("未找到uid为{}的账号，请重新选择", select_uid);
                    app.error_banner_start_time = Some(std::time::Instant::now());
                    app.error_banner_opacity = 1.0;
                    return;
                }
            };
            let bilibili_ticket: BilibiliTicket = BilibiliTicket::new(

                &app.grab_mode,
                &app.default_ua,
                &app.custom_config,
                selected_account,

                &app.push_config,
                &app.status_delay,
                &project_id,
                &referer,
            );
            app.bilibiliticket_list.push(bilibili_ticket);
            log::debug!("当前抢票对象列表：{:?}", app.bilibiliticket_list);
            match app.grab_mode{
                0|1 => {
                    app.show_screen_info = Some(select_uid);
                }
                2 => {
                    app.confirm_ticket_info = Some(select_uid.to_string());
                }
                _ => {
                    log::error!("当前模式不支持！请检查输入！");
                }
            }


        }

        //底部状态文本
        ui.add_space(30.0);
       /*  let status_text = match app.is_loading {
            true => egui::RichText::new(&app.running_status).color(egui::Color32::from_rgb(255, 165, 0)),
            false => egui::RichText::new("等待开始...").color(egui::Color32::GRAY),
        };
        ui.label(status_text); */
    });
}

//输入框
fn styled_ticket_input(ui: &mut egui::Ui, text: &mut String) -> egui::Response {
    //创建一个适当大小的容器
    let desired_width = 250.0;

    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() - desired_width) / 2.0);

        egui::Frame::none()
            .fill(egui::Color32::from_rgb(245, 245, 250))
            .rounding(10.0)
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 200, 220)))
            .shadow(egui::epaint::Shadow::small_light())
            .inner_margin(egui::vec2(12.0, 10.0))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

                // 左侧图标
                ui.label(egui::RichText::new("🎫").size(18.0));

                // 输入框
                let font_id = egui::FontId::new(20.0, egui::FontFamily::Proportional);
                ui.style_mut().override_font_id = Some(font_id.clone());

                let input = egui::TextEdit::singleline(text)
                    .hint_text("输入票务ID")
                    .desired_width(180.0)
                    .text_color(egui::Color32::BLACK) //指定文本颜色防止深色模式抽风
                    .margin(egui::vec2(0.0, 6.0))
                    .frame(false);

                ui.add(input)
            })
            .inner
    }).inner
}

//选择模式区域UI
fn styled_selection_area(ui: &mut egui::Ui, app: &mut Myapp) {
    // 容器宽度与抢票按钮相同，保持一致性
    let panel_width = 400.0;

    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() - panel_width) / 2.0);

        egui::Frame::none()
            .fill(egui::Color32::from_rgb(245, 245, 250))
            .rounding(8.0)
            .stroke(egui::Stroke::new(0.5, egui::Color32::from_rgb(200, 200, 220)))
            .shadow(egui::epaint::Shadow::small_light())
            .inner_margin(egui::vec2(16.0, 12.0))
            .show(ui, |ui| {
                ui.set_width(panel_width - 32.0); // 减去内边距

                ui.vertical(|ui| {
                    // 账号选择
                    account_selection(ui, app);

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(12.0);

                    // 抢票模式选择
                    grab_mode_selection(ui, app);
                });
            });
    });
}

// 账号选择UI
fn account_selection(ui: &mut egui::Ui, app: &mut Myapp) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("选择账号：").color(egui::Color32::BLACK).size(16.0).strong());

        // 如果没有账号，显示提示
        if app.account_manager.accounts.is_empty() {
            ui.label(egui::RichText::new("未登录账号").color(egui::Color32::RED).italics());
            ui.add_space(8.0);
            if egui::Button::new(egui::RichText::new("去登录").size(14.0).color(egui::Color32::BLUE))
                .fill(egui::Color32::LIGHT_GRAY) // 设置背景颜色
                .ui(ui)
                .clicked() {
                app.show_login_windows = true;
            }
        } else {
            // 初始化选中账号（如果未选择）
            if app.selected_account_uid.is_none() && !app.account_manager.accounts.is_empty() {
                app.selected_account_uid = Some(app.account_manager.accounts[0].uid);
            }

            // 创建账号ComboBox
            let selected_account = app.account_manager.accounts.iter()
                .find(|a| Some(a.uid) == app.selected_account_uid);

            let selected_text = match selected_account {
                Some(account) => format!("{} ({})", account.name, account.uid),
                None => "选择账号".to_string(),
            };

            egui::ComboBox::from_id_source("account_selector")
                .selected_text(selected_text)
                .width(200.0)
                .show_ui(ui, |ui| {
                    for account in &app.account_manager.accounts {
                        let text = format!("{} ({})", account.name, account.uid);
                        let is_selected = Some(account.uid) == app.selected_account_uid;

                        if ui.selectable_label(is_selected, text).clicked() {
                            app.selected_account_uid = Some(account.uid);
                        }
                    }
                });

            // 显示会员等级和状态（如果有选中账号）
            if let Some(account) = selected_account {
                ui.add_space(10.0);
                if !account.vip_label.is_empty() {
                    let vip_text = egui::RichText::new(&account.vip_label)
                        .size(13.0)
                        .color(egui::Color32::from_rgb(251, 114, 153));
                    ui.label(vip_text);
                }

                let level_text = egui::RichText::new(format!("LV{}", account.level))
                    .size(13.0)
                    .color(egui::Color32::from_rgb(0, 161, 214));
                ui.label(level_text);
            }
        }
    });
}

// 抢票模式选择UI
fn grab_mode_selection(ui: &mut egui::Ui, app: &mut Myapp) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new("抢票模式：").color(egui::Color32::BLACK).size(16.0).strong());
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.style_mut().spacing.item_spacing.x = 12.0;

            // 第一种模式 - 自动抢票（推荐）
            let selected = app.grab_mode == 0;
            if mode_selection_button(ui, "🎫 自动抢票（推荐）",
                "自动检测开票时间抢票", selected).clicked() {
                app.grab_mode = 0;
            }

            // 第二种模式 - 直接抢票
            let selected = app.grab_mode == 1;
            if mode_selection_button(ui, "⚡ 直接抢票",
                "直接开始尝试下单（适合已开票项目！，未开票项目使用会导致冻结账号！）", selected).clicked() {
                app.grab_mode = 1;
            }

            // 第三种模式 - 捡漏模式
            let selected = app.grab_mode == 2;
            if mode_selection_button(ui, "🔄 捡漏模式",
                "对于已开票项目，监测是否出现余票并尝试下单", selected).clicked() {
                app.grab_mode = 2;
            }
        });
    });
}

// 抢票模式按钮
fn mode_selection_button(ui: &mut egui::Ui, title: &str, tooltip: &str, selected: bool) -> egui::Response {
    let btn = ui.add(
        egui::widgets::Button::new(
            egui::RichText::new(title)
                .size(14.0)
                .color(if selected {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::from_rgb(70, 70, 70)
                })
        )
        .min_size(egui::vec2(110.0, 36.0))
        .fill(if selected {
            egui::Color32::from_rgb(102, 204, 255)
        } else {
            egui::Color32::from_rgb(230, 230, 235)
        })
        .rounding(6.0)
        .stroke(egui::Stroke::new(
            0.5,
            if selected {
                egui::Color32::from_rgb(25, 118, 210)
            } else {
                egui::Color32::from_rgb(180, 180, 190)
            }
        ))
    );

    // 添加悬停提示
    btn.clone().on_hover_text(tooltip);

    btn
}
//抢票按钮
fn styled_grab_button(ui: &mut egui::Ui) -> egui::Response {
    let button_width = 200.0;
    let button_height = 60.0;

    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() - button_width) / 2.0);

        let button = egui::Button::new(
            egui::RichText::new("开始抢票")
                .size(24.0)
                .strong()
                .color(egui::Color32::from_rgb(255,255,255))
        )
        .min_size(egui::vec2(button_width, button_height))
        .fill(egui::Color32::from_rgb(102, 204, 255))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(25, 118, 210)))
        .rounding(12.0);

        ui.add(button)
    }).inner
}

fn check_input_ticket(ticket_input: &str) -> Option<(String, String)> {
    if ticket_input.is_empty() {
        log::info!("请输入有效的票务id");
        return None;
    }

    if ticket_input.contains("https") {
        if let Some(id_start) = ticket_input.find("id=") {
            let id_str = &ticket_input[id_start + 3..];
            let id_str = id_str.split('&').next().unwrap_or(id_str);
            
            if (5..=8).contains(&id_str.len()) && id_str.parse::<u32>().is_ok() {
                log::info!("获取到的id为：{}", id_str);
                return Some((id_str.to_string(), ticket_input.to_string()));
            }
            log::error!("输入的id不合法，请检查输入，可尝试直接输入id");
            return None;
        }
        log::error!("未找到对应的id，请不要使用b23开头的短连接，正确链接以show.bilibili或mall.bilibili开头");
        return None;
    }

    if ticket_input.parse::<u32>().is_ok() {
        log::info!("获取到的id为：{}", ticket_input);
        let referer = format!("https://show.bilibili.com/platform/detail.html?id={}", ticket_input);
        return Some((ticket_input.to_string(), referer));
    }
    
    log::error!("输入的id不是数字类型，请检查输入");
    None
}
