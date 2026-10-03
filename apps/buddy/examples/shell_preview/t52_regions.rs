//! T52 的紧凑页 / 输入框 / 设置边界，使用同一个主窗口。
use super::{drag, input, moved, native, rect, same};
use buddy_ui::chat::router::PageRouter;
use buddy_ui::gpui::{AsyncApp, Entity, WindowHandle};
use buddy_ui::shell::AppShell;

pub(super) async fn run(
    handle: WindowHandle<AppShell>,
    router: &Entity<PageRouter>,
    cx: &mut AsyncApp,
) -> bool {
    let composer = router.read_with(cx, |r, _| r.composer().clone());
    router.update(cx, |r, cx| r.invoked_after_idle(cx));
    composer.update(cx, |c, cx| c.set_draft("", cx));
    input::settle(handle, cx).await;
    let compact_edge = moved(handle, (30.0, 5.0), cx).await;
    let bounds = composer.read_with(cx, |c, app| {
        c.text_area().read(app).painted_bounds_for_test()
    });
    let Some(bounds) = bounds else {
        println!("FAIL T52：紧凑输入框没有绘制bounds");
        return false;
    };
    let point = (
        f64::from(f32::from(bounds.center().x)),
        f64::from(f32::from(bounds.center().y)),
    );
    let empty_input = moved(handle, point, cx).await;
    composer.update(cx, |c, cx| c.set_draft("输入内容不能拖窗", cx));
    input::settle(handle, cx).await;
    let Some(before) = rect(handle, cx) else {
        return false;
    };
    let input_sent = drag(handle, point, (35.0, 0.0), cx).await;
    let filled_input = input_sent && rect(handle, cx).is_some_and(|r| same(before, r));
    router.update(cx, |r, cx| r.open_settings(cx));
    input::settle(handle, cx).await;
    let settings_top = moved(handle, (300.0, 5.0), cx).await;
    let Some(bounds) = rect(handle, cx) else {
        return false;
    };
    let settings_right = moved(handle, (bounds.size.width - 5.0, 100.0), cx).await;
    // Back is an actual clickable control, so press/release without dragging.
    native::log_native_input_state("back 注入前", handle, cx);
    if !native::wait_native_input_ready("back", handle, cx).await {
        return false;
    }
    let Some(before) = rect(handle, cx) else {
        return false;
    };
    let clicked = super::os_input::click_screen(before.origin.x + 28.0, before.origin.y + 28.0);
    input::settle(handle, cx).await;
    let back = clicked && !router.read_with(cx, |r, _| r.settings_present());
    native::log_native_input_state("back 点击后", handle, cx);
    let ok = compact_edge && empty_input && filled_input && settings_top && settings_right && back;
    println!(
        "T52 regions compact_edge={compact_edge} empty_input={empty_input} filled_input={filled_input} settings_top={settings_top} settings_right={settings_right} back={back}"
    );
    ok
}
