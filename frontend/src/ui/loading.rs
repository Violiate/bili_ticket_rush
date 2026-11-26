use crate::app::Myapp;
use eframe::egui;

pub fn render_loading_overlay(app: &mut Myapp, ctx: &egui::Context) {
    let screen_rect = ctx.screen_rect();

    // 使用帧率无关的 dt 推进加载动画
    let dt = ctx.input(|i| i.unstable_dt).clamp(0.0, 0.1);
    app.loading_angle = (app.loading_angle + dt * 4.5) % std::f32::consts::TAU;

    // 使用可交互的 Foreground Area 拦截所有底层点击与输入事件
    egui::Area::new("loading_overlay")
        .order(egui::Order::Foreground)
        .fixed_pos(screen_rect.min)
        .show(ctx, |ui| {
            let (_rect, _resp) = ui.allocate_exact_size(screen_rect.size(), egui::Sense::click());
            let painter = ui.painter();

            // 半透明背景遮罩
            painter.rect_filled(
                screen_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 175),
            );

            let center = screen_rect.center();

            // 背景圆环
            painter.circle_stroke(
                center,
                30.0,
                egui::Stroke::new(5.0, egui::Color32::from_gray(100)),
            );

            // 动画圆弧
            let segments = 32;
            let start_angle = app.loading_angle;
            let end_angle = start_angle + std::f32::consts::PI;
            let mut points = Vec::with_capacity(segments + 1);
            for i in 0..=segments {
                let angle = start_angle + (end_angle - start_angle) * (i as f32 / segments as f32);
                points.push(center + 30.0 * egui::Vec2::new(angle.cos(), angle.sin()));
            }
            painter.add(egui::Shape::line(
                points,
                egui::Stroke::new(5.0, egui::Color32::from_rgb(66, 150, 250)),
            ));

            // 加载提示文字
            let label = if app.loading_text.trim().is_empty() {
                "加载中..."
            } else {
                app.loading_text.as_str()
            };
            painter.text(
                center + egui::vec2(0.0, 52.0),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(16.0),
                egui::Color32::WHITE,
            );
        });

    ctx.request_repaint_after(std::time::Duration::from_millis(16));
}
