use crate::app::Myapp;
use eframe::egui;

pub fn render_sidebar(app: &mut Myapp, ctx: &egui::Context) {
    let running_count = app.task_manager.list_running_tasks().len();

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

            let spacing_y = ui.spacing().item_spacing.y;
            let item_count = tabs.len() as f32;
            let total_spacing = spacing_y * (item_count - 1.0).max(0.0);
            let available_height = (ui.available_height() - total_spacing - 4.0).max(160.0);
            let item_height = (available_height / item_count).max(40.0);

            let dark_mode = ctx.style().visuals.dark_mode;
            let selected_text_color = if dark_mode {
                egui::Color32::WHITE
            } else {
                egui::Color32::from_rgb(15, 75, 160)
            };

            for (idx, (icon, name)) in tabs.iter().enumerate() {
                let is_selected = app.selected_tab == idx;
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), item_height),
                    egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                    |ui| {
                        let mut text =
                            egui::RichText::new(format!("{} {}", icon, name)).size(16.0);
                        if is_selected {
                            text = text.strong().color(selected_text_color);
                        }
                        ui.selectable_value(&mut app.selected_tab, idx, text);
                    },
                );
            }
        });
}
