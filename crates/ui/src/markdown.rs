//! Markdown 渲染接入（S04-*）
//!
//! 渲染器为 vendored zed markdown（`crates/markdown`，lib 名 `zed_markdown`，GPL-3.0-or-later，
//! 来源与 patch 清单见 `crates/markdown/VENDOR.md`）。本模块负责按 Buddy 排版令牌初始化它。
//!
//! # v1 消息排版的实际取值（`v1-final`，Phase 05 渲染消息行时沿用）
//!
//! | 位置 | 字号 | 行高 | 出处 |
//! |------|------|------|------|
//! | 助手消息正文 | 14px（= `--font-size-md`） | **1.6**（写死，非令牌） | `MessageBubble.tsx:234-235` |
//! | 用户消息正文 | 14px | 1.5 | `MessageBubble.tsx:223-224` |
//! | 代码块 | 13px 等宽（= `--font-size-base`） | 1.5 | `CodeBlock.tsx:206-208` |

pub use zed_markdown;

use crate::theme_system::{fonts, tokens::metrics};
use gpui::{App, px};

/// 安装 markdown 渲染所需的字体设置。须在 [`crate::init_theme`] 之后调用。
pub fn init(cx: &mut App) {
    let ui_font = fonts::ui_font(cx);
    let code_font = fonts::mono_font(cx);
    zed_markdown::install_theme_settings(
        cx,
        ui_font,
        code_font,
        px(metrics::FONT_SIZE_MD),
        px(metrics::FONT_SIZE_BASE),
    );
}
