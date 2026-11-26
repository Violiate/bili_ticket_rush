use std::time::Duration;

use eframe::egui::{self, Color32, Image, RichText, Rounding, Stroke, Vec2};

use crate::app::{Myapp, PayQrItem};
use crate::windows::login_windows::create_qrcode_safe;
use crate::windows::{bounded_window_sizes, show_resizable_body_with};

#[derive(Clone, Copy)]
struct PayPalette {
    card_bg: Color32,
    card_border: Color32,
    sub_bg: Color32,
    sub_border: Color32,
    hero_bg: Color32,
    hero_border: Color32,
    emerald: Color32,
    emerald_soft: Color32,
    blue: Color32,
    blue_soft: Color32,
    pink: Color32,
    pink_soft: Color32,
    amber: Color32,
    amber_soft: Color32,
    red: Color32,
    red_soft: Color32,
    text_primary: Color32,
    text_secondary: Color32,
    text_muted: Color32,
    divider: Color32,
    btn_bg: Color32,
    btn_border: Color32,
    qr_frame_border: Color32,
    qr_corner: Color32,
}

impl PayPalette {
    fn from_dark_mode(dark: bool) -> Self {
        if dark {
            Self {
                card_bg: Color32::from_rgb(30, 33, 41),
                card_border: Color32::from_rgb(56, 62, 76),
                sub_bg: Color32::from_rgb(38, 42, 52),
                sub_border: Color32::from_rgb(62, 68, 84),
                hero_bg: Color32::from_rgb(20, 48, 36),
                hero_border: Color32::from_rgb(34, 128, 86),
                emerald: Color32::from_rgb(52, 211, 153),
                emerald_soft: Color32::from_rgb(22, 64, 44),
                blue: Color32::from_rgb(96, 165, 250),
                blue_soft: Color32::from_rgb(26, 48, 84),
                pink: Color32::from_rgb(251, 114, 153),
                pink_soft: Color32::from_rgb(76, 30, 46),
                amber: Color32::from_rgb(251, 191, 36),
                amber_soft: Color32::from_rgb(66, 48, 20),
                red: Color32::from_rgb(248, 113, 113),
                red_soft: Color32::from_rgb(66, 26, 30),
                text_primary: Color32::from_rgb(240, 243, 250),
                text_secondary: Color32::from_rgb(186, 193, 210),
                text_muted: Color32::from_rgb(134, 142, 162),
                divider: Color32::from_rgb(54, 60, 74),
                btn_bg: Color32::from_rgb(44, 49, 62),
                btn_border: Color32::from_rgb(74, 82, 102),
                qr_frame_border: Color32::from_rgb(80, 88, 108),
                qr_corner: Color32::from_rgb(52, 211, 153),
            }
        } else {
            Self {
                card_bg: Color32::from_rgb(255, 255, 255),
                card_border: Color32::from_rgb(220, 226, 238),
                sub_bg: Color32::from_rgb(246, 248, 252),
                sub_border: Color32::from_rgb(228, 233, 242),
                hero_bg: Color32::from_rgb(236, 253, 245),
                hero_border: Color32::from_rgb(167, 243, 208),
                emerald: Color32::from_rgb(5, 150, 105),
                emerald_soft: Color32::from_rgb(209, 250, 229),
                blue: Color32::from_rgb(37, 99, 235),
                blue_soft: Color32::from_rgb(228, 239, 255),
                pink: Color32::from_rgb(225, 65, 118),
                pink_soft: Color32::from_rgb(254, 232, 240),
                amber: Color32::from_rgb(180, 96, 10),
                amber_soft: Color32::from_rgb(254, 243, 199),
                red: Color32::from_rgb(225, 29, 72),
                red_soft: Color32::from_rgb(254, 226, 226),
                text_primary: Color32::from_rgb(22, 28, 42),
                text_secondary: Color32::from_rgb(72, 82, 102),
                text_muted: Color32::from_rgb(122, 132, 152),
                divider: Color32::from_rgb(232, 236, 244),
                btn_bg: Color32::from_rgb(246, 248, 252),
                btn_border: Color32::from_rgb(208, 216, 230),
                qr_frame_border: Color32::from_rgb(214, 222, 236),
                qr_corner: Color32::from_rgb(16, 185, 129),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PayChannelKind {
    WeChat,
    Alipay,
    UnifiedCashier,
    OrderDetailFallback,
}

pub(crate) fn classify_pay_url(url: &str) -> PayChannelKind {
    let lower = url.trim().to_ascii_lowercase();
    if lower.contains("orderdetail.html") {
        PayChannelKind::OrderDetailFallback
    } else if lower.starts_with("weixin://") || lower.contains("wxpay") {
        PayChannelKind::WeChat
    } else if lower.starts_with("alipays://") || lower.contains("alipay.com") {
        PayChannelKind::Alipay
    } else {
        PayChannelKind::UnifiedCashier
    }
}

pub(crate) fn summarize_url_for_display(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return "-".to_string();
    }
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let chars: Vec<char> = without_scheme.chars().collect();
    if chars.len() <= 40 {
        without_scheme.to_string()
    } else {
        let head: String = chars[..26].iter().collect();
        let tail: String = chars[chars.len() - 10..].iter().collect();
        format!("{}…{}", head, tail)
    }
}

struct PayModalActions {
    close_current: bool,
    next_order: bool,
    copied_pay_url: bool,
    copied_order_id: bool,
    open_in_browser: bool,
}

impl Default for PayModalActions {
    fn default() -> Self {
        Self {
            close_current: false,
            next_order: false,
            copied_pay_url: false,
            copied_order_id: false,
            open_in_browser: false,
        }
    }
}

pub fn show(app: &mut Myapp, ctx: &egui::Context) {
    let Some(item) = app.pending_pay_qrs.front().cloned() else {
        return;
    };

    // 支付等待计时器每秒刷新一次，确保超时提醒实时可见
    ctx.request_repaint_after(Duration::from_secs(1));

    let queue_len = app.pending_pay_qrs.len();
    let mut window_open = true;

    // 仅在 pay_url 变化时重新编码二维码，避免每帧重复编码与上传纹理
    let cached_matches = app
        .pay_qr_texture_cache
        .as_ref()
        .map(|(url, _)| url == &item.pay_url)
        .unwrap_or(false);

    let qr_result = if cached_matches {
        Ok(app.pay_qr_texture_cache.as_ref().unwrap().1.clone())
    } else {
        match create_qrcode_safe(ctx, "pay_qrcode", item.pay_url.as_str()) {
            Ok(tex) => {
                app.pay_qr_texture_cache = Some((item.pay_url.clone(), tex.clone()));
                Ok(tex)
            }
            Err(e) => {
                app.pay_qr_texture_cache = None;
                Err(e)
            }
        }
    };

    let actions = render_pay_window(ctx, &item, queue_len, &qr_result, &mut window_open);

    if actions.open_in_browser
        && (item.pay_url.starts_with("https://") || item.pay_url.starts_with("http://"))
    {
        ctx.open_url(egui::OpenUrl::new_tab(item.pay_url.clone()));
    }

    if actions.copied_pay_url {
        app.show_success("已复制支付链接");
    } else if actions.copied_order_id {
        app.show_success("已复制订单号");
    }

    if !window_open || actions.close_current {
        app.pending_pay_qrs.pop_front();
        app.pay_qr_texture_cache = None;
    } else if actions.next_order && app.pending_pay_qrs.len() > 1 {
        if let Some(front) = app.pending_pay_qrs.pop_front() {
            app.pending_pay_qrs.push_back(front);
        }
        app.pay_qr_texture_cache = None;
    }
}

fn render_pay_window(
    ctx: &egui::Context,
    item: &PayQrItem,
    queue_len: usize,
    qr_result: &Result<egui::TextureHandle, String>,
    window_open: &mut bool,
) -> PayModalActions {
    let mut actions = PayModalActions::default();
    let waited_secs = item.created_at.elapsed().as_secs();
    let palette = PayPalette::from_dark_mode(ctx.style().visuals.dark_mode);
    let channel = classify_pay_url(&item.pay_url);

    let (default_sz, min_sz, max_sz) = bounded_window_sizes(ctx, 700.0, 476.0, 440.0, 380.0);

    egui::Window::new("抢票成功 · 扫码支付")
        .open(window_open)
        .resizable(true)
        .constrain(true)
        .default_size(default_sz)
        .min_width(min_sz[0])
        .min_height(min_sz[1])
        .resize(|r| r.max_size(max_sz))
        .show(ctx, |ui| {
            show_resizable_body_with(
                ui,
                "pay_qrcode_scroll",
                &mut actions,
                |ui, acts| {
                    render_success_hero_header(ui, item, queue_len, waited_secs, &palette, acts);
                    ui.add_space(10.0);
                },
                |ui, acts| {
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("付款完成后可直接关闭，可在「全部订单」核对出票状态")
                                .size(11.5)
                                .color(palette.text_muted),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let close_btn = egui::Button::new(
                                RichText::new("完成并关闭")
                                    .size(13.0)
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(palette.emerald)
                            .stroke(Stroke::NONE)
                            .rounding(Rounding::same(7.0))
                            .min_size(Vec2::new(104.0, 30.0));
                            if ui.add(close_btn).clicked() {
                                acts.close_current = true;
                            }

                            if queue_len > 1 {
                                let next_btn = egui::Button::new(
                                    RichText::new("下一笔待支付")
                                        .size(12.5)
                                        .color(palette.text_primary),
                                )
                                .fill(palette.btn_bg)
                                .stroke(Stroke::new(1.0, palette.btn_border))
                                .rounding(Rounding::same(7.0))
                                .min_size(Vec2::new(100.0, 30.0));
                                if ui
                                    .add(next_btn)
                                    .on_hover_text("将当前订单移至队尾，查看下一笔待支付订单")
                                    .clicked()
                                {
                                    acts.next_order = true;
                                }
                            }
                        });
                    });
                },
                |ui, acts| {
                    let body_w = (ui.available_width() - 4.0).max(260.0);
                    if body_w >= 560.0 {
                        let gap = 12.0;
                        let right_w = (body_w * 0.43).clamp(244.0, 296.0);
                        let left_w = (body_w - right_w - gap).max(240.0);
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            ui.allocate_ui_with_layout(
                                Vec2::new(left_w, 0.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_min_width(left_w);
                                    ui.set_max_width(left_w);
                                    render_order_receipt_card(ui, item, &palette, acts);
                                },
                            );
                            ui.allocate_ui_with_layout(
                                Vec2::new(right_w, 0.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_min_width(right_w);
                                    ui.set_max_width(right_w);
                                    render_qr_stage_card(
                                        ui,
                                        channel,
                                        qr_result,
                                        &palette,
                                        196.0,
                                    );
                                },
                            );
                        });
                    } else {
                        // 窄窗自适应：优先展示二维码方便第一时间扫码，下方展示订单凭证
                        render_qr_stage_card(ui, channel, qr_result, &palette, 200.0);
                        ui.add_space(10.0);
                        render_order_receipt_card(ui, item, &palette, acts);
                    }
                    ui.add_space(4.0);
                },
            );
        });

    actions
}

