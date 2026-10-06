//! 主窗口显示、隐藏与 macOS 外部点击桥。
//!
//! 原生回调只把事件投递到 channel；它不读取或修改 GPUI `App`，避免在 App 借用期间
//! 重入窗口更新。调用方应在 GPUI 主线程重新取得窗口后处理事件。

use gpui::Window;
use tokio::sync::mpsc::UnboundedSender;

#[cfg(target_os = "macos")]
use objc::{sel, sel_impl};
#[cfg(target_os = "windows")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    Foundation::HWND,
    System::Threading::{AttachThreadInput, GetCurrentProcessId, GetCurrentThreadId},
    UI::Input::KeyboardAndMouse::{SetActiveWindow, SetFocus},
    UI::WindowsAndMessaging::{
        BringWindowToTop, GWL_EXSTYLE, GWL_STYLE, GetForegroundWindow, GetWindowLongPtrW,
        GetWindowThreadProcessId, IsWindowVisible, SW_HIDE, SW_RESTORE, SetForegroundWindow,
        ShowWindow,
    },
};

/// 主窗口可见性操作失败的诊断。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum VisibilityError {
    /// 当前平台没有本模块的原生窗口实现。
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    UnsupportedPlatform,
    /// AppKit 对象只能从主线程读取或修改。
    #[cfg(target_os = "macos")]
    NotMainThread,
    /// GPUI 没有返回可用的原生窗口句柄。
    WindowHandle(String),
    /// 窗口句柄不是 AppKit 的 NSView 句柄。
    UnsupportedWindowHandle,
    /// AppKit 返回了空对象。
    #[cfg(target_os = "macos")]
    NullObject(&'static str),
    /// NSEvent 全局监听器注册失败。
    #[cfg(target_os = "macos")]
    MonitorRegistrationFailed,
}

impl std::fmt::Display for VisibilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            Self::UnsupportedPlatform => f.write_str("当前平台不支持原生窗口可见性控制"),
            #[cfg(target_os = "macos")]
            Self::NotMainThread => f.write_str("macOS 原生窗口只能从主线程读取或修改"),
            Self::WindowHandle(error) => write!(f, "读取 GPUI 原生窗口句柄失败：{error}"),
            Self::UnsupportedWindowHandle => f.write_str("GPUI 窗口句柄不是 AppKit 类型"),
            #[cfg(target_os = "macos")]
            Self::NullObject(name) => write!(f, "AppKit 返回空对象：{name}"),
            #[cfg(target_os = "macos")]
            Self::MonitorRegistrationFailed => f.write_str("注册 macOS 外部点击监听失败"),
        }
    }
}

impl std::error::Error for VisibilityError {}

/// 原生窗口状态快照。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VisibilitySnapshot {
    /// 窗口是否可见。
    pub(crate) is_visible: bool,
    /// 窗口是否为当前 key window。
    pub(crate) is_key: bool,
    /// 窗口是否为应用的 main window。
    pub(crate) is_main: bool,
    /// Buddy 应用是否处于前台。
    pub(crate) app_is_active: bool,
}

/// Native state reported by the Windows self-check if the foreground lock denies activation.
#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug)]
pub(crate) struct WindowsFocusDiagnostics {
    window_hwnd: isize,
    foreground_hwnd: isize,
    window_pid: u32,
    foreground_pid: u32,
    window_thread: u32,
    foreground_thread: u32,
    style: isize,
    ex_style: isize,
}

#[cfg(target_os = "windows")]
impl WindowsFocusDiagnostics {
    pub(crate) fn foreground_is_other_process(&self) -> bool {
        self.foreground_hwnd != self.window_hwnd && self.foreground_pid != self.window_pid
    }
}

#[cfg(target_os = "windows")]
impl std::fmt::Display for WindowsFocusDiagnostics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "hwnd=0x{:X} pid={} thread={} style=0x{:X} ex_style=0x{:X}; foreground=0x{:X} pid={} thread={}",
            self.window_hwnd,
            self.window_pid,
            self.window_thread,
            self.style,
            self.ex_style,
            self.foreground_hwnd,
            self.foreground_pid,
            self.foreground_thread,
        )
    }
}

