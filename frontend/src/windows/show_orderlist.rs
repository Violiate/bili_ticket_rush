use eframe::egui::{self, Color32, Image, RichText, Stroke};

use crate::app::{ImageLoadState, Myapp};

const ORDER_PAGE_SIZE: usize = 10;

pub fn show(app: &mut Myapp, ctx: &egui::Context) {
    let mut window_open = app.show_orderlist_window.is_some();
    if !window_open {
        return;
    }

    let uid_str = app.show_orderlist_window.clone().unwrap_or_default();
    let uid = uid_str.parse::<i64>().unwrap_or(0);
    let account_name = app
        .account_manager
        .accounts
        .iter()
        .find(|a| a.uid == uid)
        .map(|a| a.name.clone())
        .unwrap_or_else(|| format!("UID {}", uid));

    let current_orders = app
        .total_order_data
        .as_ref()
        .filter(|d| d.account_id == uid_str && d.page == app.orderlist_page)
        .and_then(|d| d.data.clone());

    let total_orders = current_orders
        .as_ref()
        .map(|d| d.data.total.max(0) as usize)
        .or_else(|| {
            app.total_order_data
                .as_ref()
                .filter(|d| d.account_id == uid_str)
                .and_then(|d| d.data.as_ref())
                .map(|d| d.data.total.max(0) as usize)
        })
        .unwrap_or(0);
    let total_pages = ((total_orders + ORDER_PAGE_SIZE - 1) / ORDER_PAGE_SIZE).max(1);
    let current_page = app.orderlist_page;

    let mut page_to_request: Option<usize> = None;
    let mut images_to_request: Vec<(String, String)> = Vec::new();
    let mut copied_order_id: Option<String> = None;

    egui::Window::new("订单列表")
        .open(&mut window_open)
        .default_height(620.0)
        .default_width(840.0)
        .resizable(true)
        .show(ctx, |ui| {
            // 顶部信息与刷新栏
            ui.horizontal_wrapped(|ui| {
                ui.heading("订单列表");
                ui.separator();
                ui.label(
                    RichText::new(format!("账号: {} ({})", account_name, uid)).strong(),
                );
                if current_orders.is_some() {
                    ui.separator();
                    ui.label(
                        RichText::new(format!("共 {} 条订单", total_orders))
                            .color(ui.visuals().weak_text_color()),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(!app.orderlist_requesting, egui::Button::new("刷新"))
                        .clicked()
                    {
                        page_to_request = Some(current_page);
                    }
                    if app.orderlist_requesting {
                        ui.label(
                            RichText::new("正在加载...")
                                .color(ui.visuals().weak_text_color()),
                        );
                        ui.spinner();
                    }
                });
            });

            ui.separator();

            // 错误提示卡片
            if let Some(ref err_msg) = app.orderlist_error {
                egui::Frame::none()
                    .fill(if ui.visuals().dark_mode {
                        Color32::from_rgb(60, 24, 24)
                    } else {
                        Color32::from_rgb(254, 242, 242)
                    })
                    .stroke(Stroke::new(1.0, Color32::from_rgb(220, 68, 55)))
                    .rounding(8.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(format!("获取订单列表失败: {}", err_msg))
                                    .color(Color32::from_rgb(220, 68, 55)),
                            );
                            if ui
                                .add_enabled(!app.orderlist_requesting, egui::Button::new("重试"))
                                .clicked()
                            {
                                page_to_request = Some(current_page);
                            }
                        });
                    });
                ui.add_space(8.0);
            }

            // 主订单滚动区域（为底部分页栏预留空间）
            let scroll_max_height = (ui.available_height() - 48.0).max(160.0);
            egui::ScrollArea::vertical()
                .id_source("orderlist_scroll")
                .max_height(scroll_max_height)
                .show(ui, |ui| {
                    if let Some(ref order_resp) = current_orders {
                        if order_resp.data.list.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(60.0);
                                let empty_msg = if current_page > 0 {
                                    "当前页暂无订单记录"
                                } else {
                                    "暂无订单记录"
                                };
                                ui.label(
                                    RichText::new(empty_msg)
                                        .size(16.0)
                                        .color(ui.visuals().weak_text_color()),
                                );
                            });
                        } else {
                            let card_bg = if ui.visuals().dark_mode {
                                Color32::from_rgb(32, 35, 42)
                            } else {
                                Color32::from_rgb(248, 249, 252)
                            };
                            let detail_bg = if ui.visuals().dark_mode {
                                Color32::from_rgb(24, 27, 33)
                            } else {
                                Color32::from_rgb(240, 242, 248)
                            };

                            for order in &order_resp.data.list {
                                ui.add_space(8.0);

                                egui::Frame::none()
                                    .fill(card_bg)
                                    .rounding(8.0)
                                    .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                                    .inner_margin(egui::vec2(12.0, 12.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            // 缩略图区域（通过统一异步图片缓存，无阻塞、无重试风暴）
                                            let image_size = egui::vec2(80.0, 80.0);
                                            let raw_img_url = if !order.img.url.trim().is_empty() {
                                                order.img.url.as_str()
                                            } else {
                                                order.item_info.image.as_deref().unwrap_or("")
                                            };

                                            ui.add_sized(image_size, |ui: &mut egui::Ui| {
                                                if let Some(thumb_url) =
                                                    normalize_order_thumb_url(raw_img_url)
                                                {
                                                    let cache_key = format!("order:{}", thumb_url);
                                                    match app.image_cache.get(&cache_key) {
                                                        Some(ImageLoadState::Loaded(tex)) => ui
                                                            .centered_and_justified(|ui| {
                                                                ui.add(
                                                                    Image::new(tex)
                                                                        .fit_to_exact_size(
                                                                            image_size,
                                                                        ),
                                                                )
                                                            })
                                                            .inner,
                                                        Some(ImageLoadState::Loading) => ui
                                                            .centered_and_justified(|ui| {
                                                                ui.spinner()
                                                            })
                                                            .inner,
                                                        Some(ImageLoadState::Failed) => ui
                                                            .centered_and_justified(|ui| {
                                                                ui.label(
                                                                    RichText::new("暂无图片")
                                                                        .size(12.0)
                                                                        .color(
                                                                            ui.visuals()
                                                                                .weak_text_color(),
                                                                        ),
                                                                )
                                                            })
                                                            .inner,
                                                        None => {
                                                            images_to_request
                                                                .push((cache_key, thumb_url));
                                                            ui.centered_and_justified(|ui| {
                                                                ui.spinner()
                                                            })
                                                            .inner
                                                        }
                                                    }
                                                } else {
                                                    ui.centered_and_justified(|ui| {
                                                        ui.label(
                                                            RichText::new("无封面")
                                                                .size(12.0)
                                                                .color(
                                                                    ui.visuals().weak_text_color(),
                                                                ),
                                                        )
                                                    })
                                                    .inner
                                                }
                                            });

                                            ui.add_space(12.0);

                                            // 订单主体信息
                                            ui.vertical(|ui| {
                                                ui.horizontal(|ui| {
                                                    ui.add(
                                                        egui::Label::new(
                                                            RichText::new(&order.item_info.name)
                                                                .size(15.5)
                                                                .strong(),
                                                        )
                                                        .wrap(true),
                                                    );

                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            let status_color = match order.status {
                                                                2 => Color32::from_rgb(34, 197, 94),
                                                                1 => {
                                                                    Color32::from_rgb(245, 158, 11)
                                                                }
                                                                4 => {
                                                                    Color32::from_rgb(220, 68, 55)
                                                                }
                                                                _ => ui.visuals().weak_text_color(),
                                                            };
                                                            ui.label(
                                                                RichText::new(
                                                                    &order.sub_status_name,
                                                                )
                                                                .color(status_color)
                                                                .strong(),
                                                            );
                                                        },
                                                    );
                                                });

                                                ui.add_space(4.0);

                                                ui.horizontal_wrapped(|ui| {
                                                    ui.label(
                                                        RichText::new("订单号:")
                                                            .color(ui.visuals().weak_text_color()),
                                                    );
                                                    ui.monospace(&order.order_id);
                                                });

                                                ui.horizontal_wrapped(|ui| {
                                                    ui.label(
                                                        RichText::new("场次:")
                                                            .color(ui.visuals().weak_text_color()),
                                                    );
                                                    ui.label(&order.item_info.screen_name);
                                                });

                                                ui.horizontal_wrapped(|ui| {
                                                    ui.label(
                                                        RichText::new("下单时间:")
                                                            .color(ui.visuals().weak_text_color()),
                                                    );
                                                    ui.label(&order.ctime);
                                                });

                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        RichText::new("实付:")
                                                            .color(ui.visuals().weak_text_color()),
                                                    );
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "¥{:.2}",
                                                            order.pay_money as f64 / 100.0
                                                        ))
                                                        .strong()
                                                        .color(Color32::from_rgb(239, 68, 68)),
                                                    );

                                                    if let Some(ref channel) = order.pay_channel {
                                                        if !channel.trim().is_empty() {
                                                            ui.label(
                                                                RichText::new(format!(
                                                                    "({})",
                                                                    channel
                                                                ))
                                                                .color(
                                                                    ui.visuals().weak_text_color(),
                                                                ),
                                                            );
                                                        }
                                                    }

                                                    // 右侧操作按钮
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            let is_expanded = app
                                                                .orderlist_detail_id
                                                                .as_deref()
                                                                == Some(order.order_id.as_str());
                                                            let toggle_text = if is_expanded {
                                                                "收起详情"
                                                            } else {
                                                                "查看详情"
                                                            };
                                                            if ui.button(toggle_text).clicked() {
                                                                if is_expanded {
                                                                    app.orderlist_detail_id = None;
                                                                } else {
                                                                    app.orderlist_detail_id = Some(
                                                                        order.order_id.clone(),
                                                                    );
                                                                }
                                                            }

                                                            if order.status == 1 {
                                                                let copy_btn = egui::Button::new(
                                                                    RichText::new(
                                                                        "待支付 · 复制订单号",
                                                                    )
                                                                    .color(Color32::WHITE),
                                                                )
                                                                .fill(Color32::from_rgb(
                                                                    245, 158, 11,
                                                                ));
                                                                if ui
                                                                    .add(copy_btn)
                                                                    .on_hover_text(
                                                                        "复制订单号后，请前往哔哩哔哩 App -> 会员购 -> 订单中心完成支付",
                                                                    )
                                                                    .clicked()
                                                                {
                                                                    ui.output_mut(|o| {
                                                                        o.copied_text =
                                                                            order.order_id.clone();
                                                                    });
                                                                    copied_order_id =
                                                                        Some(order.order_id.clone());
                                                                }
                                                            }
                                                        },
                                                    );
                                                });
                                            });
                                        });

                                        // 展开的订单详情面板
                                        if app.orderlist_detail_id.as_deref()
                                            == Some(order.order_id.as_str())
                                        {
                                            ui.add_space(8.0);
                                            egui::Frame::none()
                                                .fill(detail_bg)
                                                .rounding(6.0)
                                                .inner_margin(10.0)
                                                .show(ui, |ui| {
                                                    egui::Grid::new(format!(
                                                        "order_detail_{}",
                                                        order.order_id
                                                    ))
                                                    .num_columns(2)
                                                    .spacing([16.0, 6.0])
                                                    .show(ui, |ui| {
                                                        ui.label(
                                                            RichText::new("订单编号:")
                                                                .color(ui.visuals().weak_text_color()),
                                                        );
                                                        ui.horizontal(|ui| {
                                                            ui.monospace(&order.order_id);
                                                            if ui.small_button("复制订单号").clicked()
                                                            {
                                                                ui.output_mut(|o| {
                                                                    o.copied_text =
                                                                        order.order_id.clone();
                                                                });
                                                                copied_order_id =
                                                                    Some(order.order_id.clone());
                                                            }
                                                        });
                                                        ui.end_row();

                                                        ui.label(
                                                            RichText::new("项目ID / 场次ID:")
                                                                .color(ui.visuals().weak_text_color()),
                                                        );
                                                        ui.label(format!(
                                                            "{} / {}",
                                                            order.item_id,
                                                            order.item_info.screen_id
                                                        ));
                                                        ui.end_row();

                                                        ui.label(
                                                            RichText::new("场次名称:")
                                                                .color(ui.visuals().weak_text_color()),
                                                        );
                                                        ui.label(&order.item_info.screen_name);
                                                        ui.end_row();

                                                        if !order
                                                            .item_info
                                                            .screen_start_time
                                                            .trim()
                                                            .is_empty()
                                                        {
                                                            ui.label(
                                                                RichText::new("演出时间:")
                                                                    .color(
                                                                        ui.visuals()
                                                                            .weak_text_color(),
                                                                    ),
                                                            );
                                                            let range = if order
                                                                .item_info
                                                                .screen_end_time
                                                                .trim()
                                                                .is_empty()
                                                            {
                                                                order
                                                                    .item_info
                                                                    .screen_start_time
                                                                    .clone()
                                                            } else {
                                                                format!(
                                                                    "{} ~ {}",
                                                                    order.item_info.screen_start_time,
                                                                    order.item_info.screen_end_time
                                                                )
                                                            };
                                                            ui.label(range);
                                                            ui.end_row();
                                                        }

                                                        ui.label(
                                                            RichText::new("数量 / 总金额 / 实付:")
                                                                .color(ui.visuals().weak_text_color()),
                                                        );
                                                        ui.label(format!(
                                                            "{} 张  |  总金额 ¥{:.2}  |  实付 ¥{:.2}",
                                                            order.count,
                                                            order.total_money as f64 / 100.0,
                                                            order.pay_money as f64 / 100.0
                                                        ));
                                                        ui.end_row();

                                                        ui.label(
                                                            RichText::new("创建时间:")
                                                                .color(ui.visuals().weak_text_color()),
                                                        );
                                                        ui.label(&order.ctime);
                                                        ui.end_row();

                                                        ui.label(
                                                            RichText::new("支付状态 / 订单类型:")
                                                                .color(ui.visuals().weak_text_color()),
                                                        );
                                                        ui.label(format!(
                                                            "{} (status={}, sub={})  |  {}",
                                                            order.sub_status_name,
                                                            order.status,
                                                            order.sub_status,
                                                            order_type_label(order.order_type)
                                                        ));
                                                        ui.end_row();
                                                    });

                                                    if order.status == 1 {
                                                        ui.add_space(6.0);
                                                        ui.label(
                                                            RichText::new(
                                                                "提示：该订单尚未支付，请打开哔哩哔哩 App -> 我的 -> 会员购 -> 订单中心及时付款。",
                                                            )
                                                            .size(12.5)
                                                            .color(Color32::from_rgb(217, 119, 6)),
                                                        );
                                                    }
                                                });
                                        }
                                    });
                            }
                        }
                    } else if app.orderlist_requesting {
                        ui.vertical_centered(|ui| {
                            ui.add_space(60.0);
                            ui.spinner();
                            ui.add_space(10.0);
                            ui.label("正在获取订单列表...");
                        });
                    } else if app.orderlist_error.is_none() {
                        ui.vertical_centered(|ui| {
                            ui.add_space(60.0);
                            ui.label(
                                RichText::new("暂无订单数据，请点击右上角刷新")
                                    .color(ui.visuals().weak_text_color()),
                            );
                        });
                    }
                    ui.add_space(6.0);
                });

            ui.separator();

            // 底部分页控制栏
            ui.horizontal(|ui| {
                let can_prev = !app.orderlist_requesting && current_page > 0;
                let can_next = !app.orderlist_requesting
                    && (current_page + 1) * ORDER_PAGE_SIZE < total_orders;

                if ui.add_enabled(can_prev, egui::Button::new("上一页")).clicked() {
                    page_to_request = Some(current_page - 1);
                }

                ui.label(format!("第 {} / {} 页", current_page + 1, total_pages));

                if ui.add_enabled(can_next, egui::Button::new("下一页")).clicked() {
                    page_to_request = Some(current_page + 1);
                }

                if total_orders > 0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("每页 {} 条 · 共 {} 条", ORDER_PAGE_SIZE, total_orders))
                                .color(ui.visuals().weak_text_color()),
                        );
                    });
                }
            });
        });

    for (key, url) in images_to_request {
        app.request_image(&key, &url);
    }

    if let Some(oid) = copied_order_id {
        app.show_success(format!("已复制订单号: {}", oid));
    }

    if let Some(target_page) = page_to_request {
        if uid > 0 {
            app.orderlist_error = None;
            app.request_orderlist(uid, target_page);
        }
    }

    if !window_open {
        app.show_orderlist_window = None;
        app.orderlist_requesting = false;
        app.orderlist_active_task_id = None;
        app.orderlist_error = None;
        app.orderlist_detail_id = None;
        app.orderlist_need_reload = false;
        app.clear_order_thumbnails();
    }
}

