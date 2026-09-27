//! Markdown 渲染接入（S04-*）
//!
//! 渲染器为 vendored zed markdown（`crates/markdown`，lib 名 `zed_markdown`，GPL-3.0-or-later，
//! 来源与 patch 清单见 `crates/markdown/VENDOR.md`）。本模块负责按 Buddy 令牌初始化它。
//!
//! # v1 消息排版的实际取值（`v1-final`，Phase 05 渲染消息行时沿用）
//!
//! | 位置 | 字号 | 行高 | 出处 |
//! |------|------|------|------|
//! | 助手消息正文 | 14px（= `--font-size-md`） | **1.6**（写死，非令牌） | `MessageBubble.tsx:234-235` |
//! | 用户消息正文 | 14px | 1.5 | `MessageBubble.tsx:223-224` |
//! | 代码块 | 13px 等宽（= `--font-size-base`） | 1.5 | `CodeBlock.tsx:206-208` |

pub use zed_markdown;

use crate::theme_system::{Theme, fonts, tokens::metrics};
use gpui::{App, FontStyle, FontWeight, HighlightStyle, Hsla, px};
use std::sync::Arc;
use theme::SyntaxTheme;
use zed_markdown::syntax::SYNTAX_CATEGORIES;

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

/// 按当前主题构造代码高亮配色（S04-02）
///
/// 顺序必须与 [`SYNTAX_CATEGORIES`] 一致（`HighlightId` 即下标）。取值与 v1 `CodeBlock.tsx`
/// `buddyCodeTheme` 相同：只设前景色，**注释另加斜体、关键字另加字重 600**（v1 即如此）。
/// 代码块为等宽字体，字重 / 斜体不改变字符宽度与行高 → 不影响布局（S04-02 测试 T03 验证）。
pub fn syntax_theme(theme: &Theme) -> Arc<SyntaxTheme> {
    let c = theme.colors;
    let color = |rgba| Some(Hsla::from(rgba));
    let styles = SYNTAX_CATEGORIES.map(|category| {
        let style = match category {
            "comment" => HighlightStyle {
                color: color(c.code_syntax_comment),
                font_style: Some(FontStyle::Italic),
                ..Default::default()
            },
            "punctuation" => HighlightStyle { color: color(c.code_syntax_punctuation), ..Default::default() },
            "property" => HighlightStyle { color: color(c.code_syntax_property), ..Default::default() },
            "number" => HighlightStyle { color: color(c.code_syntax_number), ..Default::default() },
            "string" => HighlightStyle { color: color(c.code_syntax_string), ..Default::default() },
            "operator" => HighlightStyle { color: color(c.code_syntax_operator), ..Default::default() },
            "keyword" => HighlightStyle {
                color: color(c.code_syntax_keyword),
                font_weight: Some(FontWeight(600.0)),
                ..Default::default()
            },
            "function" => HighlightStyle { color: color(c.code_syntax_function), ..Default::default() },
            "special" => HighlightStyle { color: color(c.code_syntax_special), ..Default::default() },
            other => unreachable!("未映射的高亮类别 {other}（SYNTAX_CATEGORIES 与本函数须同步）"),
        };
        (category.to_string(), style)
    });
    Arc::new(SyntaxTheme::new(styles))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme_system::Appearance;
    use zed_markdown::syntax::{Rope, language_for_tag};

    fn categories(tag: &str, src: &str) -> Vec<(String, &'static str)> {
        let lang = language_for_tag(tag).expect("应有高亮语言");
        let resolved = lang.highlight_text_resolved(&Rope::from(src), 0..src.len());
        resolved
            .runs
            .iter()
            .map(|(r, id)| (src[r.clone()].to_string(), SYNTAX_CATEGORIES[usize::from(*id)]))
            .collect()
    }

    #[test]
    fn rust_code_gets_v1_categories() {
        let runs = categories("rust", "fn main() {\n    let x = 42; // hi\n}\n");
        let has = |text: &str, cat: &str| runs.iter().any(|(t, c)| t == text && *c == cat);
        assert!(has("fn", "keyword"), "{runs:?}");
        assert!(has("let", "keyword"), "{runs:?}");
        assert!(has("42", "number"), "{runs:?}");
        assert!(has("// hi", "comment"), "{runs:?}");
        assert!(has("main", "function"), "{runs:?}");
        // 第二行的区间已换算为绝对偏移：`42` 在全文中的位置
        let lang = language_for_tag("rust").unwrap();
        let src = "fn main() {\n    let x = 42; // hi\n}\n";
        let resolved = lang.highlight_text_resolved(&Rope::from(src), 0..src.len());
        assert!(resolved.runs.iter().any(|(r, _)| &src[r.clone()] == "42" && r.start == src.find("42").unwrap()));
    }

    #[test]
    fn only_v1_languages_are_highlighted() {
        for tag in ["rust", "ts", "tsx", "python", "go", "json", "yaml", "cpp", "css", "html", "xml", "svg", "flow", "kotlin", "swift", "sql", "markdown"] {
            assert!(language_for_tag(tag).is_some(), "v1 有高亮的 {tag} 应被识别");
        }
        // v1（prism-react-renderer 默认集）无高亮 → 保持纯文本
        for tag in ["bash", "sh", "toml", "java", "ruby", "php", "lua", "text", "plain", "objc", "graphql"] {
            assert!(language_for_tag(tag).is_none(), "{tag} 在 v1 中无高亮，不应启用");
        }
    }

    #[test]
    fn syntax_theme_follows_category_order_and_v1_styles() {
        for appearance in [Appearance::Light, Appearance::Dark] {
            let theme = Theme::of(appearance);
            let st = syntax_theme(&theme);
            let keyword = st.get(6usize).unwrap();
            assert_eq!(keyword.color, Some(Hsla::from(theme.colors.code_syntax_keyword)));
            assert_eq!(keyword.font_weight, Some(FontWeight(600.0)));
            let comment = st.get(0usize).unwrap();
            assert_eq!(comment.font_style, Some(FontStyle::Italic));
            // 其余类别只改前景色
            for i in [1usize, 2, 3, 4, 5, 7, 8] {
                let s = st.get(i).unwrap();
                assert!(s.color.is_some() && s.font_weight.is_none() && s.font_style.is_none());
            }
        }
    }
}
