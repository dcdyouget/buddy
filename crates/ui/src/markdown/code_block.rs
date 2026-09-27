//! 代码块渲染与复制（S04-07）—— 外观逐项取自 v1 `CodeBlock.tsx` 与 `global.css` 的 `.markdown-code-*`
//!
//! 结构（v1 同）：
//!
//! ```text
//! ┌ 容器：圆角 md、1px --code-border、--code-bg、inset 高光 + --shadow-static、上下外边距 space-2
//! │ ┌ 头部：padding space-1 × space-3、下边框、--code-header-bg
//! │ │  [语言标签（胶囊，纯文本块不显示）]              [复制图标 + 「复制」 / 「已复制」]
//! │ └
//! │ 代码：padding space-3、等宽 13px、行高 1.5（纯文本块 1.75）、横向滚动不换行
//! └
//! ```
//!
//! 渲染器经 vendored markdown 的 `CodeBlockRenderer::Custom`（上游本 rev 未接通，S04-07 补丁接通，
//! 见 `crates/markdown/VENDOR.md`）：本模块构造外层容器与头部，代码行由 markdown 作为其子节点追加。

use super::zed_markdown::{
    CodeBlockRenderer, Markdown, ParsedMarkdown,
    parser::{CodeBlockKind, CodeBlockMetadata, MarkdownEvent, MarkdownTag, MarkdownTagEnd},
};
use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, Theme, box_shadows, fonts, tokens::metrics as m};
use gpui::{
    App, ClipboardItem, Div, EntityId, FontWeight, Global, Hsla, SharedString, StyleRefinement,
    TextStyleRefinement, WeakEntity, div, prelude::*, px, relative,
};
use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

/// v1 视为「纯文本」的语言名：不显示语言标签、行高 1.75、不高亮
const PLAIN_LANGUAGES: [&str; 4] = ["plain", "plaintext", "text", "txt"];

/// 「已复制」反馈时长（v1 `setTimeout(..., 2000)`）
const COPIED_FEEDBACK: Duration = Duration::from_secs(2);

/// v1 的语言标识：信息串首词的 `[\w-]+` 前缀（`/language-([\w-]+)/`），缺省为 `text`
///
/// react-markdown 把信息串首词写成 `class="language-<词>"`，v1 再用上述正则取出；
/// JS 的 `\w` 只含 ASCII 字母数字与下划线。缩进代码块无信息串 → `text`。
pub fn v1_language(source: &str, block_range: &Range<usize>, kind: &CodeBlockKind) -> String {
    if matches!(kind, CodeBlockKind::Indented) {
        return "text".into();
    }
    let block = &source[block_range.clone()];
    let first_line = block.split('\n').next().unwrap_or_default();
    let info = first_line.trim_start().trim_start_matches(['`', '~']);
    let word: String = info
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if word.is_empty() { "text".into() } else { word }
}

/// 是否纯文本块（v1 `isPlainText`，大小写不敏感）
pub fn is_plain_language(language: &str) -> bool {
    PLAIN_LANGUAGES.contains(&language.to_lowercase().as_str())
}

/// 复制内容：与 v1 相同 —— 代码块的文本内容（CommonMark 语义：已去掉列表缩进与围栏），
/// 再去掉**一个**末尾换行（v1 `String(children).replace(/\n$/, '')`）
///
/// 从解析事件拼接而不是切源码：嵌在列表里的代码块，源码每行带列表缩进，切片会把缩进一并复制。
pub fn code_text(parsed: &ParsedMarkdown, block_range: &Range<usize>) -> String {
    let source = parsed.source();
    let mut text = String::new();
    let mut inside = false;
    for (range, event) in parsed.events().iter() {
        match event {
            MarkdownEvent::Start(MarkdownTag::CodeBlock { .. }) if range == block_range => inside = true,
            MarkdownEvent::End(MarkdownTagEnd::CodeBlock) if inside => break,
            MarkdownEvent::Text if inside => text.push_str(&source[range.clone()]),
            MarkdownEvent::SubstitutedText(s) if inside => text.push_str(s),
            _ => {}
        }
    }
    if text.ends_with('\n') {
        text.pop();
    }
    text
}

/// 处于「已复制」反馈期的代码块：(markdown 实体, 代码块起点)
#[derive(Default)]
struct CopiedCodeBlocks(HashSet<(EntityId, usize)>);

impl Global for CopiedCodeBlocks {}

fn is_copied(markdown: EntityId, block_start: usize, cx: &App) -> bool {
    cx.try_global::<CopiedCodeBlocks>().is_some_and(|c| c.0.contains(&(markdown, block_start)))
}

