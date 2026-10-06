//! macOS AppKit 定位桥。
//!
//! `prepare` 只在 GPUI 借用窗口时取得一个强引用；`snapshot` / `set_rect` 在借用
//! 释放后同步执行，避免 AppKit 的 resize 回调重入 GPUI 的 `App` 借用。

use super::positioning::{Point, Rect, Screen};
#[cfg(target_os = "macos")]
use super::positioning::{appkit_to_top_left, top_left_to_appkit};
use gpui::Window;

#[cfg(target_os = "macos")]
use objc::{sel, sel_impl};
#[cfg(target_os = "windows")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, POINT, RECT},
    Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO,
        MonitorFromPoint, MonitorFromWindow,
    },
    UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
    UI::WindowsAndMessaging::{
        GetCursorPos, GetWindowRect, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
    },
};
#[cfg(target_os = "macos")]
mod ffi;

/// 当前原生窗口的几何与所在屏幕。
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// 当前 NSWindow 的全局逻辑矩形。
    pub rect: Rect,
    /// 当前窗口中心所在显示器。
    pub screen: Screen,
}

/// 已从 GPUI 借用中分离的 NSWindow 强引用。
#[cfg(target_os = "macos")]
pub struct PreparedPosition {
    native: objc::rc::StrongPtr,
}

/// Windows HWND borrowed from GPUI. It remains valid while the corresponding GPUI window exists.
#[cfg(target_os = "windows")]
pub struct PreparedPosition {
    hwnd: HWND,
}

/// 非 macOS 平台的定位占位类型。
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub struct PreparedPosition;

/// 取得主线程上的 NSWindow 强引用；调用者必须在退出 GPUI `App` borrow 后操作它。
pub fn prepare(window: &Window) -> Result<PreparedPosition, String> {
    #[cfg(target_os = "macos")]
    {
        ensure_main_thread()?;
        let native = ns_window(window)?;
        return Ok(PreparedPosition {
            native: unsafe { objc::rc::StrongPtr::retain(native) },
        });
    }
    #[cfg(target_os = "windows")]
    {
        let handle = <Window as HasWindowHandle>::window_handle(window)
            .map_err(|error| error.to_string())?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return Err("GPUI 窗口句柄不是 Win32 类型".into());
        };
        return Ok(PreparedPosition {
            hwnd: handle.hwnd.get() as HWND,
        });
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = window;
        Err("当前平台不支持 AppKit 窗口定位".into())
    }
}

/// 返回当前已连接屏幕。所有坐标均为 AppKit 全局逻辑点，包含负坐标。
pub fn screens() -> Result<Vec<Screen>, String> {
    #[cfg(target_os = "macos")]
    {
        ensure_main_thread()?;
        return unsafe { read_screens() };
    }
    #[cfg(target_os = "windows")]
    {
        let mut monitors = Vec::new();
        unsafe {
            if EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                Some(collect_monitor),
                &mut monitors as *mut Vec<Screen> as LPARAM,
            ) == 0
            {
                return Err(format!(
                    "枚举 Windows 显示器失败：{}",
                    std::io::Error::last_os_error()
                ));
            }
        }
        if monitors.is_empty() {
            return Err("没有可用的 Windows 显示器".into());
        }
        return Ok(monitors);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err("当前平台不支持 AppKit 屏幕定位".into())
    }
}

/// 按 v1 来源顺序选择屏幕；`current_key` 是主窗口当前所在屏幕的 UUID。
pub fn target_screen(focused: bool, current_key: Option<&str>) -> Result<Screen, String> {
    let screens = screens()?;
    let focused_key = focused_screen_key()?;
    let cursor_key = cursor_screen_key()?;
    super::positioning::choose_display(
        &screens,
        if focused {
            super::positioning::PositionSource::Focused
        } else {
            super::positioning::PositionSource::Cursor
        },
        focused_key.as_deref(),
        cursor_key.as_deref(),
        current_key,
    )
    .cloned()
    .ok_or_else(|| "没有可用的显示器".into())
}

