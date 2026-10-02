//! S06-05/S06-06 的真实输入辅助函数。

use super::*;
use buddy_ui::gpui::{
    Capslock, KeyDownEvent, KeyUpEvent, Modifiers, ModifiersChangedEvent, PlatformInput,
};
use std::time::Duration;

pub(crate) async fn settle(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) {
    for _ in 0..12 {
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
        draw(handle, cx).await;
    }
}

pub(crate) async fn draw(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) {
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

pub(crate) async fn click(
    handle: WindowHandle<PageRouter>,
    bounds: Bounds<Pixels>,
    cx: &mut AsyncApp,
) {
    let p = bounds.center();
    super::input::click(handle, f32::from(p.x), f32::from(p.y), cx).await;
}

pub(crate) async fn key_down(
    handle: WindowHandle<PageRouter>,
    key: &str,
    modifiers: Modifiers,
    cx: &mut AsyncApp,
) {
    let event = PlatformInput::KeyDown(KeyDownEvent {
        keystroke: Keystroke {
            modifiers,
            key: key.into(),
            key_char: None,
        },
        is_held: false,
        prefer_character_input: false,
    });
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_event(event, cx)
    });
    draw(handle, cx).await;
}

pub(crate) async fn key_up(
    handle: WindowHandle<PageRouter>,
    key: &str,
    modifiers: Modifiers,
    cx: &mut AsyncApp,
) {
    let event = PlatformInput::KeyUp(KeyUpEvent {
        keystroke: Keystroke {
            modifiers,
            key: key.into(),
            key_char: None,
        },
    });
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_event(event, cx)
    });
    draw(handle, cx).await;
}

pub(crate) async fn modifiers(
    handle: WindowHandle<PageRouter>,
    modifiers: Modifiers,
    cx: &mut AsyncApp,
) {
    let event = PlatformInput::ModifiersChanged(ModifiersChangedEvent {
        modifiers,
        capslock: Capslock::default(),
    });
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_event(event, cx)
    });
    draw(handle, cx).await;
}

pub(crate) async fn record(
    handle: WindowHandle<PageRouter>,
    mods: Modifiers,
    key: &str,
    cx: &mut AsyncApp,
) {
    self::modifiers(handle, mods, cx).await;
    key_down(handle, key, mods, cx).await;
    key_up(handle, key, mods, cx).await;
    modifiers(handle, Modifiers::none(), cx).await;
}

pub(crate) fn viewport_size(
    handle: WindowHandle<PageRouter>,
    cx: &mut AsyncApp,
) -> Option<buddy_ui::gpui::Size<Pixels>> {
    cx.update_window(handle.into(), |_, window, _| window.viewport_size())
        .ok()
}
