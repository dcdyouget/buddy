use super::protocol::{POLL, TIMEOUT};
use buddy_ui::gpui::{AppContext, AsyncApp, EntityId, WindowHandle};
use buddy_ui::shell::{self, AppShell, native, workspaces};
use std::path::Path;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct WindowState {
    pub(super) visible: bool,
    pub(super) key: bool,
    pub(super) app_active: bool,
    pub(super) window_number: i64,
}

pub(super) fn window_state(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<WindowState> {
    cx.update_window(handle.into(), |_, window, _| {
        let native = native::probe_main_window(window).ok()?;
        let workspace = workspaces::probe(window).ok()?;
        Some(WindowState {
            visible: native.is_visible,
            key: native.is_key,
            app_active: workspace.app_is_active,
            window_number: workspace.window_number,
        })
    })
    .ok()
    .flatten()
}

pub(super) fn router_id(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<EntityId> {
    handle
        .read_with(cx, |shell, _| shell.router().entity_id())
        .ok()
}

pub(super) fn registered_hotkey(cx: &mut AsyncApp) -> Option<String> {
    cx.update(|app| shell::runtime::registered_hotkey(app))
}

pub(super) async fn wait_marker(path: &Path, cx: &mut AsyncApp) -> bool {
    let started = Instant::now();
    while started.elapsed() < TIMEOUT {
        if path.is_file() {
            return true;
        }
        cx.background_executor().timer(POLL).await;
    }
    false
}

pub(super) async fn wait_active_same_window(
    handle: WindowHandle<AppShell>,
    expected_router: EntityId,
    expected_window: i64,
    cx: &mut AsyncApp,
) -> Option<WindowState> {
    let started = Instant::now();
    while started.elapsed() < TIMEOUT {
        if let Some(state) = window_state(handle, cx)
            && state.visible
            && state.key
            && state.app_active
            && state.window_number == expected_window
            && router_id(handle, cx) == Some(expected_router)
        {
            return Some(state);
        }
        cx.background_executor().timer(POLL).await;
    }
    let state = window_state(handle, cx);
    let same_router = router_id(handle, cx) == Some(expected_router);
    println!(
        "[S07-11] 断言未满足：wait_active_same_window 状态未在期限内稳定：state={state:?} same_router={same_router} expected_window={expected_window} registered_hotkey={:?}",
        registered_hotkey(cx)
    );
    None
}
