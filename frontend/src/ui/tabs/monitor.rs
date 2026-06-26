use eframe::egui;
use crate::app::Myapp;
use common::taskmanager::TaskStatus;

fn grab_mode_label(mode: u8) -> &'static str {
    match mode {
        0 => "定时",
        1 => "直接",
        2 => "捡漏",
        _ => "未知",
    }
}

/// 不同抢票模式对应的标签底色
fn mode_color(mode: u8) -> egui::Color32 {
    match mode {
        0 => egui::Color32::from_rgb(64, 150, 238),  // 定时 - 蓝
        1 => egui::Color32::from_rgb(76, 175, 80),   // 直接 - 绿
        2 => egui::Color32::from_rgb(255, 145, 77),  // 捡漏 - 橙
        _ => egui::Color32::from_rgb(150, 150, 160),
    }
}

/// 小圆角标签（彩色底 + 白字）
fn badge(ui: &mut egui::Ui, text: &str, fill: egui::Color32) {
    egui::Frame::none()
        .fill(fill)
        .rounding(7.0)
        .inner_margin(egui::vec2(7.0, 2.0))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .color(egui::Color32::WHITE)
                    .size(12.5)
                    .strong(),
            );
        });
}

pub fn render(app: &mut Myapp, ui: &mut egui::Ui){
    app.show_log_window = true;

    // 配色（卡片为暗色主题上的亮色岛屿，与 account 标签页风格一致）
    let c_dark = egui::Color32::from_rgb(55, 58, 74);
    let c_gray = egui::Color32::from_rgb(120, 124, 140);
    let c_weak = egui::Color32::from_rgb(150, 154, 170);
    let c_card = egui::Color32::from_rgb(247, 248, 251);
    let c_border = egui::Color32::from_rgb(228, 230, 240);
    let c_slate = egui::Color32::from_rgb(99, 108, 130);
    let c_pink = egui::Color32::from_rgb(251, 114, 153);
    let c_cancel = egui::Color32::from_rgb(235, 87, 87);
    let c_cancelling = egui::Color32::from_rgb(245, 166, 35);

    if let Some(accounce) = app.announce3.clone() {
        ui.label(accounce);
    } else {
        ui.label("无法连接服务器");
    }

    ui.separator();
    ui.add_space(4.0);

    // 运行中任务列表（自日志窗口迁移至此监视面板）
    let running_tasks = app.task_manager.list_running_tasks();

    // 区块标题 + 数量徽标
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("🎫 运行中任务").size(17.0).strong());
        ui.add_space(6.0);
        badge(ui, &running_tasks.len().to_string(), c_pink);
    });
    ui.add_space(8.0);

    let mut tasks_to_cancel: Vec<String> = Vec::new();

    if running_tasks.is_empty() {
        // 空状态卡片
        egui::Frame::none()
            .fill(c_card)
            .rounding(10.0)
            .stroke(egui::Stroke::new(1.0, c_border))
            .inner_margin(egui::Margin::symmetric(12.0, 18.0))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("当前暂无运行中的抢票任务").size(14.0).color(c_gray));
                    ui.add_space(3.0);
                    ui.label(egui::RichText::new("开始抢票后，任务会显示在这里").size(12.0).color(c_weak));
                });
            });
    } else {
        egui::ScrollArea::vertical()
            .id_source("running_tasks_scroll")
            .auto_shrink([false, true])
            .max_height(320.0)
            .show(ui, |ui| {
                for task in &running_tasks {
                    egui::Frame::none()
                        .fill(c_card)
                        .rounding(10.0)
                        .stroke(egui::Stroke::new(1.0, c_border))
                        .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                        .show(ui, |ui| {
                            // 第一行：序号 + 账号 + 模式标签 ……（右）取消
                            ui.horizontal(|ui| {
                                badge(ui, &format!("#{}", task.seq), c_slate);
                                ui.add_space(6.0);
                                ui.label(
                                    egui::RichText::new(&task.account_name)
                                        .size(16.0)
                                        .strong()
                                        .color(c_dark),
                                );
                                ui.add_space(6.0);
                                badge(ui, grab_mode_label(task.grab_mode), mode_color(task.grab_mode));

                                // 右侧操作区
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if let TaskStatus::Failed(msg) = &task.status {
                                        badge(ui, msg, c_cancelling);
                                    } else {
                                        let cancel_btn = egui::Button::new(
                                                egui::RichText::new("取消").color(egui::Color32::WHITE).size(13.0),
                                            )
                                            .fill(c_cancel)
                                            .rounding(7.0)
                                            .min_size(egui::vec2(54.0, 24.0));
                                        if ui.add(cancel_btn).on_hover_text("停止该抢票任务").clicked() {
                                            tasks_to_cancel.push(task.task_id.clone());
                                        }
                                    }
                                });
                            });

                            ui.add_space(6.0);

                            // 第二行：项目 + 已运行时长
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("项目").size(12.0).color(c_weak));
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new(&task.project_name).size(13.0).color(c_gray));

                                ui.add_space(16.0);

                                let mins = task.elapsed_secs / 60;
                                let secs = task.elapsed_secs % 60;
                                ui.label(egui::RichText::new("已运行").size(12.0).color(c_weak));
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(format!("{:02}:{:02}", mins, secs))
                                        .size(13.0)
                                        .strong()
                                        .color(c_gray),
                                );
                            });
                        });
                    ui.add_space(8.0);
                }
            });
    }
    // 借用结束后统一执行取消，避免对 app 的可变借用冲突
    for task_id in tasks_to_cancel {
        match app.task_manager.cancel_task(&task_id) {
            Ok(_) => log::info!("已请求取消任务: {}", task_id),
            Err(e) => log::error!("取消任务失败: {}", e),
        }
    }
}
