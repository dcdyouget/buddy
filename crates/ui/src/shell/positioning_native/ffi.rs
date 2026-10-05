//! AppKit display-number and CoreGraphics UUID FFI helpers for .

use objc::{sel, sel_impl};

/// 读取 NSScreen 的 CGDirectDisplayID。
pub(super) unsafe fn screen_number(screen: *mut objc::runtime::Object) -> Result<u32, String> {
    let description: *mut objc::runtime::Object =
        unsafe { objc::msg_send![screen, deviceDescription] };
    let key: *mut objc::runtime::Object = unsafe {
        objc::msg_send![objc::class!(NSString), stringWithUTF8String: b"NSScreenNumber\0".as_ptr()]
    };
    let number: *mut objc::runtime::Object =
        unsafe { objc::msg_send![description, objectForKey: key] };
    if number.is_null() {
        return Err("NSScreen 缺少 NSScreenNumber".into());
    }
    Ok(unsafe { objc::msg_send![number, unsignedIntValue] })
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CFUUIDBytes {
    byte0: u8,
    byte1: u8,
    byte2: u8,
    byte3: u8,
    byte4: u8,
    byte5: u8,
    byte6: u8,
    byte7: u8,
    byte8: u8,
    byte9: u8,
    byte10: u8,
    byte11: u8,
    byte12: u8,
    byte13: u8,
    byte14: u8,
    byte15: u8,
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGDisplayCreateUUIDFromDisplayID(display: u32) -> *const std::ffi::c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFUUIDGetUUIDBytes(uuid: *const std::ffi::c_void) -> CFUUIDBytes;
    fn CFRelease(value: *const std::ffi::c_void);
}

/// 将 CGDirectDisplayID 转成可跨重排复用的屏幕 UUID。
pub(super) fn display_uuid(display_id: u32) -> String {
    let uuid = unsafe { CGDisplayCreateUUIDFromDisplayID(display_id) };
    if uuid.is_null() {
        return format!("display-{display_id}");
    }
    let bytes = unsafe { CFUUIDGetUUIDBytes(uuid) };
    unsafe { CFRelease(uuid) };
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes.byte0,
        bytes.byte1,
        bytes.byte2,
        bytes.byte3,
        bytes.byte4,
        bytes.byte5,
        bytes.byte6,
        bytes.byte7,
        bytes.byte8,
        bytes.byte9,
        bytes.byte10,
        bytes.byte11,
        bytes.byte12,
        bytes.byte13,
        bytes.byte14,
        bytes.byte15,
    )
}
