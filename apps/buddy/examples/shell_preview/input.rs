//! AppShell 窗口的真实 GPUI 输入与帧推进辅助。

use super::*;
use buddy_ui::gpui::{
    AppContext, ClipboardItem, Focusable, KeyDownEvent, Keystroke, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, PlatformInput, WindowHandle, point, px,
};
use buddy_ui::shell::AppShell;
use std::time::Duration;

pub(crate) async fn draw(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) {
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

pub(crate) async fn settle(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) {
    for _ in 0..15 {
        draw(handle, cx).await;
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
}

pub(crate) async fn press(handle: WindowHandle<AppShell>, keys: &str, cx: &mut AsyncApp) {
    let event = PlatformInput::KeyDown(KeyDownEvent {
        keystroke: Keystroke::parse(keys).expect("解析 shell 按键"),
        is_held: false,
        prefer_character_input: false,
    });
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_event(event, cx)
    });
    draw(handle, cx).await;
}

pub(crate) async fn paste_text(
    handle: WindowHandle<AppShell>,
    text: &str,
    cx: &mut AsyncApp,
) -> bool {
    let Some(focus) = handle
        .read_with(cx, |shell, app| {
            shell
                .router()
                .read(app)
                .composer()
                .read(app)
                .focus_handle(app)
        })
        .ok()
    else {
        return false;
    };
    cx.update(|app| app.write_to_clipboard(ClipboardItem::new_string(text.to_owned())));
    let _ = cx.update_window(handle.into(), |_, window, cx| window.focus(&focus, cx));
    press(handle, "cmd-v", cx).await;
    for _ in 0..60 {
        if handle
            .read_with(cx, |shell, app| {
                shell.router().read(app).composer().read(app).draft(app) == text
            })
            .unwrap_or(false)
        {
            return true;
        }
        draw(handle, cx).await;
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    false
}

pub(crate) async fn click(
    handle: WindowHandle<AppShell>,
    x: f32,
    y: f32,
    first_mouse: bool,
    cx: &mut AsyncApp,
) {
    let position = point(px(x), px(y));
    for event in [
        PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers: Modifiers::default(),
        }),
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse,
        }),
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
        }),
    ] {
        let _ = cx.update_window(handle.into(), |_, window, cx| {
            window.dispatch_event(event, cx)
        });
        draw(handle, cx).await;
    }
}

pub(crate) fn size(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<(Pixels, Pixels)> {
    cx.update_window(handle.into(), |_, window, _| {
        let bounds = window.bounds();
        (bounds.size.width, bounds.size.height)
    })
    .ok()
}

pub(crate) fn resize(handle: WindowHandle<AppShell>, width: f32, height: f32, cx: &mut AsyncApp) {
    let _ = cx.update_window(handle.into(), |_, window, _| {
        window.resize(buddy_ui::gpui::size(px(width), px(height)));
    });
}

pub(crate) async fn wait_size(
    handle: WindowHandle<AppShell>,
    expected: (f32, f32),
    cx: &mut AsyncApp,
) -> bool {
    for _ in 0..80 {
        settle_one(handle, cx).await;
        if size(handle, cx).is_some_and(|(w, h)| {
            (f32::from(w) - expected.0).abs() < 1.0 && (f32::from(h) - expected.1).abs() < 1.0
        }) {
            return true;
        }
    }
    false
}

async fn settle_one(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) {
    draw(handle, cx).await;
    cx.background_executor()
        .timer(Duration::from_millis(20))
        .await;
}
