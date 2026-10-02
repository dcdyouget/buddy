//! S07-05 测试侧的原生层级与工作区读回。
//!
//! 这里不通过产品属性推断结果：AppKit 的 `isOnActiveSpace` 直接从当前 NSWindow
//! 读取，前台覆盖则由 CoreGraphics 的 on-screen window stack 按窗口号和相交区域判断。

use buddy_ui::gpui::{AppContext, AsyncApp, Bounds, Pixels, WindowHandle};
use buddy_ui::shell::{
    native::{NativeWindowSnapshot, probe_main_window},
    workspaces,
};

pub(crate) const NORMAL_LEVEL: i64 = 0;
pub(crate) const ALL_SPACES_FULLSCREEN_AUXILIARY: u64 = 0x101;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScreenRect {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

impl ScreenRect {
    fn intersects(self, other: Self) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ScreenWindow {
    pub(crate) number: i64,
    pub(crate) layer: i64,
    pub(crate) bounds: ScreenRect,
}

#[derive(Clone, Debug)]
pub(crate) struct NativeLevelSnapshot {
    pub(crate) native: NativeWindowSnapshot,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) fullscreen: bool,
    pub(crate) active: bool,
    pub(crate) is_on_active_space: bool,
    pub(crate) window_number: Option<i64>,
    pub(crate) window_in_stack: bool,
    pub(crate) overlapping_windows: Vec<i64>,
    pub(crate) front_window_numbers: Vec<i64>,
    pub(crate) front_window_layers: Vec<i64>,
    pub(crate) front_window_intersects: bool,
}

#[cfg(target_os = "macos")]
mod cg_stack {
    use super::{ScreenRect, ScreenWindow};
    use std::ffi::{CString, c_char, c_void};

    type Ref = *const c_void;
    const UTF8: u32 = 0x0800_0100;
    const NUMBER_SINT64: i32 = 4;
    const NUMBER_DOUBLE: i32 = 6;
    const ON_SCREEN_ONLY: u32 = 1;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn CGWindowListCopyWindowInfo(options: u32, relative_to_window: u32) -> Ref;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFArrayGetCount(array: Ref) -> isize;
        fn CFArrayGetValueAtIndex(array: Ref, index: isize) -> Ref;
        fn CFDictionaryGetValue(dictionary: Ref, key: Ref) -> Ref;
        fn CFStringCreateWithCString(allocator: Ref, string: *const c_char, encoding: u32) -> Ref;
        fn CFNumberGetValue(number: Ref, number_type: i32, value: *mut c_void) -> bool;
        fn CFRelease(value: Ref);
    }

    fn key(name: &str) -> Ref {
        let Ok(name) = CString::new(name) else {
            return std::ptr::null();
        };
        unsafe { CFStringCreateWithCString(std::ptr::null(), name.as_ptr(), UTF8) }
    }

    fn value(dictionary: Ref, name: &str) -> Ref {
        let key = key(name);
        if key.is_null() || dictionary.is_null() {
            return std::ptr::null();
        }
        let value = unsafe { CFDictionaryGetValue(dictionary, key) };
        unsafe { CFRelease(key) };
        value
    }

    fn number_i64(value: Ref) -> Option<i64> {
        if value.is_null() {
            return None;
        }
        let mut output = 0_i64;
        (unsafe { CFNumberGetValue(value, NUMBER_SINT64, (&mut output as *mut i64).cast()) })
            .then_some(output)
    }

    fn number_f64(value: Ref) -> Option<f64> {
        if value.is_null() {
            return None;
        }
        let mut output = 0.0_f64;
        (unsafe { CFNumberGetValue(value, NUMBER_DOUBLE, (&mut output as *mut f64).cast()) })
            .then_some(output)
    }

    fn bounds(dictionary: Ref) -> Option<ScreenRect> {
        let x = number_f64(value(dictionary, "X"))?;
        let y = number_f64(value(dictionary, "Y"))?;
        let width = number_f64(value(dictionary, "Width"))?;
        let height = number_f64(value(dictionary, "Height"))?;
        Some(ScreenRect {
            x,
            y,
            width,
            height,
        })
    }

    pub(super) fn on_screen_stack() -> Option<Vec<ScreenWindow>> {
        unsafe {
            let array = CGWindowListCopyWindowInfo(ON_SCREEN_ONLY, 0);
            if array.is_null() {
                return None;
            }
            let result = (|| {
                let mut output = Vec::new();
                for index in 0..CFArrayGetCount(array) {
                    let dictionary = CFArrayGetValueAtIndex(array, index);
                    let number = number_i64(value(dictionary, "kCGWindowNumber"))?;
                    let layer = number_i64(value(dictionary, "kCGWindowLayer"))?;
                    let bounds = bounds(value(dictionary, "kCGWindowBounds"))?;
                    output.push(ScreenWindow {
                        number,
                        layer,
                        bounds,
                    });
                }
                Some(output)
            })();
            CFRelease(array);
            result
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod cg_stack {
    use super::ScreenWindow;

    pub(super) fn on_screen_stack() -> Option<Vec<ScreenWindow>> {
        None
    }
}

pub(crate) fn on_screen_stack() -> Option<Vec<ScreenWindow>> {
    cg_stack::on_screen_stack()
}

pub(crate) fn probe<V: 'static>(
    handle: WindowHandle<V>,
    cx: &mut AsyncApp,
) -> Option<NativeLevelSnapshot> {
    cx.update_window(handle.into(), |_, window, _| {
        let native = probe_main_window(window).ok()?;
        let bounds = window.bounds();
        let workspace = workspaces::probe(window).ok()?;
        let stack = on_screen_stack();
        let window_number = Some(workspace.window_number);
        let window_in_stack = stack.as_ref().is_some_and(|windows| {
            windows
                .iter()
                .any(|entry| entry.number == workspace.window_number)
        });
        let overlapping_windows = stack
            .as_ref()
            .and_then(|windows| {
                let own = windows
                    .iter()
                    .find(|entry| entry.number == workspace.window_number)?;
                Some(
                    windows
                        .iter()
                        .filter(|entry| {
                            entry.number != own.number && entry.bounds.intersects(own.bounds)
                        })
                        .map(|entry| entry.number)
                        .collect(),
                )
            })
            .unwrap_or_default();
        let (front_window_numbers, front_window_layers) = stack
            .as_ref()
            .and_then(|windows| {
                let index = window_number
                    .and_then(|number| windows.iter().position(|window| window.number == number))?;
                let screen_bounds = windows[index].bounds;
                let windows = windows[..index]
                    .iter()
                    .filter(|window| window.bounds.intersects(screen_bounds));
                let (numbers, layers): (Vec<_>, Vec<_>) =
                    windows.map(|window| (window.number, window.layer)).unzip();
                Some((numbers, layers))
            })
            .unwrap_or_default();
        let front_window_intersects = !front_window_numbers.is_empty();
        let active = window.is_window_active();
        Some(NativeLevelSnapshot {
            native,
            bounds,
            fullscreen: workspace.is_fullscreen,
            active,
            is_on_active_space: workspace.is_on_active_space,
            window_number,
            window_in_stack,
            overlapping_windows,
            front_window_numbers,
            front_window_layers,
            front_window_intersects,
        })
    })
    .ok()
    .flatten()
}

pub(crate) fn expected_main(snapshot: &NativeLevelSnapshot) -> bool {
    snapshot.native.level == NORMAL_LEVEL
        && snapshot.native.collection_behavior == ALL_SPACES_FULLSCREEN_AUXILIARY
        && snapshot.native.is_visible
        && snapshot.is_on_active_space
        && snapshot.window_in_stack
        && snapshot.bounds.size.width > Pixels::ZERO
        && snapshot.bounds.size.height > Pixels::ZERO
}

pub(crate) fn describe(snapshot: Option<&NativeLevelSnapshot>) -> String {
    let Some(snapshot) = snapshot else {
        return "<native probe failed>".to_owned();
    };
    format!(
        "level={} collection=0x{:x} active={} active_space={} fullscreen={} window={} front_intersects={} front_windows={:?} layers={:?} bounds=({},{} {}x{}) visible={}",
        snapshot.native.level,
        snapshot.native.collection_behavior,
        snapshot.active,
        snapshot.is_on_active_space,
        snapshot.fullscreen,
        snapshot
            .window_number
            .map_or_else(|| "<none>".to_owned(), |number| number.to_string()),
        snapshot.front_window_intersects,
        snapshot.front_window_numbers,
        snapshot.front_window_layers,
        f32::from(snapshot.bounds.origin.x),
        f32::from(snapshot.bounds.origin.y),
        f32::from(snapshot.bounds.size.width),
        f32::from(snapshot.bounds.size.height),
        snapshot.native.is_visible,
    )
}
