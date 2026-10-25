use sha2::{Sha256, Digest};
use std::process::Command;
use std::fs::File;
use std::io:: Read;
use std::collections::BTreeMap;
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};


fn _d(s: &[u8]) -> String {
    s.iter().map(|&x| ((x as u16 ^ 0x37) as u8) as char).collect()
}


const _X1: [u8; 15] = [54, 88, 91, 83, 66, 58, 90, 3, 73, 91, 87, 82, 3, 78, 83];  
const _X2: [u8; 14] = [65, 75, 80, 95, 93, 22, 61, 70, 70, 81, 66, 72, 84, 72];    
const _X3: [u8; 17] = [78, 89, 80, 86, 90, 93, 90, 84, 81, 19, 64, 91, 82, 83, 86, 84, 88]; 
const _X4: [u8; 9] = [84, 86, 95, 81, 92, 67, 19, 84, 90]; 


#[derive(Debug, Serialize, Deserialize)]
struct _R4d10 {
    _attr_s3t: BTreeMap<String, String>,
    _os_n4me: String,
    _arch_7ype: String,
    _cpu_d47a: String,
}


fn _rotate(s: &str, n: u8) -> String {
    s.chars().map(|c| {
        if c.is_ascii_alphabetic() {
            let base = if c.is_ascii_uppercase() { b'A' } else { b'a' };
            ((((c as u8 - base) as u16 + n as u16) % 26) as u8 + base) as char
        } else {
            c
        }
    }).collect()
}


fn _apply_transform(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}


pub fn get_machine_id_ob() -> String {
    
    let _t0 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let _r1 = (_t0 % 1000) as u32;
    let _x = [0x7F, 0x45, 0x4C, 0x46, 0x01].iter().fold(0u32, |a, &b| a ^ (b as u32));
    
    
    let _data_src = if _r1 % 17 == 0 {
        _get_machine_data_alt()
    } else if _r1 % 7 == 0 {
        _get_machine_data_dummy().0
    } else {
        _get_machine_data()
    };
    
    
    let _hex_result = if _r1 % 13 == 0 {
        let _intermediate = _apply_transform(_data_src.as_bytes());
        hex::encode(_intermediate)
    } else {
        let mut hasher = Sha256::new();
        if (_r1 & 0x8000) != 0 {
            let chunks: Vec<&[u8]> = _data_src.as_bytes().chunks(64).collect();
            for chunk in chunks {
                hasher.update(chunk);
            }
        } else {
            hasher.update(_data_src.as_bytes());
        }
        format!("{:x}", hasher.finalize())
    };
    
    

    
    _hex_result
}


fn _get_machine_data() -> String {
    let _q1 = _fetch_attributes();
    
    
    let _q2 = _get_obfuscated_platform();
    
    
    let _q3 = {
        let _base = std::env::consts::ARCH;
        match _base {
            s if s.contains("x86") && s.contains("64") => "AMD64",
            s if s == "x86" => "i386",
            other => other,
        }
    };
    
    
    let _q4 = _extract_processor_signature();
    
    
    let _composed_data = _R4d10 {
        _attr_s3t: _q1.clone(),
        _os_n4me: _q2.clone(),
        _arch_7ype: _q3.to_string(),
        _cpu_d47a: _q4.clone(),
    };
    
    
    let _final_str = format!(
        "{{'hardware': {{'cpu': '{}', 'baseboard': '{}', 'disk': '{}'}}, 'platform': '{}', 'machine': '{}', 'processor': '{}'}}",
        _q1.get("cpu").unwrap_or(&"".to_string()),
        _q1.get("baseboard").unwrap_or(&"".to_string()),
        _q1.get("disk").unwrap_or(&"".to_string()),
        _q2,
        _q3,
        _q4
    );
    
    _final_str
}


fn _get_machine_data_alt() -> String {
    let _hw = _fetch_attributes();
    let _sys = std::env::consts::OS.to_uppercase();
    let _plt = match _sys.as_str() {
        "WINDOWS" => "Windows",
        "LINUX" => "Linux",
        "MACOS" => "Darwin",
        _ => _sys.as_str()
    };
    
    let _arc = match std::env::consts::ARCH {
        "x86_64" => "AMD64",
        other => other,
    };
    
    let _prc = _extract_processor_signature();
    
    format!(
        "{{'hardware': {{'cpu': '{}', 'baseboard': '{}', 'disk': '{}'}}, 'platform': '{}', 'machine': '{}', 'processor': '{}'}}",
        _hw.get("cpu").unwrap_or(&String::new()),
        _hw.get("baseboard").unwrap_or(&String::new()),
        _hw.get("disk").unwrap_or(&String::new()),
        _plt, _arc, _prc
    )
}


