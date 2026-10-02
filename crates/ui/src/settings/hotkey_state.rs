//! 快捷键录制的纯状态机与 v1 键名格式化。

use gpui::Modifiers;

/// 录制结果；纯修饰键和无修饰裸键都保持录制状态，不会产生候选值。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RecordOutcome {
    None,
    Cancelled,
    Changed(String),
}

/// 不依赖 GPUI 窗口的录制状态，便于覆盖真实按键序列。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RecorderState {
    pub(crate) recording: bool,
    modifiers: Modifiers,
    captured_modifiers: Modifiers,
    captured_key: Option<String>,
    pub(crate) preview: Vec<String>,
}

impl RecorderState {
    pub(crate) fn start(&mut self) {
        self.recording = true;
        self.modifiers = Modifiers::none();
        self.captured_modifiers = Modifiers::none();
        self.captured_key = None;
        self.preview.clear();
    }

    pub(crate) fn cancel(&mut self) -> RecordOutcome {
        self.recording = false;
        self.clear_capture();
        RecordOutcome::Cancelled
    }

    fn clear_capture(&mut self) {
        self.captured_modifiers = Modifiers::none();
        self.captured_key = None;
        self.preview.clear();
    }

    pub(crate) fn key_down(&mut self, modifiers: Modifiers, key: &str) -> RecordOutcome {
        if !self.recording {
            return RecordOutcome::None;
        }
        self.modifiers = modifiers;
        if is_escape(key) {
            return self.cancel();
        }
        if is_modifier_key(key) {
            self.preview = modifier_names(modifiers);
            return RecordOutcome::None;
        }

        let normalized = canonical_key(key);
        self.captured_modifiers = modifiers;
        self.captured_key = Some(normalized.clone());
        self.preview = combination_names(modifiers, &normalized);
        RecordOutcome::None
    }

    pub(crate) fn modifiers_changed(&mut self, modifiers: Modifiers) {
        if !self.recording {
            return;
        }
        self.modifiers = modifiers;
        if self.captured_key.is_none() {
            self.preview = modifier_names(modifiers);
        }
    }

    pub(crate) fn key_up(&mut self, modifiers: Modifiers, key: &str) -> RecordOutcome {
        if !self.recording || is_modifier_key(key) {
            if self.recording && is_modifier_key(key) && !modifiers.modified() {
                self.preview.clear();
            }
            return RecordOutcome::None;
        }
        let Some(captured_key) = self.captured_key.clone() else {
            return RecordOutcome::None;
        };

        // Some platforms omit modifiers from KeyUp. Keep the KeyDown snapshot, as v1 does.
        let captured_modifiers = self.captured_modifiers;
        if modifier_names(captured_modifiers).is_empty() {
            self.clear_capture();
            return RecordOutcome::None;
        }
        let value = combination_names(captured_modifiers, &captured_key).join("+");
        self.recording = false;
        self.clear_capture();
        RecordOutcome::Changed(value)
    }
}

pub(crate) fn modifier_names(modifiers: Modifiers) -> Vec<String> {
    let mut names = Vec::new();
    if modifiers.platform || modifiers.control {
        names.push("CmdOrCtrl".to_string());
    }
    if modifiers.shift {
        names.push("Shift".to_string());
    }
    if modifiers.alt {
        names.push("Alt".to_string());
    }
    names
}

fn combination_names(modifiers: Modifiers, key: &str) -> Vec<String> {
    let mut names = modifier_names(modifiers);
    names.push(key.to_string());
    names
}

fn is_escape(key: &str) -> bool {
    matches!(key.to_ascii_lowercase().as_str(), "escape" | "esc")
}

fn is_modifier_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "cmd"
            | "command"
            | "meta"
            | "super"
            | "win"
            | "ctrl"
            | "control"
            | "shift"
            | "alt"
            | "option"
            | "fn"
            | "function"
    )
}

pub(crate) fn canonical_key(key: &str) -> String {
    let lower = key.to_ascii_lowercase();
    match lower.as_str() {
        " " | "space" => "Space".to_string(),
        "enter" | "return" => "Enter".to_string(),
        "escape" | "esc" => "Escape".to_string(),
        "tab" => "Tab".to_string(),
        "backspace" | "back" => "Backspace".to_string(),
        "delete" | "del" => "Delete".to_string(),
        "arrowup" | "up" => "ArrowUp".to_string(),
        "arrowdown" | "down" => "ArrowDown".to_string(),
        "arrowleft" | "left" => "ArrowLeft".to_string(),
        "arrowright" | "right" => "ArrowRight".to_string(),
        _ if key.chars().count() == 1 => key.to_uppercase(),
        _ => {
            let mut chars = lower.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        }
    }
}

