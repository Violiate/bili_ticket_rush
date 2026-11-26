use std::collections::BTreeMap;
#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::io::Read;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn run_command_hidden(program: &str, args: &[&str]) -> Option<Output> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.output().ok()
}

fn _apply_transform(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

fn normalized_arch() -> &'static str {
    match std::env::consts::ARCH {
        s if s.contains("x86") && s.contains("64") => "AMD64",
        "x86" => "i386",
        other => other,
    }
}

pub fn get_machine_id_ob() -> String {
    let _t0 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let _r1 = (_t0 % 1000) as u32;

    let _data_src = if _r1 % 17 == 0 {
        _get_machine_data_alt()
    } else if _r1 % 7 == 0 {
        _get_machine_data_dummy().0
    } else {
        _get_machine_data()
    };

    if _r1 % 13 == 0 {
        let _intermediate = _apply_transform(_data_src.as_bytes());
        hex::encode(_intermediate)
    } else {
        let mut hasher = Sha256::new();
        if (_r1 & 0x8000) != 0 {
            for chunk in _data_src.as_bytes().chunks(64) {
                hasher.update(chunk);
            }
        } else {
            hasher.update(_data_src.as_bytes());
        }
        format!("{:x}", hasher.finalize())
    }
}

fn _get_machine_data() -> String {
    let _q1 = _fetch_attributes();
    let _q2 = _get_obfuscated_platform();
    let _q3 = normalized_arch();
    let _q4 = _extract_processor_signature();

    format!(
        "{{'hardware': {{'cpu': '{}', 'baseboard': '{}', 'disk': '{}'}}, 'platform': '{}', 'machine': '{}', 'processor': '{}'}}",
        _q1.get("cpu").map(String::as_str).unwrap_or(""),
        _q1.get("baseboard").map(String::as_str).unwrap_or(""),
        _q1.get("disk").map(String::as_str).unwrap_or(""),
        _q2,
        _q3,
        _q4
    )
}

fn _get_machine_data_alt() -> String {
    let _hw = _fetch_attributes();
    let _sys = std::env::consts::OS.to_uppercase();
    let _plt = match _sys.as_str() {
        "WINDOWS" => "Windows",
        "LINUX" => "Linux",
        "MACOS" => "Darwin",
        _ => _sys.as_str(),
    };

    let _arc = normalized_arch();
    let _prc = _extract_processor_signature();

    format!(
        "{{'hardware': {{'cpu': '{}', 'baseboard': '{}', 'disk': '{}'}}, 'platform': '{}', 'machine': '{}', 'processor': '{}'}}",
        _hw.get("cpu").map(String::as_str).unwrap_or(""),
        _hw.get("baseboard").map(String::as_str).unwrap_or(""),
        _hw.get("disk").map(String::as_str).unwrap_or(""),
        _plt,
        _arc,
        _prc
    )
}

fn _get_machine_data_dummy() -> (String, Vec<u8>) {
    let real_data = _get_machine_data();
    let dummy_bytes = real_data
        .as_bytes()
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ ((i % 255) as u8))
        .collect();
    (real_data, dummy_bytes)
}

fn _get_obfuscated_platform() -> String {
    let _raw_os = std::env::consts::OS.as_bytes();
    let _encoded = _raw_os
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ (0x20 + (i % 7) as u8))
        .collect::<Vec<u8>>();

    let _decoded = _encoded
        .iter()
        .enumerate()
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

#[cfg(any(target_os = "linux", test))]
fn parse_linux_cpu_model(content: &str) -> String {
    for line in content.lines() {
        if let Some((key, value)) = line.split_once(':') {
            if key.trim().eq_ignore_ascii_case("model name") {
                let model = value.trim();
                if !model.is_empty() {
                    return model.to_string();
                }
            }
        }
    }
    String::new()
}