impl PreparedPosition {
    /// 读取窗口 frame，并将其归一到统一的全局左上坐标。
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        #[cfg(target_os = "macos")]
        {
            ensure_main_thread()?;
            let top = unsafe { primary_top() }?;
            let rect = unsafe { frame_of(*self.native, top) }?;
            let all = unsafe { read_screens()? };
            let center = Point {
                x: rect.origin.x + rect.size.width / 2.0,
                y: rect.origin.y + rect.size.height / 2.0,
            };
            let screen = all
                .iter()
                .find(|screen| contains(screen.frame, center))
                .cloned()
                .or_else(|| all.iter().find(|screen| screen.primary).cloned())
                .or_else(|| all.first().cloned())
                .ok_or_else(|| "窗口不属于任何已连接显示器".to_string())?;
            return Ok(Snapshot { rect, screen });
        }
        #[cfg(target_os = "windows")]
        unsafe {
            let mut rect = RECT::default();
            if GetWindowRect(self.hwnd, &mut rect) == 0 {
                return Err(format!(
                    "读取 Windows 窗口位置失败：{}",
                    std::io::Error::last_os_error()
                ));
            }
            return Ok(Snapshot {
                rect: Rect::new(
                    rect.left as f64,
                    rect.top as f64,
                    (rect.right - rect.left) as f64,
                    (rect.bottom - rect.top) as f64,
                ),
                screen: screen_from_monitor(MonitorFromWindow(
                    self.hwnd,
                    MONITOR_DEFAULTTONEAREST,
                ))?,
            });
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Err("当前平台不支持 AppKit 窗口定位".into())
        }
    }

    /// 只设置原点，保留当前原生尺寸。
    pub fn move_to(&self, origin: Point) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            ensure_main_thread()?;
            let top = unsafe { primary_top() }?;
            let mut rect = unsafe { frame_of(*self.native, top) }?;
            rect.origin = origin;
            return self.set_rect(rect);
        }
        #[cfg(target_os = "windows")]
        unsafe {
            if SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                origin.x.round() as i32,
                origin.y.round() as i32,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            ) == 0
            {
                return Err(format!(
                    "移动 Windows 窗口失败：{}",
                    std::io::Error::last_os_error()
                ));
            }
            return Ok(());
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = origin;
            Err("当前平台不支持 AppKit 窗口定位".into())
        }
    }

    /// 一次原生提交位置与尺寸，避免 `Window::resize` 的异步回调破坏底边锚定。
    pub fn set_rect(&self, rect: Rect) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            ensure_main_thread()?;
            let native = *self.native;
            let top = unsafe { primary_top() }?;
            let frame = NSRect::from_top_left(rect, top);
            unsafe {
                let _: () = objc::msg_send![native, setFrame: frame display: objc::runtime::YES animate: objc::runtime::NO];
            }
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        unsafe {
            let x = rect.origin.x.round() as i32;
            let y = rect.origin.y.round() as i32;
            let width = rect.size.width.round().max(1.0) as i32;
            let height = rect.size.height.round().max(1.0) as i32;
            if SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                x,
                y,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            ) == 0
            {
                return Err(format!(
                    "设置 Windows 窗口位置失败：{}",
                    std::io::Error::last_os_error()
                ));
            }
            return Ok(());
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = rect;
            Err("当前平台不支持 AppKit 窗口定位".into())
        }
    }
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct NSPoint {
    x: f64,
    y: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct NSSize {
    width: f64,
    height: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct NSRect {
    origin: NSPoint,
    size: NSSize,
}

#[cfg(target_os = "macos")]
impl From<NSRect> for Rect {
    fn from(rect: NSRect) -> Self {
        Self::new(
            rect.origin.x,
            rect.origin.y,
            rect.size.width,
            rect.size.height,
        )
    }
}

#[cfg(target_os = "windows")]
fn cursor_screen() -> Result<Screen, String> {
    unsafe {
        let mut point = POINT::default();
        if GetCursorPos(&mut point) == 0 {
            return Err(format!(
                "读取鼠标位置失败：{}",
                std::io::Error::last_os_error()
            ));
        }
        screen_from_monitor(MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST))
    }
}

#[cfg(target_os = "windows")]
fn screen_from_monitor(monitor: HMONITOR) -> Result<Screen, String> {
    if monitor.is_null() {
        return Err("找不到可用的 Windows 显示器".into());
    }
    unsafe {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return Err(format!(
                "读取 Windows 显示器信息失败：{}",
                std::io::Error::last_os_error()
            ));
        }
        let frame = Rect::new(
            info.rcMonitor.left as f64,
            info.rcMonitor.top as f64,
            (info.rcMonitor.right - info.rcMonitor.left) as f64,
            (info.rcMonitor.bottom - info.rcMonitor.top) as f64,
        );
        let work_area = Rect::new(
            info.rcWork.left as f64,
            info.rcWork.top as f64,
            (info.rcWork.right - info.rcWork.left) as f64,
            (info.rcWork.bottom - info.rcWork.top) as f64,
        );
        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        let dpi_ok = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) >= 0;
        let scale = if dpi_ok && dpi_x > 0 {
            dpi_x as f64 / 96.0
        } else {
            1.0
        };
        Ok(Screen {
            key: format!(
                "monitor:{}:{}:{}:{}",
                info.rcMonitor.left,
                info.rcMonitor.top,
                info.rcMonitor.right,
                info.rcMonitor.bottom
            ),
            frame,
            work_area,
            scale,
            primary: info.rcMonitor.left == 0 && info.rcMonitor.top == 0,
        })
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> i32 {
    let screens = unsafe { &mut *(data as *mut Vec<Screen>) };
    if let Ok(screen) = screen_from_monitor(monitor) {
        screens.push(screen);
    }
    1
}

#[cfg(target_os = "macos")]
impl NSRect {
    fn from_top_left(rect: Rect, primary_top: f64) -> Self {
        let rect = top_left_to_appkit(rect, primary_top);
        Self {
            origin: NSPoint {
                x: rect.origin.x,
                y: rect.origin.y,
            },
            size: NSSize {
                width: rect.size.width,
                height: rect.size.height,
            },
        }
    }
}

#[cfg(target_os = "macos")]
fn contains(rect: Rect, point: Point) -> bool {
    point.x >= rect.origin.x
        && point.x < rect.origin.x + rect.size.width
        && point.y >= rect.origin.y
        && point.y < rect.origin.y + rect.size.height
}

#[cfg(target_os = "macos")]
fn ensure_main_thread() -> Result<(), String> {
    let is_main: objc::runtime::BOOL =
        unsafe { objc::msg_send![objc::class!(NSThread), isMainThread] };
    if is_main == objc::runtime::YES {
        Ok(())
    } else {
        Err("macOS 原生窗口定位必须在主线程执行".into())
    }
}

#[cfg(target_os = "macos")]
fn ns_window(window: &Window) -> Result<*mut objc::runtime::Object, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle =
        <Window as HasWindowHandle>::window_handle(window).map_err(|error| error.to_string())?;
    let view = match handle.as_raw() {
        RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr() as *mut objc::runtime::Object,
        _ => return Err("GPUI 窗口句柄不是 AppKit 类型".into()),
    };
    if view.is_null() {
        return Err("GPUI 返回空 NSView".into());
    }
    let native: *mut objc::runtime::Object = unsafe { objc::msg_send![view, window] };
    if native.is_null() {
        Err("GPUI 返回空 NSWindow".into())
    } else {
        Ok(native)
    }
}