pub(crate) fn display_key(key: &str) -> String {
    let mac = cfg!(target_os = "macos");
    match (mac, key) {
        (true, "CmdOrCtrl") => "⌘".into(),
        (true, "Cmd") => "⌘".into(),
        (true, "Ctrl") => "⌃".into(),
        (true, "Shift") => "⇧".into(),
        (true, "Alt") | (true, "Option") => "⌥".into(),
        (true, "Space") => "␣".into(),
        (true, "Enter") | (true, "Return") => "↵".into(),
        (true, "Escape") => "Esc".into(),
        (true, "Tab") => "⇥".into(),
        (true, "Backspace") => "⌫".into(),
        (false, "CmdOrCtrl") | (false, "Ctrl") => "Ctrl".into(),
        (false, "Cmd") => "Win".into(),
        (false, "Alt") | (false, "Option") => "Alt".into(),
        (false, "Space") => "Space".into(),
        (false, "Enter") | (false, "Return") => "Enter".into(),
        (false, "Escape") => "Esc".into(),
        (false, "Backspace") => "Bksp".into(),
        _ => key.to_string(),
    }
}

pub(crate) fn display_keys(current: &str, state: &RecorderState) -> Vec<String> {
    let keys = if state.recording && !state.preview.is_empty() {
        state.preview.clone()
    } else {
        current
            .split('+')
            .filter(|key| !key.is_empty())
            .map(str::to_owned)
            .collect()
    };
    keys.into_iter().map(|key| display_key(&key)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods() -> Modifiers {
        Modifiers {
            platform: true,
            shift: true,
            ..Modifiers::none()
        }
    }

    #[test]
    fn records_combination_on_main_key_release() {
        let mut state = RecorderState::default();
        state.start();
        assert_eq!(state.key_down(mods(), "k"), RecordOutcome::None);
        assert_eq!(
            state.key_up(Modifiers::none(), "k"),
            RecordOutcome::Changed("CmdOrCtrl+Shift+K".into())
        );
        assert!(!state.recording);
    }

    #[test]
    fn pure_modifiers_clear_preview_and_keep_recording() {
        let mut state = RecorderState::default();
        state.start();
        state.key_down(mods(), "shift");
        assert_eq!(state.preview, ["CmdOrCtrl", "Shift"]);
        state.key_up(Modifiers::none(), "shift");
        assert!(state.recording);
        assert!(state.preview.is_empty());
    }

    #[test]
    fn bare_key_is_rejected() {
        let mut state = RecorderState::default();
        state.start();
        state.key_down(Modifiers::none(), "a");
        assert_eq!(state.key_up(Modifiers::none(), "a"), RecordOutcome::None);
        assert!(state.recording);
    }

    #[test]
    fn function_only_modifier_is_rejected() {
        let mut state = RecorderState::default();
        state.start();
        let function = Modifiers {
            function: true,
            ..Modifiers::none()
        };
        state.key_down(function, "k");
        assert_eq!(state.key_up(Modifiers::none(), "k"), RecordOutcome::None);
        assert!(state.recording);
    }

    #[test]
    fn keeps_keydown_modifiers_after_modifiers_changed_release() {
        let mut state = RecorderState::default();
        state.start();
        state.modifiers_changed(mods());
        state.key_down(mods(), "k");
        state.modifiers_changed(Modifiers::none());
        assert_eq!(
            state.key_up(Modifiers::none(), "k"),
            RecordOutcome::Changed("CmdOrCtrl+Shift+K".into())
        );
    }

    #[test]
    fn escape_cancels_without_candidate() {
        let mut state = RecorderState::default();
        state.start();
        assert_eq!(state.key_down(mods(), "escape"), RecordOutcome::Cancelled);
        assert!(!state.recording);
        assert!(state.preview.is_empty());
    }

    #[test]
    fn canonical_names_match_v1_display_input() {
        assert_eq!(canonical_key(" "), "Space");
        assert_eq!(canonical_key("arrowup"), "ArrowUp");
        assert_eq!(
            display_key("CmdOrCtrl"),
            if cfg!(target_os = "macos") {
                "⌘"
            } else {
                "Ctrl"
            }
        );
    }
}
