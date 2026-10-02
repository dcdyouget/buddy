//! macOS 工作区与原生窗口身份读回。
//!
//! 这是测试和诊断用的只读桥：从 GPUI 的 raw window handle 取得真实 NSWindow，
//! 再同步读取窗口号、工作区归属、全屏 style mask 和 frame。调用方不得用 bounds
//! 猜测另一个窗口的身份。

use gpui::Window;

/// A read-only snapshot of one native NSWindow workspace state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkspaceWindowSnapshot {
    /// AppKit `windowNumber`，可与 CGWindowList 的窗口栈稳定关联。
    pub window_number: i64,
    /// AppKit `isOnActiveSpace` 的真实返回值。
    pub is_on_active_space: bool,
    /// NSWindow style mask 是否包含 `NSFullScreenWindowMask`。
    pub is_fullscreen: bool,
    /// AppKit frame，坐标系保持原生值。
    pub frame: [f64; 4],
}

/// Errors returned while reading a native workspace window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceWindowError {
    /// 当前平台没有 AppKit 工作区实现。
    UnsupportedPlatform,
    /// GPUI 没有返回可用的原生窗口句柄。
    WindowHandle(String),
    /// GPUI 返回的句柄不是 AppKit NSView。
    UnsupportedWindowHandle,
    /// 调用发生在 macOS 主线程之外。
    NotMainThread,
    /// NSView 或 NSWindow 指针为空。
    NullWindow,
}

impl std::fmt::Display for WorkspaceWindowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform => f.write_str("当前平台不支持 macOS 工作区读回"),
            Self::WindowHandle(error) => write!(f, "读取 GPUI 原生窗口句柄失败：{error}"),
            Self::UnsupportedWindowHandle => f.write_str("GPUI 窗口句柄不是 AppKit 类型"),
            Self::NotMainThread => f.write_str("macOS 原生窗口只能在主线程读取"),
            Self::NullWindow => f.write_str("AppKit 原生窗口为空"),
        }
    }
}

impl std::error::Error for WorkspaceWindowError {}

/// 从当前 GPUI 窗口同步读取 AppKit 工作区状态。
#[cfg(target_os = "macos")]
pub fn probe(window: &Window) -> Result<WorkspaceWindowSnapshot, WorkspaceWindowError> {
    use objc::runtime::{BOOL, Object, YES};
    use objc::{class, msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    #[repr(C)]
    struct Point {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    struct Size {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    struct Rect {
        origin: Point,
        size: Size,
    }

    let handle = <Window as HasWindowHandle>::window_handle(window)
        .map_err(|error| WorkspaceWindowError::WindowHandle(error.to_string()))?;
    let view = match handle.as_raw() {
        RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr() as *mut Object,
        _ => return Err(WorkspaceWindowError::UnsupportedWindowHandle),
    };
    if view.is_null() {
        return Err(WorkspaceWindowError::NullWindow);
    }
    // SAFETY: this function is called from a GPUI window update, but keep the check local so
    // future callers cannot accidentally move AppKit access to a worker thread.
    unsafe {
        let is_main: BOOL = msg_send![class!(NSThread), isMainThread];
        if is_main != YES {
            return Err(WorkspaceWindowError::NotMainThread);
        }
        // GPUI owns this AppKit NSView and its NSWindow for the duration of the call;
        // all selectors below are synchronous, read-only NSWindow queries.
        let native: *mut Object = msg_send![view, window];
        if native.is_null() {
            return Err(WorkspaceWindowError::NullWindow);
        }
        let window_number: i64 = msg_send![native, windowNumber];
        let on_active_space: BOOL = msg_send![native, isOnActiveSpace];
        let style_mask: u64 = msg_send![native, styleMask];
        let frame: Rect = msg_send![native, frame];
        Ok(WorkspaceWindowSnapshot {
            window_number,
            is_on_active_space: on_active_space == YES,
            is_fullscreen: style_mask & (1 << 14) != 0,
            frame: [
                frame.origin.x,
                frame.origin.y,
                frame.size.width,
                frame.size.height,
            ],
        })
    }
}

/// On non-macOS platforms this diagnostic has no native implementation.
#[cfg(not(target_os = "macos"))]
pub fn probe(_: &Window) -> Result<WorkspaceWindowSnapshot, WorkspaceWindowError> {
    Err(WorkspaceWindowError::UnsupportedPlatform)
}
