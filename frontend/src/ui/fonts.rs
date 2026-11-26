use eframe::egui;
use std::fs::read;
#[cfg(not(target_os = "windows"))]
use std::path::Path;
#[cfg(target_os = "windows")]
use std::path::PathBuf;

// 配置字体函数
pub fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let font_data = load_system_font();
    fonts.font_data.insert(
        "chinese_font".to_owned(),
        egui::FontData::from_owned(font_data),
    );

    // Proportional 优先使用中文字体；Monospace 追加中文字体作为回退，保持等宽字体对齐（订单号/日志/时间戳）
    if let Some(proportional) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        proportional.insert(0, "chinese_font".to_owned());
    }
    if let Some(monospace) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        monospace.push("chinese_font".to_owned());
    }

    ctx.set_fonts(fonts);
}

// 根据操作系统加载合适的字体
fn load_system_font() -> Vec<u8> {
    #[cfg(target_os = "windows")]
    {
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:/Windows".to_string());
        let fonts_dir = PathBuf::from(system_root).join("Fonts");
        let font_files = ["msyh.ttc", "msyh.ttf", "simhei.ttf", "simsun.ttc"];

        for file in font_files {
            let path = fonts_dir.join(file);
            if path.exists() {
                if let Ok(data) = read(&path) {
                    log::info!("加载系统字体: {}", path.display());
                    return data;
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let font_paths = [
            "/System/Library/Fonts/PingFang.ttc",
            "/Library/Fonts/Arial Unicode.ttf",
            "/System/Library/Fonts/STHeiti Light.ttc",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
        ];

        for path in font_paths {
            if Path::new(path).exists() {
                if let Ok(data) = read(path) {
                    log::info!("加载系统字体: {}", path);
                    return data;
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let font_paths = [
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
        ];

        for path in font_paths {
            if Path::new(path).exists() {
                if let Ok(data) = read(path) {
                    log::info!("加载系统字体: {}", path);
                    return data;
                }
            }
        }
    }

    log::warn!("无法加载系统中文字体，使用内置字体");
    include_bytes!("../../../resources/fonts/NotoSansSC-Regular.otf").to_vec()
}
