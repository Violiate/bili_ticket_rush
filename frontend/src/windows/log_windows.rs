use eframe::egui::{self, Color32, RichText};

use crate::app::Myapp;

pub fn show(app: &mut Myapp, ctx: &egui::Context) {
    let mut window_open = app.show_log_window;
    if !window_open {
        return;
    }

    let running_count = app.task_manager.list_running_tasks().len();
    let mut copy_clicked = false;
    let mut copied_count: Option<usize> = None;

    egui::Window::new("日志监视")
        .open(&mut window_open)
        .default_size([660.0, 440.0])
        .resizable(true)
        .show(ctx, |ui| {
            // 顶部状态与操作工具栏
            ui.horizontal_wrapped(|ui| {
                ui.label("当前状态:");
                if running_count > 0 {
                    ui.label(
                        RichText::new(format!("运行中 ({} 个任务)", running_count))
                            .strong()
                            .color(Color32::from_rgb(34, 197, 94)),
                    );
                } else {
                    ui.label(RichText::new("空闲").color(ui.visuals().weak_text_color()));
                }

                ui.separator();

                ui.checkbox(&mut app.log_auto_scroll, "自动滚动");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("清空日志").clicked() {
                        app.logs.clear();
                    }
                    if ui.button("复制当前日志").clicked() {
                        copy_clicked = true;
                    }
                });
            });

            // 级别过滤 + 关键字搜索栏
            ui.horizontal_wrapped(|ui| {
                let levels = [(0u8, "全部"), (1, "INFO+"), (2, "WARN+"), (3, "ERROR")];
                for (level, label) in levels {
                    ui.selectable_value(&mut app.log_filter_level, level, label);
                }

                ui.separator();
                ui.label("搜索:");
                ui.add(
                    egui::TextEdit::singleline(&mut app.log_search_query)
                        .hint_text("按关键字过滤日志...")
                        .desired_width(180.0),
                );
                if !app.log_search_query.is_empty() && ui.small_button("×").clicked() {
                    app.log_search_query.clear();
                }
            });

            ui.separator();

            let query_lower = app.log_search_query.trim().to_lowercase();
            let has_filter = app.log_filter_level > 0 || !query_lower.is_empty();
            let filtered_indices: Option<Vec<usize>> = if has_filter {
                Some(
                    app.logs
                        .iter()
                        .enumerate()
                        .filter(|(_, line)| {
                            matches_log_filter(line, app.log_filter_level, &query_lower)
                        })
                        .map(|(idx, _)| idx)
                        .collect(),
                )
            } else {
                None
            };

            let visible_count = filtered_indices
                .as_ref()
                .map(|v| v.len())
                .unwrap_or(app.logs.len());

            if copy_clicked {
                if visible_count > 0 {
                    let text = match &filtered_indices {
                        Some(indices) => indices
                            .iter()
                            .map(|&idx| app.logs[idx].as_str())
                            .collect::<Vec<_>>()
                            .join("\n"),
                        None => app.logs.join("\n"),
                    };
                    ui.output_mut(|o| o.copied_text = text);
                    copied_count = Some(visible_count);
                } else {
                    copied_count = Some(0);
                }
            }

            // 动态填充窗口高度，无硬编码 300px 上限；保留 O(visible) 虚拟滚动
            let scroll_height = (ui.available_height() - 34.0).max(120.0);
            if visible_count == 0 {
                ui.allocate_ui(egui::vec2(ui.available_width(), scroll_height), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        let msg = if app.logs.is_empty() {
                            "暂无日志记录"
                        } else {
                            "没有匹配当前过滤条件的日志"
                        };
                        ui.label(RichText::new(msg).color(ui.visuals().weak_text_color()));
                    });
                });
            } else {
                let row_height = ui.text_style_height(&egui::TextStyle::Body);
                let dark_mode = ui.visuals().dark_mode;
                let default_color = ui.visuals().text_color();
                let weak_color = ui.visuals().weak_text_color();
                let logs = &app.logs;

                egui::ScrollArea::both()
                    .id_source("log_scroll")
                    .auto_shrink([false, false])
                    .stick_to_bottom(app.log_auto_scroll)
                    .max_height(scroll_height)
                    .show_rows(ui, row_height, visible_count, |ui, row_range| {
                        for row in row_range {
                            let line = match &filtered_indices {
                                Some(indices) => logs[indices[row]].as_str(),
                                None => logs[row].as_str(),
                            };
                            let color =
                                log_line_color(line, dark_mode, default_color, weak_color);
                            ui.add(
                                egui::Label::new(RichText::new(line).color(color)).wrap(false),
                            );
                        }
                    });
            }

            ui.separator();

            // 底部状态栏
            ui.horizontal(|ui| {
                if has_filter {
                    ui.label(format!("显示 {} / 共 {} 条日志", visible_count, app.logs.len()));
                } else {
                    ui.label(format!("共 {} 条日志", app.logs.len()));
                }
            });
        });

    if let Some(count) = copied_count {
        if count > 0 {
            app.show_success(format!("已复制 {} 条日志到剪贴板", count));
        } else {
            app.show_info("当前没有可复制的日志");
        }
    }

    app.show_log_window = window_open;
}

fn log_line_level(line: &str) -> u8 {
    if line.contains("] ERROR") {
        3
    } else if line.contains("] WARN") {
        2
    } else if line.contains("] DEBUG") || line.contains("] TRACE") {
        0
    } else {
        1
    }
}

fn matches_log_filter(line: &str, min_level: u8, query_lower: &str) -> bool {
    if min_level > 0 && log_line_level(line) < min_level {
        return false;
    }
    if !query_lower.is_empty() && !line.to_lowercase().contains(query_lower) {
        return false;
    }
    true
}

fn log_line_color(
    line: &str,
    dark_mode: bool,
    default_color: Color32,
    weak_color: Color32,
) -> Color32 {
    if line.contains("] ERROR") {
        if dark_mode {
            Color32::from_rgb(248, 113, 113)
        } else {
            Color32::from_rgb(200, 38, 38)
        }
    } else if line.contains("] WARN") {
        if dark_mode {
            Color32::from_rgb(251, 191, 36)
        } else {
            Color32::from_rgb(180, 100, 8)
        }
    } else if line.contains("] DEBUG") || line.contains("] TRACE") {
        weak_color
    } else {
        default_color
    }
}
