
use eframe::egui;
use crate::app::Myapp;

pub fn show(app: &mut Myapp, ctx: &egui::Context) {

    let mut window_open = app.show_log_window;
    let mut user_close = false;

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
                        // 标记关闭，循环外统一回写，避免与 .open() 的可变借用冲突
                        user_close = true;
                    }
                });
            });

            ui.separator();

            // 当前状态（放在滚动区外，保持常驻可见）
            ui.label(format!("当前状态: {}",
                if app.running_status.is_empty() { "未知状态" } else { &app.running_status }));

            ui.separator();

            // 日志内容区域
            // 关键性能修复：原实现每帧为全部 5000 条日志各创建一个 Label + Separator，
            // 抢票时横幅持续触发 60fps 重绘，导致严重卡顿。改用 show_rows 行虚拟化，
            // 仅渲染可见行；并去掉逐行 separator（show_rows 要求行高一致）。
            if app.logs.is_empty() {
                ui.label("暂无日志记录");
            } else {
                let row_height = ui.text_style_height(&egui::TextStyle::Body);
                let total_rows = app.logs.len();
                let logs = &app.logs;
                egui::ScrollArea::both()
                    .id_source("log_scroll")
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .max_height(300.0)
                    .show_rows(ui, row_height, total_rows, |ui, row_range| {
                        for row in row_range {
                            // 单行不换行：保证行高一致，长行可横向滚动查看
                            ui.add(egui::Label::new(logs[row].as_str()).wrap(false));
                        }
                    });
            }

            ui.separator();

            // 底部状态栏
            ui.label(format!("共 {} 条日志", app.logs.len()));
        });

    // 更新窗口状态（标题栏 X 经 window_open 生效；自定义 ❌ 经 user_close 生效）
    app.show_log_window = window_open && !user_close;
}
