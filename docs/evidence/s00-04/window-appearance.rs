//! S00-04 产物：Buddy 窗口的「外观」配置（macOS）—— **不透明 + 圆角，无毛玻璃**
//!
//! **为什么需要这个文件**：`spikes/` 被 `.gitignore` 排除，spike 代码会丢失。
//! 本文件是已实测并被产品确认的**最终外观配置**，后续 spec 直接取用：
//!   - `S03-03` 玻璃与阴影令牌迁移
//!   - `S07-02` 无装饰 / 去阴影
//!   - `S09-01` 渲染差异对齐（Windows 侧只需对齐圆角与阴影，**不涉及 backdrop**）
//!
//! 实测依据见 `docs/specs/phase-00/S00-04-backdrop.md` 的「证据」段。
//!
//! ## 产品决策（2026-09-10）
//!
//! > **不要毛玻璃，不要半透明。** 面板为不透明实色 + 16px 圆角。
//!
//! 这不是降级 —— **v1 本来就是实色界面**（`src-tauri/src/platform/macos.rs:154`：
//! 「Buddy 当前使用实色白色界面；不启用原生 vibrancy」）。新版本保持 v1 观感。
//!
//! ## 附带的重大收益：**不需要 fork GPUI**
//!
//! 基础 gpui **没有「窗内卡片级 backdrop blur」原语**（只有 Windows 专用的
//! `MicaBackdrop` 枚举值，在 macOS 上是 no-op）。而「不透明 + 圆角」完全用
//! gpui 内置能力 + objc2 运行时补丁就能做到。
//!
//! 至此窗口外壳（S00-02/S00-03）与外观（S00-04）**都不需要 fork**。

#![allow(dead_code)]

use gpui::WindowBackgroundAppearance;
use objc2::msg_send;
use objc2::runtime::AnyObject;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

// ═══════════════════════════════════════════════════════════════
// 最终窗口配置总览
// ═══════════════════════════════════════════════════════════════

/// **最终外观配置**（逐项均已实测）
///
/// | 项 | 值 | 实现 |
/// |----|-----|------|
/// | 窗口 kind | `WindowKind::PopUp` | gpui `WindowOptions` |
/// | 窗口背景 | `Transparent` | gpui `WindowOptions` —— **仅为圆角，非毛玻璃** |
/// | 零装饰 | 去 Titled/Closable/Miniaturizable | objc2 `setStyleMask:` |
/// | 阴影 | 关闭 | objc2 `setHasShadow:` |
/// | 圆角 | 16px | objc2 CALayer `setCornerRadius:` |
/// | 面板填充 | **不透明** Theme 色值 | gpui `Styled::bg` |
/// | 面板边框 | **无** | 见下方「白边框」说明 |
/// | 模糊 / vibrancy | **不使用** | — |
pub const FINAL_WINDOW_BACKGROUND: WindowBackgroundAppearance =
    WindowBackgroundAppearance::Transparent;

/// 圆角半径（与 v1 一致，见 `src-tauri/src/platform/macos.rs` 的 `setCornerRadius: 16.0`）
pub const CORNER_RADIUS: f64 = 16.0;

// ── NSWindowStyleMask ──
const NS_TITLED: u64 = 1 << 0;
const NS_CLOSABLE: u64 = 1 << 1;
const NS_MINIATURIZABLE: u64 = 1 << 2;
const NS_RESIZABLE: u64 = 1 << 3;
const NS_NONACTIVATING_PANEL: u64 = 1 << 7;

// ── NSWindowLevel / NSWindowCollectionBehavior ──
const NS_FLOATING_WINDOW_LEVEL: i64 = 3;
const NS_CAN_JOIN_ALL_SPACES: u64 = 1 << 0;
const NS_FULL_SCREEN_AUXILIARY: u64 = 1 << 8;

/// 取原生 `NSWindow` 指针
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

