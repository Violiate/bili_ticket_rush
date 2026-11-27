use eframe::egui;

use crate::app::Myapp;
use common::taskmanager::TaskStatus;

struct MonitorPalette {
    card_fill: egui::Color32,
    card_border: egui::Color32,
    text_primary: egui::Color32,
    text_secondary: egui::Color32,
    text_muted: egui::Color32,
    notice_fill: egui::Color32,
    notice_border: egui::Color32,
    notice_text: egui::Color32,
    slate: egui::Color32,
    pink: egui::Color32,
    cancel: egui::Color32,
    cancelling: egui::Color32,
    success: egui::Color32,
    cancelled_gray: egui::Color32,
    failed_red: egui::Color32,
}

impl MonitorPalette {
    fn from_dark_mode(dark_mode: bool) -> Self {
        if dark_mode {
            Self {
                card_fill: egui::Color32::from_rgb(36, 38, 46),
                card_border: egui::Color32::from_rgb(62, 66, 80),
                text_primary: egui::Color32::from_rgb(235, 238, 245),
                text_secondary: egui::Color32::from_rgb(180, 186, 202),
                text_muted: egui::Color32::from_rgb(138, 144, 160),
                notice_fill: egui::Color32::from_rgb(52, 44, 32),
                notice_border: egui::Color32::from_rgb(156, 108, 48),
                notice_text: egui::Color32::from_rgb(255, 192, 122),
                slate: egui::Color32::from_rgb(86, 98, 122),
                pink: egui::Color32::from_rgb(251, 114, 153),
                cancel: egui::Color32::from_rgb(224, 76, 76),
                cancelling: egui::Color32::from_rgb(236, 154, 32),
                success: egui::Color32::from_rgb(46, 160, 67),
                cancelled_gray: egui::Color32::from_rgb(112, 120, 136),
                failed_red: egui::Color32::from_rgb(220, 72, 68),
            }
        } else {
            Self {
                card_fill: egui::Color32::from_rgb(248, 249, 252),
                card_border: egui::Color32::from_rgb(218, 222, 234),
                text_primary: egui::Color32::from_rgb(40, 44, 58),
                text_secondary: egui::Color32::from_rgb(92, 98, 114),
                text_muted: egui::Color32::from_rgb(132, 138, 154),
                notice_fill: egui::Color32::from_rgb(255, 247, 235),
                notice_border: egui::Color32::from_rgb(240, 188, 128),
                notice_text: egui::Color32::from_rgb(188, 86, 22),
                slate: egui::Color32::from_rgb(92, 104, 128),
                pink: egui::Color32::from_rgb(251, 114, 153),
                cancel: egui::Color32::from_rgb(228, 80, 80),
                cancelling: egui::Color32::from_rgb(240, 158, 32),
                success: egui::Color32::from_rgb(46, 160, 67),
                cancelled_gray: egui::Color32::from_rgb(122, 130, 146),
                failed_red: egui::Color32::from_rgb(216, 66, 62),
            }
        }
    }
}

fn grab_mode_label(mode: u8) -> &'static str {
    match mode {
        0 => "定时",
        1 => "直接",
        2 => "捡漏",
        _ => "未知",
    }
}

fn mode_color(mode: u8) -> egui::Color32 {
    match mode {
        0 => egui::Color32::from_rgb(64, 150, 238), // 定时 - 蓝
        1 => egui::Color32::from_rgb(66, 168, 78),  // 直接 - 绿
        2 => egui::Color32::from_rgb(242, 138, 62), // 捡漏 - 橙
        _ => egui::Color32::from_rgb(136, 140, 152),
    }
}

/// 格式化运行时长：不足 1 小时为 MM:SS，满 1 小时为 HH:MM:SS
pub(crate) fn format_elapsed(secs: u64) -> String {
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let rem_secs = secs % 60;
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, mins, rem_secs)
    } else {
        format!("{:02}:{:02}", mins, rem_secs)
    }
}

fn badge(ui: &mut egui::Ui, text: &str, fill: egui::Color32) {
    egui::Frame::none()
        .fill(fill)
        .rounding(7.0)
        .inner_margin(egui::vec2(7.0, 2.0))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .color(egui::Color32::WHITE)
                    .size(12.0)
                    .strong(),
            );
        });
}