#[cfg(target_os = "windows")]
fn run_powershell_single_line(script: &str) -> Option<String> {
    let output = run_command_hidden(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", script],
    )?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

fn _extract_processor_signature() -> String {
    #[cfg(target_os = "windows")]
    {
        // 优先保持原 wmic 逻辑（带 CREATE_NO_WINDOW 避免弹窗），确保现有用户 machine_id 不变
        if let Some(output) = run_command_hidden(
            "cmd",
            &[
                "/c",
                "wmic path Win32_Processor get Caption,Manufacturer",
            ],
        ) {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<&str> = stdout.lines().collect();
                if lines.len() > 1 {
                    let processor_info = lines[1].trim();
                    if !processor_info.is_empty() {
                        if let Some(idx) = processor_info.find("  ") {
                            let mut result = processor_info.to_string();
                            result.replace_range(idx..idx + 2, ", ");
                            return result;
                        }
                        return processor_info.to_string();
                    }
                }
            }
        }

        // Win11 24H2+ 移除 wmic 后的无弹窗降级方案
        if let Some(ps_info) = run_powershell_single_line(
            "$p = Get-CimInstance Win32_Processor | Select-Object -First 1; if ($p) { \"$($p.Caption), $($p.Manufacturer)\" }",
        ) {
            return ps_info;
        }
        if let Ok(env_proc) = std::env::var("PROCESSOR_IDENTIFIER") {
            let trimmed = env_proc.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        return String::new();
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(mut file) = File::open("/proc/cpuinfo") {
            let mut content = String::new();
            if file.read_to_string(&mut content).is_ok() {
                return parse_linux_cpu_model(&content);
            }
        }
        return String::new();
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(output) = run_command_hidden("sysctl", &["-n", "machdep.cpu.brand_string"]) {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return stdout.trim().to_string();
            }
        }
        return String::new();
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        String::new()
    }
}

#[cfg(target_os = "windows")]
fn transform_wmic_output(args: &[&str], filter_idx: usize) -> (bool, Option<String>) {
    if let Some(output) = run_command_hidden("wmic", args) {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = stdout.split('\n').collect();
            if lines.len() > filter_idx {
                let value = lines[filter_idx].trim();
                if !value.is_empty() {
                    return (true, Some(value.to_string()));
                }
            }
            return (true, None);
        }
    }
    (false, None)
}

