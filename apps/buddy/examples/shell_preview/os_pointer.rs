//! 自测专用 OS pointer；调用前必须由 os_input 做锁屏 / 权限预检。
#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::c_void;
    #[repr(C)]
    struct Point {
        x: f64,
        y: f64,
    }
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreateMouseEvent(
            source: *const c_void,
            kind: u32,
            point: Point,
            button: u32,
        ) -> *mut c_void;
        fn CGEventCreateScrollWheelEvent(
            source: *const c_void,
            units: u32,
            count: u32,
            wheel: i32,
            ...
        ) -> *mut c_void;
        fn CGEventPost(tap: u32, event: *mut c_void);
        fn CFRelease(value: *const c_void);
    }
    fn post(event: *mut c_void) -> bool {
        if event.is_null() {
            return false;
        }
        unsafe {
            CGEventPost(0, event);
            CFRelease(event.cast());
        }
        true
    }
    fn mouse(kind: u32, x: f64, y: f64) -> bool {
        post(unsafe { CGEventCreateMouseEvent(std::ptr::null(), kind, Point { x, y }, 0) })
    }
    // AppKit's native window move enters a nested event loop. Keep posting on
    // a background worker and always release the button, including failures.
    pub(super) fn drag(from: (f64, f64), to: (f64, f64)) -> bool {
        let mut ok = mouse(5, from.0, from.1);
        std::thread::sleep(std::time::Duration::from_millis(50));
        ok &= mouse(1, from.0, from.1);
        for step in 1..=12 {
            std::thread::sleep(std::time::Duration::from_millis(25));
            let t = step as f64 / 12.0;
            ok &= mouse(
                6,
                from.0 + (to.0 - from.0) * t,
                from.1 + (to.1 - from.1) * t,
            );
        }
        ok & mouse(2, to.0, to.1)
    }
    pub(super) fn scroll(x: f64, y: f64, delta: i32) -> bool {
        let moved = mouse(5, x, y);
        std::thread::sleep(std::time::Duration::from_millis(50));
        moved & post(unsafe { CGEventCreateScrollWheelEvent(std::ptr::null(), 0, 1, delta) })
    }
}
#[cfg(target_os = "macos")]
pub(crate) fn drag(from: (f64, f64), to: (f64, f64)) -> bool {
    macos::drag(from, to)
}
#[cfg(not(target_os = "macos"))]
pub(crate) fn drag(_: (f64, f64), _: (f64, f64)) -> bool {
    false
}
#[cfg(target_os = "macos")]
pub(crate) fn scroll(x: f64, y: f64, delta: i32) -> bool {
    macos::scroll(x, y, delta)
}
#[cfg(not(target_os = "macos"))]
pub(crate) fn scroll(_: f64, _: f64, _: i32) -> bool {
    false
}
