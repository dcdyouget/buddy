//! T46：真实原生窗口属性与首次点击输入。

use super::*;
use buddy_ui::chat::page_state::Page;
use buddy_ui::gpui::{AppContext, AsyncApp, WindowHandle};
use buddy_ui::shell::native::NativeWindowSnapshot;
use buddy_ui::shell::native::probe_main_window;
use buddy_ui::shell::{AppShell, open_main_window};
use std::time::Duration;

fn probe(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<NativeWindowSnapshot> {
    cx.update_window(handle.into(), |_, window, _| probe_main_window(window).ok())
        .ok()
        .flatten()
}

fn expected(snapshot: &NativeWindowSnapshot) -> bool {
    snapshot.is_decoration_free()
        && snapshot.style_mask & (1 << 3) != 0
        && snapshot.level == 0
        && !snapshot.has_shadow
        && !snapshot.is_opaque
        && snapshot.wants_layer
        && (snapshot.corner_radius - 16.0).abs() < 0.1
        && snapshot.masks_to_bounds
        && snapshot.accepts_first_mouse
        && snapshot.is_visible
        && !snapshot.is_key
}

pub(crate) async fn run(cx: &mut AsyncApp) -> bool {
    macro_rules! fail {
        () => {{
            println!("FAIL T46：原生窗口验收提前结束");
            return false;
        }};
    }
    let engine = fixture::manual_engine(false);
    let handle = match open_main_window(engine, fixture::shell_config(), cx).await {
        Ok(handle) => handle,
        Err(error) => {
            println!("FAIL T46：打开主窗口失败：{error}");
            fail!();
        }
    };
    let immediate = probe(handle, cx);
    input::draw(handle, cx).await;
    let composed = probe(handle, cx);
    cx.background_executor()
        .timer(Duration::from_millis(140))
        .await;
    let delayed = probe(handle, cx);
    cx.update(|app| app.activate(true));
    let first_mouse_input = if input::size(handle, cx).is_some() {
        input::click(handle, 483.0, 39.0, true, cx).await;
        handle
            .read_with(cx, |shell, app| {
                shell.router().read(app).page() == Page::Settings
            })
            .unwrap_or(false)
    } else {
        false
    };
    input::settle(handle, cx).await;
    let active_handle =
        match open_main_window(fixture::manual_engine(false), fixture::shell_config(), cx).await {
            Ok(handle) => handle,
            Err(error) => {
                println!("FAIL T46：应用激活后开窗失败：{error}");
                fail!();
            }
        };
    input::settle(active_handle, cx).await;
    let active = probe(active_handle, cx);
    let properties = immediate.as_ref().is_some_and(expected)
        && composed.as_ref().is_some_and(expected)
        && delayed.as_ref().is_some_and(expected)
        && active.as_ref().is_some_and(expected);
    let ok = properties && first_mouse_input;
    println!(
        "T46: 原生属性立即/合成后/延迟读取 {:?}/{:?}/{:?}；请求激活后新窗口 {:?}；首击输入 {first_mouse_input}；字段探测 {properties}",
        immediate, composed, delayed, active
    );
    // Keep the window until process exit so pending platform / animation callbacks
    // do not target an already removed test window.
    ok
}
