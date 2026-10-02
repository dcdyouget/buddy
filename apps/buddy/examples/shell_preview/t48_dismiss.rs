//! T48：Esc、设置/菜单/审批优先级和专用 child 外部点击关闭。

use super::{input, os_input};
use buddy_ui::gpui::{AppContext, AsyncApp, Entity, WindowHandle};
use buddy_ui::shell::native::{NativeWindowSnapshot, probe_main_window};
use buddy_ui::shell::{self, AppShell};
use std::time::Duration;

#[path = "t48_overlay.rs"]
mod overlay;

fn probe(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<NativeWindowSnapshot> {
    cx.update_window(handle.into(), |_, window, _| probe_main_window(window).ok())
        .ok()
        .flatten()
}

fn visible(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<bool> {
    probe(handle, cx).map(|snapshot| snapshot.is_visible)
}

async fn wait_visible(handle: WindowHandle<AppShell>, expected: bool, cx: &mut AsyncApp) -> bool {
    for attempt in 0..120 {
        if attempt == 0 {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
        }
        input::draw(handle, cx).await;
        if visible(handle, cx) == Some(expected) {
            return true;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    false
}

async fn wait_visible_without_draw(
    handle: WindowHandle<AppShell>,
    expected: bool,
    cx: &mut AsyncApp,
) -> bool {
    for attempt in 0..120 {
        if attempt == 0 {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
        }
        if visible(handle, cx) == Some(expected) {
            return true;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    false
}

async fn wait_streaming(handle: WindowHandle<AppShell>, expected: bool, cx: &mut AsyncApp) -> bool {
    for _ in 0..200 {
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
                    == expected
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

async fn wait_streaming_without_draw(
    handle: WindowHandle<AppShell>,
    expected: bool,
    cx: &mut AsyncApp,
) -> bool {
    for _ in 0..220 {
        if handle
            .read_with(cx, |shell, app| {
                shell
                    .router()
                    .read(app)
                    .conversation()
                    .read(app)
                    .state
                    .is_streaming()
                    == expected
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

fn router_of(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<Entity<buddy_ui::chat::router::PageRouter>> {
    handle.read_with(cx, |shell, _| shell.router()).ok()
}

pub(crate) async fn run(cx: &mut AsyncApp) -> bool {
    macro_rules! fail {
        ($message:expr) => {{
            println!("FAIL T48：{}", $message);
            return false;
        }};
    }

    let preflight = os_input::preflight();
    if preflight != os_input::Preflight::Ready {
        println!("FAIL T48：{}", os_input::preflight_message(preflight));
        return false;
    }
    let Some(handle) = cx.update(|app| shell::runtime::main_window(app)) else {
        fail!("T47 未安装可复用的主窗口运行时");
    };
    input::settle(handle, cx).await;
    let Some(router) = router_of(handle, cx) else {
        fail!("读取 Router 失败");
    };

    let stream_started = overlay::start_slow_stream(handle, cx).await;
    input::press(handle, "escape", cx).await;
    let esc_hidden = wait_visible_without_draw(handle, false, cx).await;
    let stream_finished_hidden = wait_streaming_without_draw(handle, false, cx).await;
    let same_content = handle
        .read_with(cx, |shell, app| {
            shell
                .router()
                .read(app)
                .conversation()
                .read(app)
                .state
                .messages
                .last()
                .is_some_and(|message| crate::fixture::complete_slow_response(&message.content))
        })
        .unwrap_or(false);
    let _ = shell::runtime::show(handle, cx).await;
    let reopened = wait_visible(handle, true, cx).await;

    let overlay = overlay::run(handle, router, cx).await;
    let settings_hidden = overlay.settings_hidden;
    let provider_consumed = overlay.provider_consumed;
    let hotkey_consumed = overlay.hotkey_consumed;
    let model_consumed = overlay.model_consumed;
    let approval_consumed = overlay.approval_consumed;
    let external_stream_started = overlay.external_stream_started;
    let external_clicked = overlay.external_clicked;
    let external_finished = overlay.external_finished;
    let external_reopened = overlay.external_reopened;
    let external_text = overlay.external_text;
    let same_router = overlay.same_router;
    let ok = stream_started
        && esc_hidden
        && stream_finished_hidden
        && same_content
        && reopened
        && settings_hidden
        && provider_consumed
        && hotkey_consumed
        && model_consumed
        && approval_consumed
        && external_stream_started
        && external_clicked
        && external_finished
        && external_reopened
        && external_text
        && same_router;
    println!(
        "T48: 慢流开始/无 overlay Esc 隐藏/隐藏后完成/重显/内容 {}/{}/{}/{}/{}；设置隐藏 {}/Provider 优先 {}/热键录制优先 {}/模型菜单优先 {}/审批优先 {}/外点慢流/隐藏/完成/重显/内容 {}/{}/{}/{}/{}；同 Router {}",
        stream_started,
        esc_hidden,
        stream_finished_hidden,
        reopened,
        same_content,
        settings_hidden,
        provider_consumed,
        hotkey_consumed,
        model_consumed,
        approval_consumed,
        external_stream_started,
        external_clicked,
        external_finished,
        external_reopened,
        external_text,
        same_router,
    );
    ok
}