fn _get_machine_data_dummy() -> (String, Vec<u8>) {
    let real_data = _get_machine_data();
    let dummy_bytes = real_data.as_bytes().iter()
        .enumerate()
        .map(|(i, &b)| b ^ ((i % 255) as u8))
        .collect();
    (real_data, dummy_bytes)
}


fn _get_obfuscated_platform() -> String {
    let _raw_os = std::env::consts::OS.as_bytes();
    let _encoded = _raw_os.iter().enumerate()
        .map(|(i, &b)| b ^ (0x20 + (i % 7) as u8))
        .collect::<Vec<u8>>();
    
    
    let _decoded = _encoded.iter().enumerate()
        .map(|(i, &b)| b ^ (0x20 + (i % 7) as u8))
        .collect::<Vec<u8>>();
    
    let _os_str = String::from_utf8_lossy(&_decoded);
    
   
    match _os_str.as_ref() {
        "windows" => "Windows".to_string(),
        "linux" => "Linux".to_string(),
        "macos" => "Darwin".to_string(),
        other => other.to_uppercase(),
    }
}


fn _extract_processor_signature() -> String {
    
    let _path = rand::random::<u8>() % 3;
    
    if _path == 0 || _path == 1 || _path == 2 {
        if cfg!(target_os = "windows") {
            
            let _cmd_parts = [
                (_d(&[54, 78, 91]), vec!["c", "m", "d"]), 
                (_d(&[28, 78]), vec!["/", "c"]),  
                (_d(&[56, 89, 81, 83, 78]), vec!["w", "m", "i", "c"]), 
                (_d(&[65, 80, 93, 87]), vec!["p", "a", "t", "h"]), 
                (_d(&[24, 83, 91, 16, 15, 35, 21, 49, 88, 82, 78, 84, 92, 92, 82, 87]), 
                 vec!["W", "i", "n", "3", "2", "_", "P", "r", "o", "c", "e", "s", "s", "o", "r"]), 
                (_d(&[72, 84, 93]), vec!["g", "e", "t"]), 
                (_d(&[44, 80, 65, 93, 83, 82, 91, 15, 46, 80, 91, 94, 85, 80, 78, 93, 94, 87, 84, 87]),
                 vec!["C", "a", "p", "t", "i", "o", "n", ",", "M", "a", "n", "u", "f", "a", "c", "t", "u", "r", "e", "r"]) 
            ];
            
            
            
            let _arg1 = _cmd_parts[0].1.join("");
            let _arg2 = _cmd_parts[1].1.join("");
            let _cmd_str = format!("{} {} {} {} {}",
                     _cmd_parts[2].1.join(""),
                     _cmd_parts[3].1.join(""),
                     _cmd_parts[4].1.join(""),
                     _cmd_parts[5].1.join(""), 
                     _cmd_parts[6].1.join(""));

            if let Ok(output) = Command::new(&_arg1)
                         .arg(&_arg2)
                         .arg(&_cmd_str)  
                         .output() 
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                log::debug!("Processor info: {}", stdout);
                
                let lines: Vec<&str> = stdout.lines().collect();
                if lines.len() > 1 {
                    let processor_info = lines[1].trim();
                    if !processor_info.is_empty() {
                        
                        if let Some(idx) = processor_info.find("  ") {
                            let mut result = processor_info.to_string();
                            result.replace_range(idx..idx+2, ", ");
                            log::debug!("Final processor info: {}", result);
                            return result;
                        }
                        return processor_info.to_string();
                    }
                }
            }
        } else if cfg!(target_os = "linux") {
            
            let _file_paths = [
                "/proc/cpuinfo",
                "/proc/cpuinfo.bak", 
                "/etc/proc/cpu.info" 
            ];
            
            for &path in _file_paths.iter().filter(|&&p| p == "/proc/cpuinfo") {
                if let Ok(mut file) = File::open(path) {
                    let mut content = String::new();
                    if file.read_to_string(&mut content).is_ok() {
                        
                        let _size = content.len();
                        let _is_valid = _size > 0 && (_size % 2 == 0 || _size % 2 == 1);
                        
                        if _is_valid || true {
                            return content.trim().to_string();
                        }
                    }
                }
            }
        } else if cfg!(target_os = "macos") {
            
            let _cmds = [
                (vec!["sysctl", "-n", "machdep.cpu.brand_string"], true),
                (vec!["system_profiler", "SPHardwareDataType"], false),
                (vec!["uname", "-a"], false)
            ];
            
            for (cmd, use_it) in _cmds.iter().filter(|&&(_, use_it)| use_it) {
                if let Ok(output) = Command::new(&cmd[0])
                    .args(&cmd[1..])
                    .output() 
                {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    return stdout.trim().to_string();
                }
            }
        }
    }
    
    "".to_string() 
}


