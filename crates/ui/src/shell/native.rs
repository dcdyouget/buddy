//! 主窗口的原生外观补丁与窗口拖动桥。
//!
//! macOS 只在主线程同步操作：先取得短暂强引用，释放 GPUI App 借用后补丁，再显示。
//! Windows 保留 `WS_THICKFRAME` 的缩放命中区，只关闭 DWM 外框；模型菜单不应用补丁。

use gpui::Window;

#[cfg(target_os = "macos")]
use objc::{sel, sel_impl};

/// 原生窗口属性读取或补丁失败时的中文诊断。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeWindowError {
    /// 当前平台没有 AppKit 原生窗口实现。
    UnsupportedPlatform,
    /// AppKit 对象只能从主线程访问。
    NotMainThread,
    /// GPUI 没有返回可用窗口句柄。
    WindowHandle(String),
    /// 句柄不是 AppKit 的 NSView 句柄。
    UnsupportedWindowHandle,
    /// Objective-C 返回了空指针。
    NullObject(&'static str),
    /// 外观补丁应在首次显示前完成，避免原生标题栏闪烁。
    AlreadyVisible,
    /// Windows DWM 无法关闭主窗口的非客户区外框。
    #[cfg(target_os = "windows")]
    WindowsFrame {
        /// 设置无边框色失败时的 HRESULT。
        border_color_hresult: i32,
        /// 关闭非客户区绘制失败时的 HRESULT。
        nc_rendering_hresult: i32,
    },
    /// Windows 无法把标题栏拖动消息投递到主窗口。
    #[cfg(target_os = "windows")]
    WindowsMessage(String),
}

impl std::fmt::Display for NativeWindowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform => f.write_str("当前平台不支持 macOS 原生窗口外观"),
            Self::NotMainThread => f.write_str("macOS 原生窗口只能在主线程读取或修改"),
            Self::WindowHandle(error) => write!(f, "读取 GPUI 原生窗口句柄失败：{error}"),
            Self::UnsupportedWindowHandle => f.write_str("GPUI 窗口句柄类型与当前平台不匹配"),
            Self::NullObject(name) => write!(f, "AppKit 返回空对象：{name}"),
            Self::AlreadyVisible => f.write_str("主窗口必须在应用原生外观后显示"),
            #[cfg(target_os = "windows")]
            Self::WindowsFrame {
                border_color_hresult,
                nc_rendering_hresult,
            } => write!(
                f,
                "关闭 Windows 原生外框失败：border=0x{border_color_hresult:08X}, non-client=0x{nc_rendering_hresult:08X}"
            ),
            #[cfg(target_os = "windows")]
            Self::WindowsMessage(error) => write!(f, "投递 Windows 原生窗口消息失败：{error}"),
        }
    }
}

impl std::error::Error for NativeWindowError {}

/// 应用补丁后从 AppKit 实际读回的窗口属性。
#[derive(Clone, Debug, PartialEq)]
pub struct NativeWindowSnapshot {
    /// NSWindow style mask 的原始位集合。
    pub style_mask: u64,
    /// NSWindow 层级；主窗口应为 0（NSNormalWindowLevel）。
    pub level: i64,
    /// 系统窗口阴影是否开启。
    pub has_shadow: bool,
    /// GPUI Transparent 背景的真实 opaque 状态。
    pub is_opaque: bool,
    /// 当前 collection behavior 只读取，不修改工作区策略。
    pub collection_behavior: u64,
    /// contentView 是否启用 layer-backed 绘制。
    pub wants_layer: bool,
    /// contentView layer 的圆角半径（Core Animation 点，非 Retina 像素）。
    pub corner_radius: f64,
    /// contentView layer 是否裁剪到圆角边界。
    pub masks_to_bounds: bool,
    /// GPUI contentView `acceptsFirstMouse:` 的实际返回值。
    pub accepts_first_mouse: bool,
    /// NSWindow 当前是否已显示。
    pub is_visible: bool,
    /// NSWindow 当前是否为 key window；显示主窗口不应强行置 key。
    pub is_key: bool,
    /// GPUI contentView 是否为第一响应者。否则 NSTextInputContext 不激活：
    /// 键盘事件仍到达 GPUI（快捷键可用），但任何文字（含输入法）都无法输入。
    pub view_is_first_responder: bool,
}

