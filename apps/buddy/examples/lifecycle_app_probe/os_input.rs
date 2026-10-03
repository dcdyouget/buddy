//! 独立进程发送专用 F19，证明重绑后的系统事件可以从 owner 外部到达。

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Preflight {
    Ready,
    SessionLocked,
    EventAccessDenied,
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;

    type EventRef = *mut c_void;

    const HID_EVENT_TAP: u32 = 0;
    const KEY_F19: u16 = 0x50;
    const KEY_SHIFT: u16 = 56;
    const KEY_COMMAND: u16 = 55;
    const KEY_ALTERNATE: u16 = 58;
    const FLAG_SHIFT: u64 = 1 << 17;
    const FLAG_COMMAND: u64 = 1 << 20;
    const FLAG_ALTERNATE: u64 = 1 << 19;
    // F19 needs the Function classification bit as well as the three modifiers.
    // Omitting it posts a key event but did not invoke the registered hotkey.
    const FLAG_FUNCTION: u64 = 1 << 23;
    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreateKeyboardEvent(source: *const c_void, key: u16, key_down: bool) -> EventRef;
        fn CGEventSetFlags(event: EventRef, flags: u64);
        fn CGEventPost(tap: u32, event: EventRef);
        fn CGPreflightPostEventAccess() -> bool;
        fn CGSessionCopyCurrentDictionary() -> *const c_void;
        fn CFDictionaryGetValue(dictionary: *const c_void, key: *const c_void) -> *const c_void;
        fn CFStringCreateWithCString(
            allocator: *const c_void,
            string: *const std::ffi::c_char,
            encoding: u32,
        ) -> *const c_void;
        fn CFBooleanGetValue(value: *const c_void) -> u8;
        fn CFRelease(value: *const c_void);
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
        unsafe { CGPreflightPostEventAccess() }
    }

    fn post(key: u16, flags: u64, down: bool) -> bool {
        // SAFETY: the event is created by CoreGraphics and released after posting.
        let event = unsafe { CGEventCreateKeyboardEvent(std::ptr::null(), key, down) };
        if event.is_null() {
            return false;
        }
        unsafe {
            CGEventSetFlags(event, flags);
            CGEventPost(HID_EVENT_TAP, event);
            CFRelease(event.cast());
        }
        // The sender is a separate short-lived process. Space physical events so
        // WindowServer can dispatch them before its connection is torn down.
        std::thread::sleep(std::time::Duration::from_millis(15));
        true
    }

    pub(crate) fn send_f19() -> Result<(), String> {
        match super::preflight() {
            super::Preflight::Ready => {}
            super::Preflight::SessionLocked => {
                eprintln!("[S07-11] BLOCKED：macOS 会话已锁定，未发送 F19");
                return Err("BLOCKED：macOS 会话已锁定".into());
            }
            super::Preflight::EventAccessDenied => {
                eprintln!("[S07-11] BLOCKED：F19 sender 没有 CGEventPost 权限");
                return Err("BLOCKED：F19 sender 没有 CGEventPost 权限".into());
            }
        }
        let down = post(KEY_COMMAND, FLAG_COMMAND, true)
            && post(KEY_ALTERNATE, FLAG_COMMAND | FLAG_ALTERNATE, true)
            && post(KEY_SHIFT, FLAG_COMMAND | FLAG_ALTERNATE | FLAG_SHIFT, true)
            && post(
                KEY_F19,
                FLAG_COMMAND | FLAG_ALTERNATE | FLAG_SHIFT | FLAG_FUNCTION,
                true,
            );
        // Do not short-circuit releases: a partial key-down sequence must never leave
        // Command/Option/Shift logically held for the rest of the desktop session.
        let release_f19 = post(
            KEY_F19,
            FLAG_COMMAND | FLAG_ALTERNATE | FLAG_SHIFT | FLAG_FUNCTION,
            false,
        );
        let release_shift = post(KEY_SHIFT, FLAG_COMMAND | FLAG_ALTERNATE, false);
        let release_alternate = post(KEY_ALTERNATE, FLAG_COMMAND, false);
        let release_command = post(KEY_COMMAND, 0, false);
        let up = release_f19 && release_shift && release_alternate && release_command;
        std::thread::sleep(std::time::Duration::from_millis(100));
        if down && up {
            Ok(())
        } else {
            Err("F19 系统事件创建失败".into())
        }
    }
}

#[cfg(target_os = "macos")]
pub(super) fn preflight() -> Preflight {
    if macos::session_locked() {
        Preflight::SessionLocked
    } else if macos::can_post() {
        Preflight::Ready
    } else {
        Preflight::EventAccessDenied
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn preflight() -> Preflight {
    Preflight::EventAccessDenied
}

#[cfg(not(target_os = "macos"))]
pub(super) fn send_f19() -> Result<(), String> {
    Err("F19 系统输入探针仅支持 macOS".into())
}

#[cfg(target_os = "macos")]
pub(crate) use macos::send_f19;
