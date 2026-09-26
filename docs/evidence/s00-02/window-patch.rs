//! S00-02 产物：Buddy 形态窗口的 AppKit 补丁（macOS）
//!
//! **为什么需要这个文件**：`spikes/` 目录被 `.gitignore` 排除，spike 代码会丢失。
//! 本文件是已实测可用的**规范化产物**，后续 spec 直接取用：
//!   - `S07-02` 无装饰 / 去阴影
//!   - `S07-04` 点走消失（hide/show）
//!   - `S07-05` 置顶与全工作区
//!
//! 实测依据见 `docs/specs/phase-00/S00-02-floating-panel.md` 的「证据」段。
//!
//! ## 关键结论：**不需要 patch GPUI fork**
//! 下列全部在运行时通过 objc2 达成，GPUI 公开接口之外无需改源码：
//!   - 零装饰（`styleMask` 去 Titled/Closable/Miniaturizable）
//!   - 去系统阴影（`setHasShadow:(NO)`）
//!   - 窗口显隐（`orderOut:` / `makeKeyAndOrderFront:`）
//!   - 窗口层级与全工作区
//!
//! GPUI **已经提供**的（无需自己实现）：
//!   - `acceptsFirstMouse:` → 硬编码返回 `YES`（`gpui_macos/src/window.rs:3394`）
//!   - `canBecomeKeyWindow` / `canBecomeMainWindow` → 硬编码 `YES`（同文件 :447/:451）
//!   - 完整 `NSTextInputClient`（`setMarkedText:` / `insertText:` / `firstRectForCharacterRange:` …）
//!   - `WindowBackgroundAppearance::Transparent` → `isOpaque` 自动变 `false`
//!
//! ## 依赖
//! ```toml
//! objc2 = "0.6"                 # 版本须与 gpui 内部一致，否则 HasWindowHandle 不统一
//! raw-window-handle = "0.6"
//! ```

#![allow(dead_code)]

use gpui::{Window, WindowKind};
use objc2::msg_send;
use objc2::runtime::{AnyObject, Bool};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

// ── NSWindowStyleMask ──
pub const NS_TITLED: u64 = 1 << 0;
pub const NS_CLOSABLE: u64 = 1 << 1;
pub const NS_MINIATURIZABLE: u64 = 1 << 2;
pub const NS_RESIZABLE: u64 = 1 << 3;
pub const NS_UTILITY: u64 = 1 << 4;
pub const NS_NONACTIVATING_PANEL: u64 = 1 << 7;
pub const NS_FULL_SIZE_CONTENT_VIEW: u64 = 1 << 15;

// ── NSWindowLevel ──
pub const NS_NORMAL_WINDOW_LEVEL: i64 = 0;
pub const NS_FLOATING_WINDOW_LEVEL: i64 = 3;
pub const NS_POP_UP_WINDOW_LEVEL: i64 = 101;

// ── NSWindowCollectionBehavior ──
pub const NS_CAN_JOIN_ALL_SPACES: u64 = 1 << 0;
pub const NS_MOVE_TO_ACTIVE_SPACE: u64 = 1 << 1;
pub const NS_FULL_SCREEN_AUXILIARY: u64 = 1 << 8;

/// Buddy 面板推荐的窗口 kind。
///
/// **实测对照**（未打补丁时的原生属性）：
///
/// | kind | 类 | styleMask | level | collectionBehavior |
/// |------|----|-----------|-------|--------------------|
/// | `Normal` | `GPUIWindow` | Titled \| Closable \| Miniaturizable \| Resizable | 0 | 0x0 |
/// | `PopUp` | `GPUIPanel` | 同上 **+ NonactivatingPanel** | 101 | **CanJoinAllSpaces \| FullScreenAuxiliary** |
/// | `Floating` | `GPUIPanel` | 同上（无 Nonactivating） | 3 | **0x0（无全工作区）** |
/// | `Dialog` | `GPUIPanel` | 同上 | 0 | 0x0 |
///
/// `PopUp` 是**唯一**同时给出「非激活面板 + 全工作区可见 + 高层级」的取值。
/// 注意 `Floating` **不给全工作区** —— 不要用它来同时解决「置顶 + 全工作区」。
pub const BUDDY_WINDOW_KIND: WindowKind = WindowKind::PopUp;

/// 从 gpui `Window` 取原生 `NSWindow` 指针。
///
/// 路径：`Window::window_handle()` → `RawWindowHandle::AppKit(ns_view)` → `[ns_view window]`。
pub fn ns_window_ptr(window: &mut Window) -> Option<*mut AnyObject> {
    let handle = window.window_handle().ok()?;
    let ns_view_ptr = match handle.as_raw() {
        RawWindowHandle::AppKit(h) => h.ns_view.as_ptr(),
        _ => return None,
    };
    unsafe {
        let ns_view: &AnyObject = &*(ns_view_ptr as *const AnyObject);
        let w: Option<&AnyObject> = msg_send![ns_view, window];
        w.map(|w| w as *const AnyObject as *mut AnyObject)
    }
}