impl NativeWindowSnapshot {
    /// 是否已移除标题栏和三个系统窗口按钮。
    pub fn is_decoration_free(&self) -> bool {
        self.style_mask & (NS_TITLED | NS_CLOSABLE | NS_MINIATURIZABLE) == 0
    }
}

/// Windows 主窗口从 DWM 与 User32 读回的真实属性。
#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowsNativeWindowSnapshot {
    /// `GWL_STYLE`；保留 `WS_THICKFRAME` 才不会为去框牺牲缩放能力。
    pub style: isize,
    /// `GWL_EXSTYLE`，供真实窗口自检诊断。
    pub ex_style: isize,
    /// Win11 的 DWM 边框色；旧系统不支持该属性时为 `None`。
    pub border_color: Option<u32>,
    /// DWM 非客户区绘制策略；作为旧系统隐藏外框的后备路径。
    pub nc_rendering_policy: Option<i32>,
    /// 本次补丁是否成功提交了无边框色；部分合成环境不支持随后读回。
    pub border_color_applied: bool,
    /// 本次补丁是否成功关闭了非客户区绘制；部分合成环境不支持随后读回。
    pub nc_rendering_disabled: bool,
}

#[cfg(target_os = "windows")]
impl WindowsNativeWindowSnapshot {
    /// DWM 已停止绘制产品圆角内容之外的系统外框。
    pub fn is_frame_hidden(&self) -> bool {
        use windows_sys::Win32::Graphics::Dwm::{DWMNCRP_DISABLED, DWMWA_COLOR_NONE};

        self.border_color == Some(DWMWA_COLOR_NONE)
            || self.nc_rendering_policy == Some(DWMNCRP_DISABLED)
            || self.border_color_applied
            || self.nc_rendering_disabled
    }

    /// 去框后仍保留系统缩放命中区。
    pub fn is_resizable(&self) -> bool {
        use windows_sys::Win32::UI::WindowsAndMessaging::WS_THICKFRAME;

        self.style & WS_THICKFRAME as isize != 0
    }
}

// NSWindowStyleMask / NSWindowCollectionBehavior 的 AppKit 常量。
const NS_TITLED: u64 = 1 << 0;
const NS_CLOSABLE: u64 = 1 << 1;
const NS_MINIATURIZABLE: u64 = 1 << 2;
#[cfg(target_os = "macos")]
const NS_RESIZABLE: u64 = 1 << 3;
#[cfg(target_os = "macos")]
const NS_NONACTIVATING_PANEL: u64 = 1 << 7;
#[cfg(target_os = "macos")]
const NS_NORMAL_WINDOW_LEVEL: i64 = 0;

/// 仅在主线程短暂持有原生对象；在 GPUI 的 App 借用释放后连续执行补丁和显示。
/// 不跨 await、不进入实体 / 后台任务；强引用保证同步 AppKit 回调期间对象存活。
#[cfg(target_os = "macos")]
pub(crate) struct PreparedMainWindow {
    native: objc::rc::StrongPtr,
    view: objc::rc::StrongPtr,
    resizable: bool,
}

/// 收集原生对象，不修改外观、不触发窗口 resize 回调。
#[cfg(target_os = "macos")]
pub(crate) fn prepare_main_window(
    window: &Window,
) -> Result<PreparedMainWindow, NativeWindowError> {
    ensure_main_thread()?;
    unsafe {
        Ok(PreparedMainWindow {
            native: objc::rc::StrongPtr::retain(ns_window(window)?),
            view: objc::rc::StrongPtr::retain(ns_view(window)?),
            resizable: window.is_resizable(),
        })
    }
}

#[cfg(target_os = "macos")]
const NS_WINDOW_ANIMATION_NONE: i64 = 2;