fn order_type_label(order_type: i32) -> String {
    match order_type {
        1 => "普通票务订单".to_string(),
        2 => "周边商品订单".to_string(),
        other => format!("类型 {}", other),
    }
}

/// 规范化订单缩略图 URL：补全协议头，且仅在不含 `?` 与 `@` 时追加缩略图后缀
pub(crate) fn normalize_order_thumb_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = if trimmed.starts_with("//") {
        format!("https:{}", trimmed)
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        format!("https://{}", rest)
    } else {
        trimmed.to_string()
    };

    if normalized.contains('?') || normalized.contains('@') {
        Some(normalized)
    } else {
        Some(format!("{}@74w_74h.jpeg", normalized))
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_order_thumb_url;

    #[test]
    fn normalize_order_thumb_url_handles_empty_query_at_and_normal_urls() {
        assert_eq!(normalize_order_thumb_url(""), None);
        assert_eq!(normalize_order_thumb_url("   "), None);
        assert_eq!(
            normalize_order_thumb_url("https://i0.hdslb.com/bfs/show/poster.jpg"),
            Some("https://i0.hdslb.com/bfs/show/poster.jpg@74w_74h.jpeg".to_string())
        );
        assert_eq!(
            normalize_order_thumb_url("//i0.hdslb.com/bfs/show/poster.jpg"),
            Some("https://i0.hdslb.com/bfs/show/poster.jpg@74w_74h.jpeg".to_string())
        );
        assert_eq!(
            normalize_order_thumb_url("http://i0.hdslb.com/bfs/show/poster.jpg"),
            Some("https://i0.hdslb.com/bfs/show/poster.jpg@74w_74h.jpeg".to_string())
        );
        assert_eq!(
            normalize_order_thumb_url("https://i0.hdslb.com/bfs/show/poster.jpg@100w_100h.webp"),
            Some("https://i0.hdslb.com/bfs/show/poster.jpg@100w_100h.webp".to_string())
        );
        assert_eq!(
            normalize_order_thumb_url("https://i0.hdslb.com/bfs/show/poster.jpg?token=abc"),
            Some("https://i0.hdslb.com/bfs/show/poster.jpg?token=abc".to_string())
        );
    }
}
