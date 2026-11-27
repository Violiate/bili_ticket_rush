use crate::app::{BannerKind, Myapp};
use crate::windows::confirm_ticket::{
    draw_vector_checkmark, draw_vector_exclamation, draw_vector_info,
};
use eframe::egui;

pub fn render_notification_banner(app: &Myapp, ctx: &egui::Context) {
    let Some(banner) = &app.active_banner else {
        return;
    };

    let alpha = (banner.opacity.clamp(0.0, 1.0) * 235.0) as u8;
    if alpha == 0 {
        return;
    }

    let text_alpha = (banner.opacity.clamp(0.0, 1.0) * 255.0) as u8;
    let screen_rect = ctx.screen_rect();
    let banner_width = (screen_rect.width() - 40.0).clamp(240.0, 760.0);
    let pos_x = screen_rect.center().x - banner_width / 2.0;
    let pos_y = screen_rect.top() + 12.0;

    let (fill_color, stroke_color) = match banner.kind {
        BannerKind::Success => (
            egui::Color32::from_rgba_unmultiplied(22, 163, 74, alpha),
            egui::Color32::from_rgba_unmultiplied(21, 128, 61, alpha),
        ),
        BannerKind::Error => (
            egui::Color32::from_rgba_unmultiplied(220, 56, 52, alpha),
            egui::Color32::from_rgba_unmultiplied(185, 45, 35, alpha),
        ),
        BannerKind::Info => (
            egui::Color32::from_rgba_unmultiplied(45, 136, 245, alpha),
            egui::Color32::from_rgba_unmultiplied(37, 99, 235, alpha),
        ),
    };
    let fg_white = egui::Color32::from_rgba_unmultiplied(255, 255, 255, text_alpha);
    let badge_bg = egui::Color32::from_rgba_unmultiplied(255, 255, 255, (alpha as u16 * 48 / 255) as u8);

    // 限制横幅显示长度，避免超长文本遮挡整个窗口
    let display_text = truncate_banner_text(&banner.text, 160);

    egui::Area::new("notification_banner")
        .order(egui::Order::Foreground)
        .interactable(false)
        .fixed_pos(egui::pos2(pos_x, pos_y))
        .show(ctx, |ui| {
            let frame = egui::Frame::none()
                .fill(fill_color)
                .stroke(egui::Stroke::new(1.0, stroke_color))
                .rounding(egui::Rounding::same(9.0))
                .inner_margin(egui::Margin::symmetric(14.0, 9.0));

            frame.show(ui, |ui| {
                ui.set_min_width(banner_width);
                ui.set_max_width(banner_width);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let (icon_rect, _) =
                        ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                    let painter = ui.painter();
                    painter.circle_filled(icon_rect.center(), 9.0, badge_bg);
                    match banner.kind {
                        BannerKind::Success => {
                            draw_vector_checkmark(painter, icon_rect.center(), 7.5, 2.0, fg_white);
                        }
                        BannerKind::Error => {
                            draw_vector_exclamation(painter, icon_rect.center(), 7.5, fg_white);
                        }
                        BannerKind::Info => {
                            draw_vector_info(painter, icon_rect.center(), 7.5, fg_white);
                        }
                    }
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(display_text)
                                .color(fg_white)
                                .size(14.0)
                                .strong(),
                        )
                        .wrap(true),
                    );
                });
            });
        });
}

fn truncate_banner_text(text: &str, max_chars: usize) -> String {
    let single_line = text.replace(['\r', '\n'], " ");
    let mut chars = single_line.chars();
    let collected: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", collected)
    } else {
        collected
    }
}

#[cfg(test)]
mod tests {
    use super::truncate_banner_text;

    #[test]
    fn banner_truncation_is_utf8_safe_and_single_line() {
        let s = "中文错误信息\n第二行内容";
        assert_eq!(truncate_banner_text(s, 50), "中文错误信息 第二行内容");
        assert_eq!(truncate_banner_text("一二三四五六", 4), "一二三四...");
    }
}