fn render_success_hero_header(
    ui: &mut egui::Ui,
    item: &PayQrItem,
    queue_len: usize,
    waited_secs: u64,
    palette: &PayPalette,
    actions: &mut PayModalActions,
) {
    let (wait_bg, wait_fg, wait_suffix) = if waited_secs >= 420 {
        (palette.red_soft, palette.red, "请立即支付")
    } else if waited_secs >= 180 {
        (palette.amber_soft, palette.amber, "即将超时")
    } else {
        (palette.amber_soft, palette.amber, "建议尽快支付")
    };

    egui::Frame::none()
        .fill(palette.hero_bg)
        .stroke(Stroke::new(1.0, palette.hero_border))
        .rounding(Rounding::same(10.0))
        .inner_margin(egui::Margin::symmetric(14.0, 10.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let is_wide_header = ui.available_width() >= 520.0;
            ui.horizontal(|ui| {
                // 矢量绿色对勾徽章（跨平台清晰锐利，无 emoji 缺字）
                let (icon_rect, _) =
                    ui.allocate_exact_size(Vec2::splat(34.0), egui::Sense::hover());
                let painter = ui.painter();
                let center = icon_rect.center();
                painter.circle_filled(center, 16.0, palette.emerald);
                let p1 = center + Vec2::new(-6.2, 0.4);
                let p2 = center + Vec2::new(-1.5, 5.2);
                let p3 = center + Vec2::new(6.8, -4.8);
                let check_stroke = Stroke::new(2.5, Color32::WHITE);
                painter.line_segment([p1, p2], check_stroke);
                painter.line_segment([p2, p3], check_stroke);

                ui.add_space(4.0);

                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    ui.label(
                        RichText::new("下单成功！请尽快扫码完成支付")
                            .size(15.0)
                            .strong()
                            .color(palette.text_primary),
                    );
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                        render_pill(
                            ui,
                            &format!("账号 · {}", item.account_name),
                            palette.emerald_soft,
                            palette.emerald,
                        );
                        render_pill(
                            ui,
                            &format!("已等待 {} · {}", format_waited_secs(waited_secs), wait_suffix),
                            wait_bg,
                            wait_fg,
                        );
                    });
                });

                if queue_len > 1 && is_wide_header {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let next_small = egui::Button::new(
                            RichText::new("下一笔 >")
                                .size(12.0)
                                .strong()
                                .color(palette.blue),
                        )
                        .fill(palette.blue_soft)
                        .stroke(Stroke::new(1.0, palette.blue.linear_multiply(0.45)))
                        .rounding(Rounding::same(6.0));
                        if ui.add(next_small).clicked() {
                            actions.next_order = true;
                        }
                        render_pill(
                            ui,
                            &format!("待支付 1 / {}", queue_len),
                            palette.blue,
                            Color32::WHITE,
                        );
                    });
                }
            });
            if queue_len > 1 && !is_wide_header {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    render_pill(
                        ui,
                        &format!("待支付队列 1 / {}", queue_len),
                        palette.blue,
                        Color32::WHITE,
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let next_small = egui::Button::new(
                            RichText::new("查看下一笔 >")
                                .size(12.0)
                                .strong()
                                .color(palette.blue),
                        )
                        .fill(palette.blue_soft)
                        .stroke(Stroke::new(1.0, palette.blue.linear_multiply(0.45)))
                        .rounding(Rounding::same(6.0));
                        if ui.add(next_small).clicked() {
                            actions.next_order = true;
                        }
                    });
                });
            }
        });
}