#[cfg(target_os = "macos")]
impl PreparedMainWindow {
    pub(crate) fn apply_and_show(self) -> Result<NativeWindowSnapshot, NativeWindowError> {
        ensure_main_thread()?;
        unsafe {
            let native = *self.native;
            let visible: objc::runtime::BOOL = objc::msg_send![native, isVisible];
            if visible == objc::runtime::YES {
                return Err(NativeWindowError::AlreadyVisible);
            }
            let before: u64 = objc::msg_send![native, styleMask];
            // GPUI's titlebar=None branch omits Resizable even when configured.
            let mask = (before & !(NS_TITLED | NS_CLOSABLE | NS_MINIATURIZABLE | NS_RESIZABLE))
                | NS_NONACTIVATING_PANEL
                | if self.resizable { NS_RESIZABLE } else { 0 };
            let _: () = objc::msg_send![native, setStyleMask: mask];
            let _: () = objc::msg_send![native, setHasShadow: objc::runtime::NO];
            // GPUI PopUp 默认 UtilityWindow 动画与 AppShell 入场叠加，实测第三帧
            // 阻塞约 477ms。首次显示前关闭 AppKit 自动显隐动画，统一由 GPUI 绘制。
            let _: () = objc::msg_send![native, setAnimationBehavior: NS_WINDOW_ANIMATION_NONE];
            let _: () = objc::msg_send![native, setLevel: NS_NORMAL_WINDOW_LEVEL];
            apply_content_layer(native)?;
            // 修改 styleMask 会重建窗口边框视图并把第一响应者重置为窗口本身；
            // 不恢复则 NSTextInputContext 不激活，键盘事件仍到达 GPUI，但任何文字（含输入法）都无法输入。
            let _: objc::runtime::BOOL = objc::msg_send![native, makeFirstResponder: *self.view];
            let _: () = objc::msg_send![native, orderFrontRegardless];
            probe_native(native, *self.view)
        }
    }
}

/// 在主线程读取当前主窗口的原生属性，不修改任何窗口设置。
pub fn probe_main_window(window: &Window) -> Result<NativeWindowSnapshot, NativeWindowError> {
    #[cfg(target_os = "macos")]
    {
        ensure_main_thread()?;
        let native = ns_window(window)?;
        unsafe { probe_native(native, ns_view(window)?) }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        Err(NativeWindowError::UnsupportedPlatform)
    }
}

/// 隐藏 Windows 11 的 DWM 边框，并关闭会在透明内容外留下阴影/框线的非客户区绘制。
/// 这不会移除 GPUI 创建的 `WS_THICKFRAME`，因此窗口仍可缩放。
#[cfg(target_os = "windows")]
pub(crate) fn apply_windows_main_window_appearance(
    window: &Window,
) -> Result<WindowsNativeWindowSnapshot, NativeWindowError> {
    use windows_sys::Win32::Graphics::Dwm::{
        DWMNCRP_DISABLED, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_NCRENDERING_POLICY,
        DwmSetWindowAttribute,
    };

    let hwnd = hwnd(window)?;
    let border_color = DWMWA_COLOR_NONE;
    let border_result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            std::ptr::from_ref(&border_color).cast(),
            std::mem::size_of_val(&border_color) as u32,
        )
    };
    let policy = DWMNCRP_DISABLED;
    let nc_rendering_result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY as u32,
            std::ptr::from_ref(&policy).cast(),
            std::mem::size_of_val(&policy) as u32,
        )
    };
    if border_result < 0 && nc_rendering_result < 0 {
        return Err(NativeWindowError::WindowsFrame {
            border_color_hresult: border_result,
            nc_rendering_hresult: nc_rendering_result,
        });
    }
    let mut snapshot = probe_windows_main_window(window)?;
    snapshot.border_color_applied = border_result >= 0;
    snapshot.nc_rendering_disabled = nc_rendering_result >= 0;
    Ok(snapshot)
}

/// 读取 Windows 主窗口真实样式与 DWM 外框状态。
#[cfg(target_os = "windows")]
pub(crate) fn probe_windows_main_window(
    window: &Window,
) -> Result<WindowsNativeWindowSnapshot, NativeWindowError> {
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_NCRENDERING_POLICY, DwmGetWindowAttribute,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GWL_EXSTYLE, GWL_STYLE, GetWindowLongPtrW};

    let hwnd = hwnd(window)?;
    let mut border_color = 0u32;
    let border_result = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            std::ptr::from_mut(&mut border_color).cast(),
            std::mem::size_of_val(&border_color) as u32,
        )
    };
    let mut policy = 0i32;
    let policy_result = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY as u32,
            std::ptr::from_mut(&mut policy).cast(),
            std::mem::size_of_val(&policy) as u32,
        )
    };
    Ok(WindowsNativeWindowSnapshot {
        style: unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) },
        ex_style: unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) },
        border_color: (border_result >= 0).then_some(border_color),
        nc_rendering_policy: (policy_result >= 0).then_some(policy),
        border_color_applied: false,
        nc_rendering_disabled: false,
    })
}

