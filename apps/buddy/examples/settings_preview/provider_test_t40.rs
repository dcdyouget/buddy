//! T40 provider focus and exit guards through real keyboard/mouse events.

use super::*;

fn focused_control(
    handle: WindowHandle<PageRouter>,
    panel: &Entity<AddProviderPanel>,
    cx: &mut AsyncApp,
) -> Option<String> {
    cx.update_window(handle.into(), |_, window, app| {
        panel.read(app).focused_control(window, app)
    })
    .ok()
    .flatten()
}

pub(super) async fn run(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> bool {
    let Some(panel) = open_panel(handle, cx).await else {
        println!("T40: 打开面板 false");
        return false;
    };
    let initial = focused_control(handle, &panel, cx).as_deref() == Some("back");
    input::press(handle, "tab", cx).await;
    let first_preset = focused_control(handle, &panel, cx).as_deref() == Some("preset-deepseek");
    input::press(handle, "enter", cx).await;
    let activated = panel.read_with(cx, |panel, _| {
        panel.form().preset.as_deref() == Some("deepseek")
    });
    let mouse_focus = click_id(handle, &panel, "custom", cx).await
        && focused_control(handle, &panel, cx).as_deref() == Some("custom");
    input::press(handle, "tab", cx).await;
    let tab_focus = focused_control(handle, &panel, cx).as_deref() == Some("protocol");
    input::press(handle, "shift-tab", cx).await;
    let reverse_focus = focused_control(handle, &panel, cx).as_deref() == Some("custom");
    input::press(handle, "tab", cx).await;
    input::press(handle, "tab", cx).await;
    input::press(handle, "enter", cx).await;
    input::press(handle, "tab", cx).await;
    let thinking_focus = focused_control(handle, &panel, cx).as_deref() == Some("thinking");
    input::press(handle, "enter", cx).await;
    input::press(handle, "down", cx).await;
    input::press(handle, "tab", cx).await;
    let max_focus = focused_control(handle, &panel, cx).as_deref() == Some("max-tokens");
    input::press(handle, "enter", cx).await;
    input::press(handle, "down", cx).await;
    settle(handle, cx).await;
    let compat_selected = panel.read_with(cx, |panel, _| {
        panel.form().thinking_format == "deepseek"
            && panel.form().max_tokens_field == "max_completion_tokens"
    });
    let cancel_clicked = click_id(handle, &panel, "cancel", cx).await;
    settle(handle, cx).await;
    let exited = handle
        .read_with(cx, |router, app| {
            !router.settings_view().read(app).provider_open()
        })
        .unwrap_or(false);
    let Some(panel) = open_panel(handle, cx).await else {
        return false;
    };
    input::press(handle, "shift-tab", cx).await;
    let wrapped = focused_control(handle, &panel, cx).as_deref() == Some("cancel");
    input::press(handle, "enter", cx).await;
    settle(handle, cx).await;
    let keyboard_exit = handle
        .read_with(cx, |router, app| {
            !router.settings_view().read(app).provider_open()
        })
        .unwrap_or(false);
    let ok = initial
        && first_preset
        && activated
        && mouse_focus
        && tab_focus
        && reverse_focus
        && thinking_focus
        && max_focus
        && compat_selected
        && cancel_clicked
        && exited
        && wrapped
        && keyboard_exit;
    println!(
        "T40: 初始焦点 {initial}；Tab/Enter 选预设 {first_preset}/{activated}；鼠标焦点 {mouse_focus}；Tab/Shift+Tab {tab_focus}/{reverse_focus}；兼容选项焦点 {thinking_focus}/{max_focus} / 选择 {compat_selected}；退出 {exited}；循环/键盘退出 {wrapped}/{keyboard_exit}"
    );
    ok
}
