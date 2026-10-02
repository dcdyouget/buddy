//! macOS 选中文本取词：在主窗口显示前复制前台应用的选区。
//!
//! `begin_before_show` 必须在显示窗口之前调用。它在同一个 GPUI 前台更新中读取旧
//! 剪贴板并同步发送 Cmd+C，随后由 `finish_after_copy` 在 GPUI 的异步上下文中等待
//! 剪贴板更新。等待期间不阻塞主线程，也不会触碰正在运行的对话任务。

use gpui::{App, AsyncApp, ClipboardItem};
use std::time::Duration;

const COPY_SETTLE_DELAY: Duration = Duration::from_millis(50);

/// 一次选中文本捕获的临时状态。
///
/// 同时保存旧剪贴板项目和其中的文本。文本用于判断选区是否变化，完整项目用于
/// 在取词结束后恢复用户原来的剪贴板内容。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionCapture {
    previous_item: Option<ClipboardItem>,
    previous_text: String,
}

/// 在窗口显示前开始一次取词捕获。
///
/// 读取旧剪贴板和发送 Cmd+C 都在调用线程同步完成。调用方应在该函数返回后再
/// 显示或激活窗口，这样目标应用仍是前台应用，模拟按键才会送到正确的接收者。
#[cfg(target_os = "macos")]
pub fn begin_before_show(cx: &mut AsyncApp) -> SelectionCapture {
    let (previous_item, previous_text) = cx.update(|cx| {
        let previous_item = cx.read_from_clipboard();
        let previous_text = previous_item
            .as_ref()
            .and_then(ClipboardItem::text)
            .unwrap_or_default();
        post_command_c();
        (previous_item, previous_text)
    });

    SelectionCapture {
        previous_item,
        previous_text,
    }
}

/// 非 macOS 平台没有 v1 的系统取词路径，也不触碰 GPUI 剪贴板。
#[cfg(not(target_os = "macos"))]
pub fn begin_before_show(_cx: &mut AsyncApp) -> SelectionCapture {
    SelectionCapture {
        previous_item: None,
        previous_text: String::new(),
    }
}

/// 等待复制结果、读取选中文本并恢复旧剪贴板。
///
/// 计时使用 GPUI 后台执行器，不在 UI 线程 sleep。返回值已经 trim；空文本或与旧
/// 剪贴板文本相同的结果返回 `None`。无论读取是否成功，旧剪贴板都会尝试恢复。
#[cfg(target_os = "macos")]
pub async fn finish_after_copy(
    cx: &mut AsyncApp,
    capture: SelectionCapture,
) -> Option<String> {
    cx.background_executor().timer(COPY_SETTLE_DELAY).await;

    cx.update(|cx| {
        let selected = clipboard_text(cx);
        let result = changed_selection(&capture.previous_text, selected.as_deref());
        let restore = capture
            .previous_item
            .unwrap_or_else(|| ClipboardItem::new_string(capture.previous_text));
        cx.write_to_clipboard(restore);
        result
    })
}

/// 非 macOS 平台没有系统取词路径，不读取、不恢复窗口剪贴板。
#[cfg(not(target_os = "macos"))]
pub async fn finish_after_copy(
    _cx: &mut AsyncApp,
    _capture: SelectionCapture,
) -> Option<String> {
    None
}

/// 对剪贴板候选文本执行取词规则。
///
/// 该函数保持纯逻辑，供热键流程和测试复用：先判断复制结果非空且与旧文本不同，
/// 再 trim 后返回。比较顺序与 v1 保持一致，避免把选区前后的空白变化误判为无变化。
pub fn changed_selection(previous: &str, candidate: Option<&str>) -> Option<String> {
    let candidate = candidate?;
    if candidate.is_empty() || candidate == previous {
        return None;
    }
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_owned())
}

fn clipboard_text(cx: &App) -> Option<String> {
    cx.read_from_clipboard().and_then(|item| item.text())
}

#[cfg(target_os = "macos")]
fn post_command_c() {
    // CoreGraphics 的 CGEventPost 需要在仍拥有前台目标的调用线程同步执行。
    // 这里只使用系统 FFI，避免给 UI crate 增加第二套 clipboard / event loop 依赖。
    unsafe {
        let mut source = CGEventSourceCreate(CG_EVENT_SOURCE_PRIVATE);
        if source.is_null() {
            source = CGEventSourceCreate(CG_EVENT_SOURCE_COMBINED);
        }
        if source.is_null() {
            return;
        }

        let key_down = CGEventCreateKeyboardEvent(source, KEYCODE_C, true);
        let key_up = CGEventCreateKeyboardEvent(source, KEYCODE_C, false);
        if !key_down.is_null() {
            CGEventSetFlags(key_down, CG_EVENT_FLAG_COMMAND);
            CGEventPost(CG_EVENT_TAP_HID, key_down);
            CFRelease(key_down as *const _);
        }
        if !key_up.is_null() {
            CGEventSetFlags(key_up, CG_EVENT_FLAG_COMMAND);
            CGEventPost(CG_EVENT_TAP_HID, key_up);
            CFRelease(key_up as *const _);
        }
        CFRelease(source as *const _);
    }
}

#[cfg(target_os = "macos")]
type CgEventRef = *mut std::ffi::c_void;

#[cfg(target_os = "macos")]
type CgEventSourceRef = *mut std::ffi::c_void;

#[cfg(target_os = "macos")]
const CG_EVENT_SOURCE_COMBINED: i32 = 0;

#[cfg(target_os = "macos")]
const CG_EVENT_SOURCE_PRIVATE: i32 = -1;

#[cfg(target_os = "macos")]
const CG_EVENT_TAP_HID: u32 = 0;

#[cfg(target_os = "macos")]
const CG_EVENT_FLAG_COMMAND: u64 = 1 << 20;

#[cfg(target_os = "macos")]
const KEYCODE_C: u16 = 8;

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceCreate(state_id: i32) -> CgEventSourceRef;
    fn CGEventCreateKeyboardEvent(
        source: CgEventSourceRef,
        virtual_key: u16,
        key_down: bool,
    ) -> CgEventRef;
    fn CGEventSetFlags(event: CgEventRef, flags: u64);
    fn CGEventPost(tap: u32, event: CgEventRef);
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: *const std::ffi::c_void);
}

#[cfg(test)]
mod tests {
    use super::changed_selection;

    #[test]
    fn trims_changed_selection() {
        assert_eq!(
            changed_selection("旧文本", Some("  新文本\n")),
            Some("新文本".to_owned())
        );
    }

    #[test]
    fn rejects_empty_or_unchanged_selection() {
        assert_eq!(changed_selection("旧文本", Some(" \n\t")), None);
        assert_eq!(changed_selection("旧文本", Some("旧文本")), None);
        assert_eq!(changed_selection("旧文本", None), None);
    }

    #[test]
    fn compares_trimmed_candidate_to_original_text() {
        assert_eq!(
            changed_selection("旧文本", Some(" 旧文本\n")),
            Some("旧文本".into())
        );
        assert_eq!(changed_selection("", Some(" 新文本 ")), Some("新文本".into()));
    }
}
