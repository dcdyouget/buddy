use super::*;

async fn draw_controls(handle: WindowHandle<ControlsPreview>, cx: &mut AsyncApp) {
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

async fn click_controls(handle: WindowHandle<ControlsPreview>, x: f32, y: f32, cx: &mut AsyncApp) {
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
        draw_controls(handle, cx).await;
    }
}

pub(crate) async fn selftest_controls(
    handle: WindowHandle<ControlsPreview>,
    cx: &mut AsyncApp,
) -> bool {
    cx.background_executor()
        .timer(Duration::from_millis(100))
        .await;
    draw_controls(handle, cx).await;
    let field = handle
        .read_with(cx, |preview, _| preview.field.clone())
        .unwrap();
    let select = handle
        .read_with(cx, |preview, _| preview.select.clone())
        .unwrap();
    let field_bounds = field
        .read_with(cx, |field, app| field.painted_bounds_for_test(app))
        .expect("字段边界");
    click_controls(
        handle,
        f32::from(field_bounds.origin.x + px(12.0)),
        f32::from(field_bounds.origin.y + field_bounds.size.height / 2.0),
        cx,
    )
    .await;
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string("设置\n字段".to_string())));
    press_controls(handle, "cmd-v", cx).await;
    let typed = field.read_with(cx, |field, app| field.text(app) == "设置字段");

    let toggle_bounds = handle
        .read_with(cx, |preview, _| *preview.toggle_bounds.borrow())
        .unwrap()
        .expect("开关边界");
    click_controls(
        handle,
        f32::from(toggle_bounds.origin.x + toggle_bounds.size.width / 2.0),
        f32::from(toggle_bounds.origin.y + toggle_bounds.size.height / 2.0),
        cx,
    )
    .await;
    let toggled = handle.read_with(cx, |preview, _| preview.enabled).unwrap();

    // Tab / Shift+Tab 顺序与可见控件一致；只使用真实键盘事件改变焦点。
    click_controls(
        handle,
        f32::from(field_bounds.origin.x + px(12.0)),
        f32::from(field_bounds.origin.y + field_bounds.size.height / 2.0),
        cx,
    )
    .await;
    press_controls(handle, "tab", cx).await;
    let tab_forward = handle
        .update(cx, |preview, window, _| {
            preview.toggle_focus.is_focused(window)
        })
        .unwrap();
    press_controls(handle, "shift-tab", cx).await;
    let tab_backward = handle
        .update(cx, |preview, window, cx| {
            preview.field.focus_handle(cx).is_focused(window)
        })
        .unwrap();

    let select_bounds = select
        .read_with(cx, |select, _| select.painted_bounds_for_test())
        .expect("选择器边界");
    let sx = f32::from(select_bounds.origin.x + select_bounds.size.width / 2.0);
    let sy = f32::from(select_bounds.origin.y + select_bounds.size.height / 2.0);
    click_controls(handle, sx, sy, cx).await;
    let mouse_focus = handle
        .update(cx, |preview, window, cx| {
            preview.select.focus_handle(cx).is_focused(window)
        })
        .unwrap();
    cx.background_executor()
        .timer(Duration::from_millis(20))
        .await;
    draw_controls(handle, cx).await;
    let mouse_open = select.read_with(cx, |select, _| select.is_open());
    click_controls(
        handle,
        sx,
        f32::from(select_bounds.origin.y) + m::SPACE_8 + m::SPACE_1 + m::SPACE_8 * 1.5,
        cx,
    )
    .await;
    let mouse_selected = select.read_with(cx, |select, _| {
        select.selected_index() == 1 && !select.is_open()
    });
    click_controls(handle, sx, sy, cx).await;
    press_controls(handle, "up", cx).await;
    let keyboard_up = select.read_with(cx, |select, _| select.selected_index() == 0);
    click_controls(handle, sx, sy, cx).await;
    press_controls(handle, "down", cx).await;
    let keyboard_down = select.read_with(cx, |select, _| select.selected_index() == 1);
    click_controls(handle, sx, sy, cx).await;
    let opened_before_escape = select.read_with(cx, |select, _| select.is_open());
    press_controls(handle, "escape", cx).await;
    let escaped = select.read_with(cx, |select, _| !select.is_open());
    let selected = mouse_focus
        && mouse_open
        && mouse_selected
        && keyboard_up
        && keyboard_down
        && opened_before_escape
        && escaped;

    let button_bounds = handle
        .read_with(cx, |preview, _| *preview.button_bounds.borrow())
        .unwrap()
        .expect("按钮边界");
    click_controls(
        handle,
        f32::from(button_bounds.origin.x + button_bounds.size.width / 2.0),
        f32::from(button_bounds.origin.y + button_bounds.size.height / 2.0),
        cx,
    )
    .await;
    let button_hit = handle
        .read_with(cx, |preview, _| preview.button_hits == 1)
        .unwrap();
    println!(
        "T36: 粘贴单行归一 {typed}；开关 {toggled}；Tab/ShiftTab {tab_forward}/{tab_backward}；下拉焦点 {mouse_focus}、展开 {mouse_open}、鼠标选择 {mouse_selected}、键盘up/down {keyboard_up}/{keyboard_down}、Esc {opened_before_escape}→{escaped}；按钮 {button_hit}"
    );
    let ok = typed && toggled && selected && button_hit && tab_forward && tab_backward;
    println!(
        "{} S06-01 T36 共用设置控件真实交互",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

async fn press_controls(handle: WindowHandle<ControlsPreview>, keys: &str, cx: &mut AsyncApp) {
    let event = PlatformInput::KeyDown(KeyDownEvent {
        keystroke: Keystroke::parse(keys).expect("按键解析"),
        is_held: false,
        prefer_character_input: false,
    });
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_event(event, cx)
    });
    draw_controls(handle, cx).await;
}
