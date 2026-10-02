//! T50：全屏 Space 与普通 level 主窗口的真实互相覆盖验收。
//!
//! 主窗口从 Runtime 取回，避免第二次 install；候选 child 通过独立进程真实进入全屏
//! Space。流程中的三段 checkpoint 可供主 agent 用 `screencapture` 读取真实屏幕，普通
//! 自测没有人工等待。

use super::{fullscreen_target, input, level_native, os_input};
use buddy_ui::gpui::{AsyncApp, WindowHandle};
use buddy_ui::shell::{self, AppShell};
use std::path::{Path, PathBuf};
use std::time::Duration;

const HOTKEY: &str = "N";
const CHECKPOINT_ENV: &str = "BUDDY_SHELL_T50_CAPTURE_DIR";

async fn wait_file(path: &Path, cx: &mut AsyncApp) -> bool {
    for _ in 0..250 {
        if path.is_file() {
            return true;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    false
}

async fn checkpoint(dir: &Option<PathBuf>, phase: &str, cx: &mut AsyncApp) -> bool {
    let Some(dir) = dir else {
        return true;
    };
    let ready = dir.join(format!("{phase}.ready"));
    let proceed = dir.join(format!("{phase}.continue"));
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::remove_file(&proceed);
    if std::fs::write(&ready, phase).is_err() {
        return false;
    }
    let mut passed = false;
    for _ in 0..4500 {
        if proceed.is_file() {
            passed = true;
            break;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    let _ = std::fs::remove_file(&ready);
    let _ = std::fs::remove_file(&proceed);
    passed
}

async fn wait_main_active(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<level_native::NativeLevelSnapshot> {
    for _ in 0..150 {
        input::draw(handle, cx).await;
        if let Some(snapshot) = level_native::probe(handle, cx)
            && snapshot.native.is_visible
            && snapshot.active
            && snapshot.window_in_stack
        {
            return Some(snapshot);
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    None
}

fn read_status(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn status_window_number(status: Option<&String>) -> Option<i64> {
    status?
        .split_whitespace()
        .find_map(|part| part.strip_prefix("window=")?.parse().ok())
}

pub(crate) async fn run(cx: &mut AsyncApp) -> bool {
    macro_rules! fail {
        ($message:expr) => {{
            println!("FAIL T50：{}", $message);
            return false;
        }};
    }
    let Some(handle) = cx.update(|app| shell::runtime::main_window(app)) else {
        fail!("Runtime 没有唯一主窗口");
    };
    let capture_dir = std::env::var_os(CHECKPOINT_ENV).map(PathBuf::from);
    let child = match fullscreen_target::spawn() {
        Ok(child) => child,
        Err(error) => fail!(format!("启动全屏 child 失败：{error}")),
    };
    let ready = wait_file(&child.ready, cx).await;
    let child_before = read_status(&child.status);
    let child_ready = ready
        && child_before
            .as_deref()
            .is_some_and(|value| value.contains("fullscreen=true active=true"));
    let _ = shell::runtime::hide(handle, cx).await;
    let before_capture = checkpoint(&capture_dir, "fullscreen-child", cx).await;

    let hotkey_sent = child_ready && os_input::combo(HOTKEY);
    let main_snapshot = if hotkey_sent {
        wait_main_active(handle, cx).await
    } else {
        None
    };
    let main_level = main_snapshot
        .as_ref()
        .is_some_and(level_native::expected_main);
    // Request a fresh child-side native sample after Buddy activates; stale READY is not evidence.
    let _ = std::fs::write(child.status.with_extension("query"), "buddy-shown");
    let mut child_during_main = None;
    for _ in 0..100 {
        let status = read_status(&child.status);
        if status
            .as_deref()
            .is_some_and(|s| s.contains("sample=buddy-shown"))
        {
            child_during_main = status;
            break;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    let child_remains_fullscreen = child_during_main.as_deref().is_some_and(|value| {
        value.contains("fullscreen=true") && value.contains("active_space=true")
    });
    let child_id = status_window_number(child_during_main.as_ref());
    let main_in_front = main_snapshot.as_ref().is_some_and(|snapshot| {
        child_id.is_some_and(|id| {
            snapshot.overlapping_windows.contains(&id)
                && !snapshot.front_window_numbers.contains(&id)
        })
    });
    let main_capture = checkpoint(&capture_dir, "buddy-in-fullscreen", cx).await;

    let _ = std::fs::write(&child.activate, "ACTIVATE");
    let child_reactivated = wait_file(&child.ack, cx).await;
    let child_after = read_status(&child.status);
    let main_after_child = level_native::probe(handle, cx);
    let child_number = status_window_number(child_after.as_ref());
    let main_is_covered = main_after_child.as_ref().is_some_and(|snapshot| {
        snapshot.native.is_visible
            && !snapshot.active
            && snapshot.front_window_intersects
            && child_number.is_some_and(|number| snapshot.front_window_numbers.contains(&number))
    });
    let child_overlays_main = child_reactivated
        && main_is_covered
        && child_after.as_deref().is_some_and(|value| {
            value.contains("fullscreen=true active=true") && value.contains("active_space=true")
        });
    let child_capture = checkpoint(&capture_dir, "child-recovered", cx).await;
    let _ = std::fs::write(&child.exit, "EXIT");
    let child_done = wait_file(&child.done, cx).await;
    let returned_main = child_done
        && shell::runtime::show(handle, cx).await.is_ok()
        && wait_main_active(handle, cx)
            .await
            .as_ref()
            .is_some_and(level_native::expected_main);
    let ok = ready
        && child_ready
        && before_capture
        && hotkey_sent
        && main_level
        && main_in_front
        && child_remains_fullscreen
        && main_capture
        && child_overlays_main
        && child_capture
        && child_done
        && returned_main;
    println!(
        "T50: child READY/全屏活跃/主窗热键/主窗 level0+0x101/child仍全屏/child重激活/退出恢复 {}/{}/{}/{}/{}/{}/{}；截图 checkpoint {}/{}/{}；主窗={}",
        ready,
        child_ready,
        hotkey_sent,
        main_level,
        child_remains_fullscreen,
        child_overlays_main,
        child_done,
        before_capture,
        main_capture,
        child_capture,
        level_native::describe(main_snapshot.as_ref()),
    );
    println!(
        "T50: 主窗在全屏 child 之前={main_in_front}；退出全屏后原窗口普通 Space 可用={returned_main}；同一 runtime=true"
    );
    drop(child);
    ok
}
