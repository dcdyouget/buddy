use super::*;

pub(crate) async fn draw(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) {
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

pub(crate) async fn press(handle: WindowHandle<PageRouter>, keys: &str, cx: &mut AsyncApp) {
    let event = PlatformInput::KeyDown(KeyDownEvent {
        keystroke: Keystroke::parse(keys).expect("按键解析"),
        is_held: false,
        prefer_character_input: false,
    });
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_event(event, cx)
    });
    draw(handle, cx).await;
}

pub(crate) async fn click(handle: WindowHandle<PageRouter>, x: f32, y: f32, cx: &mut AsyncApp) {
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
            first_mouse: false,
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

pub(crate) async fn wheel(handle: WindowHandle<PageRouter>, dy: f32, cx: &mut AsyncApp) {
    let position = point(px(WIDTH - 40.0), px(240.0));
    for event in [
        PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers: Modifiers::default(),
        }),
        PlatformInput::ScrollWheel(ScrollWheelEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(0.0), px(dy))),
            modifiers: Modifiers::default(),
            touch_phase: TouchPhase::Moved,
        }),
    ] {
        let _ = cx.update_window(handle.into(), |_, window, cx| {
            window.dispatch_event(event, cx)
        });
        draw(handle, cx).await;
    }
    for _ in 0..4 {
        cx.background_executor()
            .timer(Duration::from_millis(16))
            .await;
        draw(handle, cx).await;
    }
}

pub(crate) fn viewport_size(
    handle: WindowHandle<PageRouter>,
    cx: &mut AsyncApp,
) -> Option<buddy_ui::gpui::Size<buddy_ui::gpui::Pixels>> {
    cx.update_window(handle.into(), |_, window, _| window.viewport_size())
        .ok()
}