/// 当前 GPUI Windows 后端没有实现 `Window::start_window_move`，因此把已命中的左键
/// 按下转换为原生标题栏按下。先释放 GPUI 的 client-area capture，再异步投递消息：
/// 必须让当前 GPUI 输入回调释放 `App` / `Window` 借用，系统移动循环才能安全派发
/// `WM_MOVE`、重绘与位置观察回调。
#[cfg(target_os = "windows")]
pub(crate) fn start_window_move(window: &Window) -> Result<(), NativeWindowError> {
    use windows_sys::Win32::UI::{
        Input::KeyboardAndMouse::ReleaseCapture,
        WindowsAndMessaging::{HTCAPTION, PostMessageW, WM_NCLBUTTONDOWN},
    };

    let hwnd = hwnd(window)?;
    unsafe {
        ReleaseCapture();
        if PostMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0) == 0 {
            return Err(NativeWindowError::WindowsMessage(
                std::io::Error::last_os_error().to_string(),
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn hwnd(window: &Window) -> Result<windows_sys::Win32::Foundation::HWND, NativeWindowError> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = <Window as HasWindowHandle>::window_handle(window)
        .map_err(|error| NativeWindowError::WindowHandle(error.to_string()))?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err(NativeWindowError::UnsupportedWindowHandle);
    };
    Ok(handle.hwnd.get() as windows_sys::Win32::Foundation::HWND)
}

#[cfg(target_os = "macos")]
fn ensure_main_thread() -> Result<(), NativeWindowError> {
    use objc::runtime::{BOOL, YES};
    use objc::{class, msg_send};

    let is_main: BOOL = unsafe { msg_send![class!(NSThread), isMainThread] };
    if is_main == YES {
        Ok(())
    } else {
        Err(NativeWindowError::NotMainThread)
    }
}

#[cfg(target_os = "macos")]
fn ns_window(window: &Window) -> Result<*mut objc::runtime::Object, NativeWindowError> {
    let ns_view = ns_view(window)?;
    unsafe {
        let native: *mut objc::runtime::Object = objc::msg_send![ns_view, window];
        if native.is_null() {
            Err(NativeWindowError::NullObject("NSWindow"))
        } else {
            Ok(native)
        }
    }
}

#[cfg(target_os = "macos")]
fn ns_view(window: &Window) -> Result<*mut objc::runtime::Object, NativeWindowError> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = <Window as HasWindowHandle>::window_handle(window)
        .map_err(|error| NativeWindowError::WindowHandle(error.to_string()))?;
    let ns_view = match handle.as_raw() {
        RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr() as *mut objc::runtime::Object,
        _ => return Err(NativeWindowError::UnsupportedWindowHandle),
    };
    if ns_view.is_null() {
        return Err(NativeWindowError::NullObject("NSView"));
    }
    Ok(ns_view)
}

#[cfg(target_os = "macos")]
unsafe fn content_layer(
    native: *mut objc::runtime::Object,
) -> Result<(*mut objc::runtime::Object, *mut objc::runtime::Object), NativeWindowError> {
    unsafe {
        let content = content_view(native)?;
        let layer: *mut objc::runtime::Object = objc::msg_send![content, layer];
        if layer.is_null() {
            return Err(NativeWindowError::NullObject("contentView.layer"));
        }
        Ok((content, layer))
    }
}

#[cfg(target_os = "macos")]
unsafe fn content_view(
    native: *mut objc::runtime::Object,
) -> Result<*mut objc::runtime::Object, NativeWindowError> {
    unsafe {
        let content: *mut objc::runtime::Object = objc::msg_send![native, contentView];
        if content.is_null() {
            Err(NativeWindowError::NullObject("contentView"))
        } else {
            Ok(content)
        }
    }
}

#[cfg(target_os = "macos")]
unsafe fn apply_content_layer(native: *mut objc::runtime::Object) -> Result<(), NativeWindowError> {
    use crate::theme_system::tokens::metrics as m;
    use objc::runtime::YES;

    unsafe {
        let content = content_view(native)?;
        let _: () = objc::msg_send![content, setWantsLayer: YES];
        let layer: *mut objc::runtime::Object = objc::msg_send![content, layer];
        if layer.is_null() {
            return Err(NativeWindowError::NullObject("contentView.layer"));
        }
        let _: () = objc::msg_send![layer, setCornerRadius: m::RADIUS_XL as f64];
        let _: () = objc::msg_send![layer, setMasksToBounds: YES];
        Ok(())
    }
}

#[cfg(target_os = "macos")]
unsafe fn probe_native(
    native: *mut objc::runtime::Object,
    view: *mut objc::runtime::Object,
) -> Result<NativeWindowSnapshot, NativeWindowError> {
    use objc::runtime::{BOOL, YES};

    unsafe {
        let (content, layer) = content_layer(native)?;
        let style_mask: u64 = objc::msg_send![native, styleMask];
        let level: i64 = objc::msg_send![native, level];
        let has_shadow: BOOL = objc::msg_send![native, hasShadow];
        let is_opaque: BOOL = objc::msg_send![native, isOpaque];
        let collection_behavior: u64 = objc::msg_send![native, collectionBehavior];
        let wants_layer: BOOL = objc::msg_send![content, wantsLayer];
        let corner_radius: f64 = objc::msg_send![layer, cornerRadius];
        let masks_to_bounds: BOOL = objc::msg_send![layer, masksToBounds];
        // GPUI registers acceptsFirstMouse: on GPUIView, not GPUIPanel.
        // AppKit asks the hit-tested view whether the first click is accepted.
        let accepts_first_mouse: BOOL = objc::msg_send![
            view,
            acceptsFirstMouse: std::ptr::null_mut::<objc::runtime::Object>()
        ];
        let is_visible: BOOL = objc::msg_send![native, isVisible];
        let is_key: BOOL = objc::msg_send![native, isKeyWindow];
        let first_responder: *mut objc::runtime::Object = objc::msg_send![native, firstResponder];

        Ok(NativeWindowSnapshot {
            style_mask,
            level,
            has_shadow: has_shadow == YES,
            is_opaque: is_opaque == YES,
            collection_behavior,
            wants_layer: wants_layer == YES,
            corner_radius,
            masks_to_bounds: masks_to_bounds == YES,
            accepts_first_mouse: accepts_first_mouse == YES,
            is_visible: is_visible == YES,
            is_key: is_key == YES,
            view_is_first_responder: first_responder == view,
        })
    }
}

#[cfg(all(test, target_os = "windows"))]
mod windows_tests {
    use super::WindowsNativeWindowSnapshot;
    use windows_sys::Win32::{
        Graphics::Dwm::{DWMNCRP_DISABLED, DWMWA_COLOR_NONE},
        UI::WindowsAndMessaging::WS_THICKFRAME,
    };

    fn snapshot() -> WindowsNativeWindowSnapshot {
        WindowsNativeWindowSnapshot {
            style: WS_THICKFRAME as isize,
            ex_style: 0,
            border_color: None,
            nc_rendering_policy: None,
            border_color_applied: false,
            nc_rendering_disabled: false,
        }
    }

    #[test]
    fn windows_frame_check_accepts_readback_or_successful_patch() {
        let mut state = snapshot();
        assert!(!state.is_frame_hidden());

        state.border_color = Some(DWMWA_COLOR_NONE);
        assert!(state.is_frame_hidden());
        state.border_color = None;
        state.nc_rendering_policy = Some(DWMNCRP_DISABLED);
        assert!(state.is_frame_hidden());
        state.nc_rendering_policy = None;
        state.nc_rendering_disabled = true;
        assert!(state.is_frame_hidden());
    }

    #[test]
    fn windows_frame_patch_keeps_resize_style() {
        let mut state = snapshot();
        assert!(state.is_resizable());
        state.style &= !(WS_THICKFRAME as isize);
        assert!(!state.is_resizable());
    }
}
