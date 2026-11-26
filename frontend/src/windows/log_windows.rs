use eframe::egui::{self, Color32, RichText};

use crate::app::Myapp;
use crate::windows::{bounded_window_sizes, show_resizable_frame_with};

pub const LOG_FILTER_ALL: u8 = 0;
pub const LOG_FILTER_INFO_PLUS: u8 = 1;
pub const LOG_FILTER_WARN_PLUS: u8 = 2;
pub const LOG_FILTER_ERROR: u8 = 3;
pub const LOG_FILTER_SUCCESS: u8 = 4;

pub fn is_success_log(line: &str) -> bool {
    line.contains("] SUCCESS")
}

pub fn show(app: &mut Myapp, ctx: &egui::Context) {
    let mut window_open = app.show_log_window;
    if !window_open {
        return;
    }

    let running_count = app.task_manager.list_running_tasks().len();
    let mut copy_clicked = false;
    let mut copied_count: Option<usize> = None;

    let (default_sz, min_sz, max_sz) = bounded_window_sizes(ctx, 740.0, 480.0, 480.0, 320.0);

    egui::Window::new("日志监视")
        .open(&mut window_open)
        .default_size(default_sz)
        .min_width(min_sz[0])
        .min_height(min_sz[1])
        .resize(|r| r.max_size(max_sz))
        .constrain(true)
        .resizable(true)
        .show(ctx, |ui| {
            let query_lower = app.log_search_query.trim().to_lowercase();
            let has_filter = app.log_filter_level != LOG_FILTER_ALL || !query_lower.is_empty();

            let mut success_count: usize = 0;
            let mut error_count: usize = 0;
            for line in &app.logs {
                if is_success_log(line) {
                    success_count += 1;
                } else if line.contains("] ERROR") {
                    error_count += 1;
                }
            }

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
            let total_logs = app.logs.len();

            let mut shared = (&mut *app, &mut copy_clicked);
            show_resizable_frame_with(
                ui,
                &mut shared,
                |ui, (app, copy_btn)| {
                    let dark_mode = ui.visuals().dark_mode;
                    let success_fg = if dark_mode {
                        Color32::from_rgb(52, 211, 153)
                    } else {
                        Color32::from_rgb(21, 128, 61)
                    };
                    let success_bg = if dark_mode {
                        Color32::from_rgb(20, 62, 42)
                    } else {
                        Color32::from_rgb(220, 252, 231)
                    };
                    let error_fg = if dark_mode {
                        Color32::from_rgb(248, 113, 113)
                    } else {
                        Color32::from_rgb(200, 38, 38)
                    };

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

                        if success_count > 0 {
                            ui.separator();
                            let pill = egui::Button::new(
                                RichText::new(format!("✅ 已成功下单 {} 笔", success_count))
                                    .strong()
                                    .size(12.5)
                                    .color(success_fg),
                            )
                            .fill(success_bg)
                            .rounding(8.0);
                            if ui
                                .add(pill)
                                .on_hover_text("点击仅查看下单成功记录")
                                .clicked()
                            {
                                app.log_filter_level = LOG_FILTER_SUCCESS;
                            }
                        }

                        ui.separator();

                        ui.checkbox(&mut app.log_auto_scroll, "自动滚动");

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("清空日志").clicked() {
                                app.logs.clear();
                            }
                            if ui.button("复制当前日志").clicked() {
                                **copy_btn = true;
                            }
                        });
                    });

                    // 分类过滤（含独立 SUCCESS 下单成功分类）+ 关键字搜索栏
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut app.log_filter_level, LOG_FILTER_ALL, "全部");

                        let success_tab_label = if success_count > 0 {
                            format!("✅ SUCCESS ({})", success_count)
                        } else {
                            "✅ SUCCESS".to_string()
                        };
                        ui.selectable_value(
                            &mut app.log_filter_level,
                            LOG_FILTER_SUCCESS,
                            RichText::new(success_tab_label).strong().color(success_fg),
                        )
                        .on_hover_text("仅显示已核验的下单成功日志");

                        ui.selectable_value(
                            &mut app.log_filter_level,
                            LOG_FILTER_INFO_PLUS,
                            "INFO+",
                        );
                        ui.selectable_value(
                            &mut app.log_filter_level,
                            LOG_FILTER_WARN_PLUS,
                            "WARN+",
                        );

                        let err_tab_label = if error_count > 0 {
                            format!("ERROR ({})", error_count)
                        } else {
                            "ERROR".to_string()
                        };
                        let err_text = if error_count > 0 {
                            RichText::new(err_tab_label).color(error_fg)
                        } else {
                            RichText::new(err_tab_label)
                        };
                        ui.selectable_value(&mut app.log_filter_level, LOG_FILTER_ERROR, err_text);

                        ui.separator();
                        ui.label("搜索:");
                        ui.add(
                            egui::TextEdit::singleline(&mut app.log_search_query)
                                .hint_text("按关键字过滤日志...")
                                .desired_width(160.0),
                        );
                        if !app.log_search_query.is_empty() && ui.small_button("×").clicked() {
                            app.log_search_query.clear();
                        }
                    });

                    ui.separator();
                },
                |ui, _| {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        if has_filter {
                            ui.label(format!("显示 {} / 共 {} 条日志", visible_count, total_logs));
                        } else {
                            ui.label(format!("共 {} 条日志", total_logs));
                        }
                        if success_count > 0 {
                            let success_fg = if ui.visuals().dark_mode {
                                Color32::from_rgb(52, 211, 153)
                            } else {
                                Color32::from_rgb(21, 128, 61)
                            };
                            ui.separator();
                            ui.label(
                                RichText::new(format!("✅ 下单成功: {} 笔", success_count))
                                    .strong()
                                    .color(success_fg),
                            );
                        }
                        if error_count > 0 {
                            let error_fg = if ui.visuals().dark_mode {
                                Color32::from_rgb(248, 113, 113)
                            } else {
                                Color32::from_rgb(200, 38, 38)
                            };
                            ui.separator();
                            ui.label(
                                RichText::new(format!("ERROR: {} 条", error_count)).color(error_fg),
                            );
                        }
                    });
                },
                |ui, (app, copy_btn)| {
                    if **copy_btn {
                        if visible_count > 0 && !app.logs.is_empty() {
                            let text = match &filtered_indices {
                                Some(indices) => indices
                                    .iter()
                                    .filter_map(|&idx| app.logs.get(idx).map(String::as_str))
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

                    let actual_visible = if app.logs.is_empty() { 0 } else { visible_count };
                    if actual_visible == 0 {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            let msg = if app.logs.is_empty() {
                                "暂无日志记录"
                            } else if app.log_filter_level == LOG_FILTER_SUCCESS
                                && query_lower.is_empty()
                            {
                                "暂无下单成功记录"
                            } else {
                                "没有匹配当前过滤条件的日志"
                            };
                            ui.label(RichText::new(msg).color(ui.visuals().weak_text_color()));
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
                            .min_scrolled_height(0.0)
                            .stick_to_bottom(app.log_auto_scroll)
                            .show_rows(ui, row_height, actual_visible, |ui, row_range| {
                                for row in row_range {
                                    let line = match &filtered_indices {
                                        Some(indices) => indices
                                            .get(row)
                                            .and_then(|&idx| logs.get(idx))
                                            .map(String::as_str)
                                            .unwrap_or(""),
                                        None => logs.get(row).map(String::as_str).unwrap_or(""),
                                    };
                                    let color =
                                        log_line_color(line, dark_mode, default_color, weak_color);
                                    let mut text = RichText::new(line).color(color);
                                    if is_success_log(line) {
                                        text = text.strong();
                                    }
                                    ui.add(egui::Label::new(text).wrap(false));
                                }
                            });
                    }
                },
            );
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
        LOG_FILTER_ERROR
    } else if line.contains("] WARN") {
        LOG_FILTER_WARN_PLUS
    } else if line.contains("] DEBUG") || line.contains("] TRACE") {
        LOG_FILTER_ALL
    } else {
        // INFO 与 SUCCESS 均属于 INFO+ 级别
        LOG_FILTER_INFO_PLUS
    }
}

