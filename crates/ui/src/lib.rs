//! Buddy 界面层（GPL-3.0-or-later）
//!
//! # GPUI 接入的三个**必需**前置条件（S00-01 实测）
//!
//! 缺任一即失败，且其中两个是**静默失败**：
//!
//! 1. **`gpui_platform` 必含 `runtime_shaders`**
//!    否则 `gpui_apple` 的 build script 调用 `xcrun metal` 失败
//!    （MetalToolchain 自 Xcode 16 起需单独下载）。
//!    已在 workspace 的 `[workspace.dependencies]` 中声明。
//!
//! 2. **`gpui_platform` 必含 `font-kit`**
//!    否则 **完全没有文字**，且只打一条 WARN 就静默退化 ——
//!    背景与组件照常绘制，唯独没有字形。已在 workspace 中声明。
//!
//! 3. **必须在 `theme::init` 之后安装 `ThemeSettingsProvider`**
//!    否则 `crates/ui` 首次渲染时 panic：
//!    `no state of type GlobalThemeSettingsProvider exists`
//!    见 [`init_theme`]。
//!
//! 完整证据：`docs/tasks/v2.0.0-gpui/research-log.md` §10
//!
//! # 模块（S03-* / S05-* / S07-* 将逐一迁入）
//!
//! - `theme_system` —— 设计令牌（`S03-*`）
//! - `markdown` —— vendored markdown（`S04-*`）
//! - `chat` / `settings` —— 页面（`S05-*` / `S06-*`）
//! - `shell` —— 窗口外壳（`S07-*`）
//! - [`chat_bridge`] —— tokio（engine）↔ GPUI 前台的事件桥接（`S02-06`）

#![warn(missing_docs)]

// ═══════════════════════════════════════════════════════════════
// 重导出：让 `apps/buddy` 只依赖 `buddy-ui`，不必自己长出 zed 依赖
// ═══════════════════════════════════════════════════════════════
//
// 理由：app 是「组装层」，它需要 GPUI 的类型（`div` / `App` / `Window` …）。
// 若让 app 直接依赖 `gpui` / `theme`，那么「哪些 zed crate 被引入」
// 这件事就散落在多个 crate 里，日后换 rev 或替换实现时要改多处。
//
// 统一从 `buddy-ui` 重导出：**UI 层是唯一接触 zed 的地方**。
pub use gpui;
pub use gpui_platform;
pub use theme::{self, ActiveTheme};

pub mod chat_bridge;
pub mod theme_system;

use gpui::{App, Font, Pixels};
use theme::{LoadThemes, ThemeSettingsProvider, UiDensity};

/// Buddy 自己的字体与密度设置
///
/// **为什么需要**：`crates/ui`（zed 的）通过 `theme::theme_settings(cx)` 读取
/// 字体与密度。zed 官方实现是 `theme_settings` crate，但它会拖入整个 `settings` 框架
/// （`settings` / `settings_content` / `settings_json` / `settings_migrator` / `watch`
/// / `release_channel`）。
///
/// 该 trait 只有 5 个方法，自行实现即可**完全跳过**那条依赖链。
/// 见 `docs/evidence/s00-01`（实为 S00-01 的证据段）与 `research-log` §10.3。
pub struct BuddyThemeSettings {
    /// UI 字体
    pub ui_font: Font,
    /// 等宽字体（代码块 / 终端）
    pub buffer_font: Font,
    /// UI 字号
    pub ui_font_size: Pixels,
    /// 等宽字号
    pub buffer_font_size: Pixels,
    /// UI 密度
    pub ui_density: UiDensity,
}

impl Default for BuddyThemeSettings {
    fn default() -> Self {
        Self {
            // `.SystemUIFont` 是 gpui 约定的系统 UI 字体占位名
            // （macOS 侧会映射为 `.AppleSystemUIFont`）
            ui_font: gpui::font(".SystemUIFont"),
            buffer_font: gpui::font("Menlo"),
            ui_font_size: gpui::px(14.0),
            buffer_font_size: gpui::px(12.5),
            ui_density: UiDensity::Default,
        }
    }
}

impl ThemeSettingsProvider for BuddyThemeSettings {
    fn ui_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        &self.ui_font
    }
    fn buffer_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        &self.buffer_font
    }
    fn ui_font_size(&self, _cx: &App) -> Pixels {
        self.ui_font_size
    }
    fn buffer_font_size(&self, _cx: &App) -> Pixels {
        self.buffer_font_size
    }
    fn ui_density(&self, _cx: &App) -> UiDensity {
        self.ui_density
    }
}

/// 初始化主题系统并安装 Buddy 的 `ThemeSettingsProvider`
///
/// **必须在任何窗口创建之前调用。** 顺序不可颠倒：
/// `theme::init` → `set_theme_settings_provider`。
///
/// 漏掉第二步会在首次渲染 `crates/ui` 的组件时 panic。
pub fn init_theme(cx: &mut App) {
    theme::init(LoadThemes::JustBase, cx);
    theme::set_theme_settings_provider(Box::new(BuddyThemeSettings::default()), cx);
}