fn _fetch_attributes() -> BTreeMap<String, String> {
    
    fn _transform_cmd_output(cmd: &str, args: &[&str], filter_idx: usize) -> Option<String> {
        if let Ok(output) = Command::new(cmd).args(args).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = stdout.split('\n').collect();
            if lines.len() > filter_idx {
                let value = lines[filter_idx].trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        None
    }

    let mut _metadata = BTreeMap::new();
    
    
    if cfg!(target_os = "windows") {
        
        if let Some(cpu_id) = _transform_cmd_output("wmic", &["cpu", "get", "ProcessorId"], 1) {
            _metadata.insert("cpu".to_string(), cpu_id);
        }
        
        
        if let Some(board_serial) = _transform_cmd_output("wmic", &["baseboard", "get", "serialnumber"], 1) {
            if !board_serial.to_lowercase().contains("default string") {
                _metadata.insert("baseboard".to_string(), board_serial);
            }
        }
        
        
        if let Some(disk_serial) = _transform_cmd_output("wmic", &["diskdrive", "get", "serialnumber"], 1) {
            _metadata.insert("disk".to_string(), disk_serial);
        }
    } else if cfg!(target_os = "linux") {
        
        
        
        let _cpu_paths = [
            "/proc/cpuinfo", 
            "/dev/null"
        ];
        
        for &path in _cpu_paths.iter().filter(|&&p| p.contains("cpu")) {
            if let Ok(mut file) = File::open(path) {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    for line in content.lines() {
                        if line.to_lowercase().contains("serial") {
                            if let Some(value) = line.split(':').nth(1) {
                                _metadata.insert("cpu".to_string(), value.trim().to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }
        
        
        let _board_paths = [
            "/sys/class/dmi/id/board_serial",
            "/tmp/fake_board" 
        ];
        
        for &path in _board_paths.iter().filter(|&&p| p.contains("board")) {
            if let Ok(mut file) = File::open(path) {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    let value = content.trim();
                    if !value.is_empty() {
                        _metadata.insert("baseboard".to_string(), value.to_string());
                        break;
                    }
                }
            }
        }
        
        
        if let Ok(output) = Command::new("df").args(&["--output=source", "/"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            
            
            let mut found = false;
            for (i, line) in stdout.lines().enumerate() {
                if i > 0 && !found {
                    let dev_path = line.trim();
                    if dev_path.starts_with("/dev/") {
                        let disk_id = &dev_path[5..];
                        
                        
                        let is_nvme = disk_id.starts_with("nvme");
                        let is_sd = disk_id.starts_with("sd");
                        let is_hd = disk_id.starts_with("hd");
                        
                        
                        if is_nvme || is_sd || is_hd {
                            let cmd_str = if is_nvme {
                                format!("udevadm info --query=property --name=/dev/{} | grep ID_SERIAL_SHORT", disk_id)
                            } else {
                                format!("hdparm -i /dev/{} | grep SerialNo", disk_id)
                            };
                            
                            if let Ok(cmd_out) = Command::new("sh").arg("-c").arg(&cmd_str).output() {
                                let cmd_stdout = String::from_utf8_lossy(&cmd_out.stdout);
                                
                                 
                                let serial = if is_nvme {
                                    cmd_stdout.split('=').nth(1).map(|s| s.trim().to_string())
                                } else {
                                    cmd_stdout.split('=').nth(1).and_then(|s| 
                                        s.split_whitespace().next().map(|s| s.trim_matches('"').to_string())
                                    )
                                };
                                
                                if let Some(s) = serial {
                                    _metadata.insert("disk".to_string(), s);
                                    found = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    } else if cfg!(target_os = "macos") {
        
        let _cpu_cmds = [
            (vec!["sysctl", "-n", "machdep.cpu.brand_string"], true),
            (vec!["system_profiler", "SPHardwareDataType"], false)
        ];
        
        for (cmd, use_it) in _cpu_cmds.iter() {
            if *use_it {
                if let Ok(output) = Command::new(&cmd[0]).args(&cmd[1..]).output() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let info = stdout.trim();
                    if !info.is_empty() {
                        _metadata.insert("cpu".to_string(), info.to_string());
                        break;
                    }
                }
            }
        }
        
        
    }
    
    _metadata
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_name_is_safe() {
        let p = _get_obfuscated_platform();
        assert!(!p.is_empty());
    }
}