/// 外部鼠标按下事件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum VisibilityEvent {
    /// 其他应用中的鼠标按下；本应用内的弹窗事件不会经此 monitor 投递。
    ExternalMouseDown,
}

/// Whether the foreground native window belongs to this Buddy process. A model popup is
/// a separate native window, so its activation must not count as an external click.
#[cfg(target_os = "windows")]
pub(crate) fn foreground_window_belongs_to_current_process() -> bool {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.is_null() {
            return false;
        }
        let mut process_id = 0;
        GetWindowThreadProcessId(foreground, &mut process_id);
        process_id != 0 && process_id == GetCurrentProcessId()
    }
}

/// 已取得原生窗口强引用的可见性操作器。
///
/// `prepare` 在 GPUI 窗口借用期间只取得句柄；后续操作可在释放 GPUI `App` 借用后连续
/// 执行，从而避免同步原生回调重入 GPUI。
pub(crate) struct PreparedVisibility {
    #[cfg(target_os = "macos")]
    native: objc::rc::StrongPtr,
    #[cfg(target_os = "windows")]
    hwnd: HWND,
}

/// 取得主窗口的原生可见性操作器。
pub(crate) fn prepare(window: &Window) -> Result<PreparedVisibility, VisibilityError> {
    #[cfg(target_os = "macos")]
    {
        ensure_main_thread()?;
        let native = ns_window(window)?;
        return Ok(PreparedVisibility {
            native: unsafe { objc::rc::StrongPtr::retain(native) },
        });
    }
    #[cfg(target_os = "windows")]
    {
        let handle = <Window as HasWindowHandle>::window_handle(window)
            .map_err(|error| VisibilityError::WindowHandle(error.to_string()))?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return Err(VisibilityError::UnsupportedWindowHandle);
        };
        return Ok(PreparedVisibility {
            hwnd: handle.hwnd.get() as HWND,
        });
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = window;
        Err(VisibilityError::UnsupportedPlatform)
    }
}

impl PreparedVisibility {
    /// Gate a hidden window's stale drawable until GPUI has presented its first frame.
    pub(crate) fn set_alpha(&self, alpha: f64) -> Result<(), VisibilityError> {
        #[cfg(target_os = "macos")]
        {
            ensure_main_thread()?;
            unsafe {
                let _: () = objc::msg_send![*self.native, setAlphaValue: alpha];
            }
            Ok(())
        }
        #[cfg(target_os = "windows")]
        {
            // Windows does not expose an NSWindow-like per-window alpha API through the
            // minimal Win32 surface used here. GPUI renders the next frame synchronously
            // before ShowWindow, so keeping the normal opacity avoids a stale drawable.
            let _ = alpha;
            return Ok(());
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = alpha;
            Err(VisibilityError::UnsupportedPlatform)
        }
    }
    /// 播放呼入动效（缩放 + 淡入）；须在 `set_alpha(1.0)` 之前调用。
    pub(crate) fn summon(&self) -> Result<(), VisibilityError> {
        #[cfg(target_os = "macos")]
        {
            ensure_main_thread()?;
            super::window_motion::summon(*self.native);
            Ok(())
        }
        #[cfg(target_os = "windows")]
        {
            return Ok(());
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Err(VisibilityError::UnsupportedPlatform)
        }
    }

    /// 播放呼出动效（收缩 + 淡出），停在透明终帧等待 [`Self::hide`]。
    pub(crate) fn dismiss(&self) -> Result<(), VisibilityError> {
        #[cfg(target_os = "macos")]
        {
            ensure_main_thread()?;
            super::window_motion::dismiss(*self.native);
            Ok(())
        }
        #[cfg(target_os = "windows")]
        {
            return Ok(());
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Err(VisibilityError::UnsupportedPlatform)
        }
    }

