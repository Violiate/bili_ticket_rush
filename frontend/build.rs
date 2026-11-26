use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/app_icon.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_dir = match env::var("CARGO_MANIFEST_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => return,
    };
    let out_dir = match env::var("OUT_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => return,
    };

    let ico_src = manifest_dir.join("assets").join("app_icon.ico");
    if !ico_src.exists() {
        println!("cargo:warning=未找到 frontend/assets/app_icon.ico，跳过 Windows exe 图标嵌入");
        return;
    }

    // 复制到 OUT_DIR 并使用纯 ASCII 相对文件名，彻底规避 rc.exe / windres 解析含中文路径失败的问题
    let ico_dst = out_dir.join("app_icon.ico");
    let rc_path = out_dir.join("app_icon.rc");
    let res_path = out_dir.join("app_icon.res");

    if let Err(err) = fs::copy(&ico_src, &ico_dst) {
        println!("cargo:warning=复制 app_icon.ico 到 OUT_DIR 失败: {}", err);
        return;
    }
    if let Err(err) = fs::write(&rc_path, "1 ICON \"app_icon.ico\"\n") {
        println!("cargo:warning=写入 app_icon.rc 失败: {}", err);
        return;
    }

    let compiled = try_compile_with_rc(&out_dir) || try_compile_with_windres(&out_dir);
    if compiled && res_path.exists() {
        // 仅对可执行二进制链接资源，避免影响 lib/test 目标
        println!("cargo:rustc-link-arg-bins={}", res_path.display());
    } else {
        println!(
            "cargo:warning=未找到可用的 rc.exe 或 windres，Windows exe 文件图标未嵌入（窗口运行时图标仍正常生效）"
        );
    }
}

fn try_compile_with_rc(out_dir: &Path) -> bool {
    let mut candidates: Vec<PathBuf> = vec![PathBuf::from("rc.exe")];
    if let Some(sdk_dir) = env::var_os("WindowsSdkDir") {
        if let Some(sdk_ver) = env::var_os("WindowsSDKVersion") {
            let ver = sdk_ver.to_string_lossy().trim_end_matches('\\').to_string();
            candidates.push(PathBuf::from(sdk_dir).join("bin").join(ver).join("x64").join("rc.exe"));
        }
    }
    let kits_bin = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    if let Ok(entries) = fs::read_dir(kits_bin) {
        let mut versions: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        versions.sort();
        for dir in versions.into_iter().rev() {
            let rc = dir.join("x64").join("rc.exe");
            if rc.exists() {
                candidates.push(rc);
            }
        }
    }

    for rc in candidates {
        let ok = Command::new(&rc)
            .current_dir(out_dir)
            .args(["/nologo", "/fo", "app_icon.res", "app_icon.rc"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return true;
        }
    }
    false
}

fn try_compile_with_windres(out_dir: &Path) -> bool {
    let candidates = [
        "windres",
        "x86_64-w64-mingw32-windres",
        r"E:\msys64\mingw64\bin\windres.exe",
        r"C:\msys64\mingw64\bin\windres.exe",
    ];
    for tool in candidates {
        let ok = Command::new(tool)
            .current_dir(out_dir)
            .args(["-i", "app_icon.rc", "-O", "res", "-o", "app_icon.res"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return true;
        }
    }
    false
}