pub fn render(app: &mut Myapp, ui: &mut egui::Ui) {
    let dark_mode = ui.ctx().style().visuals.dark_mode;
    let palette = MonitorPalette::from_dark_mode(dark_mode);

    let running_tasks = app.task_manager.list_running_tasks();
    let recent_tasks = app.task_manager.list_recent_tasks();

    // 顶部栏：标题 + 状态概览 + 实时日志窗口快捷按钮
    ui.horizontal(|ui| {
        ui.heading(
            egui::RichText::new("监视面板")
                .size(22.0)
                .strong()
                .color(palette.text_primary),
        );
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(format!(
                "运行中 {} · 近期记录 {}",
                running_tasks.len(),
                recent_tasks.len()
            ))
            .size(13.0)
            .color(palette.text_secondary),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let log_count = app.logs.len();
            let log_btn_text = if app.show_log_window {
                "📜 收起实时日志窗口".to_string()
            } else if log_count > 0 {
                format!("📜 打开实时日志 ({})", log_count)
            } else {
                "📜 打开实时日志窗口".to_string()
            };
            let (fill, stroke) = if app.show_log_window {
                (
                    egui::Color32::from_rgb(16, 152, 104),
                    egui::Stroke::new(1.2, egui::Color32::from_rgb(52, 211, 153)),
                )
            } else {
                (
                    egui::Color32::from_rgb(37, 122, 235),
                    egui::Stroke::new(1.2, egui::Color32::from_rgb(112, 176, 255)),
                )
            };
            let log_btn = egui::Button::new(
                egui::RichText::new(log_btn_text)
                    .color(egui::Color32::WHITE)
                    .strong()
                    .size(13.5),
            )
            .fill(fill)
            .stroke(stroke)
            .rounding(8.0)
            .min_size(egui::vec2(152.0, 32.0));
            if ui
                .add(log_btn)
                .on_hover_text("实时查看抢票执行详情、接口返回与错误排查日志")
                .clicked()
            {
                app.show_log_window = !app.show_log_window;
            }
        });
    });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(6.0);

    let mut tasks_to_cancel: Vec<String> = Vec::new();
    let mut clear_recent = false;
    let mut reconfigure_action: Option<(i64, String, u8)> = None;

    egui::ScrollArea::vertical()
        .id_source("monitor_tab_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // 仅在存在非空服务端公告时展示横幅（无公告时不误报「无法连接服务器」）
            if let Some(announce) = app
                .announce3
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                egui::Frame::none()
                    .fill(palette.notice_fill)
                    .rounding(8.0)
                    .stroke(egui::Stroke::new(1.0, palette.notice_border))
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width().max(240.0));
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(announce)
                                    .size(13.5)
                                    .color(palette.notice_text),
                            )
                            .wrap(true),
                        );
                    });
                ui.add_space(10.0);
            }

            // ================= 运行中任务 =================
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("🎫 运行中任务")
                        .size(16.5)
                        .strong()
                        .color(palette.text_primary),
                );
                ui.add_space(6.0);
                badge(ui, &running_tasks.len().to_string(), palette.pink);

                if running_tasks.len() > 1 {
                    let any_cancellable = running_tasks
                        .iter()
                        .any(|t| !matches!(t.status, TaskStatus::Cancelling));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let cancel_all_btn = egui::Button::new(
                            egui::RichText::new("全部取消")
                                .color(egui::Color32::WHITE)
                                .size(12.5)
                                .strong(),
                        )
                        .fill(palette.cancel)
                        .rounding(7.0)
                        .min_size(egui::vec2(76.0, 26.0));
                        if ui
                            .add_enabled(any_cancellable, cancel_all_btn)
                            .on_hover_text("停止所有正在运行的抢票任务")
                            .clicked()
                        {
                            for task in &running_tasks {
                                if !matches!(task.status, TaskStatus::Cancelling) {
                                    tasks_to_cancel.push(task.task_id.clone());
                                }
                            }
                        }
                    });
                }
            });
            ui.add_space(8.0);

            if running_tasks.is_empty() {
                egui::Frame::none()
                    .fill(palette.card_fill)
                    .rounding(10.0)
                    .stroke(egui::Stroke::new(1.0, palette.card_border))
                    .inner_margin(egui::Margin::symmetric(14.0, 20.0))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width().max(240.0));
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("当前暂无运行中的抢票任务")
                                    .size(14.5)
                                    .color(palette.text_secondary),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new("在「开始抢票」发起任务后，可在此实时查看与取消")
                                    .size(12.5)
                                    .color(palette.text_muted),
                            );
                        });
                    });
            } else {
                for task in &running_tasks {
                    egui::Frame::none()
                        .fill(palette.card_fill)
                        .rounding(10.0)
                        .stroke(egui::Stroke::new(1.0, palette.card_border))
                        .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width().max(240.0));

                            // 第一行（固定紧凑元信息 + 右侧操作区，确保计时器与取消按钮永不被长文本挤出）
                            ui.horizontal(|ui| {
                                badge(ui, &format!("#{}", task.seq), palette.slate);
                                ui.add_space(6.0);
                                badge(
                                    ui,
                                    grab_mode_label(task.grab_mode),
                                    mode_color(task.grab_mode),
                                );
                                ui.add_space(10.0);
                                ui.label(
                                    egui::RichText::new(format!(
                                        "已运行 {}",
                                        format_elapsed(task.elapsed_secs)
                                    ))
                                    .size(13.0)
                                    .strong()
                                    .color(palette.text_secondary),
                                );

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let is_cancelling =
                                            matches!(task.status, TaskStatus::Cancelling);
                                        let cancel_btn = egui::Button::new(
                                            egui::RichText::new("取消")
                                                .color(egui::Color32::WHITE)
                                                .size(12.5),
                                        )
                                        .fill(palette.cancel)
                                        .rounding(7.0)
                                        .min_size(egui::vec2(54.0, 24.0));

                                        if is_cancelling {
                                            ui.add_enabled(false, cancel_btn);
                                            ui.add_space(6.0);
                                            badge(ui, "取消中...", palette.cancelling);
                                        } else if ui
                                            .add(cancel_btn)
                                            .on_hover_text("停止该抢票任务")
                                            .clicked()
                                        {
                                            tasks_to_cancel.push(task.task_id.clone());
                                        }
                                    },
                                );
                            });

                            ui.add_space(6.0);

                            // 第二行：账号名称（自动换行）
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(6.0, 2.0);
                                ui.label(
                                    egui::RichText::new("账号")
                                        .size(12.0)
                                        .color(palette.text_muted),
                                );
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(&task.account_name)
                                            .size(14.5)
                                            .strong()
                                            .color(palette.text_primary),
                                    )
                                    .wrap(true),
                                );
                            });

                            ui.add_space(3.0);

                            // 第三行：项目名称（自动换行）
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(6.0, 2.0);
                                ui.label(
                                    egui::RichText::new("项目")
                                        .size(12.0)
                                        .color(palette.text_muted),
                                );
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(&task.project_name)
                                            .size(13.5)
                                            .color(palette.text_secondary),
                                    )
                                    .wrap(true),
                                );
                            });
                        });
                    ui.add_space(8.0);
                }
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(10.0);

            // ================= 近期任务记录 =================
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("🕘 近期任务记录")
                        .size(16.5)
                        .strong()
                        .color(palette.text_primary),
                );
                ui.add_space(6.0);
                badge(ui, &recent_tasks.len().to_string(), palette.slate);

                if !recent_tasks.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let clear_btn = egui::Button::new(
                            egui::RichText::new("清空记录")
                                .size(12.5)
                                .color(palette.text_primary),
                        )
                        .rounding(7.0)
                        .min_size(egui::vec2(74.0, 26.0));
                        if ui.add(clear_btn).clicked() {
                            clear_recent = true;
                        }
                    });
                }
            });
            ui.add_space(8.0);

            if recent_tasks.is_empty() {
                egui::Frame::none()
                    .fill(palette.card_fill)
                    .rounding(10.0)
                    .stroke(egui::Stroke::new(1.0, palette.card_border))
                    .inner_margin(egui::Margin::symmetric(14.0, 16.0))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width().max(240.0));
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("暂无近期已结束的任务记录")
                                    .size(13.5)
                                    .color(palette.text_muted),
                            );
                        });
                    });
            } else {
                for task in &recent_tasks {
                    let (status_label, status_color, msg_color) = match &task.status {
                        TaskStatus::Completed(true) => {
                            ("成功", palette.success, palette.success)
                        }
                        TaskStatus::Cancelled => {
                            ("已取消", palette.cancelled_gray, palette.text_secondary)
                        }
                        TaskStatus::Failed(_) | TaskStatus::Completed(false) => {
                            ("失败", palette.failed_red, palette.failed_red)
                        }
                        _ => ("已结束", palette.slate, palette.text_secondary),
                    };

                    let duration_secs = task
                        .finished_at
                        .saturating_duration_since(task.started_at)
                        .as_secs();

                    egui::Frame::none()
                        .fill(palette.card_fill)
                        .rounding(10.0)
                        .stroke(egui::Stroke::new(1.0, palette.card_border))
                        .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width().max(240.0));

                            ui.horizontal(|ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 4.0);
                                    badge(ui, status_label, status_color);
                                    badge(
                                        ui,
                                        grab_mode_label(task.grab_mode),
                                        mode_color(task.grab_mode),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "耗时 {}",
                                            format_elapsed(duration_secs)
                                        ))
                                        .size(12.5)
                                        .color(palette.text_secondary),
                                    );
                                    if !task.project_id.trim().is_empty() {
                                        ui.label(
                                            egui::RichText::new(format!("ID: {}", task.project_id))
                                                .size(12.0)
                                                .color(palette.text_muted),
                                        );
                                    }
                                });

                                if !task.project_id.trim().is_empty() {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new("重新配置")
                                                            .size(12.0)
                                                            .color(palette.text_primary),
                                                    )
                                                    .rounding(6.0)
                                                    .min_size(egui::vec2(68.0, 24.0)),
                                                )
                                                .on_hover_text(
                                                    "使用该记录的项目ID、账号与模式直接重新打开选票/确认窗口",
                                                )
                                                .clicked()
                                            {
                                                reconfigure_action = Some((
                                                    task.uid,
                                                    task.project_id.clone(),
                                                    task.grab_mode,
                                                ));
                                            }
                                        },
                                    );
                                }
                            });

                            ui.add_space(6.0);

                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(6.0, 2.0);
                                ui.label(
                                    egui::RichText::new("账号")
                                        .size(12.0)
                                        .color(palette.text_muted),
                                );
                                ui.label(
                                    egui::RichText::new(&task.account_name)
                                        .size(13.5)
                                        .strong()
                                        .color(palette.text_primary),
                                );
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("项目")
                                        .size(12.0)
                                        .color(palette.text_muted),
                                );
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(&task.project_name)
                                            .size(13.5)
                                            .color(palette.text_secondary),
                                    )
                                    .wrap(true),
                                );
                            });

                            let detail = task.message.trim();
                            if !detail.is_empty() {
                                ui.add_space(4.0);
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!("结果：{}", detail))
                                            .size(12.5)
                                            .color(msg_color),
                                    )
                                    .wrap(true),
                                );
                            }
                        });
                    ui.add_space(8.0);
                }
            }

            ui.add_space(12.0);
        });

    for task_id in tasks_to_cancel {
        match app.task_manager.cancel_task(&task_id) {
            Ok(_) => log::info!("已请求取消任务: {}", task_id),
            Err(e) => log::error!("取消任务失败: {}", e),
        }
    }

    if clear_recent {
        app.task_manager.clear_recent_tasks();
        app.show_info("已清空近期任务记录");
    }

    if let Some((uid, project_id, mode)) = reconfigure_action {
        app.ticket_id = project_id;
        app.persist_grab_mode(mode);
        if app.account_manager.accounts.iter().any(|a| a.uid == uid) {
            app.selected_account_uid = Some(uid);
        }
        app.selected_tab = 0;
        crate::ui::tabs::home::trigger_start_grab(app);
    }
}

#[cfg(test)]
mod tests {
    use super::format_elapsed;

    #[test]
    fn formats_seconds_below_and_above_one_hour() {
        assert_eq!(format_elapsed(0), "00:00");
        assert_eq!(format_elapsed(5), "00:05");
        assert_eq!(format_elapsed(65), "01:05");
        assert_eq!(format_elapsed(3599), "59:59");
        assert_eq!(format_elapsed(3600), "01:00:00");
        assert_eq!(format_elapsed(3661), "01:01:01");
    }
}
