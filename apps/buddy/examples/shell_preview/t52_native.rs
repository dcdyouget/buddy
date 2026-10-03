//! T52 原生输入前置状态：确认 AppKit 窗口实际可接收 OS 事件。

use super::AppShell;
use buddy_ui::gpui::{AppContext, AsyncApp, WindowHandle};
use buddy_ui::shell::{self, positioning::Rect, positioning_native};
use std::time::{Duration, Instant};

pub(super) type NativeInputState = (bool, bool, bool, Option<Rect>);

pub(super) fn native_input_state(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<NativeInputState> {
    cx.update_window(handle.into(), |_, window, _| {
        let native = shell::native::probe_main_window(window).ok()?;
        let workspace = shell::workspaces::probe(window).ok()?;
        let rect = positioning_native::prepare(window)
            .and_then(|prepared| prepared.snapshot())
            .ok()
            .map(|snapshot| snapshot.rect);
        Some((
            native.is_visible,
            native.is_key,
            workspace.app_is_active,
            rect,
        ))
    })
    .ok()
    .flatten()
}

pub(super) fn log_native_input_state(
    label: &str,
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<NativeInputState> {
    let state = native_input_state(handle, cx);
    println!("T52 {label} native visible/key/appactive/bounds={state:?}");
    state
}

pub(super) async fn wait_native_input_ready(
    label: &str,
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if native_input_state(handle, cx)
            .is_some_and(|(visible, key, active, _)| visible && key && active)
        {
            log_native_input_state(&format!("{label} ready"), handle, cx);
            return true;
        }
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        cx.background_executor()
            .timer((deadline - now).min(Duration::from_millis(20)))
            .await;
    }
    let final_state = log_native_input_state(&format!("{label} 等待失败"), handle, cx);
    println!("FAIL T52：{label} 前主窗口未达到 visible/key/appactive：{final_state:?}");
    false
}
