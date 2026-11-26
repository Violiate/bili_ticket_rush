use reqwest::{header, Client, Error, Response};
use serde_json::Value;

pub async fn request_get(client: &Client, url: &str, cookie: Option<&str>) -> Result<Response, Error> {
    let mut req = client.get(url);

    if let Some(cookie_str) = cookie {
        req = req.header(header::COOKIE, cookie_str);
    }

    req.send().await
}

pub(crate) fn json_value_to_form(value: &Value) -> Option<Vec<(String, String)>> {
    let obj = value.as_object()?;
    let mut form = Vec::with_capacity(obj.len());

    for (key, val) in obj {
        match val {
            Value::String(s) => form.push((key.clone(), s.clone())),
            Value::Number(n) => form.push((key.clone(), n.to_string())),
            Value::Bool(b) => form.push((key.clone(), b.to_string())),
            Value::Null => continue,
            Value::Array(_) | Value::Object(_) => match serde_json::to_string(val) {
                Ok(serialized) => form.push((key.clone(), serialized)),
                Err(e) => log::warn!("表单字段 {} 序列化失败: {}", key, e),
            },
        }
    }

    Some(form)
}

pub async fn request_post<T: serde::Serialize + ?Sized>(
    client: &Client,
    url: &str,
    cookie: Option<&str>,
    json_data: Option<&T>,
) -> Result<Response, Error> {
    let mut req = client.post(url);

    if let Some(cookie_str) = cookie {
        req = req.header(header::COOKIE, cookie_str);
    }

    if let Some(data) = json_data {
        match serde_json::to_value(data) {
            Ok(json_value) => {
                if let Some(form) = json_value_to_form(&json_value) {
                    req = req.form(&form);
                } else {
                    log::warn!("request_post 收到非 JSON 对象载荷，已跳过表单编码");
                }
            }
            Err(e) => {
                log::warn!("request_post 序列化请求参数失败: {}", e);
            }
        }
    }

    req.send().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_value_to_form_handles_types_without_corruption() {
        let payload = json!({
            "code": "012345",
            "quoted": "\"hello\"",
            "cid": 86,
            "flag": true,
            "ignored": null,
            "nested": {"a": 1},
            "arr": [1, 2]
        });

        let pairs = json_value_to_form(&payload).expect("should convert object");
        let map: std::collections::HashMap<_, _> = pairs.into_iter().collect();

        assert_eq!(map.get("code").map(String::as_str), Some("012345"));
        assert_eq!(map.get("quoted").map(String::as_str), Some("\"hello\""));
        assert_eq!(map.get("cid").map(String::as_str), Some("86"));
        assert_eq!(map.get("flag").map(String::as_str), Some("true"));
        assert!(!map.contains_key("ignored"));
        assert_eq!(map.get("nested").map(String::as_str), Some("{\"a\":1}"));
        assert_eq!(map.get("arr").map(String::as_str), Some("[1,2]"));
    }
}
