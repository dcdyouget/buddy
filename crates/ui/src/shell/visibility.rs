//! 主窗口显示、隐藏与 macOS 外部点击桥。
//!
//! 原生回调只把事件投递到 channel；它不读取或修改 GPUI `App`，避免在 App 借用期间
//! 重入窗口更新。调用方应在 GPUI 主线程重新取得窗口后处理事件。

use gpui::Window;
use tokio::sync::mpsc::UnboundedSender;

#[cfg(target_os = "macos")]
use objc::{sel, sel_impl};

/// 主窗口可见性操作失败的诊断。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum VisibilityError {
    /// 当前平台没有本模块的原生窗口实现。
    #[cfg(not(target_os = "macos"))]
    UnsupportedPlatform,
    /// AppKit 对象只能从主线程读取或修改。
    NotMainThread,
    /// GPUI 没有返回可用的原生窗口句柄。
    WindowHandle(String),
    /// 窗口句柄不是 AppKit 的 NSView 句柄。
    UnsupportedWindowHandle,
    /// AppKit 返回了空对象。
    NullObject(&'static str),
    /// NSEvent 全局监听器注册失败。
    MonitorRegistrationFailed,
}

impl std::fmt::Display for VisibilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(not(target_os = "macos"))]
            Self::UnsupportedPlatform => f.write_str("当前平台不支持原生窗口可见性控制"),
            Self::NotMainThread => f.write_str("macOS 原生窗口只能从主线程读取或修改"),
            Self::WindowHandle(error) => write!(f, "读取 GPUI 原生窗口句柄失败：{error}"),
            Self::UnsupportedWindowHandle => f.write_str("GPUI 窗口句柄不是 AppKit 类型"),
            Self::NullObject(name) => write!(f, "AppKit 返回空对象：{name}"),
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

/// 外部鼠标按下事件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VisibilityEvent {
    /// 其他应用中的鼠标按下；本应用内的弹窗事件不会经此 monitor 投递。
    ExternalMouseDown,
}

/// 已取得原生窗口强引用的可见性操作器。
///
/// `prepare` 在 GPUI 窗口借用期间只取得句柄；后续操作可在释放 GPUI `App` 借用后连续
/// 执行，从而避免同步原生回调重入 GPUI。
pub(crate) struct PreparedVisibility {
    #[cfg(target_os = "macos")]
    native: objc::rc::StrongPtr,
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
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        Err(VisibilityError::UnsupportedPlatform)
    }
}

impl PreparedVisibility {
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
        #[cfg(not(target_os = "macos"))]
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
        Ok(())
    }

    fn restore_normal_level(&self) -> Result<(), VisibilityError> {
        #[cfg(target_os = "macos")]
        {
            ensure_main_thread()?;
            unsafe {
                let _: () = objc::msg_send![*self.native, setLevel: 0i64];
            }
        }
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
                let _: () = objc::msg_send![*self.native, orderFrontRegardless];
                let _: () = objc::msg_send![
                    *self.native,
                    makeKeyAndOrderFront: std::ptr::null_mut::<objc::runtime::Object>()
                ];
                return snapshot(*self.native);
            }
        }
        #[cfg(not(target_os = "macos"))]
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
        #[cfg(not(target_os = "macos"))]
        {
            Err(VisibilityError::UnsupportedPlatform)
        }
    }
}

/// macOS 外部鼠标监听器；Drop 时移除 NSEvent global monitor。
pub(crate) struct OutsideClickMonitor {
    #[cfg(target_os = "macos")]
    token: *mut objc::runtime::Object,
    #[cfg(target_os = "macos")]
    _handler: block::RcBlock<(*mut objc::runtime::Object,), ()>,
}

impl OutsideClickMonitor {
    /// 注册只监听其他应用鼠标按下的 NSEvent global monitor。
    pub(crate) fn new(sender: UnboundedSender<VisibilityEvent>) -> Result<Self, VisibilityError> {
        #[cfg(target_os = "macos")]
        {
            ensure_main_thread()?;
            let handler = block::ConcreteBlock::new(move |_event: *mut objc::runtime::Object| {
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
        #[cfg(not(target_os = "macos"))]
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