/// **完整外观补丁**（零装饰 + 去阴影 + 层级 + 全工作区 + 圆角）
///
/// ⚠️ **必须在窗口创建后调用**（如视图首次 `render` 内）。
///
/// ## 教训：外壳与外观必须一起打
/// S00-04 首轮测试只打了圆角补丁、忘了零装饰补丁，结果测试窗口顶部仍带红黄绿交通灯 ——
/// **分项 Spike 的测试窗口不代表最终成品**。最终验收必须用「外壳 + 外观」完整组合的窗口。
pub unsafe fn apply_full_appearance_patch(w: *mut AnyObject) {
    unsafe {
        // 1. 零装饰（硬约束 1：无红黄绿、无标题栏）
        let before: u64 = msg_send![w, styleMask];
        let new_mask = (before & !(NS_TITLED | NS_CLOSABLE | NS_MINIATURIZABLE))
            | NS_RESIZABLE
            | NS_NONACTIVATING_PANEL;
        let _: () = msg_send![w, setStyleMask: new_mask];

        // 2. 去系统阴影
        //    注：若「不透明面板 + 深色背景」导致边界不清，可改回 YES 用阴影区分边界。
        //    当前按用户确认的结果：关闭。
        let _: () = msg_send![w, setHasShadow: false];

        // 3. 层级与全工作区
        let _: () = msg_send![w, setLevel: NS_FLOATING_WINDOW_LEVEL];
        let _: () = msg_send![
            w,
            setCollectionBehavior: NS_CAN_JOIN_ALL_SPACES | NS_FULL_SCREEN_AUXILIARY
        ];
        let _: () = msg_send![w, setHidesOnDeactivate: false];

        // 4. 圆角（需 contentView 开 layer backing）
        let content: *mut AnyObject = msg_send![w, contentView];
        if !content.is_null() {
            let _: () = msg_send![content, setWantsLayer: true];
            let layer: *mut AnyObject = msg_send![content, layer];
            if !layer.is_null() {
                let _: () = msg_send![layer, setCornerRadius: CORNER_RADIUS];
                let _: () = msg_send![layer, setMasksToBounds: true];
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// 为什么「窗口 Transparent」不等于「毛玻璃」
// ═══════════════════════════════════════════════════════════════
//
// `WindowBackgroundAppearance::Transparent` 只做一件事：让 `isOpaque = false`，
// 从而使**圆角外的区域能透出桌面**。这是圆角能成立的前提 —— 否则窗口是矩形硬边。
//
// 它与毛玻璃完全无关：
//   - `Transparent` → `isOpaque=false`，无任何模糊
//   - `Blurred`     → 额外添加 `NSVisualEffectView` 子视图做模糊
//
// Buddy 用前者，不用后者。

// ═══════════════════════════════════════════════════════════════
// 「白边框」说明（决策记录，避免后人照抄 v1 令牌）
// ═══════════════════════════════════════════════════════════════
//
// `--glass-outline` 令牌在 v1 暗色模式下是 `rgba(255,255,255,0.19)`，
// 实机观感表现为**四边白色描边**。用户明确表示不要。
//
// 因此面板**不设边框**。边界区分依靠背景色差（或必要时改回系统阴影）。
//
// 若后人看到 `docs/design/design-tokens.md` 里的 `--glass-outline` 想照抄，
// 请注意：该令牌已在实机验证中被否决。

// ═══════════════════════════════════════════════════════════════
// 三路径对比实测（存档，防止后人重复验证）
// ═══════════════════════════════════════════════════════════════
//
// 环境：macOS 26.4 (25E246) / arm64
//
// | 路径 | 图层树 | 结论 |
// |------|--------|------|
// | `Blurred`（材质 Selection=4，gpui 默认） | `CABackdropLayer` ×2 | **模糊确实在渲染** |
// | `Blurred` + 运行时改材质 UnderWindowBackground=21 | `CABackdropLayer` ×2（层数 14 vs 16） | 也渲染 |
// | `Blurred` + 材质 HUDWindow=13 | 同上 | 也渲染 |
// | `Transparent` + 不透明填充 + 16px 圆角 | — | **产品采用** |
//
// ## ⚠️ 关键教训：必须延迟复探
// 首次探测（`render` 内，合成之前）只看到 `NSViewBackingLayer`，
// **会误判为「模糊未生效、macOS 26 打坏了模糊、需要 fork」**。
// 延迟到 t≈1s 后 `CABackdropLayer` 才出现。
//
// ## 关于「macOS 26 打坏模糊」的传闻
// Comet 的 fork 注释称「macOS 26 stopped vending CABackdropLayer for Selection」。
// **在 macOS 26.4 + 本 rev 上未复现** —— 模糊正常。该传闻可能适用于更早的 26.x，
// 或与其 fork 内的其他改动相关。若将来需要模糊，请先自行复测，勿直接照搬该结论。
//
// ## 其他已排除的可能
// - 系统「降低透明度」辅助功能：实测为**关闭**，不是它导致的
// - 窗口侧属性均正确：`isOpaque=false` / `alphaValue=1` / `blendingMode=BehindWindow` / `state=Active`
// - gpui 的 `remove_layer_background` 只移除**饱和度滤镜**（`colorSaturate`）与
//   `CAChameleonLayer`（桌面染色），**不碰模糊本身**

use gpui::Window;

// ═══════════════════════════════════════════════════════════════
// 不做的事（明确记录，避免后人重做）
// ═══════════════════════════════════════════════════════════════
//
// - **不做** 窗内卡片级 backdrop blur：基础 gpui 无此原语，需 fork。产品不需要。
// - **不做** Windows Mica / Acrylic：`MicaBackdrop` 采样的是**壁纸**而非窗口后方内容，
//   语义与 macOS vibrancy 不同（见 research-log §4.2）。既然不用系统 backdrop，此差异不存在。
// - **不做** `--glass-outline` 描边：实机观感为「四边白光」，用户否决。
