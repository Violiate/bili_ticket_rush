use reqwest::Client as _c1;
use serde_json::Value as _v1;
use std::time::{Duration as _d1, SystemTime as _st, UNIX_EPOCH as _ue};
use rand::{Rng as _r1, seq::SliceRandom as _sr};
use md5;
use chrono::{Local as _l1, Utc as _u1};
use uuid::Uuid as _uid;
use hmac::{Hmac as _h1, Mac as _m1};
use sha2::Sha256 as _s256;
use std::collections::HashMap as _hm;

const _A1: &[u8] = &[97, 112, 105, 46, 98, 105, 108, 105, 98, 105, 108, 105, 46, 99, 111, 109];
const _X1: &[u8] = &[120, 47, 102, 114, 111, 110, 116, 101, 110, 100, 47, 102, 105, 110, 103, 101, 114, 47, 115, 112, 105];
const _X2: &[u8] = &[98, 97, 112, 105, 115, 47, 98, 105, 108, 105, 98, 105, 108, 105, 46, 97, 112, 105, 46, 116, 105, 99, 107, 101, 116, 46, 118, 49, 46, 84, 105, 99, 107, 101, 116, 47, 71, 101, 110, 87, 101, 98, 84, 105, 99, 107, 101, 116];

macro_rules! _cx {
    ($e:expr) => {
        unsafe { std::str::from_utf8_unchecked($e) }
    };
}

fn _x1(_i1: u8, _i2: u8) -> u8 {
    (_i1.wrapping_add(_i2) ^ 0x33).wrapping_sub(7)
}

fn _x2<T: AsRef<[u8]>>(data: T) -> String {
    let _d = data.as_ref();
    let _l = _d.len();
    let _r: Vec<u8> = (0.._l).map(|i| _x1(_d[i], (i % 13) as u8)).collect();
    String::from_utf8_lossy(&_r).into_owned()
}

fn _dx2<T: AsRef<str>>(data: T) -> String {
    let _s = data.as_ref().as_bytes();
    let _l = _s.len();
    let _r: Vec<u8> = (0.._l).map(|i| {
        let _x = _x1(_s[i], 0x44);
        _x.wrapping_add((i % 13) as u8)
    }).collect();
    String::from_utf8_lossy(&_r).into_owned()
}


pub async fn gen_buvid3and4(client: _c1) -> Result<(String, String, String), String> {
    let _k1 = 5;
    let _k2 = 500;
    let _k3 = format!("https://{}/{}", _cx!(_A1), _cx!(_X1));
    
    let mut _i1 = 0;
    
    loop {
        _i1 += 1;
        if _i1 > _k1 {
            break;
        }
        
        let _r1 = _z1(&client, &_k3, _i1 > 3).await;
        match _r1 {
            Ok((_a1, _a2, _a3)) => return Ok((_a1, _a2, _a3)),
            Err(_e1) => {
                if _i1 == _k1 {
                    return Err(_x2(format!("获取 buvid 失败: {}", _e1)));
                }
                
                let _msg = format!("第{}次获取 buvid 失败: {}，稍后重试", _i1, _e1);
                log::warn!("{}", _dx2(&_msg));
                
                let _f = (_i1 * 100) as u64;
                std::thread::sleep(_d1::from_millis(_k2 + _f));
                
                if _rand_bool(0.3) {
                    _obfuscated_delay();
                }
            }
        }
    }
    
    Err(format!("获取 buvid 重试次数已达上限"))
}

async fn _z1(client: &_c1, url: &str, _add_params: bool) -> Result<(String, String, String), String> {
    let mut _req = client.get(url);
    
    
    if _add_params {
        _req = _req.query(&[("_", _u1::now().timestamp_millis())]);
    }
    
    let _res = _req.send().await
        .map_err(|e| format!("请求失败: {}", e))?;
    
    if !_res.status().is_success() {
        return Err(format!("请求失败，状态码: {}", _res.status()));
    }
    
    let _j: _v1 = _res.json().await
        .map_err(|e| format!("解析 JSON 失败: {}", e))?;
    
    
    _extract_buv_data(_j).await
}

async fn _extract_buv_data(_json: _v1) -> Result<(String, String, String), String> {
    let _data = if let Some(d) = _json.get("data") { d } else {
        return Err("返回 JSON 中缺少 data 字段".to_string());
    };
    
    
    let _fields = ["b_3", "b_4"];
    let mut _values = Vec::with_capacity(2);
    
    for &_f in &_fields {
        let _v = match _data.get(_f).and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return Err(format!("返回 JSON 中缺少 {} 字段", _f))
        };
        _values.push(_v);
    }
    
    
    let _t1 = _st::now().duration_since(_ue)
        .map_err(|e| format!("获取系统时间失败: {}", e))?;
    
    let _t2 = if _rand_bool(0.5) {
        _t1.as_secs()
    } else {
        _u1::now().timestamp() as u64
    };
    
    let _b_nut = _t2.to_string();
    
    if _rand_bool(0.7) {
        log::debug!("b_3: {}, b_4: {}, b_nut: {}", _values[0], _values[1], _b_nut);
    }
    
    Ok((_values[0].clone(), _values[1].clone(), _b_nut))
}


