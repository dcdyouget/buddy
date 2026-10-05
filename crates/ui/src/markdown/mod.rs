//! Markdown 渲染接入
//!
//! 渲染器为 vendored zed markdown（`crates/markdown`，lib 名 `zed_markdown`，GPL-3.0-or-later，
//! 来源与 patch 清单见 `crates/markdown/VENDOR.md`）。本模块负责按 Buddy 令牌初始化它。
//!
//! # v1 消息排版的实际取值（`v1-final`）
//!
//! | 位置 | 字号 | 行高 | 出处 |
//! |------|------|------|------|
//! | 助手消息正文 | 14px（= `--font-size-md`） | **1.6**（写死，非令牌） | `MessageBubble.tsx:234-235` |
//! | 用户消息正文 | 14px | 1.5 | `MessageBubble.tsx:223-224` |
//! | 代码块 | 13px 等宽（= `--font-size-base`） | 1.5 | `CodeBlock.tsx:206-208` |

/// 源文本规范化（v1 `markdownNormalizer.ts` 的移植）。
///
/// **接入方式**：流式消息每批更新时对**完整文本**调用 [`normalize::normalize_markdown`]，再 `Markdown::replace`；
/// 不对增量片段 `append` —— 规范化需要看到完整行与成对定界符（v1 同样对整段文本规范化）。
/// 上游每次追加本就全量重解析，`replace` 不增加成本。
/// 显示与复制加粗文本时须 [`normalize::strip_guards`]（守卫为零宽空格，复制时会被带出）→ / 。
pub mod normalize;
pub mod code_block;
pub mod streaming;
pub mod reveal;
pub mod copy;
pub mod gfm;
pub use zed_markdown;

use crate::theme_system::{BuddyTheme, TextScale, Theme, fonts, tokens::metrics};
use gpui::{App, FontStyle, FontWeight, HighlightStyle, Hsla, Refineable, StyleRefinement, TextStyleRefinement, Window, px, relative};
use zed_markdown::{HeadingLevelStyles, MarkdownStyle};
use std::sync::Arc;
use theme::SyntaxTheme;
use zed_markdown::syntax::SYNTAX_CATEGORIES;

