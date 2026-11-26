use serde::{Deserialize, Deserializer, Serialize};

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + Deserialize<'de>,
    D: Deserializer<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

fn deserialize_string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrNumberVisitor;

    impl<'de> serde::de::Visitor<'de> for StringOrNumberVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string, integer, or null")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
            Ok(value.to_string())
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
            Ok(value.to_string())
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
            Ok(value.to_string())
        }

        fn visit_i128<E>(self, value: i128) -> Result<Self::Value, E> {
            Ok(value.to_string())
        }

        fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E> {
            Ok(value.to_string())
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E> {
            Ok(String::new())
        }

        fn visit_none<E>(self) -> Result<Self::Value, E> {
            Ok(String::new())
        }
    }

    deserializer.deserialize_any(StringOrNumberVisitor)
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct OrderResponse {
    pub errno: i32,
    pub errtag: i32,
    pub msg: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub data: OrderData,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct OrderData {
    pub total: i32,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub list: Vec<Order>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct Order {
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub order_id: String,
    pub order_type: i32,
    pub item_id: i64,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub item_info: ItemInfo,
    pub total_money: i64,
    pub count: i32,
    pub pay_money: i64,
    pub pay_channel: Option<String>,
    pub status: i32,
    pub sub_status: i32,
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub ctime: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub img: ImageInfo,
    pub sub_status_name: String,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct ItemInfo {
    pub name: String,
    pub image: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_number")]
    pub screen_id: String,
    pub screen_name: String,
    pub screen_start_time: String,
    pub screen_end_time: String,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct ImageInfo {
    pub url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_null_list_and_integer_order_id() {
        let null_list_json = r#"{"errno": 0, "msg": "", "data": {"total": 0, "list": null}}"#;
        let resp: OrderResponse =
            serde_json::from_str(null_list_json).expect("null list should deserialize");
        assert!(resp.data.list.is_empty());

        let int_order_json = r#"{
            "errno": 0,
            "data": {
                "total": 1,
                "list": [
                    {
                        "order_id": 123456789012345,
                        "item_id": 999,
                        "item_info": {
                            "name": "BW2026",
                            "screen_id": 54321
                        },
                        "pay_money": 12800
                    }
                ]
            }
        }"#;
        let resp2: OrderResponse =
            serde_json::from_str(int_order_json).expect("integer order_id should deserialize");
        assert_eq!(resp2.data.list.len(), 1);
        assert_eq!(resp2.data.list[0].order_id, "123456789012345");
        assert_eq!(resp2.data.list[0].item_info.screen_id, "54321");
        assert_eq!(resp2.data.list[0].pay_money, 12800);
        assert!(resp2.data.list[0].img.url.is_empty());
    }
}