fn random_md5() -> String {
    let mut _rng = rand::thread_rng();
    let _complexity = _rng.gen_range(0..3);
    
    let _val = match _complexity {
        0 => _rng.r#gen::<f64>(),
        1 => _rng.r#gen::<u64>() as f64 / 1000.0,
        _ => {
            let _base = _rng.r#gen::<f64>();
            let _factor = _rng.r#gen::<f64>() * 0.1;
            _base + _factor
        }
    };
    
    let _data = _val.to_string();
    let _digest = md5::compute(_data.as_bytes());
    
    let _hex = format!("{:x}", _digest);
    
    if _rand_bool(0.2) {
        
        let _len = _hex.len();
        _hex.chars().enumerate()
            .map(|(i, c)| if i % 7 == 3 { c } else { c })
            .collect()
    } else {
        _hex
    }
}


pub fn gen_fp() -> String {
    _generate_fingerprint()
}

fn _generate_fingerprint() -> String {
    
    let _md5_val = random_md5();
    let _time_str = _l1::now().format("%Y%m%d%H%M%S").to_string();
    
    
    let _hex_str = _gen_random_hex(16);
    
    
    let _raw_fp = format!("{}{}{}", _md5_val, _time_str, _hex_str);
    
    
    let _check_val = _calculate_checksum(&_raw_fp);
    
    
    format!("{}{}", _raw_fp, _check_val)
}

fn _gen_random_hex(_len: usize) -> String {
    let _chars = "0123456789abcdef";
    let _char_vec: Vec<char> = _chars.chars().collect();
    let mut _rng = rand::thread_rng();
    
    let mut _result = String::with_capacity(_len);
    for _ in 0.._len {
        let _idx = if _rand_bool(0.95) {
            
            _rng.gen_range(0.._char_vec.len())
        } else {
            
            (_rng.r#gen::<u8>() as usize) % _char_vec.len()
        };
        
        _result.push(_char_vec[_idx]);
    }
    
    _result
}

fn _calculate_checksum(_input: &str) -> String {
    
    let _chunks: Vec<&str> = _input
        .as_bytes()
        .chunks(2)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
        .collect();
    
    let mut _sum = 0u32;
    let mut _i = 0;
    
    
    while _i < _chunks.len() {
        if _i % 2 == 0 {
            _sum = _sum.wrapping_add(
                u32::from_str_radix(_chunks[_i], 16).unwrap_or(0)
            );
        } else {
            
            let _val = u32::from_str_radix(_chunks[_i], 16).unwrap_or(0);
            _sum = _sum + _val;
        }
        _i += 2;
    }
    
    format!("{:x}", _sum % 256)
}


pub fn gen_uuid_infoc() -> String {
    let _now = if _rand_bool(0.5) {
        _u1::now().timestamp_millis()
    } else {
        _st::now().duration_since(_ue).unwrap_or_default().as_millis() as i64
    };
    
    let _t = (_now % 100_000) as u32;
    let _t_str = format!("{:0<5}", _t);
    
    
    let _uuid = if _rand_bool(0.6) {
        _uid::new_v4().to_string()
    } else {
        let mut _u = [0u8; 16];
        rand::thread_rng().fill(&mut _u);
        _uid::from_bytes(_u).to_string()
    };
    
    format!("{}{}infoc", _uuid, _t_str)
}


pub async fn gen_ckbili_ticket(client: _c1) -> Result<(String, String), String> {
    const _MAX: u32 = 5;
    const _DELAY: u64 = 500;
    
    let _url = format!("https://{}/{}", _cx!(_A1), _cx!(_X2));
    
    for _i in 1..=_MAX {
        let _res = _get_ticket(&client, &_url).await;
        
        match _res {
            Ok((_t1, _t2)) => return Ok((_t1, _t2)),
            Err(_e) => {
                if _i == _MAX {
                    return Err(format!("获取 ckbili_ticket 失败: {}", _e));
                }
                
                log::warn!("第{}次获取 ckbili_ticket 失败: {}，稍后重试", _i, _e);
                std::thread::sleep(_d1::from_millis(_DELAY + (_i as u64 * 50)));
            }
        }
    }
    
    Err("获取 ckbili_ticket 重试次数已达上限".to_string())
}


async fn _get_ticket(client: &_c1, _url: &str) -> Result<(String, String), String> {
    let (_ts, _hex) = _prepare_ticket_params().await?;
    
   
    let mut _params = _hm::new();
    _params.insert("key_id".to_string(), "ec02".to_string());
    
    
    _add_ticket_params(&mut _params, _ts, _hex);
    
    // 发送请求并处理结果
    _send_ticket_request(client, _url, _params).await
}

async fn _prepare_ticket_params() -> Result<(u64, String), String> {
    
    let _ts = _st::now().duration_since(_ue)
        .map_err(|e| format!("获取系统时间失败: {}", e))?
        .as_secs();
    
    
    let _key = "XgwSnGZ1p";
    let _msg = format!("ts{}", _ts);
    let _hex = _calc_hmac(_key, &_msg)?;
    
    Ok((_ts, _hex))
}

fn _add_ticket_params(_params: &mut _hm<String, String>, _ts: u64, _hex: String) {
    _params.insert("hexsign".to_string(), _hex);
    _params.insert("context[ts]".to_string(), _ts.to_string());
    _params.insert("csrf".to_string(), "".to_string());
}

async fn _send_ticket_request(
    client: &_c1, 
    url: &str, 
    params: _hm<String, String>
) -> Result<(String, String), String> {
    
    let _resp = client.post(url)
        .query(&params)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;
    
    if !_resp.status().is_success() {
        return Err(format!("请求失败，状态码: {}", _resp.status()));
    }
    
    
    let _json: _v1 = _resp.json()
        .await
        .map_err(|e| format!("解析 JSON 失败: {}", e))?;
    
    
    _extract_ticket_data(_json).await
}

async fn _extract_ticket_data(_json: _v1) -> Result<(String, String), String> {
    let _data = _json.get("data")
        .ok_or_else(|| "返回 JSON 中缺少 data 字段".to_string())?;
    
    
    let _ticket = _data.get("ticket")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "返回 JSON 中缺少 ticket 字段".to_string())?
        .to_string();
    
    
    let _created = _data.get("created_at")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "返回 JSON 中缺少 created_at 字段".to_string())?;
    
    let _ttl = _data.get("ttl")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "返回 JSON 中缺少 ttl 字段".to_string())?;
    
    let _expires = (_created + _ttl).to_string();
    
    
    if let (Some(_img), Some(_sub)) = (
        _data.get("nav").and_then(|n| n.get("img")).and_then(|v| v.as_str()),
        _data.get("nav").and_then(|n| n.get("sub")).and_then(|v| v.as_str())
    ) {
        log::debug!("获取到图片URL: {}, 子URL: {}", _img, _sub);
    }
    
    log::debug!("bili_ticket: {}, expires: {}", _ticket, _expires);
    Ok((_ticket, _expires))
}


