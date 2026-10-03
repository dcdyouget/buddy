//! S07-12 窗口行为自检。
//!
//! 该入口只创建一个唯一临时数据目录中的真实 `AppShell`，读取原生窗口属性，
//! 验证工作区、移动与显隐，再清理窗口和目录。它不安装热键、托盘、单实例，
//! 也不读取或写入用户数据。

use crate::shell::{AppShell, config::ShellConfig, native, open_main_window, positioning_native};
use buddy_engine::chat::ChatEngine;
use gpui::{AppContext, AsyncApp, WindowHandle};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT_SANDBOX: AtomicU64 = AtomicU64::new(0);

/// 执行一次真实窗口行为自检。
///
/// macOS 锁屏、事件访问权限不足和非 macOS 平台都会输出明确的 `BLOCKED`
/// 并返回 `false`，不会把无法观测当作通过。
pub async fn run(cx: &mut AsyncApp) -> bool {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = cx;
        println!("BLOCKED S07-12：当前平台没有 macOS 原生窗口自检实现，Windows 待测");
        return false;
    }

    #[cfg(target_os = "macos")]
    {
        if let Err(reason) = macos_preflight() {
            println!("BLOCKED S07-12：{reason}");
            return false;
        }

        let data_dir = sandbox_dir();
        if let Err(error) = std::fs::create_dir(&data_dir) {
            println!("FAIL S07-12：无法创建临时 engine 沙盒：{error}");
            return false;
        }
        let engine = ChatEngine::new(data_dir.clone());
        let handle = match open_main_window(engine, ShellConfig::default(), cx).await {
            Ok(handle) => handle,
            Err(error) => {
                println!("FAIL S07-12：真实 shell 工厂创建窗口失败：{error}");
                let _ = std::fs::remove_dir_all(data_dir);
                return false;
            }
        };

        let result = run_native_checks(handle, cx).await;
        let window_cleanup = cx
            .update_window(handle.into(), |_, window, _| window.remove_window())
            .is_ok();
        let sandbox_cleanup = std::fs::remove_dir_all(&data_dir).is_ok();
        if !window_cleanup {
            println!("FAIL S07-12：真实窗口清理失败");
        }
        if !sandbox_cleanup {
            println!("FAIL S07-12：临时 engine 沙盒清理失败");
        }
        result && window_cleanup && sandbox_cleanup
    }
}

