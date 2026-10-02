//! T45：生产窗口工厂与页面尺寸策略。

use super::*;
use buddy_ui::chat::page_state::Page;
use buddy_ui::gpui::{AsyncApp, Entity, WindowHandle};
use buddy_ui::shell::{AppShell, open_main_window};
use std::time::Duration;

fn router(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<Entity<buddy_ui::chat::router::PageRouter>> {
    handle.read_with(cx, |shell, _| shell.router().clone()).ok()
}

fn page(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<Page> {
    handle
        .read_with(cx, |shell, app| shell.router().read(app).page())
        .ok()
}

async fn wait_page(handle: WindowHandle<AppShell>, target: Page, cx: &mut AsyncApp) -> bool {
    for _ in 0..100 {
        input::draw(handle, cx).await;
        if page(handle, cx) == Some(target) {
            return true;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    false
}

async fn wait_streaming(handle: WindowHandle<AppShell>, target: bool, cx: &mut AsyncApp) -> bool {
    for _ in 0..150 {
        input::draw(handle, cx).await;
        if handle
            .read_with(cx, |shell, app| {
                shell
                    .router()
                    .read(app)
                    .conversation()
                    .read(app)
                    .state
                    .is_streaming()
                    == target
            })
            .unwrap_or(false)
        {
            return true;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    false
}

fn settings_x(width: f32) -> f32 {
    width - 5.0 - 28.0 - 4.0 - 24.0 - 4.0 - 12.0
}

async fn open_settings_by_click(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> bool {
    let Some((width, height)) = input::size(handle, cx).map(|(w, h)| (f32::from(w), f32::from(h)))
    else {
        return false;
    };
    input::click(handle, settings_x(width), height - 21.0, false, cx).await;
    let entered = wait_page(handle, Page::Settings, cx).await;
    input::settle(handle, cx).await;
    entered
}

pub(crate) async fn run(cx: &mut AsyncApp) -> bool {
    macro_rules! fail {
        () => {{
            println!("FAIL T45：窗口输入验收提前结束");
            return false;
        }};
    }
    let engine = fixture::manual_engine(false);
    let options_match = cx.update(|app| {
        let options = fixture::shell_config().window_options(app).unwrap();
        !options.focus
            && options.titlebar.is_none()
            && options.is_resizable
            && matches!(options.kind, buddy_ui::gpui::WindowKind::PopUp)
            && options.window_min_size.is_some_and(|value| {
                f32::from(value.width) == 360.0 && f32::from(value.height) == 60.0
            })
    });
    let handle = match open_main_window(engine, fixture::shell_config(), cx).await {
        Ok(handle) => handle,
        Err(error) => {
            println!("FAIL T45：打开主窗口失败：{error}");
            fail!();
        }
    };
    input::settle(handle, cx).await;
    let initial =
        input::size(handle, cx).is_some_and(|(w, h)| f32::from(w) == 560.0 && f32::from(h) == 60.0);
    let entered_settings = open_settings_by_click(handle, cx).await;
    let settings_size = input::wait_size(handle, (760.0, 640.0), cx).await;
    input::click(handle, 32.0, 28.0, false, cx).await;
    let returned = wait_page(handle, Page::Conversation, cx).await;
    let conversation_after_settings = input::wait_size(handle, (750.0, 500.0), cx).await;

    let Some(router) = router(handle, cx) else {
        fail!();
    };
    let pasted = input::paste_text(handle, "shell 页面输入", cx).await;
    input::press(handle, "enter", cx).await;
    let first_streaming = wait_streaming(handle, true, cx).await;
    let first_conversation =
        wait_streaming(handle, false, cx).await && wait_page(handle, Page::Conversation, cx).await;
    let conversation_size = input::wait_size(handle, (750.0, 500.0), cx).await;

    input::resize(handle, 900.0, 700.0, cx);
    input::settle(handle, cx).await;
    let user_size = input::size(handle, cx)
        .is_some_and(|(w, h)| f32::from(w) == 900.0 && f32::from(h) == 700.0);
    let slow_pasted = input::paste_text(handle, "慢", cx).await;
    input::press(handle, "enter", cx).await;
    let slow_streaming = wait_streaming(handle, true, cx).await;
    let streaming_size_kept = input::size(handle, cx)
        .is_some_and(|(w, h)| f32::from(w) == 900.0 && f32::from(h) == 700.0);
    let slow_conversation =
        wait_streaming(handle, false, cx).await && wait_page(handle, Page::Conversation, cx).await;
    let settings_after_resize = open_settings_by_click(handle, cx).await;
    let preserved_settings =
        settings_after_resize && input::wait_size(handle, (900.0, 700.0), cx).await;
    input::click(handle, 32.0, 28.0, false, cx).await;
    let preserved_return = wait_page(handle, Page::Conversation, cx).await
        && input::size(handle, cx)
            .is_some_and(|(w, h)| f32::from(w) == 900.0 && f32::from(h) == 700.0);

    let Some(mut compact_config) = handle
        .read_with(cx, |shell, app| shell.router().read(app).config().clone())
        .ok()
    else {
        fail!();
    };
    compact_config.selected_model_id.clear();
    let _ = handle.update(cx, |_, _, cx| {
        router.update(cx, |router, cx| router.set_config(compact_config, cx))
    });
    let compact_again = wait_page(handle, Page::Empty, cx).await
        && input::wait_size(handle, (560.0, 60.0), cx).await;
    let cold =
        match open_main_window(fixture::manual_engine(false), fixture::shell_config(), cx).await {
            Ok(handle) => handle,
            Err(error) => {
                println!("FAIL T45：创建冷发送窗口：{error}");
                fail!();
            }
        };
    input::settle(cold, cx).await;
    let cold_pasted = input::paste_text(cold, "紧凑页发送", cx).await;
    input::press(cold, "enter", cx).await;
    let cold_streaming = wait_page(cold, Page::Streaming, cx).await;
    let cold_expanded = input::wait_size(cold, (750.0, 500.0), cx).await;
    let cold_finished =
        wait_streaming(cold, false, cx).await && wait_page(cold, Page::Conversation, cx).await;
    let ok = options_match
        && initial
        && entered_settings
        && settings_size
        && returned
        && conversation_after_settings
        && pasted
        && first_streaming
        && first_conversation
        && conversation_size
        && user_size
        && slow_pasted
        && slow_streaming
        && streaming_size_kept
        && slow_conversation
        && preserved_settings
        && preserved_return
        && compact_again
        && cold_pasted
        && cold_streaming
        && cold_expanded
        && cold_finished;
    println!(
        "T45: 创建选项 {options_match}；初始紧凑 {initial}；设置点击/尺寸 {entered_settings}/{settings_size}；返回对话/尺寸 {returned}/{conversation_after_settings}；剪贴板键盘发送 {pasted}/{first_streaming}/{first_conversation}；默认尺寸 {conversation_size}；用户尺寸 {user_size}；慢流及尺寸 {slow_pasted}/{slow_streaming}/{streaming_size_kept}/{slow_conversation}；设置与返回保留 {preserved_settings}/{preserved_return}；再次回缩 {compact_again}；紧凑直接发送 {cold_pasted}/{cold_streaming}/{cold_expanded}/{cold_finished}"
    );
    ok
}
