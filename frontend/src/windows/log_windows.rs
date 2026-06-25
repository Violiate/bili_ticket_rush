
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

fn status_label(status: &TaskStatus) -> String {
    match status {
        TaskStatus::Pending => "等待中".to_string(),
        TaskStatus::Running => "运行中".to_string(),
        TaskStatus::Completed(_) => "已完成".to_string(),
        TaskStatus::Failed(msg) => msg.clone(),
    }
}

pub fn show(app: &mut Myapp, ctx: &egui::Context) {

    let mut window_open = app.show_log_window;
    let mut user_close: bool = false;

    egui::Window::new("监视面板")
        .open(&mut window_open)
        .default_size([550.0, 400.0])
        .resizable(true)
        .show(ctx, |ui| {
            // 顶部工具栏
            ui.horizontal(|ui| {
                if ui.button("清空日志").clicked() {
                    app.logs.clear();
                }

                if ui.button("添加测试日志").clicked() {
                    app.add_log("测试日志消息");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::RIGHT), |ui| {
                    if ui.button("❌").clicked() {
                        user_close = true;
                        app.show_log_window = false;
                    }
                });
            });

            ui.separator();

            // 运行中任务列表
            let running_tasks = app.task_manager.list_running_tasks();
            ui.label(egui::RichText::new(format!("运行中任务（{}）", running_tasks.len())).strong());
            let mut tasks_to_cancel: Vec<String> = Vec::new();
            if running_tasks.is_empty() {
                ui.label("当前无运行中的抢票任务");
            } else {
                egui::ScrollArea::vertical()
                    .id_source("running_tasks_scroll")
                    .auto_shrink([false, true])
                    .max_height(150.0)
                    .show(ui, |ui| {
                        egui::Grid::new("running_tasks_grid")
                            .num_columns(6)
                            .striped(true)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("#").strong());
                                ui.label(egui::RichText::new("账号").strong());
                                ui.label(egui::RichText::new("项目").strong());
                                ui.label(egui::RichText::new("模式").strong());
                                ui.label(egui::RichText::new("已运行").strong());
                                ui.label(egui::RichText::new("操作").strong());
                                ui.end_row();

                                for task in &running_tasks {
                                    ui.label(format!("#{}", task.seq));
                                    ui.label(&task.account_name);
                                    ui.label(&task.project_name);
                                    ui.label(grab_mode_label(task.grab_mode));
                                    let mins = task.elapsed_secs / 60;
                                    let secs = task.elapsed_secs % 60;
                                    ui.label(format!("{:02}:{:02}", mins, secs));
                                    ui.horizontal(|ui| {
                                        if matches!(task.status, TaskStatus::Failed(_)) {
                                            ui.label(status_label(&task.status));
                                        } else if ui.button("取消").clicked() {
                                            tasks_to_cancel.push(task.task_id.clone());
                                        }
                                    });
                                    ui.end_row();
                                }
                            });
                    });
            }
            // 借用结束后统一执行取消，避免对 app 的可变借用冲突
            for task_id in tasks_to_cancel {
                match app.task_manager.cancel_task(&task_id) {
                    Ok(_) => log::info!("已请求取消任务: {}", task_id),
                    Err(e) => log::error!("取消任务失败: {}", e),
                }
            }

            ui.separator();

            // 日志内容区域
            egui::ScrollArea::vertical()
                .id_source("log_scroll")
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .max_height(300.0)
                .show(ui, |ui| {
                    // 显示当前状态
                    ui.label(format!("当前状态: {}", 
                        if app.running_status.is_empty() { "未知状态" } else { &app.running_status }));
                    
                    ui.separator();
                    
                    // 显示所有日志
                    if app.logs.is_empty() {
                        ui.label("暂无日志记录");
                        
                    } else {
                        for log in &app.logs {
                            ui.label(log);
                            ui.separator();
                        }
                    }
                });
                
            // 底部状态栏
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.label(format!("共 {} 条日志", app.logs.len()));
            });
        });
    
    // 更新窗口状态
    app.show_log_window = window_open;
}