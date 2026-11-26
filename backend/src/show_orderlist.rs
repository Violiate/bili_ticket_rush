use std::sync::Arc;

use common::cookie_manager::CookieManager;
use common::show_orderlist::OrderResponse;

pub async fn get_orderlist(
    cookie_manager: Arc<CookieManager>,
    page: usize,
    page_size: usize,
) -> Result<OrderResponse, String> {
    let effective_page_size = if page_size == 0 { 10 } else { page_size };

    let resp = cookie_manager
        .get("https://show.bilibili.com/api/ticket/ordercenter/ticketList")
        .await
        .query(&[
            ("page", page.to_string()),
            ("page_size", effective_page_size.to_string()),
        ])
        .send()
        .await
        .map_err(|err| err.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("获取订单不期待响应：{}", resp.status()));
    }

    let text = resp
        .text()
        .await
        .map_err(|e| format!("获取订单响应失败：{}", e))?;
    log::debug!("获取全部订单：{}", text);

    let raw: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("获取全部订单json解析失败：{}", e))?;

    if let Some((err_code, msg)) = crate::api::extract_api_error(&raw) {
        return Err(format!("获取全部订单失败({})：{}", err_code, msg));
    }

    let order_resp: OrderResponse = serde_json::from_value(raw)
        .map_err(|e| format!("获取全部订单数据解析失败：{}", e))?;

    if order_resp.errno != 0 {
        let msg = if order_resp.msg.is_empty() {
            "未知错误".to_string()
        } else {
            order_resp.msg.clone()
        };
        return Err(format!("获取全部订单失败({})：{}", order_resp.errno, msg));
    }

    Ok(order_resp)
}
