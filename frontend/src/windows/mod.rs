use eframe::egui;

pub mod add_buyer;
pub mod confirm_ticket;
pub mod confirm_ticket2;
pub mod log_windows;
pub mod login_windows;
pub mod screen_info;
pub mod show_orderlist;
pub mod show_qrcode;

/// 根据当前主窗口可视区尺寸计算弹出窗口的安全 `(default_size, min_size, max_size)`，
/// 保证弹窗既能在全屏/取消全屏时自动约束在屏幕内，又能自由手动拖拽放大或缩小。
pub fn bounded_window_sizes(
    ctx: &egui::Context,
    default_w: f32,
    default_h: f32,
    min_w: f32,
    min_h: f32,
) -> ([f32; 2], [f32; 2], [f32; 2]) {
    let screen = ctx.screen_rect();
    // Window::resize.max_size 约束的是内容区尺寸，需额外扣除标题栏与窗体边距（约 36px）
    let max_w = (screen.width() - 32.0).max(340.0);
    let max_h = (screen.height() - 64.0).max(260.0);
    let eff_min_w = min_w.min(max_w);
    let eff_min_h = min_h.min(max_h);
    let eff_def_w = default_w.clamp(eff_min_w, max_w);
    let eff_def_h = default_h.clamp(eff_min_h, max_h);
    ([eff_def_w, eff_def_h], [eff_min_w, eff_min_h], [max_w, max_h])
}

/// 在 `egui::Window` 内部提供“固定顶栏 + 自适应中间区 + 固定底栏”的零回弹可缩放骨架。
///
/// 原理说明：
/// - `egui 0.23` 的 `Window` 在 `Resize::end` 中直接以 `content_ui.min_size()` 作为最终绘制尺寸；
/// - 若内容小于 `desired_size`，拖大窗口会立刻回弹；若硬编码估算底栏高度偏小，拖小窗口会被每帧顶回去；
/// - 本函数先在 `content_ui` 上精确分配一次 `available_size()`，再以独立 `child_ui` 分区渲染顶栏、
///   中间滚动区与 `top_down` 底栏（底栏高度按帧精确测量并缓存，同 `egui::TopBottomPanel::bottom`），
///   做到 0px 误差、任意方向拖拽均不回弹、大按钮与换行提示绝不溢出底边，且窗口缩小时顶栏/底栏按钮始终可见。
pub fn show_resizable_frame(
    ui: &mut egui::Ui,
    header: impl FnOnce(&mut egui::Ui),
    footer: impl FnOnce(&mut egui::Ui),
    center: impl FnOnce(&mut egui::Ui),
) {
    let mut unit = ();
    show_resizable_frame_with(
        ui,
        &mut unit,
        |ui, _| header(ui),
        |ui, _| footer(ui),
        |ui, _| center(ui),
    );
}

