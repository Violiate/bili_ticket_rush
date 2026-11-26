// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use eframe::epaint::Vec2;

mod app;
mod ui;
mod version;
mod windows;

fn is_dir_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".biliticket_write_probe_{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn default_user_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| {
            PathBuf::from(h)
                .join("Library")
                .join("Application Support")
                .join("BiliTicketRush")
        })
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
            return Some(PathBuf::from(xdg).join("BiliTicketRush"));
        }
        std::env::var_os("HOME").map(|h| {
            PathBuf::from(h)
                .join(".local")
                .join("share")
                .join("BiliTicketRush")
        })
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .or_else(|| std::env::var_os("LOCALAPPDATA"))
            .map(|d| PathBuf::from(d).join("BiliTicketRush"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

/// 确保进程工作目录可写，避免 macOS `.app` 从 Finder 启动（CWD 为只读 `/`）
/// 或 Linux/Windows 从只读目录启动时无法写入配置、日志与缓存文件。
fn ensure_writable_working_dir() {
    let cwd = std::env::current_dir().ok();
    let is_root_dir = cwd
        .as_deref()
        .map(|p| p == Path::new("/"))
        .unwrap_or(false);

    #[cfg(target_os = "macos")]
    let launched_from_app_bundle = std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.contains(".app/Contents/MacOS")))
        .unwrap_or(false);
    #[cfg(not(target_os = "macos"))]
    let launched_from_app_bundle = false;

    let cwd_usable = !launched_from_app_bundle
        && !is_root_dir
        && cwd.as_deref().map(is_dir_writable).unwrap_or(false);

    if cwd_usable {
        return;
    }

    // Windows 优先回退到可执行文件所在目录（保持绿色便携体验），其次用户数据目录
    #[cfg(target_os = "windows")]
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            if is_dir_writable(exe_dir) && std::env::set_current_dir(exe_dir).is_ok() {
                return;
            }
        }
    }

    if let Some(data_dir) = default_user_data_dir() {
        if std::fs::create_dir_all(&data_dir).is_ok()
            && std::env::set_current_dir(&data_dir).is_ok()
        {
            return;
        }
    }

    eprintln!("警告：未能切换到可写工作目录，配置与日志可能无法正常保存");
}

fn load_app_icon() -> Option<eframe::IconData> {
    let icon_bytes = include_bytes!("../assets/app_icon.png");
    let img = match image::load_from_memory(icon_bytes) {
        Ok(img) => img.to_rgba8(),
        Err(err) => {
            log::warn!("加载内置应用图标失败: {}", err);
            return None;
        }
    };
    let (width, height) = img.dimensions();
    Some(eframe::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

fn show_single_instance_alert() {
    let message = "程序已经在运行中，请勿重复启动！";
    eprintln!("{}", message);
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use winapi::um::winuser::{MessageBoxW, MB_ICONWARNING, MB_OK};

        let title_wide: Vec<u16> = OsStr::new("提示")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let msg_wide: Vec<u16> = OsStr::new(message)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                msg_wide.as_ptr(),
                title_wide.as_ptr(),
                MB_OK | MB_ICONWARNING,
            );
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("osascript")
            .args([
                "-e",
                "display dialog \"程序已经在运行中，请勿重复启动！\" with title \"提示\" buttons {\"确定\"} default button \"确定\" with icon caution",
            ])
            .status();
    }
    #[cfg(target_os = "linux")]
    {
        let shown = std::process::Command::new("zenity")
            .args([
                "--warning",
                "--title=提示",
                "--text=程序已经在运行中，请勿重复启动！",
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !shown {
            let _ = std::process::Command::new("notify-send")
                .args(["提示", message])
                .status();
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    ensure_writable_working_dir();

    #[cfg(target_os = "linux")]
    {
        // 仅在用户显式要求软件渲染时覆盖 GL 环境变量，避免在有 GPU 的环境下无故降级性能
        if std::env::var("BILI_FORCE_SOFTWARE_GL").as_deref() == Ok("1") {
            std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
            std::env::set_var("MESA_GL_VERSION_OVERRIDE", "3.3");
            std::env::set_var("GALLIUM_DRIVER", "llvmpipe");
        }
    }

    if let Err(e) = common::init_logger() {
        eprintln!("初始化日志失败，原因: {}", e);
    }
    log::info!("日志初始化成功");

    // 避免在 panic hook 中递归调用 logger（以防日志锁已被持有导致死锁/二次 panic）
    // CollectorLogger::flush 内部使用 try_lock，可安全在退出前将已有日志刷盘
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("程序发生严重异常(panic): {}", panic_info);
        log::logger().flush();
    }));

    // 检查程序是否已经在运行
    if !common::utils::ensure_single_instance() {
        show_single_instance_alert();
        std::process::exit(1);
    }

    let options = eframe::NativeOptions {
        initial_window_size: Some(Vec2::new(1200.0, 680.0)),
        min_window_size: Some(Vec2::new(840.0, 600.0)),
        icon_data: load_app_icon(),
        vsync: true,
        ..Default::default()
    };

    eframe::run_native(
        "原神",
        options,
        Box::new(|cc| Box::new(app::Myapp::new(cc))),
    )
}
