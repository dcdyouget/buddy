//! 搜索入口和返回入口共用的平台快捷键。

pub(crate) const LABEL: &str = if cfg!(target_os = "macos") {
    "⌘F"
} else {
    "Ctrl+F"
};

pub(crate) fn matches_keystroke(key: &gpui::Keystroke) -> bool {
    matches_platform(key, cfg!(target_os = "macos"))
}

fn matches_platform(key: &gpui::Keystroke, macos: bool) -> bool {
    let modifiers = key.modifiers;
    key.key == "f"
        && !modifiers.alt
        && !modifiers.shift
        && if macos {
            modifiers.platform && !modifiers.control
        } else {
            modifiers.control && !modifiers.platform
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_uses_command_on_mac_and_control_on_windows() {
        for (key, mac, windows) in [
            ("cmd-f", true, false),
            ("ctrl-f", false, true),
            ("f", false, false),
            ("cmd-ctrl-f", false, false),
            ("cmd-shift-f", false, false),
            ("ctrl-shift-f", false, false),
            ("alt-ctrl-f", false, false),
        ] {
            let key = gpui::Keystroke::parse(key).unwrap();
            assert_eq!(matches_platform(&key, true), mac);
            assert_eq!(matches_platform(&key, false), windows);
        }
    }
}
