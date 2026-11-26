/// 应用对外展示的发布版本号。
/// 发布新版本时直接修改此处即可；也支持在编译命令中通过环境变量 `APP_VERSION` 覆盖
/// （例如：`APP_VERSION=6.7.0 cargo build --release`）。
pub const APP_VERSION: &str = "6.6.6";

/// 内部策略客户端兼容版本号（勿随意改动）。
pub(crate) const POLICY_CLIENT_VERSION: &str = "6.6.6";

/// 返回规范化后的对外展示版本号（不含前导 `v`）。
pub fn display_version() -> String {
    normalize_version(option_env!("APP_VERSION").unwrap_or(APP_VERSION))
}

fn normalize_version(raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches(['v', 'V']).trim();
    if trimmed.is_empty() {
        APP_VERSION.to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_version, APP_VERSION, POLICY_CLIENT_VERSION};

    #[test]
    fn normalize_version_strips_prefix_and_falls_back_when_empty() {
        assert_eq!(normalize_version("v6.7.0"), "6.7.0");
        assert_eq!(normalize_version(" V7.0.1-beta "), "7.0.1-beta");
        assert_eq!(normalize_version("6.6.8"), "6.6.8");
        assert_eq!(normalize_version("   "), APP_VERSION);
        assert_eq!(normalize_version("v"), APP_VERSION);
        assert_eq!(POLICY_CLIENT_VERSION, "6.6.6");
    }
}