    /// 清除呼入 / 呼出动画，图层回到原尺寸、不透明。
    pub(crate) fn settle(&self) -> Result<(), VisibilityError> {
        #[cfg(target_os = "macos")]
        {
            ensure_main_thread()?;
            super::window_motion::settle(*self.native);
            Ok(())
        }
        #[cfg(target_os = "windows")]
        {
            return Ok(());
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Err(VisibilityError::UnsupportedPlatform)
        }
    }

    /// 隐藏窗口但保留 Router、页面状态与 engine 流式任务。
    pub(crate) fn hide(&self) -> Result<VisibilitySnapshot, VisibilityError> {
        #[cfg(target_os = "macos")]
        {
            ensure_main_thread()?;
            unsafe {
                let _: () = objc::msg_send![
                    *self.native,
                    orderOut: std::ptr::null_mut::<objc::runtime::Object>()
                ];
                return snapshot(*self.native);
            }
        }
        #[cfg(target_os = "windows")]
        {
            unsafe {
                ShowWindow(self.hwnd, SW_HIDE);
            }
            return self.probe();
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Err(VisibilityError::UnsupportedPlatform)
        }
    }

    /// 全屏辅助 panel 外部失活时降低到普通应用以下；仍可见且不断流。
    pub(crate) fn sync_focus_level(&self) -> Result<(), VisibilityError> {
        #[cfg(target_os = "macos")]
        {
            ensure_main_thread()?;
            let state = self.probe()?;
            if state.is_visible && !state.app_is_active {
                unsafe {
                    let _: () = objc::msg_send![*self.native, setLevel: -1i64];
                }
            } else if state.is_visible && state.is_key {
                self.restore_normal_level()?;
            }
        }
        #[cfg(target_os = "windows")]
        {
            // Windows automatically restores normal Z-order when a window is foreground.
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn restore_normal_level(&self) -> Result<(), VisibilityError> {
        #[cfg(target_os = "macos")]
        {
            ensure_main_thread()?;
            unsafe {
                let _: () = objc::msg_send![*self.native, setLevel: 0i64];
            }
        }
        #[cfg(target_os = "windows")]
        {}
        Ok(())
    }

    /// 显示窗口并取得焦点，用于全局热键重新唤起。
    pub(crate) fn show_and_focus(&self) -> Result<VisibilitySnapshot, VisibilityError> {
        #[cfg(target_os = "macos")]
        {
            ensure_main_thread()?;
            unsafe {
                let app: *mut objc::runtime::Object =
                    objc::msg_send![objc::class!(NSApplication), sharedApplication];
                if app.is_null() {
                    return Err(VisibilityError::NullObject("NSApplication"));
                }
                let _: () = objc::msg_send![app, activateIgnoringOtherApps: objc::runtime::YES];
                self.restore_normal_level()?;
                let _: () = objc::msg_send![
                    *self.native,
                    makeKeyAndOrderFront: std::ptr::null_mut::<objc::runtime::Object>()
                ];
                return snapshot(*self.native);
            }
        }
        #[cfg(target_os = "windows")]
        {
            unsafe {
                // SetForegroundWindow is intentionally restricted by Windows when another
                // process owns the foreground. Joining that UI input queue briefly is the
                // documented native path for a user-triggered summon; detach immediately.
                let foreground = GetForegroundWindow();
                let foreground_thread = if foreground.is_null() {
                    0
                } else {
                    GetWindowThreadProcessId(foreground, std::ptr::null_mut())
                };
                let current_thread = GetCurrentThreadId();
                let attached = foreground_thread != 0
                    && foreground_thread != current_thread
                    && AttachThreadInput(current_thread, foreground_thread, 1) != 0;
                ShowWindow(self.hwnd, SW_RESTORE);
                BringWindowToTop(self.hwnd);
                SetForegroundWindow(self.hwnd);
                SetActiveWindow(self.hwnd);
                SetFocus(self.hwnd);
                if attached {
                    AttachThreadInput(current_thread, foreground_thread, 0);
                }
            }
            return self.probe();
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Err(VisibilityError::UnsupportedPlatform)
        }
    }

    /// 读取当前可见性、焦点和应用前台状态。
    pub(crate) fn probe(&self) -> Result<VisibilitySnapshot, VisibilityError> {
        #[cfg(target_os = "macos")]
        {
            ensure_main_thread()?;
            unsafe { return snapshot(*self.native) }
        }
        #[cfg(target_os = "windows")]
        {
            unsafe {
                let visible = IsWindowVisible(self.hwnd) != 0;
                let foreground = GetForegroundWindow();
                return Ok(VisibilitySnapshot {
                    is_visible: visible,
                    is_key: foreground == self.hwnd,
                    is_main: foreground == self.hwnd,
                    app_is_active: foreground == self.hwnd,
                });
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Err(VisibilityError::UnsupportedPlatform)
        }
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn focus_diagnostics(&self) -> WindowsFocusDiagnostics {
        unsafe {
            let foreground = GetForegroundWindow();
            let mut window_pid = 0;
            let mut foreground_pid = 0;
            let window_thread = GetWindowThreadProcessId(self.hwnd, &mut window_pid);
            let foreground_thread = if foreground.is_null() {
                0
            } else {
                GetWindowThreadProcessId(foreground, &mut foreground_pid)
            };
            WindowsFocusDiagnostics {
                window_hwnd: self.hwnd as isize,
                foreground_hwnd: foreground as isize,
                window_pid,
                foreground_pid,
                window_thread,
                foreground_thread,
                style: GetWindowLongPtrW(self.hwnd, GWL_STYLE),
                ex_style: GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE),
            }
        }
    }
}

/// macOS 外部鼠标监听器；Drop 时移除 NSEvent global monitor。
pub(crate) struct OutsideClickMonitor {
    #[cfg(target_os = "macos")]
    token: *mut objc::runtime::Object,
    #[cfg(target_os = "macos")]
    _handler: block::RcBlock<(*mut objc::runtime::Object,), ()>,
    #[cfg(target_os = "windows")]
    _private: (),
}

impl OutsideClickMonitor {
    /// 注册只监听其他应用鼠标按下的 NSEvent global monitor。
    pub(crate) fn new(sender: UnboundedSender<VisibilityEvent>) -> Result<Self, VisibilityError> {
        #[cfg(target_os = "macos")]
        {
            ensure_main_thread()?;
            let handler = block::ConcreteBlock::new(move |_event: *mut objc::runtime::Object| {
                // 必须在事件发生时同步判断：双击选中图片会立即关闭选择器，异步再查就晚了。
                if unsafe { system_panel_open() } {
                    return;
                }
                let _ = sender.send(VisibilityEvent::ExternalMouseDown);
            })
            .copy();
            // NSEventMaskLeftMouseDown = 1 << 1, RightMouseDown = 1 << 3,
            // OtherMouseDown = 1 << NSEventTypeOtherMouseDown (25).
            // 1 << 4 is OtherMouseUp and must not be used here.
            const LEFT_MOUSE_DOWN: usize = 1 << 1;
            const RIGHT_MOUSE_DOWN: usize = 1 << 3;
            const OTHER_MOUSE_DOWN: usize = 1 << 25;
            let mask = LEFT_MOUSE_DOWN | RIGHT_MOUSE_DOWN | OTHER_MOUSE_DOWN;
            let token: *mut objc::runtime::Object = unsafe {
                objc::msg_send![objc::class!(NSEvent), addGlobalMonitorForEventsMatchingMask: mask handler: &*handler]
            };
            if token.is_null() {
                return Err(VisibilityError::MonitorRegistrationFailed);
            }
            return Ok(Self {
                token,
                _handler: handler,
            });
        }
        #[cfg(target_os = "windows")]
        {
            // Win32 activation changes are observed by positioning_controller. A global
            // low-level mouse hook would require a DLL and causes false positives for
            // system menus, so keep the monitor alive as a capability marker only.
            let _ = sender;
            return Ok(Self { _private: () });
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = sender;
            Err(VisibilityError::UnsupportedPlatform)
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for OutsideClickMonitor {
    fn drop(&mut self) {
        if self.token.is_null() {
            return;
        }
        unsafe {
            let _: () = objc::msg_send![objc::class!(NSEvent), removeMonitor: self.token];
        }
    }
}

/// 本应用是否正显示系统文件面板（打开 / 存储）或模态窗口。
///
/// macOS 的 `NSOpenPanel` / `NSSavePanel` 由独立进程「Open and Save Panel Service」绘制，
/// 用户在面板里的点击对本进程而言是「其他应用的点击」，会被外部点击监听当作点外部收起。
#[cfg(target_os = "macos")]
unsafe fn system_panel_open() -> bool {
    use objc::runtime::{BOOL, Object, YES};
    unsafe {
        let app: *mut Object = objc::msg_send![objc::class!(NSApplication), sharedApplication];
        if app.is_null() {
            return false;
        }
        let modal: *mut Object = objc::msg_send![app, modalWindow];
        if !modal.is_null() {
            return true;
        }
        let windows: *mut Object = objc::msg_send![app, windows];
        if windows.is_null() {
            return false;
        }
        let count: usize = objc::msg_send![windows, count];
        let panel_class = objc::class!(NSSavePanel);
        (0..count).any(|ix| {
            let window: *mut Object = objc::msg_send![windows, objectAtIndex: ix];
            let is_panel: BOOL = objc::msg_send![window, isKindOfClass: panel_class];
            let visible: BOOL = objc::msg_send![window, isVisible];
            is_panel == YES && visible == YES
        })
    }
}

#[cfg(target_os = "macos")]
fn ensure_main_thread() -> Result<(), VisibilityError> {
    use objc::runtime::{BOOL, YES};
    let is_main: BOOL = unsafe { objc::msg_send![objc::class!(NSThread), isMainThread] };
    if is_main == YES {
        Ok(())
    } else {
        Err(VisibilityError::NotMainThread)
    }
}

#[cfg(target_os = "macos")]
fn ns_window(window: &Window) -> Result<*mut objc::runtime::Object, VisibilityError> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = <Window as HasWindowHandle>::window_handle(window)
        .map_err(|error| VisibilityError::WindowHandle(error.to_string()))?;
    let ns_view = match handle.as_raw() {
        RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr() as *mut objc::runtime::Object,
        _ => return Err(VisibilityError::UnsupportedWindowHandle),
    };
    if ns_view.is_null() {
        return Err(VisibilityError::NullObject("NSView"));
    }
    unsafe {
        let native: *mut objc::runtime::Object = objc::msg_send![ns_view, window];
        if native.is_null() {
            Err(VisibilityError::NullObject("NSWindow"))
        } else {
            Ok(native)
        }
    }
}

#[cfg(target_os = "macos")]
unsafe fn snapshot(
    native: *mut objc::runtime::Object,
) -> Result<VisibilitySnapshot, VisibilityError> {
    let app: *mut objc::runtime::Object =
        unsafe { objc::msg_send![objc::class!(NSApplication), sharedApplication] };
    if app.is_null() {
        return Err(VisibilityError::NullObject("NSApplication"));
    }
    let visible: objc::runtime::BOOL = unsafe { objc::msg_send![native, isVisible] };
    let key: objc::runtime::BOOL = unsafe { objc::msg_send![native, isKeyWindow] };
    let main: objc::runtime::BOOL = unsafe { objc::msg_send![native, isMainWindow] };
    let active: objc::runtime::BOOL = unsafe { objc::msg_send![app, isActive] };
    Ok(VisibilitySnapshot {
        is_visible: visible == objc::runtime::YES,
        is_key: key == objc::runtime::YES,
        is_main: main == objc::runtime::YES,
        app_is_active: active == objc::runtime::YES,
    })
}
