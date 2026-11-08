//! 系统辅助功能设置
//!
//! v1 经 `prefers-reduced-motion`（WebKit 读取 macOS「减弱动态效果」）关闭流式渐显、星标呼吸、
//! 复制成功动画等。GPUI 没有对应接口，这里直接读系统设置。每次调用都实时读取，
//! 用户在系统设置中切换后下一帧即生效。

/// 系统是否要求减弱动态效果
#[cfg(target_os = "macos")]
pub fn prefers_reduced_motion() -> bool {
    use objc::runtime::{BOOL, NO, Object};
    use objc::{class, msg_send, sel, sel_impl};
    // SAFETY: NSWorkspace.sharedWorkspace 恒存在；accessibilityDisplayShouldReduceMotion 为只读 BOOL 属性（macOS 10.12+）
    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let reduce: BOOL = msg_send![workspace, accessibilityDisplayShouldReduceMotion];
        reduce != NO
    }
}

/// Windows「辅助功能 → 视觉效果 → 动画效果」关闭时使用静态呈现。
#[cfg(target_os = "windows")]
pub fn prefers_reduced_motion() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW};
    let mut enabled: windows_sys::core::BOOL = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL to the supplied pointer.
    let ok = unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut enabled).cast(), 0)
    };
    ok != 0 && enabled == 0
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn prefers_reduced_motion() -> bool {
    false
}

#[cfg(test)]
mod tests {
    /// 与系统设置无关地可调用、不崩溃；并与 `defaults` 读出的系统值一致
    #[test]
    #[cfg(target_os = "macos")]
    fn matches_system_defaults() {
        let got = super::prefers_reduced_motion();
        let out = std::process::Command::new("defaults")
            .args(["read", "com.apple.universalaccess", "reduceMotion"])
            .output()
            .expect("defaults");
        // 从未设置过时 defaults 读不到该键 → 视为关闭
        let want = String::from_utf8_lossy(&out.stdout).trim() == "1";
        assert_eq!(got, want);
    }
}