fn render_order_receipt_card(
    ui: &mut egui::Ui,
    item: &PayQrItem,
    palette: &PayPalette,
    actions: &mut PayModalActions,
) {
    let yuan_text = format_price_yuan(item.pay_money_fen);
    let can_open_http =
        item.pay_url.starts_with("https://") || item.pay_url.starts_with("http://");

    egui::Frame::none()
        .fill(palette.card_bg)
        .stroke(Stroke::new(1.0, palette.card_border))
        .rounding(Rounding::same(12.0))
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 8.0;

            // 金额主视觉区
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("实付金额")
                        .size(12.0)
                        .color(palette.text_muted),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    render_pill(
                        ui,
                        &format!("共 {} 张", item.count.max(1)),
                        palette.pink_soft,
                        palette.pink,
                    );
                });
            });

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label(
                    RichText::new("¥")
                        .size(18.0)
                        .strong()
                        .color(palette.red),
                );
                ui.label(
                    RichText::new(yuan_text)
                        .size(30.0)
                        .strong()
                        .color(palette.red),
                );
            });

            draw_hairline_divider(ui, palette.divider);

            // 项目主标题
            ui.add(
                egui::Label::new(
                    RichText::new(if item.project_name.trim().is_empty() {
                        "B站会员购票务订单"
                    } else {
                        item.project_name.as_str()
                    })
                    .size(14.0)
                    .strong()
                    .color(palette.text_primary),
                )
                .wrap(true),
            );

            // 票务明细子卡
            egui::Frame::none()
                .fill(palette.sub_bg)
                .stroke(Stroke::new(1.0, palette.sub_border))
                .rounding(Rounding::same(8.0))
                .inner_margin(egui::Margin::symmetric(12.0, 10.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 6.0;

                    render_detail_row(ui, "场次", &item.screen_name, palette);
                    render_detail_row(
                        ui,
                        "票种",
                        &format!("{} × {} 张", item.ticket_name, item.count.max(1)),
                        palette,
                    );

                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [42.0, 18.0],
                            egui::Label::new(
                                RichText::new("订单号")
                                    .size(12.0)
                                    .color(palette.text_muted),
                            ),
                        );
                        let oid_text = if item.order_id.trim().is_empty() {
                            "-"
                        } else {
                            item.order_id.as_str()
                        };
                        ui.label(
                            RichText::new(oid_text)
                                .monospace()
                                .size(12.5)
                                .color(palette.text_primary),
                        );
                        if !item.order_id.trim().is_empty() {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let copy_oid = egui::Button::new(
                                        RichText::new("复制")
                                            .size(11.0)
                                            .color(palette.blue),
                                    )
                                    .fill(palette.blue_soft)
                                    .stroke(Stroke::NONE)
                                    .rounding(Rounding::same(5.0));
                                    if ui.add(copy_oid).clicked() {
                                        ui.output_mut(|o| {
                                            o.copied_text = item.order_id.clone()
                                        });
                                        actions.copied_order_id = true;
                                    }
                                },
                            );
                        }
                    });
                });

            ui.add_space(2.0);

            // 快捷操作按钮行（取代杂乱的长 URL 输入框）
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let avail = ui.available_width();
                let btn_w = if can_open_http {
                    ((avail - 8.0) / 2.0).max(96.0)
                } else {
                    avail.max(120.0)
                };

                let copy_url_btn = egui::Button::new(
                    RichText::new("复制支付链接")
                        .size(12.5)
                        .strong()
                        .color(palette.text_primary),
                )
                .fill(palette.btn_bg)
                .stroke(Stroke::new(1.0, palette.btn_border))
                .rounding(Rounding::same(7.0))
                .min_size(Vec2::new(btn_w, 30.0));
                if ui
                    .add(copy_url_btn)
                    .on_hover_text("复制完整支付链接到剪贴板")
                    .clicked()
                {
                    ui.output_mut(|o| o.copied_text = item.pay_url.clone());
                    actions.copied_pay_url = true;
                }

                if can_open_http {
                    let open_btn = egui::Button::new(
                        RichText::new("浏览器打开")
                            .size(12.5)
                            .strong()
                            .color(palette.blue),
                    )
                    .fill(palette.blue_soft)
                    .stroke(Stroke::new(1.0, palette.blue.linear_multiply(0.35)))
                    .rounding(Rounding::same(7.0))
                    .min_size(Vec2::new(btn_w, 30.0));
                    if ui
                        .add(open_btn)
                        .on_hover_text("在系统默认浏览器中直接打开支付页面")
                        .clicked()
                    {
                        actions.open_in_browser = true;
                    }
                }
            });

            // 精简域名预览，悬停可查看完整 URL
            ui.label(
                RichText::new(summarize_url_for_display(&item.pay_url))
                    .size(11.0)
                    .monospace()
                    .color(palette.text_muted),
            )
            .on_hover_text(item.pay_url.as_str());
        });
}