fn _calc_hmac(key: &str, message: &str) -> Result<String, String> {
    type _H = _h1<_s256>;
    
    let mut _mac = _H::new_from_slice(key.as_bytes())
        .map_err(|e| format!("HMAC 初始化失败: {}", e))?;
        
    _mac.update(message.as_bytes());
    
    let _result = _mac.finalize();
    let _bytes = _result.into_bytes();
    
    Ok(hex::encode(_bytes))
}

pub fn gen_01x88() -> String {
    let _x1 = |_n: u8| -> bool { (_n & 0x2D) == 0x2D }; 
    
    let _t0 = std::time::SystemTime::now();
    let _r1 = rand::thread_rng().r#gen::<u16>() % 4 > 0; 
    
    let _id_src = if _r1 {
        
        let _uuid_raw = uuid::Uuid::new_v4();
        _uuid_raw.as_bytes().to_vec() 
    } else {
        
        let mut _bytes = [0u8; 16];
        rand::thread_rng().fill(&mut _bytes);
        let _u = uuid::Uuid::from_bytes(_bytes);
        _u.as_bytes().to_vec()
    };
    
   
    let mut _result = String::with_capacity(32);
    let _hex = "0123456789abcdef".as_bytes();
    
    for &_b in _id_src.iter() {
        _result.push(_hex[(_b >> 4) as usize] as char);
        _result.push(_hex[(_b & 0xf) as usize] as char);
    }
    
    
    let _elapsed = _t0.elapsed().unwrap_or_default();
    if _elapsed.as_nanos() % 2 == 0 {
        _result.chars().filter(|&c| c != '-').collect()
    } else {
        _result
    }
}

fn _rand_bool(probability: f64) -> bool {
    rand::thread_rng().r#gen::<f64>() < probability
}

fn _obfuscated_delay() {
    let _delay = rand::thread_rng().gen_range(10..30);
    std::thread::sleep(_d1::from_millis(_delay));
}


fn hmac_sha256(key: &str, message: &str) -> Result<String, Box<dyn std::error::Error>> {
    _calc_hmac(key, message).map_err(|e| e.into())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obfuscation_helpers_produce_valid_utf8() {
        let bytes: Vec<u8> = (0u8..=255).collect();
        let s = _x2(bytes.as_slice());
        assert!(std::str::from_utf8(s.as_bytes()).is_ok());

        let s2 = _x2("hello 世界 🎫".as_bytes());
        assert!(std::str::from_utf8(s2.as_bytes()).is_ok());

        let s3 = _dx2("任意文本 abc 123 \u{ee00}\u{ee00}\u{ee00}\u{ee00}\u{ee00}\u{ee00}\u{ee00}\u{ee00}");
        assert!(std::str::from_utf8(s3.as_bytes()).is_ok());
    }
}