/// 安装 markdown 渲染所需的字体设置与复制快捷键。须在 [`crate::init_theme`] 之后调用。
///
/// 快捷键：上游依赖 zed 的键位表把 `cmd-c` 映射到 `Copy`；Buddy 没有该键位表，须自行绑定，
/// 否则选中文字后 Cmd+C 无反应。仅在 markdown 获得焦点（`Markdown` 上下文）时生效。
pub fn init(cx: &mut App) {
    cx.bind_keys([gpui::KeyBinding::new(COPY_KEYSTROKE, zed_markdown::Copy, Some("Markdown"))]);
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

/// 助手消息的 markdown 样式（v1 `.ai-message-content`）
///
/// 正文基础字体、代码块、GFM 元素（见 [`gfm`]）。使用时配合 [`code_block::renderer`]：
/// `MarkdownElement::new(md.clone(), message_style(window, cx)).code_block_renderer(code_block::renderer(md.downgrade(), streaming))`
pub fn message_style(window: &Window, cx: &App) -> MarkdownStyle {
    let theme = *cx.buddy_theme();
    let c = theme.colors;
    let body = TextScale::body(cx);
    let mut base_text_style = window.text_style();
    base_text_style.refine(&TextStyleRefinement {
        font_family: Some(fonts::ui_font(cx).family),
        font_features: Some(fonts::ui_font(cx).features),
        font_size: Some(px(body).into()),
        font_weight: Some(FontWeight(metrics::FONT_WEIGHT_REGULAR)),
        color: Some(c.text_primary.into()),
        // v1 `MessageBubble.tsx:234-235` 写死 1.6（非令牌）
        line_height: Some(relative(ASSISTANT_LINE_HEIGHT)),
        ..Default::default()
    });
    let heading = |level: u8| {
        Some(TextStyleRefinement {
            font_size: Some(px(body * gfm::heading_scale(level)).into()),
            font_weight: Some(FontWeight(gfm::heading_weight(level))),
            line_height: Some(relative(metrics::LINE_HEIGHT_TIGHT)),
            ..Default::default()
        })
    };
    MarkdownStyle {
        base_text_style,
        // 段落文字（`StyledText`）的字号取自外层元素继承的文本样式，`base_text_style`
        // 只决定字体与颜色；字号必须经根容器下发，否则设置页调整字号对正文无效。
        container_style: text_size_container(body),
        code_block: code_block::code_area_style(&theme, cx),
        code_block_overflow_x_scroll: true,
        // v1 `.markdown-inline-code`：淡色底 + 主色字、等宽。上游以圆角色块绘制底色
        inline_code: TextStyleRefinement {
            font_family: Some(fonts::mono_font(cx).family),
            font_features: Some(fonts::mono_font(cx).features),
            color: Some(c.markdown_accent_strong.into()),
            background_color: Some(c.markdown_accent_soft.into()),
            ..Default::default()
        },
        block_quote: TextStyleRefinement { color: Some(c.text_muted.into()), ..Default::default() },
        link: TextStyleRefinement { color: Some(c.markdown_accent_strong.into()), ..Default::default() },
        rule_color: c.markdown_accent_line.into(),
        block_quote_border_color: c.markdown_accent.into(),
        syntax: syntax_theme(&theme),
        // v1 无 `::selection` 规则，WebKit 用系统高亮色；v2 用品牌色（2026-09-27 用户决定）。
        // 上游把选区色块画在文字**之上**，必须半透明，否则遮住文字
        selection_background_color: Hsla::from(c.buddy_primary).opacity(SELECTION_ALPHA),
        heading_level_styles: Some(HeadingLevelStyles {
            h1: heading(1),
            h2: heading(2),
            h3: heading(3),
            h4: heading(4),
            h5: heading(5),
            h6: heading(6),
        }),
        // v1 `p { margin: 0 0 var(--space-2) 0 }`
        paragraph_spacing: px(metrics::SPACE_2),
        paragraph_line_height: relative(ASSISTANT_LINE_HEIGHT),
        table_columns_min_size: true,
        decorations: gfm::decorations(&theme, body),
        ..Default::default()
    }
}

/// 只携带字号的根容器样式，见 [`message_style`]。
fn text_size_container(size: f32) -> StyleRefinement {
    let mut style = StyleRefinement::default();
    style.text.font_size = Some(px(size).into());
    style
}

/// 复制快捷键（macOS Cmd+C；Windows Ctrl+C）
pub const COPY_KEYSTROKE: &str = if cfg!(target_os = "macos") { "cmd-c" } else { "ctrl-c" };

/// 选区色的不透明度（叠在文字上，需保证文字可读）
pub const SELECTION_ALPHA: f32 = 0.25;

/// 工具卡片详情的样式：代码块为紧凑变体（v1 `.tool-section .markdown-code-block pre`）
pub fn tool_detail_style(window: &Window, cx: &App) -> MarkdownStyle {
    let theme = *cx.buddy_theme();
    let mut style = message_style(window, cx);
    style.code_block = code_block::compact_code_area_style(&theme, cx);
    style
}

/// 思考块展开内容的样式：13px、弱化色（v1 `.think-section-content`：`--font-size-base`、`--text-muted`）
pub fn thinking_style(window: &Window, cx: &App) -> MarkdownStyle {
    let c = cx.buddy_theme().colors;
    let mut style = message_style(window, cx);
    let secondary = TextScale::secondary(cx);
    style.base_text_style.font_size = px(secondary).into();
    style.container_style = text_size_container(secondary);
    style.base_text_style.color = c.text_muted.into();
    style
}

/// 助手消息正文行高（v1 写死值，见模块文档表格）
pub const ASSISTANT_LINE_HEIGHT: f32 = 1.6;

/// 按当前主题构造代码高亮配色
///
/// 顺序必须与 [`SYNTAX_CATEGORIES`] 一致（`HighlightId` 即下标）。取值与 v1 `CodeBlock.tsx`
/// `buddyCodeTheme` 相同：只设前景色，**注释另加斜体、关键字另加字重 600**（v1 即如此）。
/// 代码块为等宽字体，字重 / 斜体不改变字符宽度与行高 → 不影响布局（测试 T03 验证）。
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
    fn rust_code_gets_v1_categories_via_generic_highlighting() {
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
    fn highlighted_languages() {
        // 内置语法（2026-10-04 用户决定只保留 Python / Shell / SQL）与通用高亮都会着色
        for tag in ["python", "py", "bash", "sh", "shell", "zsh", "sql", "rust", "ts", "tsx", "go", "json", "yaml", "cpp", "java", "kotlin", "swift", "xml", "objc", "graphql", "haskell"] {
            assert!(language_for_tag(tag).is_some(), "{tag} 应着色");
        }
        // 输出 / 日志 / 纯文本不着色
        for tag in ["text", "plain", "plaintext", "txt", "log", "output", "diff"] {
            assert!(language_for_tag(tag).is_none(), "{tag} 不应高亮");
        }
    }

    #[test]
    fn builtin_grammars_get_categories() {
        let runs = categories("bash", "echo \"hi\" # c\n");
        assert!(runs.iter().any(|(t, c)| t == "# c" && *c == "comment"), "{runs:?}");
        assert!(runs.iter().any(|(_, c)| *c == "string"), "{runs:?}");
        let runs = categories("python", "def f(x):\n    return 'a'  # n\n");
        assert!(runs.iter().any(|(t, c)| t == "def" && *c == "keyword"), "{runs:?}");
        assert!(runs.iter().any(|(t, c)| t == "# n" && *c == "comment"), "{runs:?}");
        let runs = categories("sql", "SELECT id FROM t WHERE n = 42;\n");
        assert!(runs.iter().any(|(t, c)| t.eq_ignore_ascii_case("select") && *c == "keyword"), "{runs:?}");
        // tree-sitter-sequel 把数字归入 literal（字符串色），沿用 起的现状
        assert!(runs.iter().any(|(t, _)| t == "42"), "{runs:?}");
    }

    #[test]
    fn generic_highlighting_covers_other_languages() {
        let runs = categories("toml", "[pkg]\nname = \"x\" # c\n");
        assert!(runs.iter().any(|(t, c)| t == "\"x\"" && *c == "string"), "{runs:?}");
        assert!(runs.iter().any(|(t, c)| t == "# c" && *c == "comment"), "{runs:?}");
        let runs = categories("java", "public class A { String s = \"q\"; }\n");
        assert!(runs.iter().any(|(t, c)| t == "class" && *c == "keyword"), "{runs:?}");
        assert!(runs.iter().any(|(t, c)| t == "String" && *c == "function"), "{runs:?}");
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
