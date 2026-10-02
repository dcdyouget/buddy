//! T47 的窗口、系统输入、录制和 engine 沙盒辅助函数。

use super::super::{fixture, input, os_input};
use buddy_ui::chat::page_state::Page;
use buddy_ui::chat_bridge::spawn_engine;
use buddy_ui::gpui::{
    AppContext, AsyncApp, Capslock, Entity, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers,
    ModifiersChangedEvent, PlatformInput, WindowHandle,
};
use buddy_ui::shell::AppShell;
use buddy_ui::shell::native::{NativeWindowSnapshot, probe_main_window};
use std::sync::Arc;
use std::time::Duration;

pub(super) fn probe(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<NativeWindowSnapshot> {
    cx.update_window(handle.into(), |_, window, _| probe_main_window(window).ok())
        .ok()
        .flatten()
}

pub(super) fn visible(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<bool> {
    probe(handle, cx).map(|snapshot| snapshot.is_visible)
}

async fn wait_visible_without_draw(
    handle: WindowHandle<AppShell>,
    expected: bool,
    cx: &mut AsyncApp,
) -> bool {
    for attempt in 0..120 {
        if attempt == 0 {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
        }
        if visible(handle, cx) == Some(expected) {
            return true;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    false
}

/// AppKit 激活和 key-window 转移可能在 `show()` 返回后才完成；给原生事件循环
/// 最多两秒完成转移，同时保留最终状态的硬断言。
pub(super) async fn wait_shown_and_focused(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<NativeWindowSnapshot> {
    for _ in 0..100 {
        if let Some(snapshot) = probe(handle, cx) {
            if snapshot.is_visible && snapshot.is_key {
                return Some(snapshot);
            }
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    None
}

pub(super) async fn press_global(
    handle: WindowHandle<AppShell>,
    key: &str,
    expected: bool,
    cx: &mut AsyncApp,
) -> bool {
    if !os_input::combo_down(key) {
        return false;
    }
    let down = if expected {
        wait_shown_and_focused(handle, cx).await.is_some()
    } else {
        wait_visible_without_draw(handle, expected, cx).await
    };
    let up = os_input::combo_up(key);
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;
    let settled = if expected {
        wait_shown_and_focused(handle, cx).await.is_some()
    } else {
        wait_visible_without_draw(handle, expected, cx).await
    };
    down && up && settled
}

pub(super) async fn config_with_hotkey(
    engine: &Arc<buddy_engine::chat::ChatEngine>,
    hotkey: &str,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let engine = engine.clone();
    let read_engine = engine.clone();
    let mut config = cx
        .update(|app| spawn_engine(app, async move { read_engine.get_config().await }))
        .await?;
    config.hotkey = hotkey.to_owned();
    let save_engine = engine.clone();
    cx.update(|app| spawn_engine(app, async move { save_engine.save_config(config).await }))
        .await?;
    Ok(())
}

pub(super) fn router_of(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<Entity<buddy_ui::chat::router::PageRouter>> {
    handle.read_with(cx, |shell, _| shell.router()).ok()
}

pub(super) fn equivalent_hotkey(left: &str, right: &str) -> bool {
    fn parts(value: &str) -> Vec<String> {
        let mut parts = value
            .split('+')
            .map(|part| part.trim().to_ascii_lowercase())
            .map(|part| match part.as_str() {
                // global-hotkey::HotKey::to_string() emits the platform
                // modifier names ("super" / "control"), while the config
                // keeps the portable "CmdOrCtrl" spelling.
                "super" | "cmd" | "meta" => "cmdorctrl".into(),
                "control" | "ctrl" => "ctrl".into(),
                "option" => "alt".into(),
                _ => part,
            })
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        parts.sort_unstable();
        parts
    }
    parts(left) == parts(right)
}

pub(super) async fn wait_streaming(
    handle: WindowHandle<AppShell>,
    expected: bool,
    cx: &mut AsyncApp,
) -> bool {
    for _ in 0..180 {
        input::draw(handle, cx).await;
        if handle
            .read_with(cx, |shell, app| {
                shell
                    .router()
                    .read(app)
                    .conversation()
                    .read(app)
                    .state
                    .is_streaming()
                    == expected
            })
            .unwrap_or(false)
        {
            return true;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    false
}

pub(super) async fn wait_streaming_without_draw(
    handle: WindowHandle<AppShell>,
    expected: bool,
    cx: &mut AsyncApp,
) -> bool {
    for _ in 0..220 {
        if handle
            .read_with(cx, |shell, app| {
                shell
                    .router()
                    .read(app)
                    .conversation()
                    .read(app)
                    .state
                    .is_streaming()
                    == expected
            })
            .unwrap_or(false)
        {
            return true;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    false
}

pub(super) async fn wait_file(path: &std::path::Path, cx: &mut AsyncApp) -> bool {
    for _ in 0..150 {
        if path.is_file() {
            return true;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    false
}

async fn dispatch_modifiers(handle: WindowHandle<AppShell>, mods: Modifiers, cx: &mut AsyncApp) {
    let event = PlatformInput::ModifiersChanged(ModifiersChangedEvent {
        modifiers: mods,
        capslock: Capslock::default(),
    });
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_event(event, cx)
    });
    input::draw(handle, cx).await;
}

async fn dispatch_key_down(
    handle: WindowHandle<AppShell>,
    key: &str,
    mods: Modifiers,
    cx: &mut AsyncApp,
) {
    let event = PlatformInput::KeyDown(KeyDownEvent {
        keystroke: Keystroke {
            modifiers: mods,
            key: key.into(),
            key_char: None,
        },
        is_held: false,
        prefer_character_input: false,
    });
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_event(event, cx)
    });
    input::draw(handle, cx).await;
}

async fn dispatch_key_up(
    handle: WindowHandle<AppShell>,
    key: &str,
    mods: Modifiers,
    cx: &mut AsyncApp,
) {
    let event = PlatformInput::KeyUp(KeyUpEvent {
        keystroke: Keystroke {
            modifiers: mods,
            key: key.into(),
            key_char: None,
        },
    });
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_event(event, cx)
    });
    input::draw(handle, cx).await;
}

pub(super) async fn record_hotkey(
    handle: WindowHandle<AppShell>,
    router: &Entity<buddy_ui::chat::router::PageRouter>,
    key: &str,
    expected_config: &str,
    cx: &mut AsyncApp,
) -> bool {
    let _ = router.update(cx, |router, cx| router.open_settings(cx));
    for _ in 0..100 {
        input::draw(handle, cx).await;
        if router.read_with(cx, |router, app| {
            router.page() == Page::Settings && router.settings_view().read(app).active()
        }) {
            break;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    input::settle(handle, cx).await;
    let recorder = router.read_with(cx, |router, app| {
        router.settings_view().read(app).hotkey_recorder().clone()
    });
    let Some(bounds) = recorder.read_with(cx, |recorder, _| recorder.button_bounds_for_test())
    else {
        return false;
    };
    input::click(
        handle,
        f32::from(bounds.origin.x + bounds.size.width / 2.0),
        f32::from(bounds.origin.y + bounds.size.height / 2.0),
        false,
        cx,
    )
    .await;
    let mods = Modifiers {
        platform: true,
        alt: true,
        shift: true,
        ..Modifiers::none()
    };
    dispatch_modifiers(handle, mods, cx).await;
    dispatch_key_down(handle, key, mods, cx).await;
    dispatch_key_up(handle, key, mods, cx).await;
    dispatch_modifiers(handle, Modifiers::none(), cx).await;
    for _ in 0..100 {
        if let Some(task) = router.update(cx, |router, _| router.take_config_save()) {
            task.await;
            break;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    router.read_with(cx, |router, app| {
        equivalent_hotkey(&router.config().hotkey, expected_config)
            && !router
                .settings_view()
                .read(app)
                .hotkey_recorder()
                .read(app)
                .saving()
    })
}

pub(super) fn block_engine_directory(
    engine: &Arc<buddy_engine::chat::ChatEngine>,
) -> Option<std::path::PathBuf> {
    let directory = engine.data_dir().to_path_buf();
    let backup = directory.with_extension("t47-directory-backup");
    if std::fs::rename(&directory, &backup).is_err() {
        return None;
    }
    if std::fs::write(&directory, b"t47 blocked").is_err() {
        let _ = std::fs::rename(&backup, &directory);
        return None;
    }
    Some(backup)
}

pub(super) fn restore_engine_directory(
    engine: &Arc<buddy_engine::chat::ChatEngine>,
    backup: std::path::PathBuf,
) -> bool {
    let directory = engine.data_dir();
    std::fs::remove_file(directory).is_ok() && std::fs::rename(backup, directory).is_ok()
}
