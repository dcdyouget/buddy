//! 仅供 shell 预览自测使用的 macOS 系统输入注入。
//!
//! 组合键限定为预览专用的 CmdOrCtrl+Alt+Shift+B/N，调用方必须先检查权限。这里不发送产品默认
//! 热键，也不向任意用户窗口发送鼠标或键盘事件。

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::{c_char, c_void};

    type EventRef = *mut c_void;

    #[repr(C)]
    struct Point {
        x: f64,
        y: f64,
    }

    const HID_EVENT_TAP: u32 = 0;
    const FLAG_SHIFT: u64 = 1 << 17;
    const FLAG_COMMAND: u64 = 1 << 20;
    const FLAG_ALTERNATE: u64 = 1 << 19;
    const B: u16 = 11;
    const N: u16 = 45;
    const KEY_SHIFT: u16 = 56;
    const KEY_COMMAND: u16 = 55;
    const KEY_ALTERNATE: u16 = 58;
    const KEY_A: u16 = 0;
    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreateKeyboardEvent(source: *const c_void, key: u16, key_down: bool) -> EventRef;
        fn CGEventSetFlags(event: EventRef, flags: u64);
        fn CGEventPost(tap: u32, event: EventRef);
        fn CGEventCreateMouseEvent(
            source: *const c_void,
            mouse_type: u32,
            mouse_cursor_position: Point,
            mouse_button: u32,
        ) -> EventRef;
        fn CFRelease(value: *const c_void);
        fn CGPreflightPostEventAccess() -> bool;
        fn CGSessionCopyCurrentDictionary() -> *const c_void;
        fn CFDictionaryGetValue(dictionary: *const c_void, key: *const c_void) -> *const c_void;
        fn CFStringCreateWithCString(
            allocator: *const c_void,
            string: *const c_char,
            encoding: u32,
        ) -> *const c_void;
        fn CFBooleanGetValue(value: *const c_void) -> u8;
    }

    pub(super) fn session_locked() -> bool {
        // This is a read-only session probe. It neither prompts nor changes TCC state.
        let dictionary = unsafe { CGSessionCopyCurrentDictionary() };
        if dictionary.is_null() {
            return false;
        }
        let key = unsafe {
            CFStringCreateWithCString(
                std::ptr::null(),
                c"CGSSessionScreenIsLocked".as_ptr(),
                K_CF_STRING_ENCODING_UTF8,
            )
        };
        if key.is_null() {
            unsafe { CFRelease(dictionary) };
            return false;
        }
        let value = unsafe { CFDictionaryGetValue(dictionary, key) };
        let locked = !value.is_null() && unsafe { CFBooleanGetValue(value) != 0 };
        unsafe {
            CFRelease(key);
            CFRelease(dictionary);
        }
        locked
    }

    pub(super) fn can_post() -> bool {
        // This is a read-only preflight. It neither prompts nor changes TCC state.
        !session_locked() && unsafe { CGPreflightPostEventAccess() }
    }

    fn post(key: u16, flags: u64, key_down: bool) -> bool {
        // SAFETY: CoreGraphics owns the event until it is released after posting.
        let event = unsafe { CGEventCreateKeyboardEvent(std::ptr::null(), key, key_down) };
        if event.is_null() {
            return false;
        }
        unsafe {
            CGEventSetFlags(event, flags);
            CGEventPost(HID_EVENT_TAP, event);
            CFRelease(event.cast());
        }
        true
    }

    fn key_for_name(name: &str) -> Option<u16> {
        match name {
            "B" => Some(B),
            "N" => Some(N),
            _ => None,
        }
    }

    /// 只发送组合键的按下阶段；用于验证 Released 不会重复切换。
    pub(super) fn combo_down(name: &str) -> bool {
        let Some(key) = key_for_name(name) else {
            return false;
        };
        post(KEY_COMMAND, FLAG_COMMAND, true)
            && post(KEY_ALTERNATE, FLAG_COMMAND | FLAG_ALTERNATE, true)
            && post(KEY_SHIFT, FLAG_COMMAND | FLAG_ALTERNATE | FLAG_SHIFT, true)
            && post(key, FLAG_COMMAND | FLAG_ALTERNATE | FLAG_SHIFT, true)
    }

    /// 只发送组合键的释放阶段；顺序为主键后修饰键。
    pub(super) fn combo_up(name: &str) -> bool {
        let Some(key) = key_for_name(name) else {
            return false;
        };
        post(key, FLAG_COMMAND | FLAG_ALTERNATE | FLAG_SHIFT, false)
            && post(KEY_SHIFT, FLAG_COMMAND | FLAG_ALTERNATE, false)
            && post(KEY_ALTERNATE, FLAG_COMMAND, false)
            && post(KEY_COMMAND, 0, false)
    }

    pub(super) fn combo(name: &str) -> bool {
        combo_down(name) && combo_up(name)
    }

    /// 向当前真实 key window 发送 Cmd+A；调用方必须先完成会话和辅助功能预检。
    pub(super) fn select_all() -> bool {
        post(KEY_A, FLAG_COMMAND, true) && post(KEY_A, FLAG_COMMAND, false)
    }

    pub(super) fn copy() -> bool {
        post(8, FLAG_COMMAND, true) && post(8, FLAG_COMMAND, false)
    }

    pub(super) fn click_screen(x: f64, y: f64) -> bool {
        const LEFT_MOUSE_DOWN: u32 = 1;
        const LEFT_MOUSE_UP: u32 = 2;
        let point = Point { x, y };
        let down = unsafe { CGEventCreateMouseEvent(std::ptr::null(), LEFT_MOUSE_DOWN, point, 0) };
        if down.is_null() {
            return false;
        }
        unsafe {
            CGEventPost(HID_EVENT_TAP, down);
            CFRelease(down.cast());
        }
        let up =
            unsafe { CGEventCreateMouseEvent(std::ptr::null(), LEFT_MOUSE_UP, Point { x, y }, 0) };
        if up.is_null() {
            return false;
        }
        unsafe {
            CGEventPost(HID_EVENT_TAP, up);
            CFRelease(up.cast());
        }
        true
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn can_post() -> bool {
    macos::can_post()
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn can_post() -> bool {
    false
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Preflight {
    Ready,
    SessionLocked,
    EventAccessDenied,
}

#[cfg(target_os = "macos")]
pub(crate) fn preflight() -> Preflight {
    if macos::session_locked() {
        Preflight::SessionLocked
    } else if macos::can_post() {
        Preflight::Ready
    } else {
        Preflight::EventAccessDenied
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn preflight() -> Preflight {
    Preflight::EventAccessDenied
}

pub(crate) fn preflight_message(status: Preflight) -> &'static str {
    match status {
        Preflight::Ready => "系统输入预检通过",
        Preflight::SessionLocked => "当前 macOS 会话已锁定，请解锁后重试；未发送系统输入",
        Preflight::EventAccessDenied => "未获得 CGEventPost 权限，未发送系统输入",
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn combo_down(name: &str) -> bool {
    macos::combo_down(name)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn combo_down(_: &str) -> bool {
    false
}

#[cfg(target_os = "macos")]
pub(crate) fn combo_up(name: &str) -> bool {
    macos::combo_up(name)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn combo_up(_: &str) -> bool {
    false
}

#[cfg(target_os = "macos")]
pub(crate) fn combo(name: &str) -> bool {
    macos::combo(name)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn combo(_: &str) -> bool {
    false
}

#[cfg(target_os = "macos")]
pub(crate) fn select_all() -> bool {
    macos::select_all()
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn select_all() -> bool {
    false
}

#[cfg(target_os = "macos")]
pub(crate) fn click_screen(x: f64, y: f64) -> bool {
    macos::click_screen(x, y)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn click_screen(_: f64, _: f64) -> bool {
    false
}

#[cfg(target_os = "macos")]
pub(crate) fn copy() -> bool {
    macos::copy()
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn copy() -> bool {
    false
}
