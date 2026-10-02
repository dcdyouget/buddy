//! T47：真实 macOS 全局热键切换、旧键失效和流式隐藏重显。

use super::{external_target, fixture, input, os_input};
use buddy_ui::chat::page_state::Page;
use buddy_ui::chat_bridge::spawn_engine;
use buddy_ui::gpui::{
    AppContext, AsyncApp, Capslock, Entity, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers,
    ModifiersChangedEvent, PlatformInput, WindowHandle,
};
use buddy_ui::shell::native::{NativeWindowSnapshot, probe_main_window};
use buddy_ui::shell::{self, AppShell};
use std::sync::Arc;
use std::time::Duration;

const OLD_KEY: &str = "F18";
const NEW_KEY: &str = "F19";
const OLD_CONFIG: &str = "CmdOrCtrl+Alt+Shift+F18";
const NEW_CONFIG: &str = "CmdOrCtrl+Alt+Shift+F19";

fn probe(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<NativeWindowSnapshot> {
    cx.update_window(handle.into(), |_, window, _| probe_main_window(window).ok())
        .ok()
        .flatten()
}

fn visible(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<bool> {
    probe(handle, cx).map(|snapshot| snapshot.is_visible)
}

async fn wait_visible(handle: WindowHandle<AppShell>, expected: bool, cx: &mut AsyncApp) -> bool {
    for attempt in 0..100 {
        if attempt == 0 {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
        }
        input::draw(handle, cx).await;
        if visible(handle, cx) == Some(expected) {
            return true;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    false
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

async fn press_global(
    handle: WindowHandle<AppShell>,
    key: &str,
    expected: bool,
    cx: &mut AsyncApp,
) -> bool {
    if !os_input::combo_down(key) {
        return false;
    }
    let down = if expected {
        wait_visible(handle, expected, cx).await
    } else {
        wait_visible_without_draw(handle, expected, cx).await
    };
    let up = os_input::combo_up(key);
    let settled = if expected {
        wait_visible(handle, expected, cx).await
    } else {
        wait_visible_without_draw(handle, expected, cx).await
    };
    down && up && settled
}

async fn config_with_hotkey(
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

fn router_of(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<Entity<buddy_ui::chat::router::PageRouter>> {
    handle.read_with(cx, |shell, _| shell.router()).ok()
}

fn equivalent_hotkey(left: &str, right: &str) -> bool {
    fn parts(value: &str) -> Vec<String> {
        let mut parts = value
            .split('+')
            .map(|part| part.trim().to_ascii_lowercase())
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        parts.sort_unstable();
        parts
    }
    parts(left) == parts(right)
}

async fn wait_streaming(handle: WindowHandle<AppShell>, expected: bool, cx: &mut AsyncApp) -> bool {
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

async fn wait_streaming_without_draw(
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

async fn wait_file(path: &std::path::Path, cx: &mut AsyncApp) -> bool {
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

async fn record_hotkey(
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

fn block_engine_directory(
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

fn restore_engine_directory(
    engine: &Arc<buddy_engine::chat::ChatEngine>,
    backup: std::path::PathBuf,
) -> bool {
    let directory = engine.data_dir();
    std::fs::remove_file(directory).is_ok() && std::fs::rename(backup, directory).is_ok()
}

pub(crate) async fn run(cx: &mut AsyncApp) -> bool {
    macro_rules! fail {
        ($message:expr) => {{
            println!("FAIL T47：{}", $message);
            return false;
        }};
    }

    let preflight = os_input::preflight();
    if preflight != os_input::Preflight::Ready {
        println!("FAIL T47：{}", os_input::preflight_message(preflight));
        return false;
    }
    let engine = fixture::manual_engine(false);
    if let Err(error) = config_with_hotkey(&engine, OLD_CONFIG, cx).await {
        fail!(format!("写入预览热键失败：{error}"));
    }
    let handle = match shell::open_main_window(engine.clone(), fixture::shell_config(), cx).await {
        Ok(handle) => handle,
        Err(error) => fail!(format!("打开主窗口失败：{error}")),
    };
    input::settle(handle, cx).await;
    let Some(router) = router_of(handle, cx) else {
        fail!("读取 Router 失败");
    };
    let initial = visible(handle, cx) == Some(true);
    let installed = shell::runtime::install(handle, cx).await.is_ok();
    let shown = shell::runtime::show(handle, cx).await.is_ok();
    cx.update(|app| app.activate(true));
    input::draw(handle, cx).await;
    let shown_and_focused =
        shown && probe(handle, cx).is_some_and(|snapshot| snapshot.is_visible && snapshot.is_key);

    // 按下与释放分别注入；释放阶段不得再次切换。
    let old_hide = press_global(handle, OLD_KEY, false, cx).await;
    let old_show = press_global(handle, OLD_KEY, true, cx).await;

    let config_updated = record_hotkey(handle, &router, "f19", NEW_CONFIG, cx).await;
    let registered = cx
        .update(|app| shell::runtime::registered_hotkey(app))
        .is_some_and(|value| value.contains("F19"));
    let _ = router.update(cx, |router, cx| router.close_settings(cx));
    let old_ignored = os_input::combo(OLD_KEY);
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;
    let old_ignored = old_ignored && visible(handle, cx) == Some(true);
    let mut unfocused_child = external_target::spawn().ok();
    let unfocused = if let Some(target) = unfocused_child.as_ref() {
        wait_file(&target.ready, cx).await
            && probe(handle, cx).is_some_and(|snapshot| snapshot.is_visible && !snapshot.is_key)
    } else {
        false
    };
    let refocused = if unfocused {
        press_global(handle, NEW_KEY, true, cx).await
            && probe(handle, cx).is_some_and(|snapshot| snapshot.is_visible && snapshot.is_key)
    } else {
        false
    };
    if let Some(mut target) = unfocused_child.take() {
        let _ = target.child.kill();
        let _ = target.child.wait();
        let _ = std::fs::remove_file(target.ready);
        let _ = std::fs::remove_file(target.ack);
    }
    let new_hide = press_global(handle, NEW_KEY, false, cx).await;
    let new_show = press_global(handle, NEW_KEY, true, cx).await;

    let Some(backup) = block_engine_directory(&engine) else {
        fail!("无法构造真实保存失败沙盒");
    };
    let failure_attempt = !record_hotkey(handle, &router, "f18", OLD_CONFIG, cx).await;
    let failure_error = router.read_with(cx, |router, app| {
        router
            .settings_view()
            .read(app)
            .hotkey_recorder()
            .read(app)
            .error()
            .is_some_and(|message| message.contains("保存快捷键失败"))
    });
    let memory_kept = router.read_with(cx, |router, _| router.config().hotkey == NEW_CONFIG);
    let registered_kept = cx
        .update(|app| shell::runtime::registered_hotkey(app))
        .is_some_and(|value| equivalent_hotkey(&value, NEW_CONFIG));
    let restored = restore_engine_directory(&engine, backup);
    let disk_kept = if restored {
        let read_engine = engine.clone();
        cx.update(|app| spawn_engine(app, async move { read_engine.get_config().await }))
            .await
            .is_ok_and(|config| config.hotkey == NEW_CONFIG)
    } else {
        false
    };
    let _ = router.update(cx, |router, cx| router.close_settings(cx));
    let failure_recovery =
        failure_attempt && failure_error && memory_kept && registered_kept && disk_kept && restored;

    let same_router = router_of(handle, cx).is_some_and(|current| current == router);
    let pasted = input::paste_text(handle, "慢", cx).await;
    input::press(handle, "enter", cx).await;
    let streaming = wait_streaming(handle, true, cx).await;
    let hidden_stream = press_global(handle, NEW_KEY, false, cx).await;
    let completed_hidden = wait_streaming_without_draw(handle, false, cx).await;
    let reopened = press_global(handle, NEW_KEY, true, cx).await;
    let complete_page = handle
        .read_with(cx, |shell, app| {
            shell.router().read(app).page() == Page::Conversation
        })
        .unwrap_or(false);
    let complete_text = handle
        .read_with(cx, |shell, app| {
            shell
                .router()
                .read(app)
                .conversation()
                .read(app)
                .state
                .messages
                .last()
                .is_some_and(|message| message.content.contains("79,"))
        })
        .unwrap_or(false);

    let ok = initial
        && installed
        && shown_and_focused
        && old_hide
        && old_show
        && config_updated
        && registered
        && old_ignored
        && unfocused
        && refocused
        && new_hide
        && new_show
        && failure_recovery
        && same_router
        && pasted
        && streaming
        && hidden_stream
        && completed_hidden
        && reopened
        && complete_page
        && complete_text;
    println!(
        "T47: 权限/初始/安装/聚焦 {}/{}/{}/{}；旧键隐藏/重显 {}/{}；更新/注册/旧键失效 {}/{}/{}；失焦/热键唤回 {}/{}；新键隐藏/重显 {}/{}；保存失败恢复 {}/{}/{}/{}/{}/{}/{}；同 Router/输入/流式 {}/{}/{}；隐藏/完成/重开/终态 {}/{}/{}/{}/{}",
        os_input::can_post(),
        initial,
        installed,
        shown_and_focused,
        old_hide,
        old_show,
        config_updated,
        registered,
        old_ignored,
        unfocused,
        refocused,
        new_hide,
        new_show,
        failure_attempt,
        failure_error,
        memory_kept,
        registered_kept,
        disk_kept,
        restored,
        failure_recovery,
        same_router,
        pasted,
        streaming,
        hidden_stream,
        completed_hidden,
        reopened,
        complete_page,
        complete_text,
    );
    ok
}
