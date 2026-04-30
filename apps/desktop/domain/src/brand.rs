pub const APP_NAME: &str = "驼铃·视频小店差评处理";
pub const APP_NAME_EN: &str = "TLS-shipinhao";
pub const AUTHOR_WECHAT: &str = "TLS-801";

pub fn get_window_title(version: &str) -> String {
    format!("{APP_NAME} {version}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_title_includes_version() {
        assert_eq!(get_window_title("5.1.0"), "驼铃·视频小店差评处理 5.1.0");
    }
}