fn copy(markdown: WeakEntity<Markdown>, block_start: usize, code: String, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(code));
    let key = (markdown.entity_id(), block_start);
    cx.default_global::<CopiedCodeBlocks>().0.insert(key);
    let _ = markdown.update(cx, |_, cx| cx.notify());
    cx.spawn(async move |cx| {
        cx.background_executor().timer(COPIED_FEEDBACK).await;
        cx.update(|cx| {
            cx.default_global::<CopiedCodeBlocks>().0.remove(&key);
            let _ = markdown.update(cx, |_, cx| cx.notify());
        });
    })
    .detach();
}

/// 代码区（v1 `<pre>`）样式，填入 `MarkdownStyle::code_block`
///
/// 行高不在此设置：纯文本块与普通块不同（1.75 / 1.5），由外层容器按块设置、代码区继承。
pub fn code_area_style(theme: &Theme, cx: &App) -> StyleRefinement {
    let c = theme.colors;
    let mut style = StyleRefinement::default().p(px(m::SPACE_3)).bg(c.code_bg);
    style.text = TextStyleRefinement {
        font_family: Some(fonts::mono_font(cx).family),
        font_features: Some(fonts::mono_font(cx).features),
        font_size: Some(px(m::FONT_SIZE_BASE).into()),
        color: Some(c.code_text.into()),
        ..Default::default()
    };
    style
}

/// 构造代码块渲染器。`markdown` 为被渲染的实体（复制反馈需要通知它重绘）。
pub fn renderer(markdown: WeakEntity<Markdown>) -> CodeBlockRenderer {
    CodeBlockRenderer::Custom {
        render: Arc::new(move |kind, parsed, range, _metadata: CodeBlockMetadata, _window, cx| {
            render_container(&markdown, kind, parsed, range, cx)
        }),
        transform: None,
    }
}

fn render_container(
    markdown: &WeakEntity<Markdown>,
    kind: &CodeBlockKind,
    parsed: &ParsedMarkdown,
    range: Range<usize>,
    cx: &App,
) -> Div {
    let theme = cx.buddy_theme();
    let c = theme.colors;
    let language = v1_language(parsed.source(), &range, kind);
    let plain = is_plain_language(&language);
    let info = |alpha: f32| Hsla::from(c.state_info).opacity(alpha);

    let mut shadows = vec![gpui::BoxShadow {
        color: c.surface_highlight.into(),
        offset: gpui::point(px(0.), px(1.)),
        blur_radius: px(0.),
        spread_radius: px(0.),
        inset: true,
    }];
    shadows.extend(box_shadows(theme.shadows.shadow_static));

    let label = (!plain).then(|| {
        div()
            .px(px(m::SPACE_2))
            .py(px(1.))
            .border_1()
            .border_color(c.code_border)
            .rounded(px(m::RADIUS_FULL))
            .bg(info(0.07))
            .text_color(c.code_syntax_keyword)
            .font_family(fonts::mono_font(cx).family)
            .text_size(px(m::FONT_SIZE_XS))
            .line_height(relative(m::LINE_HEIGHT_BASE))
            .child(SharedString::from(language.to_lowercase()))
    });

    let copied = is_copied(markdown.entity_id(), range.start, cx);
    let code = code_text(parsed, &range);
    let block_start = range.start;
    let copy_button = div()
        .id(("buddy-code-copy", block_start))
        .flex()
        .items_center()
        .gap(px(m::SPACE_1))
        .px(px(m::SPACE_2))
        .py(px(2.))
        .rounded(px(m::RADIUS_SM))
        .cursor_pointer()
        .text_size(px(m::FONT_SIZE_SM))
        .line_height(relative(m::LINE_HEIGHT_BASE))
        .font_weight(FontWeight(m::FONT_WEIGHT_REGULAR))
        .text_color(if copied { c.state_success } else { c.text_muted })
        .when(!copied, |b| b.hover(|s| s.text_color(c.code_syntax_keyword).bg(info(0.08))))
        .child(icon(if copied { IconName::Check } else { IconName::Copy }, px(12.)))
        .child(if copied { "已复制" } else { "复制" })
        .on_click({
            let markdown = markdown.clone();
            move |_, _, cx| copy(markdown.clone(), block_start, code.clone(), cx)
        });

    let header = div()
        .flex()
        .items_center()
        .when_else(label.is_some(), |h| h.justify_between(), |h| h.justify_end())
        .px(px(m::SPACE_3))
        .py(px(m::SPACE_1))
        .border_b_1()
        .border_color(c.code_border)
        .bg(c.code_header_bg)
        .children(label)
        .child(copy_button);

    div()
        .w_full()
        .my(px(m::SPACE_2))
        .rounded(px(m::RADIUS_MD))
        .overflow_hidden()
        .border_1()
        .border_color(c.code_border)
        .bg(c.code_bg)
        .shadow(shadows)
        .line_height(relative(if plain { 1.75 } else { m::LINE_HEIGHT_BASE }))
        .child(header)
}