#[cfg(target_os = "macos")]
async fn run_native_checks(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> bool {
    let Some(initial) = probe_native(handle, cx) else {
        println!("FAIL S07-12：无法读取窗口原生属性");
        return false;
    };
    let Some(initial_position) = probe_position(handle, cx) else {
        println!("FAIL S07-12：无法读取窗口原生位置");
        return false;
    };
    let native_ok = expected_native(&initial);
    println!(
        "T12-01 原生属性：装饰={} 可调整={} level={} shadow={} opaque={} layer={} radius={:.1} masks={} 首击={} visible={}；{}",
        initial.is_decoration_free(),
        initial.style_mask & (1 << 3) != 0,
        initial.level,
        initial.has_shadow,
        initial.is_opaque,
        initial.wants_layer,
        initial.corner_radius,
        initial.masks_to_bounds,
        initial.accepts_first_mouse,
        initial.is_visible,
        if native_ok { "PASS" } else { "FAIL" },
    );

    let workspace = probe_workspace(handle, cx);
    let workspace_ok = workspace.as_ref().is_some_and(|state| {
        state.is_on_active_space && !state.is_fullscreen && state.window_number > 0
    });
    println!(
        "T12-02 工作区：读取={} active_space={} fullscreen={} window_number={} app_active={}；{}",
        workspace.is_some(),
        workspace
            .as_ref()
            .is_some_and(|state| state.is_on_active_space),
        workspace.as_ref().is_some_and(|state| state.is_fullscreen),
        workspace.as_ref().map_or(0, |state| state.window_number),
        workspace.as_ref().is_some_and(|state| state.app_is_active),
        if workspace_ok { "PASS" } else { "FAIL" },
    );

    let moved = check_move_and_restore(handle, initial_position.rect.origin, cx).await;
    println!(
        "T12-03 原生移动并恢复：{}",
        if moved { "PASS" } else { "FAIL" }
    );

    let visibility = check_visibility(handle, cx).await;
    println!(
        "T12-04 原生隐藏 / 显示：{}",
        if visibility { "PASS" } else { "FAIL" }
    );

    let ok = native_ok && workspace_ok && moved && visibility;
    println!("{} S07-12 窗口行为自检", if ok { "PASS" } else { "FAIL" });
    ok
}

#[cfg(target_os = "macos")]
fn expected_native(snapshot: &native::NativeWindowSnapshot) -> bool {
    snapshot.is_decoration_free()
        && snapshot.style_mask & (1 << 3) != 0
        && snapshot.level == 0
        && !snapshot.has_shadow
        && !snapshot.is_opaque
        && snapshot.collection_behavior == 0x101
        && snapshot.wants_layer
        && (snapshot.corner_radius - 16.0).abs() < 0.1
        && snapshot.masks_to_bounds
        && snapshot.accepts_first_mouse
        && snapshot.is_visible
}

#[cfg(target_os = "macos")]
fn probe_native(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<native::NativeWindowSnapshot> {
    cx.update_window(handle.into(), |_, window, _| {
        native::probe_main_window(window).ok()
    })
    .ok()
    .flatten()
}

#[cfg(target_os = "macos")]
fn probe_workspace(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<crate::shell::workspaces::WorkspaceWindowSnapshot> {
    cx.update_window(handle.into(), |_, window, _| {
        crate::shell::workspaces::probe(window).ok()
    })
    .ok()
    .flatten()
}

#[cfg(target_os = "macos")]
async fn check_move_and_restore(
    handle: WindowHandle<AppShell>,
    original: crate::shell::positioning::Point,
    cx: &mut AsyncApp,
) -> bool {
    let target = crate::shell::positioning::Point {
        x: original.x + 12.0,
        y: original.y + 12.0,
    };
    let moved = move_native(handle, target, cx);
    cx.background_executor()
        .timer(Duration::from_millis(80))
        .await;
    let observed = probe_position(handle, cx).is_some_and(|snapshot| {
        close(snapshot.rect.origin.x, target.x) && close(snapshot.rect.origin.y, target.y)
    });
    let restored = move_native(handle, original, cx);
    cx.background_executor()
        .timer(Duration::from_millis(80))
        .await;
    let back = probe_position(handle, cx).is_some_and(|snapshot| {
        close(snapshot.rect.origin.x, original.x) && close(snapshot.rect.origin.y, original.y)
    });
    moved && observed && restored && back
}

#[cfg(target_os = "macos")]
fn move_native(
    handle: WindowHandle<AppShell>,
    origin: crate::shell::positioning::Point,
    cx: &mut AsyncApp,
) -> bool {
    let prepared = cx
        .update_window(handle.into(), |_, window, _| {
            positioning_native::prepare(window)
        })
        .ok()
        .and_then(Result::ok);
    prepared.is_some_and(|native| native.move_to(origin).is_ok())
}

#[cfg(target_os = "macos")]
fn probe_position(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<positioning_native::Snapshot> {
    let prepared = cx
        .update_window(handle.into(), |_, window, _| {
            positioning_native::prepare(window)
        })
        .ok()
        .and_then(Result::ok)?;
    prepared.snapshot().ok()
}

#[cfg(target_os = "macos")]
async fn check_visibility(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> bool {
    let prepared = cx
        .update_window(handle.into(), |_, window, _| {
            crate::shell::visibility::prepare(window)
        })
        .ok()
        .and_then(Result::ok);
    let Some(native) = prepared else {
        return false;
    };
    let visible_before = native.probe().is_ok_and(|snapshot| snapshot.is_visible);
    let hidden = native.hide().is_ok_and(|snapshot| !snapshot.is_visible);
    cx.background_executor()
        .timer(Duration::from_millis(80))
        .await;
    let hidden_after = native.probe().is_ok_and(|snapshot| !snapshot.is_visible);
    let shown = native
        .show_and_focus()
        .is_ok_and(|snapshot| snapshot.is_visible && snapshot.is_key && snapshot.app_is_active);
    cx.background_executor()
        .timer(Duration::from_millis(80))
        .await;
    let visible_after = native.probe().is_ok_and(|snapshot| snapshot.is_visible);
    visible_before && hidden && hidden_after && shown && visible_after
}

#[cfg(target_os = "macos")]
fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1.0
}

#[cfg(target_os = "macos")]
fn sandbox_dir() -> PathBuf {
    let index = NEXT_SANDBOX.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("buddy-s07-12-{}-{index}", std::process::id()))
}

#[cfg(target_os = "macos")]
fn macos_preflight() -> Result<(), &'static str> {
    if macos_session_locked()? {
        return Err("macOS 会话已锁定，请解锁桌面后重试");
    }
    if !macos_event_access() {
        return Err("未获得 CGEventPost 权限，窗口激活自检被阻止");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_session_locked() -> Result<bool, &'static str> {
    unsafe {
        let dictionary = CGSessionCopyCurrentDictionary();
        if dictionary.is_null() {
            return Err("无法读取 macOS 会话状态，锁屏状态未知");
        }
        let key =
            CFStringCreateWithCString(std::ptr::null(), c"CGSSessionScreenIsLocked".as_ptr(), UTF8);
        if key.is_null() {
            CFRelease(dictionary);
            return Err("无法创建 macOS 会话状态查询键，锁屏状态未知");
        }
        let value = CFDictionaryGetValue(dictionary, key);
        let locked = !value.is_null() && CFBooleanGetValue(value);
        CFRelease(key);
        CFRelease(dictionary);
        Ok(locked)
    }
}

#[cfg(target_os = "macos")]
fn macos_event_access() -> bool {
    unsafe { CGPreflightPostEventAccess() }
}

#[cfg(target_os = "macos")]
const UTF8: u32 = 0x0800_0100;

#[cfg(target_os = "macos")]
type CfRef = *const std::ffi::c_void;

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGSessionCopyCurrentDictionary() -> CfRef;
    fn CGPreflightPostEventAccess() -> bool;
    fn CFDictionaryGetValue(dictionary: CfRef, key: CfRef) -> CfRef;
    fn CFStringCreateWithCString(
        allocator: CfRef,
        string: *const std::ffi::c_char,
        encoding: u32,
    ) -> CfRef;
    fn CFBooleanGetValue(value: CfRef) -> bool;
    fn CFRelease(value: CfRef);
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::{expected_native, native};

    fn fixture() -> native::NativeWindowSnapshot {
        native::NativeWindowSnapshot {
            style_mask: (1 << 3) | (1 << 7),
            level: 0,
            has_shadow: false,
            is_opaque: false,
            collection_behavior: 0x101,
            wants_layer: true,
            corner_radius: 16.0,
            masks_to_bounds: true,
            accepts_first_mouse: true,
            is_visible: true,
            is_key: false,
        }
    }

    #[test]
    fn native_expected_rejects_each_bad_field() {
        let base = fixture();
        assert!(expected_native(&base));

        let mut bad = base.clone();
        bad.style_mask |= 1 << 0;
        assert!(!expected_native(&bad));
        let mut bad = base.clone();
        bad.style_mask &= !(1 << 3);
        assert!(!expected_native(&bad));
        let mut bad = base.clone();
        bad.level = 1;
        assert!(!expected_native(&bad));
        let mut bad = base.clone();
        bad.has_shadow = true;
        assert!(!expected_native(&bad));
        let mut bad = base.clone();
        bad.is_opaque = true;
        assert!(!expected_native(&bad));
        let mut bad = base.clone();
        bad.collection_behavior = 0;
        assert!(!expected_native(&bad));
        let mut bad = base.clone();
        bad.wants_layer = false;
        assert!(!expected_native(&bad));
        let mut bad = base.clone();
        bad.corner_radius = 12.0;
        assert!(!expected_native(&bad));
        let mut bad = base.clone();
        bad.masks_to_bounds = false;
        assert!(!expected_native(&bad));
        let mut bad = base.clone();
        bad.accepts_first_mouse = false;
        assert!(!expected_native(&bad));
        let mut bad = base;
        bad.is_visible = false;
        assert!(!expected_native(&bad));
    }
}