fn render_qr_stage_card(
    ui: &mut egui::Ui,
    channel: PayChannelKind,
    qr_result: &Result<egui::TextureHandle, String>,
    palette: &PayPalette,
    qr_size: f32,
) {
    let (badge_text, badge_bg, badge_fg, guide_title, guide_sub) = match channel {
        PayChannelKind::WeChat => (
            "微信扫码支付",
            palette.emerald_soft,
            palette.emerald,
            "请使用 微信 扫一扫",
            "扫码直达本笔订单付款",
        ),
        PayChannelKind::Alipay => (
            "支付宝扫码支付",
            palette.blue_soft,
            palette.blue,
            "请使用 支付宝 扫一扫",
            "扫码直达本笔订单付款",
        ),
        PayChannelKind::UnifiedCashier => (
            "官方扫码收银台",
            palette.blue_soft,
            palette.blue,
            "支持 微信 / 支付宝 扫码",
            "扫码直达 B站收银台完成支付",
        ),
        PayChannelKind::OrderDetailFallback => (
            "B站 App 扫码付款",
            palette.pink_soft,
            palette.pink,
            "请使用 哔哩哔哩 App 扫一扫",
            "该票种未下发直扫支付码，扫码直达订单详情付款",
        ),
    };

    egui::Frame::none()
        .fill(palette.card_bg)
        .stroke(Stroke::new(1.0, palette.card_border))
        .rounding(Rounding::same(12.0))
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                render_pill(ui, badge_text, badge_bg, badge_fg);
                ui.add_space(10.0);

                match qr_result {
                    Ok(texture) => {
                        let resp = egui::Frame::none()
                            .fill(Color32::WHITE)
                            .stroke(Stroke::new(1.2, palette.qr_frame_border))
                            .rounding(Rounding::same(12.0))
                            .inner_margin(egui::Margin::same(12.0))
                            .show(ui, |ui| {
                                ui.add(
                                    Image::new(texture)
                                        .fit_to_exact_size(Vec2::splat(qr_size)),
                                );
                            });

                        // 在白底扫码框四角绘制极简扫描取景角标，增强聚焦感与现代质感
                        draw_scanner_corners(ui.painter(), resp.response.rect, palette.qr_corner);
                    }
                    Err(err_msg) => {
                        egui::Frame::none()
                            .fill(palette.red_soft)
                            .stroke(Stroke::new(1.0, palette.red))
                            .rounding(Rounding::same(10.0))
                            .inner_margin(egui::Margin::same(14.0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("二维码生成失败")
                                        .size(13.5)
                                        .strong()
                                        .color(palette.red),
                                );
                                ui.add_space(4.0);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(err_msg)
                                            .size(11.5)
                                            .color(palette.text_secondary),
                                    )
                                    .wrap(true),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("请使用左侧「复制支付链接」或「浏览器打开」完成付款")
                                        .size(11.5)
                                        .color(palette.text_muted),
                                );
                            });
                    }
                }

                ui.add_space(10.0);
                ui.label(
                    RichText::new(guide_title)
                        .size(13.5)
                        .strong()
                        .color(palette.text_primary),
                );
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(guide_sub)
                            .size(11.5)
                            .color(palette.text_muted),
                    )
                    .wrap(true),
                );
            });
        });
}