pub(crate) fn matches_log_filter(line: &str, filter_mode: u8, query_lower: &str) -> bool {
    let category_ok = match filter_mode {
        LOG_FILTER_ALL => true,
        LOG_FILTER_SUCCESS => is_success_log(line),
        level => log_line_level(line) >= level,
    };
    if !category_ok {
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
    if is_success_log(line) {
        if dark_mode {
            Color32::from_rgb(52, 211, 153)
        } else {
            Color32::from_rgb(21, 128, 61)
        }
    } else if line.contains("] ERROR") {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_log_category_filters_accurately_without_false_positives() {
        let success_line = "[2026-11-26 19:39:05.123] SUCCESS [#1 测试账号]: 下单成功！项目: BW | 订单号: 999";
        let info_success_word = "[2026-11-26 19:39:01.000] INFO [#1 测试账号]: 获取抢票token成功！";
        let warn_line = "[2026-11-26 19:39:02.000] WARN: 触发B站 412 风控拦截";
        let err_line = "[2026-11-26 19:39:03.000] ERROR: 创建订单网络失败";
        let debug_line = "[2026-11-26 19:39:00.000] DEBUG: 调试日志";

        assert!(is_success_log(success_line));
        assert!(!is_success_log(info_success_word));
        assert!(!is_success_log(warn_line));
        assert!(!is_success_log(err_line));

        // SUCCESS 专属分类只展示真正下单成功的 SUCCESS 日志
        assert!(matches_log_filter(success_line, LOG_FILTER_SUCCESS, ""));
        assert!(!matches_log_filter(info_success_word, LOG_FILTER_SUCCESS, ""));
        assert!(!matches_log_filter(warn_line, LOG_FILTER_SUCCESS, ""));
        assert!(!matches_log_filter(err_line, LOG_FILTER_SUCCESS, ""));
        assert!(matches_log_filter(success_line, LOG_FILTER_SUCCESS, "bw"));
        assert!(!matches_log_filter(success_line, LOG_FILTER_SUCCESS, "cp30"));

        // INFO+ 仍包含 SUCCESS / INFO / WARN / ERROR，过滤掉 DEBUG
        assert!(matches_log_filter(success_line, LOG_FILTER_INFO_PLUS, ""));
        assert!(matches_log_filter(info_success_word, LOG_FILTER_INFO_PLUS, ""));
        assert!(!matches_log_filter(debug_line, LOG_FILTER_INFO_PLUS, ""));

        // WARN+ 与 ERROR 不会混入 SUCCESS
        assert!(!matches_log_filter(success_line, LOG_FILTER_WARN_PLUS, ""));
        assert!(matches_log_filter(warn_line, LOG_FILTER_WARN_PLUS, ""));
        assert!(matches_log_filter(err_line, LOG_FILTER_ERROR, ""));
        assert!(!matches_log_filter(success_line, LOG_FILTER_ERROR, ""));
    }
}
