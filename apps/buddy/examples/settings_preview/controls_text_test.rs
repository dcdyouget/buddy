//! T39: narrow single-line fields, Unicode masking and inactive input.

use super::*;

pub(super) async fn run(
    handle: WindowHandle<ControlsPreview>,
    field: &Entity<SettingsField>,
    cx: &mut AsyncApp,
) -> bool {
    let bounds = field
        .read_with(cx, |field, app| field.painted_bounds_for_test(app))
        .unwrap();
    click_controls(
        handle,
        f32::from(bounds.origin.x + px(8.0)),
        f32::from(bounds.center().y),
        cx,
    )
    .await;
    let original = format!("https://example.invalid/{}", "很长的路径".repeat(20));
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string(original.clone())));
    press_controls(handle, "cmd-a", cx).await;
    press_controls(handle, "cmd-v", cx).await;
    let long_line = field.read_with(cx, |field, app| {
        let input = field.input().read(app);
        input.text() == original
            && input.scroll_x_for_test() > px(0.0)
            && input.scroll_y_for_test() == px(0.0)
    });
    press_controls(handle, "cmd-left", cx).await;
    let home = field.read_with(cx, |field, app| {
        let input = field.input().read(app);
        input.selected_range_for_test() == (0..0) && input.scroll_x_for_test() == px(0.0)
    });
    let unicode = "a中é";
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string(unicode.to_string())));
    press_controls(handle, "cmd-a", cx).await;
    press_controls(handle, "cmd-v", cx).await;
    field.update(cx, |field, cx| field.set_masked(true, cx));
    draw_controls(handle, cx).await;
    press_controls(handle, "left", cx).await;
    press_controls(handle, "backspace", cx).await;
    let masked_edit = field.read_with(cx, |field, app| {
        field.masked(app) && field.text(app) == "aé"
    });
    field.update(cx, |field, cx| field.set_masked(false, cx));
    draw_controls(handle, cx).await;
    let shown = field.read_with(cx, |field, app| {
        !field.masked(app) && field.text(app) == "aé"
    });
    field.update(cx, |field, cx| field.set_active(false, cx));
    draw_controls(handle, cx).await;
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string("关闭后输入".to_string())));
    press_controls(handle, "cmd-v", cx).await;
    press_controls(handle, "backspace", cx).await;
    let disabled = field.read_with(cx, |field, app| {
        !field.input().read(app).enabled() && field.text(app) == "aé"
    });
    field.update(cx, |field, cx| field.set_active(true, cx));
    let ok = long_line && home && masked_edit && shown && disabled;
    println!("T39: 长单行水平滚动 {long_line}；行首归零 {home}；遮罩 Unicode 编辑 {masked_edit}；显隐保留原文 {shown}；禁用输入 {disabled}");
    println!("{} S06-02 T39 单行字段", if ok { "PASS" } else { "FAIL" });
    ok
}
