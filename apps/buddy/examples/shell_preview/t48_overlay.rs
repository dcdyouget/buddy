//! T48 的设置页、覆盖层、审批和外点流程。

use super::super::{external_target, fixture, input, os_input};
use super::{
    visible, wait_streaming, wait_streaming_without_draw, wait_visible, wait_visible_without_draw,
};
use buddy_ui::chat::page_state::Page;
use buddy_ui::gpui::{AppContext, AsyncApp, Entity, WindowHandle};
use buddy_ui::shell::{self, AppShell};
use std::path::Path;
use std::time::Duration;

pub(crate) struct Result {
    pub(crate) settings_hidden: bool,
    pub(crate) provider_consumed: bool,
    pub(crate) hotkey_consumed: bool,
    pub(crate) model_consumed: bool,
    pub(crate) approval_consumed: bool,
    pub(crate) external_stream_started: bool,
    pub(crate) external_clicked: bool,
    pub(crate) external_finished: bool,
    pub(crate) external_reopened: bool,
    pub(crate) external_text: bool,
    pub(crate) same_router: bool,
}

fn settings_x(width: f32) -> f32 {
    width - 5.0 - 28.0 - 4.0 - 24.0 - 4.0 - 12.0
}

async fn wait_page(handle: WindowHandle<AppShell>, expected: Page, cx: &mut AsyncApp) -> bool {
    for _ in 0..120 {
        input::draw(handle, cx).await;
        if handle
            .read_with(cx, |shell, app| shell.router().read(app).page() == expected)
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

async fn open_settings(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> bool {
    let Some((width, height)) = input::size(handle, cx).map(|(w, h)| (f32::from(w), f32::from(h)))
    else {
        return false;
    };
    input::click(handle, settings_x(width), height - 21.0, false, cx).await;
    wait_page(handle, Page::Settings, cx).await
}

pub(crate) async fn start_slow_stream(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> bool {
    input::paste_text(handle, "慢", cx).await && {
        input::press(handle, "enter", cx).await;
        wait_streaming(handle, true, cx).await
    }
}

async fn wait_approval(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> bool {
    for _ in 0..160 {
        input::draw(handle, cx).await;
        if handle
            .read_with(cx, |shell, app| {
                shell
                    .router()
                    .read(app)
                    .conversation()
                    .read(app)
                    .state
                    .approval
                    .is_some()
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

async fn wait_approval_cleared(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> bool {
    for _ in 0..160 {
        input::draw(handle, cx).await;
        if handle
            .read_with(cx, |shell, app| {
                shell
                    .router()
                    .read(app)
                    .conversation()
                    .read(app)
                    .state
                    .approval
                    .is_none()
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

async fn wait_file(path: &Path, cx: &mut AsyncApp) -> bool {
    for _ in 0..150 {
        if path.is_file() {
            return true;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    false
}

async fn external_click_hides(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> bool {
    let Ok(mut target) = external_target::spawn() else {
        return false;
    };
    let ready = wait_file(&target.ready, cx).await;
    let click = if ready {
        let values = std::fs::read_to_string(&target.ready)
            .ok()
            .and_then(|line| {
                let mut fields = line.split_whitespace();
                if fields.next() != Some("READY") {
                    return None;
                }
                Some((
                    fields.next()?.parse::<f64>().ok()?,
                    fields.next()?.parse::<f64>().ok()?,
                    fields.next()?.parse::<f64>().ok()?,
                    fields.next()?.parse::<f64>().ok()?,
                ))
            });
        values.is_some_and(|(x, y, width, height)| {
            os_input::click_screen(x + width / 2.0, y + height / 2.0)
        })
    } else {
        false
    };
    let ack = click && wait_file(&target.ack, cx).await;
    let hidden = ack && wait_visible_without_draw(handle, false, cx).await;
    let _ = target.child.kill();
    let _ = target.child.wait();
    let _ = std::fs::remove_file(&target.ready);
    let _ = std::fs::remove_file(&target.ack);
    hidden
}

pub(crate) async fn run(
    handle: WindowHandle<AppShell>,
    router: Entity<buddy_ui::chat::router::PageRouter>,
    cx: &mut AsyncApp,
) -> Result {
    // 设置页自身沿用当前 v1 根级 Esc：先隐藏主窗口；覆盖层内的子流程再优先消费 Esc。
    let settings_open = wait_visible(handle, true, cx).await && open_settings(handle, cx).await;
    input::press(handle, "escape", cx).await;
    let settings_hidden = settings_open && wait_visible(handle, false, cx).await;
    let _ = shell::runtime::show(handle, cx).await;
    let settings_reopened =
        wait_visible(handle, true, cx).await && wait_page(handle, Page::Settings, cx).await;
    input::settle(handle, cx).await;

    let provider_open = if settings_reopened {
        let button = router.read_with(cx, |router, app| {
            router.settings_view().read(app).add_button_bounds()
        });
        if let Some(bounds) = button {
            input::click(
                handle,
                f32::from(bounds.origin.x + bounds.size.width / 2.0),
                f32::from(bounds.origin.y + bounds.size.height / 2.0),
                false,
                cx,
            )
            .await;
            router.read_with(cx, |router, app| {
                router.settings_view().read(app).provider_open()
            })
        } else {
            false
        }
    } else {
        false
    };
    input::press(handle, "escape", cx).await;
    let provider_consumed = provider_open
        && router.read_with(cx, |router, app| {
            !router.settings_view().read(app).provider_open()
        })
        && visible(handle, cx) == Some(true);

    let hotkey_recording = if settings_reopened {
        let recorder = router.read_with(cx, |router, app| {
            router.settings_view().read(app).hotkey_recorder().clone()
        });
        let bounds = recorder.read_with(cx, |recorder, _| recorder.button_bounds_for_test());
        if let Some(bounds) = bounds {
            input::click(
                handle,
                f32::from(bounds.origin.x + bounds.size.width / 2.0),
                f32::from(bounds.origin.y + bounds.size.height / 2.0),
                false,
                cx,
            )
            .await;
            recorder.read_with(cx, |recorder, _| recorder.recording())
        } else {
            false
        }
    } else {
        false
    };
    input::press(handle, "escape", cx).await;
    let hotkey_consumed = hotkey_recording
        && router.read_with(cx, |router, app| {
            router
                .settings_view()
                .read(app)
                .hotkey_recorder()
                .read(app)
                .recording()
        }) == false
        && visible(handle, cx) == Some(true);

    let _ = router.update(cx, |router, cx| router.close_settings(cx));
    wait_page(handle, Page::Conversation, cx).await;
    let model_open = if let Some(bounds) = router.read_with(cx, |router, app| {
        router.composer().read(app).model_button_bounds()
    }) {
        input::click(
            handle,
            f32::from(bounds.origin.x + bounds.size.width / 2.0),
            f32::from(bounds.origin.y + bounds.size.height / 2.0),
            false,
            cx,
        )
        .await;
        router.read_with(cx, |router, _| router.model_menu().is_some())
    } else {
        false
    };
    let model_consumed = if model_open {
        let menu = router.read_with(cx, |router, _| router.model_menu());
        if let Some(menu) = menu {
            let _ = cx.update_window(menu.into(), |_, window, cx| {
                window.dispatch_event(
                    buddy_ui::gpui::PlatformInput::KeyDown(buddy_ui::gpui::KeyDownEvent {
                        keystroke: buddy_ui::gpui::Keystroke::parse("escape").unwrap(),
                        is_held: false,
                        prefer_character_input: false,
                    }),
                    cx,
                )
            });
            input::draw(handle, cx).await;
            let menu_removed = menu.read_with(cx, |_, _| ()).is_err();
            menu_removed && visible(handle, cx) == Some(true)
        } else {
            false
        }
    } else {
        false
    };

    let approval_root = fixture::sandbox("t48-approval");
    let approval_path = approval_root.join("denied.txt");
    let approval_prompt = format!("请写文件 {}", approval_path.display());
    let approval_prompted = input::paste_text(handle, &approval_prompt, cx).await;
    if approval_prompted {
        input::press(handle, "enter", cx).await;
    }
    let approval_open = approval_prompted && wait_approval(handle, cx).await;
    input::press(handle, "escape", cx).await;
    let approval_cleared = wait_approval_cleared(handle, cx).await;
    let approval_stopped = wait_streaming(handle, false, cx).await;
    let approval_file_absent = !approval_path.is_file();
    let approval_consumed = approval_open
        && approval_cleared
        && approval_file_absent
        && approval_stopped
        && visible(handle, cx) == Some(true);
    let _ = std::fs::remove_dir_all(approval_root);

    let external_stream_started = start_slow_stream(handle, cx).await;
    let external_clicked = external_click_hides(handle, cx).await;
    let external_finished = wait_streaming_without_draw(handle, false, cx).await;
    let _ = shell::runtime::show(handle, cx).await;
    let external_reopened = wait_visible(handle, true, cx).await;
    let external_text = handle
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
    let same_router = handle
        .read_with(cx, |shell, _| shell.router())
        .is_ok_and(|current| current == router);

    Result {
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
    }
}