#[cfg(target_os = "macos")]
unsafe fn frame_of(native: *mut objc::runtime::Object, primary_top: f64) -> Result<Rect, String> {
    if native.is_null() {
        return Err("NSWindow 句柄为空".into());
    }
    let frame: NSRect = unsafe { objc::msg_send![native, frame] };
    Ok(appkit_to_top_left(frame.into(), primary_top))
}

#[cfg(target_os = "macos")]
unsafe fn read_screens() -> Result<Vec<Screen>, String> {
    let screens: *mut objc::runtime::Object =
        unsafe { objc::msg_send![objc::class!(NSScreen), screens] };
    if screens.is_null() {
        return Err("NSScreen.screens 返回空对象".into());
    }
    let count: usize = unsafe { objc::msg_send![screens, count] };
    let mut raw_frames = Vec::with_capacity(count);
    for index in 0..count {
        let screen: *mut objc::runtime::Object =
            unsafe { objc::msg_send![screens, objectAtIndex: index] };
        let frame: NSRect = unsafe { objc::msg_send![screen, frame] };
        let visible: NSRect = unsafe { objc::msg_send![screen, visibleFrame] };
        raw_frames.push((screen, frame, visible));
    }
    let primary_top = raw_frames
        .first()
        .map(|(_, frame, _)| frame.origin.y + frame.size.height)
        .ok_or_else(|| "没有可用的显示器".to_string())?;
    let mut result = Vec::with_capacity(count);
    for (index, (screen, frame, visible)) in raw_frames.into_iter().enumerate() {
        let frame = appkit_to_top_left(frame.into(), primary_top);
        let visible = appkit_to_top_left(visible.into(), primary_top);
        let display_id = unsafe { ffi::screen_number(screen)? };
        let scale: f64 = unsafe { objc::msg_send![screen, backingScaleFactor] };
        result.push(Screen {
            key: ffi::display_uuid(display_id),
            frame,
            work_area: visible,
            scale: if scale.is_finite() && scale > 0.0 {
                scale
            } else {
                1.0
            },
            primary: index == 0,
        });
    }
    Ok(result)
}

