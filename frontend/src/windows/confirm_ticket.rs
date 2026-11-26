use std::sync::Arc;

use common::cookie_manager::CookieManager;
use common::taskmanager::{GrabTicketRequest, TaskRequest, TaskStatus};
use common::ticket::{BuyerInfo, NoBindBuyerInfo};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};

use crate::app::{normalize_fullwidth_ascii, Myapp};

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
            .map(|p| p.chars().filter(|c| c.is_ascii_digit()).collect::<String>())
            .unwrap_or_default();
        app.bilibiliticket_list[biliticket_index].no_bind_buyer_info = Some(NoBindBuyerInfo {
            name: String::new(),
            tel: preinput_phone,
            uid: biliticket_uid,
        });
    }

    let mut cancel_clicked = false;
    let mut submit_clicked = false;
    let mut refresh_buyers_clicked = false;
    let mut add_buyer_clicked = false;

    egui::Window::new("确认购票信息")
        .open(&mut open)
        .collapsible(true)
        .resizable(true)
        .default_width(540.0)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 12.0);

            // 顶部目标账号与抢票模式
            ui.vertical_centered(|ui| {
                ui.heading("确认购票信息");
                ui.add_space(2.0);
                ui.label(
                    RichText::new(format!(
                        "目标账号: {} (UID: {})  |  抢票模式: {}",
                        account_name, biliticket_uid, grab_mode_text
                    ))
                    .color(ui.visuals().weak_text_color()),
                );
            });
            ui.separator();

            // 票务信息卡片
            ui.heading("已选择票种");

            let card_bg = if ui.visuals().dark_mode {
                Color32::from_rgb(32, 35, 42)
            } else {
                Color32::from_rgb(245, 246, 250)
            };

            egui::Frame::none()
                .fill(card_bg)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .rounding(8.0)
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.label(RichText::new(&project_name).strong().size(16.0));

                    if let Some((_, ref screen_name)) = screen_summary {
                        ui.label(RichText::new(format!("场次: {}", screen_name)).size(14.0));

                        if let Some((_, ref ticket_desc, ticket_price)) = ticket_summary {
                            ui.horizontal(|ui| {
                                ui.label(format!("票种: {}", ticket_desc));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(format!(
                                                "¥{:.2}",
                                                ticket_price as f64 / 100.0
                                            ))
                                            .color(Color32::from_rgb(239, 68, 68))
                                            .strong(),
                                        );
                                    },
                                );
                            });

                            ui.horizontal(|ui| {
                                ui.label("购票数量:");
                                let mut count = app.bilibiliticket_list[biliticket_index]
                                    .count
                                    .unwrap_or(1)
                                    .clamp(1, 10);

                                if ui.add_enabled(count > 1, egui::Button::new("-")).clicked() {
                                    count -= 1;
                                    app.bilibiliticket_list[biliticket_index].count = Some(count);
                                }

                                ui.label(RichText::new(format!("{} 张", count)).strong());

                                if ui.add_enabled(count < 10, egui::Button::new("+")).clicked() {
                                    count += 1;
                                    app.bilibiliticket_list[biliticket_index].count = Some(count);
                                }

                                ui.add_space(8.0);
                                ui.label(format!(
                                    "总价: ¥{:.2}",
                                    (ticket_price as f64 / 100.0) * count as f64
                                ));
                            });
                        } else {
                            ui.label(
                                RichText::new("未匹配到有效票种，请返回重新选择")
                                    .color(Color32::from_rgb(220, 68, 55)),
                            );
                        }
                    } else {
                        ui.label(
                            RichText::new("未匹配到有效场次，请返回重新选择")
                                .color(Color32::from_rgb(220, 68, 55)),
                        );
                    }
                });

            let ime_composing = app.is_ime_composing();
            match id_bind {
                0 => {
                    ui.heading("填写联系人信息（非实名制）");
                    if let Some(ref mut buyer_info) =
                        app.bilibiliticket_list[biliticket_index].no_bind_buyer_info
                    {
                        egui::Grid::new("no_bind_contact_grid")
                            .num_columns(2)
                            .spacing([12.0, 10.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("联系人姓名:").size(14.5));
                                ui.add(
                                    egui::TextEdit::singleline(&mut buyer_info.name)
                                        .hint_text("请输入联系人姓名")
                                        .desired_width(260.0),
                                );
                                ui.end_row();

                                ui.label(RichText::new("联系人手机号:").size(14.5));
                                let tel_resp = ui.add(
                                    egui::TextEdit::singleline(&mut buyer_info.tel)
                                        .hint_text("请输入手机号（仅数字）")
                                        .desired_width(260.0),
                                );
                                if (tel_resp.changed() || tel_resp.lost_focus())
                                    && (!ime_composing || tel_resp.lost_focus())
                                {
                                    buyer_info.tel = normalize_fullwidth_ascii(&buyer_info.tel)
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
                            ui.label(
                                RichText::new("请填写非空联系人姓名与 7-15 位数字手机号")
                                    .size(12.5)
                                    .color(Color32::from_rgb(217, 119, 6)),
                            );
                        }
                    }
                }
                1 | 2 => {
                    let selected_count = app
                        .selected_buyer_list
                        .as_ref()
                        .map_or(0, |list| {
                            list.iter()
                                .filter(|sel| buyers.iter().any(|b| b.id == sel.id))
                                .count()
                        });
                    let ticket_count = app.bilibiliticket_list[biliticket_index]
                        .count
                        .unwrap_or(1)
                        .clamp(1, 10) as usize;

                    ui.horizontal_wrapped(|ui| {
                        ui.heading("选择实名购票人");
                        if id_bind == 1 {
                            ui.label(
                                RichText::new(format!("(单选 · 已选 {}/1 人)", selected_count))
                                    .color(if selected_count == 1 {
                                        Color32::from_rgb(34, 197, 94)
                                    } else {
                                        ui.visuals().weak_text_color()
                                    }),
                            );
                        } else {
                            ui.label(
                                RichText::new(format!(
                                    "(多选 · 已选 {}/{} 人)",
                                    selected_count, ticket_count
                                ))
                                .color(if selected_count == ticket_count {
                                    Color32::from_rgb(34, 197, 94)
                                } else {
                                    Color32::from_rgb(220, 68, 55)
                                }),
                            );
                        }
                    });

                    // 加载态 / 错误态 / 空列表态 / 正常列表
                    if app.buyer_info_requesting {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("正在加载实名购票人信息...");
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
                            ui.label("正在准备获取购票人信息...");
                        });
                    } else if buyers.is_empty() {
                        ui.label(
                            RichText::new("当前账号暂无实名购票人，请先添加新购票人")
                                .color(Color32::from_rgb(217, 119, 6)),
                        );
                    } else {
                        egui::ScrollArea::vertical()
                            .id_source("confirm_ticket_buyers_scroll")
                            .max_height(260.0)
                            .show(ui, |ui| {
                                let available_width = ui.available_width();
                                let card_width = 235.0;
                                let columns =
                                    (available_width / card_width).max(1.0).floor() as usize;
                                let is_multi_select = id_bind == 2;

                                egui::Grid::new("buyers_grid")
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
                                                            let select_button = if is_multi_select {
                                                                if is_selected {
                                                                    ui.add(
                                                                        egui::Button::new("☑").fill(
                                                                            Color32::from_rgb(
                                                                                34, 197, 94,
                                                                            ),
                                                                        ),
                                                                    )
                                                                } else {
                                                                    ui.add(
                                                                        egui::Button::new("☐").fill(
                                                                            Color32::TRANSPARENT,
                                                                        ),
                                                                    )
                                                                }
                                                            } else if is_selected {
                                                                ui.add(
                                                                    egui::Button::new("✓").fill(
                                                                        Color32::from_rgb(
                                                                            34, 197, 94,
                                                                        ),
                                                                    ),
                                                                )
                                                            } else {
                                                                ui.add(
                                                                    egui::Button::new("○").fill(
                                                                        Color32::TRANSPARENT,
                                                                    ),
                                                                )
                                                            };

                                                            if select_button.clicked() {
                                                                app.selected_buyer_uid =
                                                                    Some(biliticket_uid);
                                                                if is_multi_select {
                                                                    let buyer_list = app
                                                                        .selected_buyer_list
                                                                        .get_or_insert_with(
                                                                            Vec::new,
                                                                        );
                                                                    if let Some(pos) = buyer_list
                                                                        .iter()
                                                                        .position(|b| {
                                                                            b.id == buyer.id
                                                                        })
                                                                    {
                                                                        buyer_list.remove(pos);
                                                                    } else {
                                                                        buyer_list
                                                                            .push(buyer.clone());
                                                                    }
                                                                } else {
                                                                    app.selected_buyer_list =
                                                                        Some(vec![buyer.clone()]);
                                                                    app.bilibiliticket_list
                                                                        [biliticket_index]
                                                                        .buyer_info =
                                                                        Some(vec![buyer.clone()]);
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
                                                                ui.label(format!(
                                                                    "手机号: {}",
                                                                    buyer.tel
                                                                ));
                                                            });
                                                        });
                                                    });
                                            });

                                            if (index + 1) % columns == 0
                                                && index < buyers.len() - 1
                                            {
                                                ui.end_row();
                                            }
                                        }
                                    });
                            });
                    }

                    // 无论列表是否为空，始终提供添加与刷新按钮
                    ui.horizontal(|ui| {
                        if ui.button("添加新购票人").clicked() {
                            add_buyer_clicked = true;
                        }
                        if ui
                            .add_enabled(
                                !app.buyer_info_requesting,
                                egui::Button::new("刷新购票人"),
                            )
                            .clicked()
                        {
                            refresh_buyers_clicked = true;
                        }
                    });
                }
                _ => {
                    ui.label(
                        RichText::new(format!(
                            "当前项目返回了未知的实名绑定状态 (id_bind={})，无法确认购票人规则，请重新获取项目详情或改用捡漏模式。",
                            id_bind
                        ))
                        .color(Color32::from_rgb(220, 68, 55)),
                    );
                }
            }

            // 底部按钮区域
            ui.separator();

            let screen_ticket_ready = screen_summary.is_some() && ticket_summary.is_some();
            let ticket_count = app.bilibiliticket_list[biliticket_index]
                .count
                .unwrap_or(1)
                .clamp(1, 10) as usize;

            let button_enabled = screen_ticket_ready
                && match id_bind {
                    0 => app.bilibiliticket_list[biliticket_index]
                        .no_bind_buyer_info
                        .as_ref()
                        .map_or(false, |info| {
                            let tel_norm = normalize_fullwidth_ascii(&info.tel);
                            !info.name.trim().is_empty() && is_valid_phone(tel_norm.trim())
                        }),
                    1 => app.selected_buyer_list.as_ref().map_or(false, |list| {
                        list.len() == 1 && buyers.iter().any(|b| b.id == list[0].id)
                    }),
                    2 => app.selected_buyer_list.as_ref().map_or(false, |list| {
                        !list.is_empty()
                            && list.len() == ticket_count
                            && list
                                .iter()
                                .all(|sel| buyers.iter().any(|b| b.id == sel.id))
                    }),
                    _ => false,
                };

            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            button_enabled,
                            egui::Button::new(
                                RichText::new("确认购票").color(Color32::WHITE),
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
    use super::mask_id;

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
}