/// 带共享可变状态引用的零回弹三段式可缩放骨架，便于顶栏/底栏/主区共用 `&mut Myapp` 或事件标志。
/// 注意：`header`、`center`、`footer` 均采用自然的 `top_down` 布局。
pub fn show_resizable_frame_with<S>(
    ui: &mut egui::Ui,
    state: &mut S,
    header: impl FnOnce(&mut egui::Ui, &mut S),
    footer: impl FnOnce(&mut egui::Ui, &mut S),
    center: impl FnOnce(&mut egui::Ui, &mut S),
) {
    let available = ui.available_rect_before_wrap();
    let safe_size = egui::vec2(available.width().max(120.0), available.height().max(120.0));

    // 在 Window 的 content_ui 上仅精确分配一次目标尺寸，使 content_ui.min_size() 严格等于 desired_size：
    // 1. 拖大窗口时不会被缩回（min_size 填满 desired_size）
    // 2. 拖小窗口时内部子区域通过独立的 child_ui 排版，绝不会反推撑大 content_ui.min_size()
    let (_id, outer_rect) = ui.allocate_space(safe_size);
    let clip_margin = ui.visuals().clip_rect_margin;

    // 1. 顶栏区（top_down）
    let mut root_ui = ui.child_ui_with_id_source(
        outer_rect,
        egui::Layout::top_down(egui::Align::Min),
        "resizable_frame_root",
    );
    root_ui.set_clip_rect(outer_rect.expand(clip_margin).intersect(ui.clip_rect()));
    header(&mut root_ui, state);

    // 2. 顶栏之后的剩余区域按精确测量的底栏高度划分为 center_rect + footer_rect
    let rem_min_y = root_ui.cursor().min.y.clamp(outer_rect.min.y, outer_rect.max.y);
    let rem_rect = egui::Rect::from_min_max(
        egui::pos2(outer_rect.min.x, rem_min_y),
        outer_rect.max,
    );

    let footer_h_id = ui.id().with("resizable_frame_footer_h");
    let cached_footer_h = ui
        .ctx()
        .data(|d| d.get_temp::<f32>(footer_h_id))
        .unwrap_or(0.0)
        .clamp(0.0, rem_rect.height());

    let split_y = (rem_rect.max.y - cached_footer_h).max(rem_rect.min.y);
    let center_rect = egui::Rect::from_min_max(
        rem_rect.min,
        egui::pos2(rem_rect.max.x, split_y),
    );
    let footer_rect = egui::Rect::from_min_max(
        egui::pos2(rem_rect.min.x, split_y),
        rem_rect.max,
    );

    // 3. 中间内容区（如 ScrollArea），严格垂直裁剪在 [rem_min_y .. split_y] 内
    if center_rect.width() > 1.0 && center_rect.height() > 1.0 {
        let mut center_ui = root_ui.child_ui_with_id_source(
            center_rect,
            egui::Layout::top_down(egui::Align::Min),
            "resizable_frame_center",
        );
        center_ui.set_clip_rect(
            center_rect
                .expand2(egui::vec2(clip_margin, 0.0))
                .intersect(root_ui.clip_rect()),
        );
        center(&mut center_ui, state);
    }

    // 4. 底栏区（自然 top_down，先铺设窗口实色背景兜底，再测量真实像素高度）
    let mut footer_ui = root_ui.child_ui_with_id_source(
        footer_rect,
        egui::Layout::top_down(egui::Align::Min),
        "resizable_frame_footer",
    );
    footer_ui.set_clip_rect(
        footer_rect
            .expand2(egui::vec2(clip_margin, 2.0))
            .intersect(root_ui.clip_rect()),
    );
    if footer_rect.is_positive() {
        footer_ui
            .painter()
            .rect_filled(footer_rect, 0.0, footer_ui.visuals().window_fill());
    }
    footer(&mut footer_ui, state);

    let actual_footer_h = footer_ui.min_rect().height().max(0.0);
    if (actual_footer_h - cached_footer_h).abs() > 0.5 {
        ui.ctx().data_mut(|d| d.insert_temp(footer_h_id, actual_footer_h));
        ui.ctx().request_repaint();
    }
}

/// 标准三段式可缩放弹窗内容：固定顶栏 + 铺满剩余空间的垂直 `ScrollArea` + 固定底栏。
/// `header`、`footer` 与 `body` 均采用自然的 `top_down` 布局。
pub fn show_resizable_body(
    ui: &mut egui::Ui,
    scroll_id: impl std::hash::Hash,
    header: impl FnOnce(&mut egui::Ui),
    footer: impl FnOnce(&mut egui::Ui),
    body: impl FnOnce(&mut egui::Ui),
) {
    let mut unit = ();
    show_resizable_body_with(
        ui,
        scroll_id,
        &mut unit,
        |ui, _| header(ui),
        |ui, _| footer(ui),
        |ui, _| body(ui),
    );
}

pub fn show_resizable_body_with<S>(
    ui: &mut egui::Ui,
    scroll_id: impl std::hash::Hash,
    state: &mut S,
    header: impl FnOnce(&mut egui::Ui, &mut S),
    footer: impl FnOnce(&mut egui::Ui, &mut S),
    body: impl FnOnce(&mut egui::Ui, &mut S),
) {
    show_resizable_frame_with(ui, state, header, footer, |ui, state| {
        let strict_center_clip = ui.clip_rect();
        egui::ScrollArea::vertical()
            .id_source(scroll_id)
            .auto_shrink([false, false])
            .min_scrolled_height(0.0)
            .show(ui, |ui| {
                // 消除 egui 0.23 ScrollArea 默认向底栏方向外扩 3px clip_rect_margin 的越界绘制
                let clamped = ui.clip_rect().intersect(strict_center_clip);
                ui.set_clip_rect(clamped);
                body(ui, state);
            });
    });
}