#[cfg(target_os = "macos")]
unsafe fn primary_top() -> Result<f64, String> {
    let screens: *mut objc::runtime::Object =
        unsafe { objc::msg_send![objc::class!(NSScreen), screens] };
    if screens.is_null() {
        return Err("NSScreen.screens 返回空对象".into());
    }
    let count: usize = unsafe { objc::msg_send![screens, count] };
    if count == 0 {
        return Err("没有可用的显示器".into());
    }
    let primary: *mut objc::runtime::Object =
        unsafe { objc::msg_send![screens, objectAtIndex: 0usize] };
    let frame: NSRect = unsafe { objc::msg_send![primary, frame] };
    Ok(frame.origin.y + frame.size.height)
}

#[cfg(target_os = "macos")]
fn focused_screen_key() -> Result<Option<String>, String> {
    let screen: *mut objc::runtime::Object =
        unsafe { objc::msg_send![objc::class!(NSScreen), mainScreen] };
    if screen.is_null() {
        return Ok(None);
    }
    Ok(Some(ffi::display_uuid(unsafe {
        ffi::screen_number(screen)?
    })))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn focused_screen_key() -> Result<Option<String>, String> {
    Ok(None)
}

#[cfg(target_os = "windows")]
fn focused_screen_key() -> Result<Option<String>, String> {
    unsafe {
        let window = windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if window.is_null() {
            return Ok(None);
        }
        Ok(Some(
            screen_from_monitor(MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST))?.key,
        ))
    }
}

#[cfg(target_os = "macos")]
fn cursor_screen_key() -> Result<Option<String>, String> {
    let point: NSPoint = unsafe { objc::msg_send![objc::class!(NSEvent), mouseLocation] };
    let screens = unsafe { read_screens()? };
    let top = unsafe { primary_top()? };
    let point = Point {
        x: point.x,
        y: top - point.y,
    };
    let key = screens
        .iter()
        .find(|screen| contains(screen.frame, point))
        .map(|screen| screen.key.clone());
    Ok(key)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn cursor_screen_key() -> Result<Option<String>, String> {
    Ok(None)
}

#[cfg(target_os = "windows")]
fn cursor_screen_key() -> Result<Option<String>, String> {
    Ok(Some(cursor_screen()?.key))
}
