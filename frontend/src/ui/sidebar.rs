use crate::app::Myapp;
use crate::windows::log_windows::is_success_log;
use eframe::egui::{self, Color32, RichText, Stroke};

pub fn render_sidebar(app: &mut Myapp, ctx: &egui::Context) {
    let running_count = app.task_manager.list_running_tasks().len();
    let total_logs = app.logs.len();
    let success_log_count = app.logs.iter().filter(|line| is_success_log(line)).count();
    let error_log_count = app
        .logs
        .iter()
        .rev()
        .take(200)
        .filter(|line| line.contains("] ERROR"))
        .count();

    egui::SidePanel::left("left_panel")
        .resizable(true)
        .default_width(app.left_panel_width)
        .width_range(160.0..=320.0)
        .show(ctx, |ui| {
            let monitor_label = if running_count > 0 {
                format!("监视面板 ({})", running_count)
            } else {
                "监视面板".to_string()
            };
            let settings_label = if app.settings_dirty || app.settings_save_error.is_some() {
                "设置/微调 *".to_string()
            } else {
                "设置/微调".to_string()
            };

            let tabs = [
                ("😎", "开始抢票".to_string()),
                ("🎫", monitor_label),
                ("👤", "账号管理".to_string()),
                ("🔧", settings_label),
                ("📋", "帮助/关于".to_string()),
            ];

            let dark_mode = ctx.style().visuals.dark_mode;
            let selected_text_color = if dark_mode {
                Color32::WHITE
            } else {
                Color32::from_rgb(15, 75, 160)
            };

            // 预留底部快捷区（分割线 + 醒目的实时日志入口按钮 + 版本号）
            let footer_reserve = 86.0;
            let spacing_y = ui.spacing().item_spacing.y;
            let item_count = tabs.len() as f32;
            let total_spacing = spacing_y * (item_count - 1.0).max(0.0);
            let available_height =
                (ui.available_height() - footer_reserve - total_spacing - 4.0).max(180.0);
            let item_height = (available_height / item_count).clamp(38.0, 88.0);

            for (idx, (icon, name)) in tabs.iter().enumerate() {
                let is_selected = app.selected_tab == idx;
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), item_height),
                    egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                    |ui| {
                        let mut text = RichText::new(format!("{} {}", icon, name)).size(16.0);
                        if is_selected {
                            text = text.strong().color(selected_text_color);
                        }
                        ui.selectable_value(&mut app.selected_tab, idx, text);
                    },
                );
            }

            // 底部常驻实时日志醒目入口（在任意页面都一键可达）
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("v{}", app.display_version))
                        .size(11.5)
                        .color(ui.visuals().weak_text_color()),
                );
                ui.add_space(4.0);

                let (btn_fill, btn_stroke, text_color) = if success_log_count > 0 {
                    (
                        Color32::from_rgb(16, 152, 104),
                        Stroke::new(1.5, Color32::from_rgb(52, 211, 153)),
                        Color32::WHITE,
                    )
                } else if app.show_log_window {
                    (
                        Color32::from_rgb(16, 152, 104),
                        Stroke::new(1.5, Color32::from_rgb(52, 211, 153)),
                        Color32::WHITE,
                    )
                } else if error_log_count > 0 {
                    (
                        Color32::from_rgb(217, 72, 68),
                        Stroke::new(1.5, Color32::from_rgb(252, 165, 165)),
                        Color32::WHITE,
                    )
                } else if running_count > 0 {
                    (
                        Color32::from_rgb(37, 122, 235),
                        Stroke::new(1.5, Color32::from_rgb(120, 180, 255)),
                        Color32::WHITE,
                    )
                } else if dark_mode {
                    (
                        Color32::from_rgb(42, 64, 102),
                        Stroke::new(1.2, Color32::from_rgb(76, 134, 224)),
                        Color32::from_rgb(228, 240, 255),
                    )
                } else {
                    (
                        Color32::from_rgb(232, 242, 255),
                        Stroke::new(1.2, Color32::from_rgb(59, 130, 246)),
                        Color32::from_rgb(22, 86, 184),
                    )
                };

                let log_title = if success_log_count > 0 {
                    format!("✅ 实时日志 · 成功 {} 单", success_log_count)
                } else if app.show_log_window {
                    "📜 实时日志 · 已开启".to_string()
                } else if error_log_count > 0 {
                    format!("📜 实时日志 · {} 条报错", error_log_count)
                } else if running_count > 0 {
                    format!("📜 实时日志 · 运行中({})", running_count)
                } else if total_logs > 0 {
                    format!("📜 打开实时日志 ({})", total_logs)
                } else {
                    "📜 打开实时日志".to_string()
                };

                let log_btn = egui::Button::new(
                    RichText::new(log_title)
                        .size(13.5)
                        .strong()
                        .color(text_color),
                )
                .fill(btn_fill)
                .stroke(btn_stroke)
                .rounding(9.0)
                .min_size(egui::vec2(ui.available_width().max(120.0), 38.0));

                if ui
                    .add(log_btn)
                    .on_hover_text("打开/隐藏实时日志监视窗口，实时查看抢票进度、倒计时与接口报错详情")
                    .clicked()
                {
                    app.show_log_window = !app.show_log_window;
                }

                ui.add_space(6.0);
                ui.separator();
            });
        });
}