#[cfg(any(target_os = "linux", test))]
fn is_safe_disk_id(disk_id: &str) -> bool {
    !disk_id.is_empty()
        && disk_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn _fetch_attributes() -> BTreeMap<String, String> {
    let mut metadata = BTreeMap::new();

    #[cfg(target_os = "windows")]
    {
        let (cpu_ok, cpu_val) = transform_wmic_output(&["cpu", "get", "ProcessorId"], 1);
        if let Some(cpu_id) = cpu_val {
            metadata.insert("cpu".to_string(), cpu_id);
        }

        let (board_ok, board_val) =
            transform_wmic_output(&["baseboard", "get", "serialnumber"], 1);
        if let Some(board_serial) = board_val {
            if !board_serial.to_lowercase().contains("default string") {
                metadata.insert("baseboard".to_string(), board_serial);
            }
        }

        let (disk_ok, disk_val) =
            transform_wmic_output(&["diskdrive", "get", "serialnumber"], 1);
        if let Some(disk_serial) = disk_val {
            metadata.insert("disk".to_string(), disk_serial);
        }

        // 仅当 wmic 不可用（如 Win11 24H2+）时才走 CIM / machine_uid 降级，保证现有用户哈希稳定
        if !cpu_ok && !board_ok && !disk_ok {
            if let Some(cpu_id) = run_powershell_single_line(
                "(Get-CimInstance Win32_Processor | Select-Object -First 1).ProcessorId",
            ) {
                metadata.insert("cpu".to_string(), cpu_id);
            }
            if let Some(board_serial) = run_powershell_single_line(
                "(Get-CimInstance Win32_BaseBoard | Select-Object -First 1).SerialNumber",
            ) {
                if !board_serial.to_lowercase().contains("default string") {
                    metadata.insert("baseboard".to_string(), board_serial);
                }
            }
            if let Some(disk_serial) = run_powershell_single_line(
                "(Get-CimInstance Win32_DiskDrive | Select-Object -First 1).SerialNumber",
            ) {
                metadata.insert("disk".to_string(), disk_serial);
            }

            if metadata.is_empty() {
                if let Ok(uid) = machine_uid::get() {
                    let trimmed = uid.trim();
                    if !trimmed.is_empty() {
                        metadata.insert("baseboard".to_string(), trimmed.to_string());
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(mut file) = File::open("/proc/cpuinfo") {
            let mut content = String::new();
            if file.read_to_string(&mut content).is_ok() {
                for line in content.lines() {
                    if let Some((key, value)) = line.split_once(':') {
                        if key.to_lowercase().contains("serial") {
                            let val = value.trim();
                            if !val.is_empty() {
                                metadata.insert("cpu".to_string(), val.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }

        if let Ok(mut file) = File::open("/sys/class/dmi/id/board_serial") {
            let mut content = String::new();
            if file.read_to_string(&mut content).is_ok() {
                let value = content.trim();
                if !value.is_empty() {
                    metadata.insert("baseboard".to_string(), value.to_string());
                }
            }
        }

        if let Some(output) = run_command_hidden("df", &["--output=source", "/"]) {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                let dev_path = line.trim();
                if let Some(disk_id) = dev_path.strip_prefix("/dev/") {
                    if !is_safe_disk_id(disk_id) {
                        continue;
                    }
                    let is_nvme = disk_id.starts_with("nvme");
                    let is_sd = disk_id.starts_with("sd");
                    let is_hd = disk_id.starts_with("hd");

                    if is_nvme || is_sd || is_hd {
                        let name_arg = format!("--name=/dev/{}", disk_id);
                        if let Some(cmd_out) =
                            run_command_hidden("udevadm", &["info", "--query=property", &name_arg])
                        {
                            if cmd_out.status.success() {
                                let cmd_stdout = String::from_utf8_lossy(&cmd_out.stdout);
                                if let Some(serial) = cmd_stdout.lines().find_map(|l| {
                                    l.strip_prefix("ID_SERIAL_SHORT=")
                                        .or_else(|| l.strip_prefix("ID_SERIAL="))
                                        .map(str::trim)
                                        .filter(|s| !s.is_empty())
                                        .map(str::to_string)
                                }) {
                                    metadata.insert("disk".to_string(), serial);
                                    break;
                                }
                            }
                        }

                        if !is_nvme {
                            let dev_arg = format!("/dev/{}", disk_id);
                            if let Some(cmd_out) = run_command_hidden("hdparm", &["-i", &dev_arg]) {
                                if cmd_out.status.success() {
                                    let cmd_stdout = String::from_utf8_lossy(&cmd_out.stdout);
                                    if let Some(serial) = cmd_stdout.lines().find_map(|l| {
                                        let idx = l.find("SerialNo=")?;
                                        let rest = &l[idx + "SerialNo=".len()..];
                                        rest.split_whitespace()
                                            .next()
                                            .map(|s| s.trim_matches('"').to_string())
                                            .filter(|s| !s.is_empty())
                                    }) {
                                        metadata.insert("disk".to_string(), serial);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if !metadata.contains_key("baseboard") {
            if let Ok(uid) = machine_uid::get() {
                let trimmed = uid.trim();
                if !trimmed.is_empty() {
                    metadata.insert("baseboard".to_string(), trimmed.to_string());
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(output) = run_command_hidden("sysctl", &["-n", "machdep.cpu.brand_string"]) {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let info = stdout.trim();
                if !info.is_empty() {
                    metadata.insert("cpu".to_string(), info.to_string());
                }
            }
        }
        if let Ok(uid) = machine_uid::get() {
            let trimmed = uid.trim();
            if !trimmed.is_empty() {
                metadata.insert("baseboard".to_string(), trimmed.to_string());
            }
        }
    }

    metadata
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_name_is_safe() {
        let p = _get_obfuscated_platform();
        assert!(!p.is_empty());
    }

    #[test]
    fn linux_cpu_model_ignores_dynamic_mhz() {
        let sample = "processor\t: 0\nvendor_id\t: GenuineIntel\nmodel name\t: Intel(R) Core(TM) i7-12700H\ncpu MHz\t\t: 2345.678\n";
        assert_eq!(parse_linux_cpu_model(sample), "Intel(R) Core(TM) i7-12700H");
    }

    #[test]
    fn disk_id_validation_blocks_shell_injection() {
        assert!(is_safe_disk_id("nvme0n1p2"));
        assert!(is_safe_disk_id("sda1"));
        assert!(!is_safe_disk_id("sda1; rm -rf /"));
        assert!(!is_safe_disk_id("$(whoami)"));
        assert!(!is_safe_disk_id(""));
    }

    #[test]
    fn machine_data_and_alt_use_consistent_arch() {
        assert!(!normalized_arch().is_empty());
    }
}