fn render_detail_row(ui: &mut egui::Ui, label: &str, value: &str, palette: &PayPalette) {
    ui.horizontal_top(|ui| {
        ui.add_sized(
            [42.0, 18.0],
            egui::Label::new(RichText::new(label).size(12.0).color(palette.text_muted)),
        );
        ui.add(
            egui::Label::new(
                RichText::new(if value.trim().is_empty() { "-" } else { value })
                    .size(12.5)
                    .color(palette.text_primary),
            )
            .wrap(true),
        );
    });
}

fn render_pill(ui: &mut egui::Ui, text: &str, fill: Color32, text_color: Color32) {
    egui::Frame::none()
        .fill(fill)
        .rounding(Rounding::same(6.0))
        .inner_margin(egui::Margin::symmetric(8.0, 2.5))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .size(11.5)
                    .strong()
                    .color(text_color),
            );
        });
}

fn draw_hairline_divider(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), rect.center().y),
            egui::pos2(rect.right(), rect.center().y),
        ],
        Stroke::new(1.0, color),
    );
}

fn draw_scanner_corners(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    let r = rect.shrink(3.0);
    let len = 14.0;
    let stroke = Stroke::new(2.2, color);

    // 左上
    painter.line_segment([r.left_top(), r.left_top() + Vec2::new(len, 0.0)], stroke);
    painter.line_segment([r.left_top(), r.left_top() + Vec2::new(0.0, len)], stroke);
    // 右上
    painter.line_segment([r.right_top(), r.right_top() + Vec2::new(-len, 0.0)], stroke);
    painter.line_segment([r.right_top(), r.right_top() + Vec2::new(0.0, len)], stroke);
    // 左下
    painter.line_segment(
        [r.left_bottom(), r.left_bottom() + Vec2::new(len, 0.0)],
        stroke,
    );
    painter.line_segment(
        [r.left_bottom(), r.left_bottom() + Vec2::new(0.0, -len)],
        stroke,
    );
    // 右下
    painter.line_segment(
        [r.right_bottom(), r.right_bottom() + Vec2::new(-len, 0.0)],
        stroke,
    );
    painter.line_segment(
        [r.right_bottom(), r.right_bottom() + Vec2::new(0.0, -len)],
        stroke,
    );
}

