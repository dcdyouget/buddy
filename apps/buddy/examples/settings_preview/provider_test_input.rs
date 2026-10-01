//! Real-input helpers shared by the provider scenarios.

use super::{input, *};
use buddy_ui::settings::provider::AddProviderPanel;
use std::time::Duration;

fn center(bounds: buddy_ui::gpui::Bounds<buddy_ui::gpui::Pixels>) -> (f32, f32) {
    (
        f32::from(bounds.origin.x + bounds.size.width / 2.0),
        f32::from(bounds.origin.y + bounds.size.height / 2.0),
    )
}

fn visible(
    bounds: buddy_ui::gpui::Bounds<buddy_ui::gpui::Pixels>,
    viewport: buddy_ui::gpui::Size<buddy_ui::gpui::Pixels>,
) -> bool {
    bounds.origin.y >= px(0.0)
        && bounds.origin.y + bounds.size.height <= viewport.height
        && bounds.origin.x >= px(0.0)
        && bounds.origin.x + bounds.size.width <= viewport.width
}

fn interaction_visible(
    bounds: buddy_ui::gpui::Bounds<buddy_ui::gpui::Pixels>,
    viewport: buddy_ui::gpui::Size<buddy_ui::gpui::Pixels>,
    panel: &Entity<AddProviderPanel>,
    id: &str,
    cx: &AsyncApp,
) -> bool {
    if matches!(id, "add" | "cancel" | "back") {
        return visible(bounds, viewport);
    }
    let content = panel.read_with(cx, |panel, _| panel.content_scroll().bounds());
    visible(bounds, viewport)
        && bounds.top() >= content.top()
        && bounds.bottom() <= content.bottom()
}

/// Click one of the panel's stable IDs, scrolling once when its real bounds
/// are outside the viewport.  The second bounds read is the only click point.
pub(crate) async fn click_id(
    handle: WindowHandle<PageRouter>,
    panel: &Entity<AddProviderPanel>,
    id: &str,
    cx: &mut AsyncApp,
) -> bool {
    let Some(mut bounds) = panel.read_with(cx, |panel, _| panel.control_bounds(id)) else {
        return false;
    };
    for _ in 0..4 {
        let Some(viewport) = input::viewport_size(handle, cx) else {
            break;
        };
        if interaction_visible(bounds, viewport, panel, id, cx) {
            break;
        }
        let top = panel.read_with(cx, |panel, _| panel.content_scroll().bounds().top());
        let delta = if bounds.origin.y < top { 420.0 } else { -420.0 };
        input::wheel(handle, delta, cx).await;
        let Some(next) = panel.read_with(cx, |panel, _| panel.control_bounds(id)) else {
            return false;
        };
        bounds = next;
    }
    if let Some(viewport) = input::viewport_size(handle, cx) {
        if !interaction_visible(bounds, viewport, panel, id, cx) {
            return false;
        }
    }
    let (x, y) = center(bounds);
    input::click(handle, x, y, cx).await;
    true
}

/// Focus a SettingsField by its painted child bounds and paste through the
/// platform clipboard, matching the same path a user takes.
pub(crate) async fn paste_field(
    handle: WindowHandle<PageRouter>,
    panel: &Entity<AddProviderPanel>,
    key: bool,
    value: &str,
    cx: &mut AsyncApp,
) -> bool {
    if !click_id(handle, panel, if key { "key" } else { "url" }, cx).await {
        return false;
    }
    let field = panel.read_with(cx, |panel, _| {
        if key {
            panel.key_field()
        } else {
            panel.url_field()
        }
    });
    let Some(bounds) = field.read_with(cx, |field, app| field.painted_bounds_for_test(app)) else {
        return false;
    };
    let (x, y) = center(bounds);
    input::click(handle, x, y, cx).await;
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string(value.to_owned())));
    input::press(handle, "cmd-a", cx).await;
    input::press(handle, "cmd-v", cx).await;
    // 订阅是下一轮前台任务交付；不重复输入来掩盖丢事件。
    settle(handle, cx).await;
    field.read_with(cx, |field, app| field.text(app) == value)
}

pub(crate) async fn settle(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) {
    for _ in 0..12 {
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
        input::draw(handle, cx).await;
    }
}

pub(crate) async fn draw(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) {
    input::draw(handle, cx).await;
}
