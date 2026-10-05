//! Buddy 修改：`ThemeSettings` **shim**
//!
//! # 为什么需要
//!
//! zed 的 `crates/markdown/src/markdown.rs` 有 **21 处**读取
//! `theme_settings::ThemeSettings::get_global(cx)`（字体族、字号、行高）。
//!
//! 而 `theme_settings` crate 依赖 zed 的 `settings` 框架，会拖入：
//! ```text
//! settings → settings_content → settings_json → settings_macros
//!         → settings_migrator → watch → release_channel
//! ```
//!
//! # 做法：保持 API 表面，替换底层
//!
//! 与其改 21 处调用点，不如提供一个**同名同 API 的 shim**，
//! 只改 vendor 后文件的一行 `use`：
//!
//! ```diff
//! - use theme_settings::ThemeSettings;
//! + use crate::theme_settings_shim::ThemeSettings;
//! ```
//!
//! 这样 patch 从「21 处」降到「1 行 + 本文件（约 120 行）」。
//!
//! # 与 `ThemeSettingsProvider` 的关系
//!
//! `theme::ThemeSettingsProvider` 是只有 5 个方法的 trait，自己实现即可满足 `ui` crate 的需求。
//!
//! 但 `markdown.rs` **不通过那个 trait**，而是直接用 `theme_settings::ThemeSettings`
//! 的具体类型（要用 `.buffer_font.weight`、`.buffer_line_height.value()` 这类字段与方法）。
//! 所以需要本 shim 提供同样的具体类型。
//!
//! 两者互补：
//! - `ThemeSettingsProvider`→ 满足 `ui` crate
//! - `ThemeSettings` shim（本文件）→ 满足 vendored `markdown`

use gpui::{App, Font, Global, Pixels, SharedString, px};
use theme::BufferLineHeight;

/// 与 `theme_settings::ThemeSettings` **API 兼容**的替代实现。
///
/// 字段名与 gpui/zed 一致，因此 vendored `markdown.rs` 无需改动调用点。
pub struct ThemeSettings {
    // ── 字段（markdown.rs 直接访问）──
    pub ui_font: Font,
    pub buffer_font: Font,
    pub buffer_line_height: BufferLineHeight,

    // ── 字体族缓存（方法需返回 &SharedString）──
    ui_family: SharedString,
    buffer_family: SharedString,
    md_preview_family: SharedString,
    agent_ui_family: SharedString,
    agent_buffer_family: SharedString,
    md_preview_code_family: SharedString,

    // ── 字号缓存 ──
    ui_size: Pixels,
    buffer_size: Pixels,
    agent_ui_size: Pixels,
    agent_buffer_size: Pixels,
    md_preview_size: Pixels,
}

impl Global for ThemeSettings {}

impl Default for ThemeSettings {
    /// 未指定时的兜底（系统 UI 字体 14 / Menlo 12.5）；Buddy 实际经 `ThemeSettings::new` 传入令牌
    fn default() -> Self {
        Self::new(gpui::font(".SystemUIFont"), gpui::font("Menlo"), px(14.0), px(12.5))
    }
}

impl ThemeSettings {
    /// 以指定字体与字号构造（由 buddy-ui 传入 Buddy 排版令牌，避免 markdown 偏离 v1 观感）
    ///
    /// markdown.rs 区分 ui / buffer（代码）两套字体；agent_* 与 markdown_preview_* 系列取同一套。
    pub fn new(ui_font: Font, buffer_font: Font, ui_size: Pixels, buffer_size: Pixels) -> Self {
        Self {
            ui_family: ui_font.family.clone(),
            buffer_family: buffer_font.family.clone(),
            md_preview_family: ui_font.family.clone(),
            agent_ui_family: ui_font.family.clone(),
            agent_buffer_family: buffer_font.family.clone(),
            md_preview_code_family: buffer_font.family.clone(),
            ui_font,
            buffer_font,
            buffer_line_height: BufferLineHeight::Standard,
            ui_size,
            buffer_size,
            agent_ui_size: ui_size,
            agent_buffer_size: buffer_size,
            md_preview_size: ui_size,
        }
    }

    /// 安装为全局（在 `theme::init` 之后调用）
    pub fn install(settings: ThemeSettings, cx: &mut App) {
        cx.set_global(settings);
    }

    /// 与 `theme_settings::ThemeSettings::get_global` 同名同签名
    pub fn get_global(cx: &App) -> &ThemeSettings {
        cx.global::<ThemeSettings>()
    }

    // ── 以下 5 个方法在 markdown.rs 中被调用 ──

    pub fn ui_font_size(&self, _cx: &App) -> Pixels {
        self.ui_size
    }

    pub fn buffer_font_size(&self, _cx: &App) -> Pixels {
        self.buffer_size
    }

    pub fn agent_ui_font_size(&self, _cx: &App) -> Pixels {
        self.agent_ui_size
    }

    pub fn agent_buffer_font_size(&self, _cx: &App) -> Pixels {
        self.agent_buffer_size
    }

    pub fn markdown_preview_font_size(&self, _cx: &App) -> Pixels {
        self.md_preview_size
    }

    // ── 以下 4 个返回字体族 ──

    pub fn markdown_preview_font_family(&self) -> &SharedString {
        &self.md_preview_family
    }

    pub fn agent_ui_font_family(&self) -> &SharedString {
        &self.agent_ui_family
    }

    pub fn agent_buffer_font_family(&self) -> &SharedString {
        &self.agent_buffer_family
    }

    pub fn markdown_preview_code_font_family(&self) -> &SharedString {
        &self.md_preview_code_family
    }
}
