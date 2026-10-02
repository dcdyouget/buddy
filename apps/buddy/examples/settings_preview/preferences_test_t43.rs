//! T43：快捷键录制、取消、保存与失败重试。

use super::*;
use buddy_ui::gpui::Modifiers;
use std::{fs, path::PathBuf};

fn config_path() -> PathBuf {
    fixture::sandbox().join("config.json")
}

pub(crate) async fn run(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> bool {
    macro_rules! fail {
        () => {{
            println!("FAIL T43: 输入控件不可用");
            return false;
        }};
    }
    let Some(recorder) = hotkey(handle, cx) else {
        fail!();
    };
    let Some(button) = recorder.read_with(cx, |recorder, _| recorder.button_bounds()) else {
        fail!();
    };
    preferences_test_input::click(handle, button, cx).await;
    let recording = recorder.read_with(cx, |recorder, _| recorder.recording());

    // 裸键与纯修饰键不应完成录制；Escape 只取消录制，设置页仍保持打开。
    preferences_test_input::record(handle, Modifiers::none(), "k", cx).await;
    let bare_kept = recorder.read_with(cx, |recorder, _| recorder.recording());
    preferences_test_input::key_down(handle, "cmd", Modifiers::command(), cx).await;
    preferences_test_input::key_up(handle, "cmd", Modifiers::none(), cx).await;
    preferences_test_input::modifiers(handle, Modifiers::none(), cx).await;
    let modifier_kept = recorder.read_with(cx, |recorder, _| {
        recorder.recording()
            && recorder
                .display_keys_for_test()
                .last()
                .is_some_and(|key| key == "J")
    });
    preferences_test_input::key_down(handle, "escape", Modifiers::none(), cx).await;
    let escaped = !recorder.read_with(cx, |recorder, _| recorder.recording())
        && handle
            .read_with(cx, |router, app| router.settings_view().read(app).active())
            .unwrap_or(false);

    // Tab 也必须被录制器截获，不能被 SettingsView 的父级焦点循环抢走。
    preferences_test_input::key_down(handle, "enter", Modifiers::none(), cx).await;
    let keyboard_start = recorder.read_with(cx, |recorder, _| recorder.recording());
    let tab_mods = Modifiers::command();
    preferences_test_input::record(handle, tab_mods, "tab", cx).await;
    wait_save(handle, cx).await;
    let tab_saved = config(handle, cx).is_some_and(|config| config.hotkey == "CmdOrCtrl+Tab");

    // 再次录制，修饰键先松开也必须保留主键按下时的组合。
    let Some(button) = recorder.read_with(cx, |recorder, _| recorder.button_bounds()) else {
        fail!();
    };
    preferences_test_input::click(handle, button, cx).await;
    let mods = Modifiers {
        platform: true,
        shift: true,
        ..Modifiers::none()
    };
    preferences_test_input::modifiers(handle, mods, cx).await;
    preferences_test_input::key_down(handle, "k", mods, cx).await;
    preferences_test_input::modifiers(handle, Modifiers::none(), cx).await;
    preferences_test_input::key_up(handle, "k", Modifiers::none(), cx).await;
    wait_save(handle, cx).await;
    let saved = config(handle, cx).is_some_and(|config| config.hotkey == "CmdOrCtrl+Shift+K")
        && persisted_config(cx, fixture::sandbox())
            .await
            .is_some_and(|config| config.hotkey == "CmdOrCtrl+Shift+K");
    let display = recorder.read_with(cx, |recorder, _| recorder.display_keys_for_test());
    let expected_display = if cfg!(target_os = "macos") {
        vec!["⌘".to_string(), "⇧".to_string(), "K".to_string()]
    } else {
        vec!["Ctrl".to_string(), "Shift".to_string(), "K".to_string()]
    };
    let display_ok = display == expected_display;

    // 把配置文件替换为目录，使真实 engine 保存失败；旧值和中文错误提示必须保留。
    let path = config_path();
    let backup = path.with_extension("t43-backup");
    let had_file = path.exists();
    if had_file {
        let _ = fs::rename(&path, &backup);
    }
    let _ = fs::create_dir(&path);
    let Some(button) = recorder.read_with(cx, |recorder, _| recorder.button_bounds()) else {
        fail!();
    };
    preferences_test_input::click(handle, button, cx).await;
    let mods = Modifiers {
        platform: true,
        alt: true,
        ..Modifiers::none()
    };
    preferences_test_input::record(handle, mods, "p", cx).await;
    wait_save(handle, cx).await;
    let failure = recorder.read_with(cx, |recorder, _| {
        !recorder.recording()
            && recorder
                .error()
                .is_some_and(|message| message.contains("保存快捷键失败"))
    });
    let memory_kept = config(handle, cx).is_some_and(|config| config.hotkey == "CmdOrCtrl+Shift+K");
    let _ = fs::remove_dir(&path);
    if had_file {
        let _ = fs::rename(&backup, &path);
    }

    // 恢复文件后再次使用真实录制按钮，证明错误状态可重试。
    let Some(button) = recorder.read_with(cx, |recorder, _| recorder.button_bounds()) else {
        fail!();
    };
    preferences_test_input::click(handle, button, cx).await;
    let mods = Modifiers {
        platform: true,
        alt: true,
        ..Modifiers::none()
    };
    preferences_test_input::record(handle, mods, "p", cx).await;
    wait_save(handle, cx).await;
    let retried = config(handle, cx).is_some_and(|config| config.hotkey == "CmdOrCtrl+Alt+P")
        && persisted_config(cx, fixture::sandbox())
            .await
            .is_some_and(|config| config.hotkey == "CmdOrCtrl+Alt+P");
    let ok = recording
        && bare_kept
        && modifier_kept
        && escaped
        && keyboard_start
        && tab_saved
        && saved
        && display_ok
        && failure
        && memory_kept
        && retried;
    println!(
        "T43: 录制态 {recording}，裸键/纯修饰键保持 {bare_kept}/{modifier_kept}，Esc 取消 {escaped}，Enter 启动 {keyboard_start}，CmdTab 保存 {tab_saved}，保存及读盘 {saved}，显示 {display_ok}，失败保留旧值 {failure}/{memory_kept}，重试 {retried}"
    );
    ok
}