/// Buddy 形态补丁：零装饰 + 去阴影 + 层级 + 全工作区 + 鼠标移动事件 + 不自动隐藏。
///
/// **必须在窗口创建后调用**（例如视图首次 `render` 内），且可安全重复调用（幂等）。
///
/// ## 为什么要自己改 styleMask
/// `WindowOptions` **两条路都拿不到零装饰窗口**（`gpui_macos/src/window.rs:990-1007`）：
/// - `titlebar: Some(..)`（`WindowOptions::default()` 的取值）→ `Closable | Titled`（+ Resizable/Miniaturizable）
/// - `titlebar: None` → 仍是 `Titled | FullSizeContentView`
///
/// 而 `window_decorations` 字段在 `gpui_macos` 中**完全没有实现**（grep 零命中）。
pub unsafe fn apply_buddy_window_patch(w: *mut AnyObject) {
    unsafe {
        // 1. 零装饰：去掉 Titled / Closable / Miniaturizable，保留 Resizable 与 NonactivatingPanel
        let before: u64 = msg_send![w, styleMask];
        let new_mask = (before & !(NS_TITLED | NS_CLOSABLE | NS_MINIATURIZABLE))
            | NS_RESIZABLE
            | NS_NONACTIVATING_PANEL;
        let _: () = msg_send![w, setStyleMask: new_mask];

        // 2. 去系统阴影
        let _: () = msg_send![w, setHasShadow: Bool::NO];

        // 3. 窗口层级：3（NSFloatingWindowLevel）比 PopUp 给的 101 保守，
        //    不会压住菜单栏。要更强势可改回 NS_POP_UP_WINDOW_LEVEL。
        let _: () = msg_send![w, setLevel: NS_FLOATING_WINDOW_LEVEL];

        // 4. 全工作区可见（PopUp 已设，这里保证幂等）
        let _: () =
            msg_send![w, setCollectionBehavior: NS_CAN_JOIN_ALL_SPACES | NS_FULL_SCREEN_AUXILIARY];

        // 5. 不因失活被系统自动隐藏 —— 显隐由 Buddy 自己控制
        let _: () = msg_send![w, setHidesOnDeactivate: Bool::NO];

        // 6. 让 hover 生效（gpui 的 is_hovered 在 macOS 上未实现，需靠此事件）
        let _: () = msg_send![w, setAcceptsMouseMovedEvents: Bool::YES];
    }
}

/// 隐藏窗口（「点走消失」）。
///
/// **gpui 没有 hide/show API** —— 只有 `Window::minimize_window()`，而最小化会飞进 Dock，
/// 不符合 Buddy 的交互。必须用 AppKit 的 `orderOut:`。
pub unsafe fn hide_window(w: *mut AnyObject) {
    unsafe {
        let nil: *mut AnyObject = std::ptr::null_mut();
        let _: () = msg_send![w, orderOut: nil];
    }
}

/// 显示并使其成为 key window（热键唤起时应使用，用户可立刻输入）。
pub unsafe fn show_window_and_focus(w: *mut AnyObject) {
    unsafe {
        let nil: *mut AnyObject = std::ptr::null_mut();
        let _: () = msg_send![w, makeKeyAndOrderFront: nil];
    }
}

/// 显示但不抢焦点（若产品选择「不夺取焦点」策略）。
pub unsafe fn show_window_no_focus(w: *mut AnyObject) {
    unsafe {
        let nil: *mut AnyObject = std::ptr::null_mut();
        let _: () = msg_send![w, orderFront: nil];
    }
}

/// 强行使窗口 resignKey（等价于用户点击外部时 AppKit 的行为）。
///
/// 用途：自动化测试「点走消失」路径。gpui 自身也用它处理 AppKit 的虚假
/// `windowDidBecomeKey` 事件（`gpui_macos/src/window.rs:3022-3038`）。
pub unsafe fn force_resign_key(w: *mut AnyObject) {
    unsafe {
        let _: () = msg_send![w, resignKeyWindow];
    }
}

// ── 「点击外部关闭」的检测方式 ──
//
// gpui **未提供**全局鼠标监听（无 `addGlobalMonitorForEvents`），
// 但 `Context::observe_window_activation` 配合 `Window::is_window_active()` 足够：
//
// ```rust
// cx.observe_window_activation(window, |this, window, cx| {
//     if !window.is_window_active() {
//         // 窗口失去 key → 点走消失
//         if let Some(w) = ns_window_ptr(window) { unsafe { hide_window(w) } }
//     }
// });
// ```
//
// `is_window_active()` 的 macOS 实现是 `[native_window isKeyWindow]`
// （`gpui_macos/src/window.rs:1808`），即**窗口级** key 状态，正合本语义。
//
// ⚠️ 未验证项：S00-02 实测中「用户真实点击外部」**未能触发** `resignKey`
// （两次运行均由兜底的 `force_resign_key` 触发）。需在 `S07-04` 用真实交互确认；
// 若确认不触发，备选方案是自行注册 `NSEvent.addGlobalMonitorForEvents`。