#[cfg(test)]
mod tests {
    use super::{bounded_window_sizes, show_resizable_body};
    use eframe::egui;

    fn run_resizable_window_frame(
        ctx: &egui::Context,
        screen_size: egui::Vec2,
        events: Vec<egui::Event>,
    ) -> (egui::Rect, egui::Rect, egui::Rect) {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen_size)),
            events,
            ..Default::default()
        };
        let mut win_rect = egui::Rect::NOTHING;
        let mut footer_btn_rect = egui::Rect::NOTHING;
        let mut scroll_rect = egui::Rect::NOTHING;

        let _ = ctx.run(raw, |ctx| {
            let (def_sz, min_sz, max_sz) = bounded_window_sizes(ctx, 700.0, 560.0, 480.0, 360.0);
            let resp = egui::Window::new("缩放测试弹窗")
                .id(egui::Id::new("resizable_helper_test_win"))
                .default_pos(egui::pos2(100.0, 60.0))
                .default_size(def_sz)
                .min_width(min_sz[0])
                .min_height(min_sz[1])
                .resize(|r| r.max_size(max_sz))
                .constrain(true)
                .resizable(true)
                .collapsible(false)
                .show(ctx, |ui| {
                    show_resizable_body(
                        ui,
                        "resizable_test_scroll",
                        |ui| {
                            ui.label("顶部固定标题栏");
                            ui.separator();
                        },
                        |ui| {
                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(4.0);
                            ui.add(
                                egui::Label::new(
                                    "状态提示文本，窄窗口下即使换行也绝不能顶回窗口高度或挤出按钮",
                                )
                                .wrap(true),
                            );
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let r = ui.add(
                                            egui::Button::new("确认购票")
                                                .min_size(egui::vec2(100.0, 34.0)),
                                        );
                                        footer_btn_rect = r.rect;
                                    },
                                );
                            });
                        },
                        |ui| {
                            scroll_rect = ui.clip_rect();
                            let card_w = (ui.available_width() - 6.0).max(240.0);
                            for i in 0..12 {
                                egui::Frame::none()
                                    .inner_margin(egui::Margin::symmetric(10.0, 8.0))
                                    .show(ui, |ui| {
                                        ui.set_width((card_w - 22.0).max(200.0));
                                        ui.label(format!("测试卡片项目 #{}", i + 1));
                                    });
                            }
                        },
                    );
                });
            if let Some(inner) = resp {
                win_rect = inner.response.rect;
            }
        });

        (win_rect, footer_btn_rect, scroll_rect)
    }

    fn simulate_drag(
        ctx: &egui::Context,
        screen_size: egui::Vec2,
        from: egui::Pos2,
        to: egui::Pos2,
    ) {
        let _ = run_resizable_window_frame(ctx, screen_size, vec![egui::Event::PointerMoved(from)]);
        let _ = run_resizable_window_frame(
            ctx,
            screen_size,
            vec![
                egui::Event::PointerMoved(from),
                egui::Event::PointerButton {
                    pos: from,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for step in 1..=6 {
            let t = step as f32 / 6.0;
            let pos = from.lerp(to, t);
            let _ =
                run_resizable_window_frame(ctx, screen_size, vec![egui::Event::PointerMoved(pos)]);
        }
        let _ = run_resizable_window_frame(
            ctx,
            screen_size,
            vec![
                egui::Event::PointerMoved(to),
                egui::Event::PointerButton {
                    pos: to,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }

    #[test]
    fn manual_edge_drag_in_fullscreen_expands_and_shrinks_without_snapback() {
        let ctx = egui::Context::default();
        let fullscreen = egui::vec2(1920.0, 1080.0);

        // 先稳定渲染 10 帧，确认 0px 自动飘移（绝不自动撞到全屏上限）
        let mut rect = egui::Rect::NOTHING;
        let mut frame_sizes = Vec::new();
        for _ in 0..10 {
            let (r, b, s) = run_resizable_window_frame(&ctx, fullscreen, vec![]);
            frame_sizes.push((r.size(), b.size(), s.size()));
            rect = r;
        }
        let initial_size = rect.size();
        let frame2_size = frame_sizes[2].0;
        assert!(
            (initial_size.x - frame2_size.x).abs() < 0.5
                && (initial_size.y - frame2_size.y).abs() < 0.5,
            "window size must stay completely stable across idle frames: frame2={:?}, frame9={:?}, frames={:?}",
            frame2_size,
            initial_size,
            frame_sizes
        );
        assert!(
            initial_size.x < 760.0 && initial_size.y < 640.0,
            "initial window should stay near default size instead of hitting fullscreen max: {:?}, frames={:?}",
            initial_size,
            frame_sizes
        );

        // 1. 全屏模式下手动拖拽右下角放大到约 1180 x 820
        let corner_start = rect.right_bottom() - egui::vec2(2.0, 2.0);
        let corner_expanded = rect.left_top() + egui::vec2(1180.0, 820.0);
        simulate_drag(&ctx, fullscreen, corner_start, corner_expanded);

        // 松开鼠标后再跑 10 帧，绝不能回弹缩小或继续撑大
        let mut expanded_rect = egui::Rect::NOTHING;
        let mut btn_rect = egui::Rect::NOTHING;
        let mut scroll_rect = egui::Rect::NOTHING;
        let mut after_expand_first = egui::Vec2::ZERO;
        for i in 0..10 {
            let (r, b, s) = run_resizable_window_frame(&ctx, fullscreen, vec![]);
            if i == 1 {
                after_expand_first = r.size();
            }
            expanded_rect = r;
            btn_rect = b;
            scroll_rect = s;
        }
        assert!(
            (expanded_rect.width() - after_expand_first.x).abs() < 0.5
                && (expanded_rect.height() - after_expand_first.y).abs() < 0.5,
            "expanded window drifted after mouse release: first={:?}, final={:?}",
            after_expand_first,
            expanded_rect.size()
        );
        assert!(
            (expanded_rect.width() - 1180.0).abs() < 25.0
                && (expanded_rect.height() - 820.0).abs() < 25.0,
            "expanded window snapped back instead of staying near 1180x820: {:?}",
            expanded_rect.size()
        );
        assert!(
            expanded_rect.contains_rect(btn_rect),
            "footer button must remain inside window bounds after expand"
        );
        assert!(
            scroll_rect.max.y <= btn_rect.min.y + 1.0,
            "scroll area must not overlap footer button after expand: scroll={:?}, btn={:?}",
            scroll_rect,
            btn_rect
        );

        // 2. 全屏模式下手动拖拽右下角缩小到约 560 x 430，绝不能被顶回去
        let corner_now = expanded_rect.right_bottom() - egui::vec2(2.0, 2.0);
        let corner_shrunk = expanded_rect.left_top() + egui::vec2(560.0, 430.0);
        simulate_drag(&ctx, fullscreen, corner_now, corner_shrunk);

        let mut shrunk_rect = egui::Rect::NOTHING;
        let mut shrunk_btn_rect = egui::Rect::NOTHING;
        let mut shrunk_scroll_rect = egui::Rect::NOTHING;
        let mut after_shrink_first = egui::Vec2::ZERO;
        for i in 0..10 {
            let (r, b, s) = run_resizable_window_frame(&ctx, fullscreen, vec![]);
            if i == 1 {
                after_shrink_first = r.size();
            }
            shrunk_rect = r;
            shrunk_btn_rect = b;
            shrunk_scroll_rect = s;
        }
        assert!(
            (shrunk_rect.width() - after_shrink_first.x).abs() < 0.5
                && (shrunk_rect.height() - after_shrink_first.y).abs() < 0.5,
            "shrunk window drifted after mouse release: first={:?}, final={:?}",
            after_shrink_first,
            shrunk_rect.size()
        );
        assert!(
            (shrunk_rect.width() - 560.0).abs() < 25.0
                && (shrunk_rect.height() - 430.0).abs() < 25.0,
            "shrunk window got pushed back instead of staying near 560x430: {:?}",
            shrunk_rect.size()
        );
        assert!(
            shrunk_rect.contains_rect(shrunk_btn_rect),
            "footer button must remain visible inside shrunk window: win={:?}, btn={:?}",
            shrunk_rect,
            shrunk_btn_rect
        );
        assert!(
            shrunk_scroll_rect.height() > 80.0
                && shrunk_scroll_rect.max.y <= shrunk_btn_rect.min.y + 1.0,
            "scroll area must remain scrollable and non-overlapping after shrink: scroll={:?}, btn={:?}",
            shrunk_scroll_rect,
            shrunk_btn_rect
        );

        // 3. 单边拖拽：单独拖右边框拉宽至 ~900，高度应保持不变
        let right_edge_start = egui::pos2(shrunk_rect.right() - 1.0, shrunk_rect.center().y);
        let right_edge_end = egui::pos2(shrunk_rect.left() + 900.0, shrunk_rect.center().y);
        simulate_drag(&ctx, fullscreen, right_edge_start, right_edge_end);
        let mut wide_rect = egui::Rect::NOTHING;
        for _ in 0..6 {
            wide_rect = run_resizable_window_frame(&ctx, fullscreen, vec![]).0;
        }
        assert!(
            (wide_rect.width() - 900.0).abs() < 25.0
                && (wide_rect.height() - shrunk_rect.height()).abs() < 20.0,
            "right-edge drag should widen without altering height: before={:?}, after={:?}",
            shrunk_rect.size(),
            wide_rect.size()
        );

        // 4. 单边拖拽：单独拖底边框拉高至 ~720，宽度应保持不变
        let bottom_edge_start = egui::pos2(wide_rect.center().x, wide_rect.bottom() - 1.0);
        let bottom_edge_end = egui::pos2(wide_rect.center().x, wide_rect.top() + 720.0);
        simulate_drag(&ctx, fullscreen, bottom_edge_start, bottom_edge_end);
        let mut tall_rect = egui::Rect::NOTHING;
        for _ in 0..6 {
            tall_rect = run_resizable_window_frame(&ctx, fullscreen, vec![]).0;
        }
        assert!(
            (tall_rect.width() - wide_rect.width()).abs() < 20.0
                && (tall_rect.height() - 720.0).abs() < 25.0,
            "bottom-edge drag should change height without altering width: before={:?}, after={:?}",
            wide_rect.size(),
            tall_rect.size()
        );

        // 5. 从全屏切回小窗口（取消全屏）：弹窗应自动约束进新视口且底栏仍完整可见
        let unmaximized = egui::vec2(820.0, 520.0);
        let mut clamped_rect = egui::Rect::NOTHING;
        let mut clamped_btn = egui::Rect::NOTHING;
        for _ in 0..6 {
            let (r, b, _) = run_resizable_window_frame(&ctx, unmaximized, vec![]);
            clamped_rect = r;
            clamped_btn = b;
        }
        assert!(
            clamped_rect.width() <= unmaximized.x + 2.0
                && clamped_rect.height() <= unmaximized.y + 2.0,
            "window must clamp into unmaximized viewport: win={:?}, screen={:?}",
            clamped_rect.size(),
            unmaximized
        );
        assert!(
            clamped_rect.contains_rect(clamped_btn),
            "footer button must remain visible after unmaximize clamp: win={:?}, btn={:?}",
            clamped_rect,
            clamped_btn
        );

        // 6. 再次切回全屏并拖拽边缘：仍能自由放大/缩小且不回弹
        let mut after_remax = egui::Rect::NOTHING;
        for _ in 0..4 {
            after_remax = run_resizable_window_frame(&ctx, fullscreen, vec![]).0;
        }
        let remax_corner = after_remax.right_bottom() - egui::vec2(2.0, 2.0);
        let remax_target = after_remax.left_top() + egui::vec2(1040.0, 680.0);
        simulate_drag(&ctx, fullscreen, remax_corner, remax_target);
        let mut final_rect = egui::Rect::NOTHING;
        for _ in 0..6 {
            final_rect = run_resizable_window_frame(&ctx, fullscreen, vec![]).0;
        }
        assert!(
            (final_rect.width() - 1040.0).abs() < 25.0
                && (final_rect.height() - 680.0).abs() < 25.0,
            "manual drag after re-maximizing should work smoothly: {:?}",
            final_rect.size()
        );
    }
}