fn format_price_yuan(fen: i64) -> String {
    let abs = fen.unsigned_abs();
    let yuan = abs / 100;
    let cents = abs % 100;
    if fen < 0 {
        format!("-{}.{:02}", yuan, cents)
    } else {
        format!("{}.{:02}", yuan, cents)
    }
}

fn format_waited_secs(secs: u64) -> String {
    if secs < 60 {
        format!("{} 秒", secs)
    } else {
        format!("{} 分 {} 秒", secs / 60, secs % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn pay_url_classification_and_summary() {
        assert_eq!(
            classify_pay_url("weixin://wxpay/bizpayurl?pr=abc123"),
            PayChannelKind::WeChat
        );
        assert_eq!(
            classify_pay_url("https://qr.alipay.com/bax0123456"),
            PayChannelKind::Alipay
        );
        assert_eq!(
            classify_pay_url("https://pay.bilibili.com/payplatform-h5/qrpay.html?payToken=xyz"),
            PayChannelKind::UnifiedCashier
        );
        assert_eq!(
            classify_pay_url("https://show.bilibili.com/platform/orderDetail.html?order_id=8001"),
            PayChannelKind::OrderDetailFallback
        );

        assert_eq!(format_price_yuan(9000), "90.00");
        assert_eq!(format_price_yuan(9950), "99.50");
        assert_eq!(format_price_yuan(5), "0.05");

        let short = summarize_url_for_display("https://qr.alipay.com/bax123");
        assert_eq!(short, "qr.alipay.com/bax123");
        let long = summarize_url_for_display(
            "https://pay.bilibili.com/payplatform-h5/qrpay.html?codeType=0&payToken=abcdef1234567890",
        );
        assert!(long.contains('…') && long.chars().count() < 42);
    }

    #[test]
    fn pay_qr_window_layout_stays_stable_in_wide_and_narrow_viewports() {
        let item = PayQrItem {
            account_name: "测试账号".to_string(),
            project_name: "北京·ICOS国际动漫节10th超长活动名称测试换行稳定性".to_string(),
            screen_name: "2027年1月1日".to_string(),
            ticket_name: "预售票（9:30进场）".to_string(),
            count: 2,
            order_id: "8001478555289364".to_string(),
            pay_url: "https://pay.bilibili.com/payplatform-h5/qrpay.html?payToken=test123456"
                .to_string(),
            pay_money_fen: 18000,
            created_at: Instant::now(),
        };

        for (vp_w, vp_h) in [(1280.0_f32, 800.0_f32), (520.0_f32, 600.0_f32)] {
            let ctx = egui::Context::default();
            let tex = create_qrcode_safe(&ctx, "test_pay_qr", &item.pay_url)
                .expect("test qr texture should render");
            let qr_res = Ok(tex);
            let mut open = true;
            let mut widths = Vec::new();

            for _ in 0..8 {
                let raw_input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(vp_w, vp_h),
                    )),
                    ..Default::default()
                };
                let _ = ctx.run(raw_input, |ctx| {
                    let _ = render_pay_window(ctx, &item, 2, &qr_res, &mut open);
                    if let Some(rect) = ctx.memory(|m| {
                        m.area_rect(egui::Id::new("抢票成功 · 扫码支付"))
                    }) {
                        widths.push(rect.width());
                    }
                });
            }

            assert!(widths.len() >= 2, "window rect should be recorded");
            let last_w = *widths.last().unwrap();
            let prev_w = widths[widths.len() - 2];
            assert!(
                last_w <= vp_w,
                "window width {} must not exceed viewport {}",
                last_w,
                vp_w
            );
            assert!(
                (last_w - prev_w).abs() < 1.0,
                "window width must converge without drift: {:?} (vp={}x{})",
                widths,
                vp_w,
                vp_h
            );
        }
    }
}
