//! Real input helpers for S06-03.  These helpers only dispatch platform
//! events; the model list never receives a direct setter call from a test.

use super::*;
use buddy_ui::settings::model_list::ModelListView;
use std::time::Duration;

fn center(bounds: Bounds<Pixels>) -> (f32, f32) {
    (
        f32::from(bounds.origin.x + bounds.size.width / 2.0),
        f32::from(bounds.origin.y + bounds.size.height / 2.0),
    )
}

fn visible(bounds: Bounds<Pixels>, viewport: buddy_ui::gpui::Size<Pixels>) -> bool {
    bounds.origin.x >= px(0.0)
        && bounds.origin.y >= px(0.0)
        && bounds.right() <= viewport.width
        && bounds.bottom() <= viewport.height
}

fn inside_scroll(bounds: Bounds<Pixels>, scroll: Bounds<Pixels>) -> bool {
    bounds.top() >= scroll.top()
        && bounds.bottom() <= scroll.bottom()
        && bounds.left() >= scroll.left()
        && bounds.right() <= scroll.right()
}

/// Read the actual control bounds after every scroll.  A stale coordinate is
/// deliberately rejected instead of being clicked a second time.
pub(crate) async fn click_id(
    handle: WindowHandle<PageRouter>,
    view: &Entity<buddy_ui::settings::SettingsView>,
    list: &Entity<ModelListView>,
    id: &str,
    cx: &mut AsyncApp,
) -> bool {
    let Some(mut bounds) = list.read_with(cx, |list, _| list.control_bounds(id)) else {
        return false;
    };
    for _ in 0..6 {
        let Some(viewport) = input::viewport_size(handle, cx) else {
            return false;
        };
        let scroll = view.read_with(cx, |view, _| view.scroll_bounds());
        if visible(bounds, viewport) && inside_scroll(bounds, scroll) {
            break;
        }
        let delta = if bounds.top() < scroll.top() {
            360.0
        } else {
            -360.0
        };
        input::wheel(handle, delta, cx).await;
        let Some(next) = list.read_with(cx, |list, _| list.control_bounds(id)) else {
            return false;
        };
        bounds = next;
    }
    let Some(viewport) = input::viewport_size(handle, cx) else {
        return false;
    };
    let scroll = view.read_with(cx, |view, _| view.scroll_bounds());
    if !visible(bounds, viewport) || !inside_scroll(bounds, scroll) {
        return false;
    }
    let (x, y) = center(bounds);
    input::click(handle, x, y, cx).await;
    true
}

/// Select the 128K preset, adapting the real current index after a previous
/// preset change.  The menu contains presets plus the current value only.
pub(crate) async fn choose_context_128000(
    handle: WindowHandle<PageRouter>,
    view: &Entity<buddy_ui::settings::SettingsView>,
    list: &Entity<ModelListView>,
    id: &str,
    cx: &mut AsyncApp,
) -> bool {
    let Some(context) = list.read_with(cx, |list, _| list.model_context(id)) else {
        return false;
    };
    if !click_id(handle, view, list, &format!("context-{id}"), cx).await {
        return false;
    }
    let Some(menu_bounds) =
        context.read_with(cx, |select, _| select.painted_menu_bounds_for_test())
    else {
        return false;
    };
    let Some(viewport) = input::viewport_size(handle, cx) else {
        return false;
    };
    if !visible(menu_bounds, viewport) {
        return false;
    }
    let selected = context.read_with(cx, |select, _| select.selected_value());
    match selected.as_ref().map(|value| value.as_ref()) {
        Some("33K") => input::press(handle, "down", cx).await,
        Some("256K") => input::press(handle, "up", cx).await,
        Some("128K") => {}
        _ => return false,
    }
    let result = context
        .read_with(cx, |select, _| select.selected_value())
        .is_some_and(|value| value.as_ref() == "128K");
    println!("S06-03 context-128000-{id}: {result}");
    result
}

/// Select the next 256K preset from the current 128K fixture value.
pub(crate) async fn choose_context_256000(
    handle: WindowHandle<PageRouter>,
    view: &Entity<buddy_ui::settings::SettingsView>,
    list: &Entity<ModelListView>,
    id: &str,
    cx: &mut AsyncApp,
) -> bool {
    let Some(context) = list.read_with(cx, |list, _| list.model_context(id)) else {
        return false;
    };
    if !click_id(handle, view, list, &format!("context-{id}"), cx).await {
        return false;
    }
    let Some(menu_bounds) =
        context.read_with(cx, |select, _| select.painted_menu_bounds_for_test())
    else {
        return false;
    };
    let Some(viewport) = input::viewport_size(handle, cx) else {
        return false;
    };
    if !visible(menu_bounds, viewport) {
        return false;
    }
    let selected = context.read_with(cx, |select, _| select.selected_value());
    match selected.as_ref().map(|value| value.as_ref()) {
        Some("128K") => input::press(handle, "down", cx).await,
        Some("256K") => {}
        _ => return false,
    }
    let result = context
        .read_with(cx, |select, _| select.selected_value())
        .is_some_and(|value| value.as_ref() == "256K");
    println!("S06-03 context-256000-{id}: {result}");
    result
}

/// Choose 256K after the caller focused a closed selector with Tab.  Enter
/// opens the menu and Down chooses the item; selection closes it automatically.
pub(crate) async fn choose_context_256000_focused(
    handle: WindowHandle<PageRouter>,
    context: &Entity<buddy_ui::settings::select::SettingsSelect>,
    cx: &mut AsyncApp,
) -> bool {
    input::press(handle, "enter", cx).await;
    input::press(handle, "down", cx).await;
    let result = context
        .read_with(cx, |select, _| select.selected_value())
        .is_some_and(|value| value.as_ref() == "256K");
    println!("S06-03 context-256000-focused: {result}");
    result
}

/// Return the currently focused model control after a real key dispatch.
/// `None` means the focus is on the settings header/add button, so callers can
/// distinguish a failed Tab transition from an expected outer-page focus.
pub(crate) fn focused_model_control(
    handle: WindowHandle<PageRouter>,
    cx: &mut AsyncApp,
) -> Option<String> {
    let list = handle
        .read_with(cx, |router, app| {
            router.settings_view().read(app).model_list().clone()
        })
        .ok()?;
    cx.update_window(*handle, |_, window, cx| {
        list.read(cx)
            .focus_order(cx)
            .into_iter()
            .find(|(_, focus)| focus.is_focused(window))
            .map(|(id, _)| id)
    })
    .ok()
    .flatten()
}

pub(crate) async fn settle(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) {
    for _ in 0..12 {
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
        input::draw(handle, cx).await;
    }
}

pub(crate) async fn wait_config_save(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) {
    let task = handle
        .update(cx, |router, _, _| router.take_config_save())
        .ok()
        .flatten();
    if let Some(task) = task {
        task.await;
    }
    settle(handle, cx).await;
}

pub(crate) async fn persisted_config(
    cx: &mut AsyncApp,
    path: std::path::PathBuf,
) -> Option<AppConfig> {
    let engine = ChatEngine::new(path);
    let task = cx.update(|app| {
        buddy_ui::chat_bridge::spawn_engine(app, async move { engine.get_config().await })
    });
    task.await.ok()
}

pub(crate) fn same_config(left: &AppConfig, right: &AppConfig) -> bool {
    serde_json::to_string(left).ok() == serde_json::to_string(right).ok()
}
