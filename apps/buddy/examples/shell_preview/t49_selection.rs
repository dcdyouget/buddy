//! T49：用真实 GPUI 输入和 macOS Cmd+C 验证选中文本唤起。
//!
//! 自测不手动写入候选文本：候选内容只能来自系统复制事件。旧剪贴板使用独特
//! 文本，流程结束后必须恢复；捕获期间不调用 engine 发送。

#[cfg(target_os = "macos")]
use super::{input, os_input};
use buddy_ui::gpui::AsyncApp;
#[cfg(target_os = "macos")]
use buddy_ui::chat::page_state::Page;
#[cfg(target_os = "macos")]
use buddy_ui::gpui::{AppContext, ClipboardItem, WindowHandle};
#[cfg(target_os = "macos")]
use buddy_ui::shell::{self, AppShell};
#[cfg(target_os = "macos")]
use buddy_ui::shell::native::probe_main_window;
#[cfg(target_os = "macos")]
use std::ops::Range;

#[cfg(target_os = "macos")]
const OLD_CLIPBOARD: &str = "T49 原始剪贴板 7f3c";
#[cfg(target_os = "macos")]
const COMPOSER_SELECTION: &str = "  T49 OS 选区文本  \n";

#[cfg(target_os = "macos")]
fn router_page(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<Page> {
    handle
        .read_with(cx, |shell, app| shell.router().read(app).page())
        .ok()
}

#[cfg(target_os = "macos")]
fn draft(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<String> {
    handle
        .read_with(cx, |shell, app| {
            shell.router().read(app).composer().read(app).draft(app)
        })
        .ok()
}

#[cfg(target_os = "macos")]
fn messages(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<usize> {
    handle
        .read_with(cx, |shell, app| {
            shell
                .router()
                .read(app)
                .conversation()
                .read(app)
                .state
                .messages
                .len()
        })
        .ok()
}

#[cfg(target_os = "macos")]
fn composer_selection(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<Range<usize>> {
    handle
        .read_with(cx, |shell, app| {
            shell
                .router()
                .read(app)
                .composer()
                .read(app)
                .text_area()
                .read(app)
                .selected_range_for_test()
        })
        .ok()
}

#[cfg(target_os = "macos")]
async fn wait_page(handle: WindowHandle<AppShell>, expected: Page, cx: &mut AsyncApp) -> bool {
    for _ in 0..100 {
        input::draw(handle, cx).await;
        if router_page(handle, cx) == Some(expected) {
            return true;
        }
        cx.background_executor()
            .timer(std::time::Duration::from_millis(20))
            .await;
    }
    false
}

#[cfg(target_os = "macos")]
fn is_visible_and_key(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> bool {
    cx.update_window(handle.into(), |_, window, _| {
        probe_main_window(window)
            .map(|snapshot| snapshot.is_visible && snapshot.is_key)
            .unwrap_or(false)
    })
    .unwrap_or(false)
}

#[cfg(target_os = "macos")]
async fn wait_visible_and_key(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> bool {
    for _ in 0..100 {
        if is_visible_and_key(handle, cx) {
            return true;
        }
        input::draw(handle, cx).await;
        cx.background_executor()
            .timer(std::time::Duration::from_millis(20))
            .await;
    }
    false
}

pub(crate) async fn run(cx: &mut AsyncApp) -> bool {
    macro_rules! fail {
        ($message:expr) => {{
            println!("FAIL T49：{}", $message);
            return false;
        }};
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = cx;
        println!("FAIL T49：选中文本取词自测仅支持 macOS");
        return false;
    }

    #[cfg(target_os = "macos")]
    {
        if !os_input::can_post() {
            fail!("CGEventPost 权限预检未通过，未发送系统 Cmd+C");
        }
        let Some(handle) = cx.update(|app| shell::runtime::main_window(app)) else {
            fail!("T47/T48 未安装可复用的主窗口运行时");
        };
        if shell::runtime::show(handle, cx).await.is_err() {
            fail!("显示并聚焦 Runtime 主窗口失败");
        }
        input::settle(handle, cx).await;

        let Some(router) = handle.read_with(cx, |shell, _app| shell.router().clone()).ok() else {
            fail!("读取 Router 失败");
        };
        if router_page(handle, cx) != Some(Page::Conversation) {
            let _ = router.update(cx, |router, cx| router.close_settings(cx));
            if !wait_page(handle, Page::Conversation, cx).await {
                fail!("返回 Conversation 失败");
            }
        }

        // 先通过真实 GPUI 粘贴建立 Composer 内容，再用真实 cmd-a 产生选区。
        let _ = router.update(cx, |router, cx| router.composer().update(cx, |composer, cx| {
            composer.set_draft("", cx);
        }));
        if !input::paste_text(handle, COMPOSER_SELECTION, cx).await {
            fail!("真实粘贴选区样本失败");
        }
        input::press(handle, "cmd-a", cx).await;
        let selection_expected = 0..COMPOSER_SELECTION.len();
        let selected_all = composer_selection(handle, cx) == Some(selection_expected);
        // show() 的 Ok 只表示调用完成；这里必须读取原生 NSWindow 的实际状态。
        let focused_and_key = wait_visible_and_key(handle, cx).await;
        if !selected_all || !focused_and_key {
            fail!(format!(
                "GPUI Composer 未形成完整选区或主窗口未聚焦：选区={} 聚焦={}",
                selected_all, focused_and_key
            ));
        }

        let before_messages = messages(handle, cx).unwrap_or(usize::MAX);
        let before_draft = draft(handle, cx).unwrap_or_default();
        cx.update(|app| app.write_to_clipboard(ClipboardItem::new_string(OLD_CLIPBOARD.into())));

        // 这里不写入候选文本；begin 会在当前 key window 的 Composer 选区上发送系统 Cmd+C。
        let capture = buddy_ui::shell::selection::begin_before_show(cx);
        let selected = buddy_ui::shell::selection::finish_after_copy(cx, capture).await;
        let expected_selected = COMPOSER_SELECTION.trim();
        let os_copy_succeeded = selected.as_deref() == Some(expected_selected);
        let restored = cx
            .update(|app| app.read_from_clipboard().and_then(|item| item.text()))
            .as_deref()
            == Some(OLD_CLIPBOARD);
        let after_messages = messages(handle, cx).unwrap_or(usize::MAX);
        let draft_unchanged = draft(handle, cx).as_deref() == Some(before_draft.as_str());
        println!(
            "T49：OS Cmd+C 选区复制 {}；旧剪贴板恢复 {}；消息数不变 {}",
            os_copy_succeeded, restored, before_messages == after_messages
        );
        if !os_copy_succeeded || !restored || before_messages != after_messages || !draft_unchanged {
            fail!(format!(
                "选区捕获结果={:?} 恢复={} 消息 {}→{} 草稿未变={}",
                selected, restored, before_messages, after_messages, draft_unchanged
            ));
        }

        // 以下是 PageRouter gate 的逻辑证据：接口直接调用，不冒充真实页面点击。
        let conversation_text = "T49 路由选区草稿";
        let _ = router.update(cx, |router, cx| router.accept_selected_text(conversation_text, cx));
        let conversation_accepts = draft(handle, cx).as_deref() == Some(conversation_text);

        let before_settings = draft(handle, cx).unwrap_or_default();
        let _ = router.update(cx, |router, cx| router.open_settings(cx));
        let settings_open = wait_page(handle, Page::Settings, cx).await;
        let _ = router.update(cx, |router, cx| {
            router.accept_selected_text("T49 设置页应忽略", cx)
        });
        let settings_ignores = settings_open && draft(handle, cx) == Some(before_settings.clone());
        let _ = router.update(cx, |router, cx| router.close_settings(cx));
        let returned = wait_page(handle, Page::Conversation, cx).await;

        let ok = conversation_accepts && settings_ignores && returned;
        println!(
            "T49：Conversation gate 接受 {}；Settings gate 忽略 {}；返回 Conversation {}",
            conversation_accepts, settings_ignores, returned
        );
        ok
    }
}
