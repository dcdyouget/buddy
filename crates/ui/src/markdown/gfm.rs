//! GFM 元素的 v1 外观（S04-08）—— 逐项取自 `v1-final:src/styles/global.css` 的 `.ai-message-content *`
//!
//! 经 vendored markdown 的 `MarkdownDecorations`（S04-08 补丁，见 `crates/markdown/VENDOR.md`）在上游样式之后覆盖。
//!
//! | 元素 | v1 | 这里 |
//! |------|----|------|
//! | h1–h3 | 左侧 3px 渐变竖条、padding-left space-3；h1 另有底部渐隐线 | 同（竖条 / 底线为绝对定位子元素） |
//! | 无序列表 | 4px 圆点 + 3px 淡色光圈，位于文字左侧 12px | 同（项目符号列宽 20px + 间距 4px = v1 的 24px 缩进） |
//! | 有序列表 | 16px 圆形序号徽章，等宽 11px | 同 |
//! | 任务列表 | 原生禁用复选框，`accent-color` | 自绘 13px 复选框（**目检**） |
//! | 引用 | 左 3px 主色边 + 其余 1px 淡边、135° 渐变底、文字弱化 | 同（左边为覆盖在边框上的竖条） |
//! | 表格 | 外框圆角 md、表头淡底主色字、单元格右 / 下细线 | 同（上游用左 / 上细线，视觉等价） |
//! | 分隔线 | 两端渐隐的 1px 线 | 同（两段各 50% 的渐变） |
//!
//! GPUI 做不到、已知有差距的见 S04-08 决策记录（行内代码边框 / 字号、链接悬停下划线、表格行悬停）。

use super::zed_markdown::{ListBulletKind, MarkdownDecorations, TableCellInfo};
use crate::icons::{IconName, icon};
use crate::theme_system::{Theme, fonts, tokens::{Palette, metrics as m}};
use gpui::{
    AnyElement, App, BoxShadow, Div, FontWeight, Hsla, Rgba, StrikethroughStyle, TextStyleRefinement, div, linear_color_stop,
    linear_gradient, point, prelude::*, px, relative,
};
use std::sync::Arc;

/// 项目符号列宽：与上游列表项的 4px 间距相加 = v1 `ul/ol { padding-left: var(--space-6) }`
const BULLET_COLUMN: f32 = m::SPACE_6 - m::SPACE_1;

/// 标题字号相对正文的倍数（v1 `h1 1.25em / h2 1.15em / h3 1.05em`，h4–h6 不改）
pub fn heading_scale(level: u8) -> f32 {
    match level {
        1 => 1.25,
        2 => 1.15,
        3 => 1.05,
        _ => 1.0,
    }
}

/// 标题字重（v1：h1–h3 `--font-weight-heading`，h4–h6 `--font-weight-emphasis`）
pub fn heading_weight(level: u8) -> f32 {
    if level <= 3 { m::FONT_WEIGHT_HEADING } else { m::FONT_WEIGHT_EMPHASIS }
}

/// 某色的透明版本：CSS 渐变到 `transparent` 时浏览器按预乘插值，等价于同色 alpha → 0
fn clear(c: Rgba) -> Hsla {
    Hsla::from(c).opacity(0.0)
}

/// `top` 以其 alpha 叠在不透明的 `bottom` 上（CSS 多层背景的合成）
fn over(top: Rgba, bottom: Rgba) -> Rgba {
    let a = top.a;
    let mut out = bottom;
    out.r = top.r * a + bottom.r * (1.0 - a);
    out.g = top.g * a + bottom.g * (1.0 - a);
    out.b = top.b * a + bottom.b * (1.0 - a);
    out.a = 1.0;
    out
}

/// 助手消息的 GFM 装饰
///
/// 闭包捕获构造时的配色；消息样式每帧按当前主题重建（[`super::message_style`]），切换主题无需额外处理。
pub fn decorations(theme: &Theme) -> MarkdownDecorations {
    let c = theme.colors;
    MarkdownDecorations {
        heading: Some(Arc::new(move |heading: Div, level, _| {
            let em = m::FONT_SIZE_MD * heading_scale(level);
            let heading = heading.relative().mt(px(m::SPACE_3)).mb(px(m::SPACE_2)).text_size(px(em));
            if level > 3 {
                return heading;
            }
            let bar = div()
                .absolute()
                .left_0()
                .top(px(em * 0.14))
                .bottom(px(em * 0.12))
                .w(px(3.))
                .rounded(px(m::RADIUS_FULL))
                .bg(linear_gradient(180., linear_color_stop(c.markdown_accent, 0.), linear_color_stop(c.markdown_accent_line, 1.)));
            let heading = heading.pl(px(m::SPACE_3)).child(bar);
            if level != 1 {
                return heading;
            }
            let underline = div()
                .absolute()
                .left_0()
                .bottom_0()
                .w_full()
                .h(px(1.))
                .bg(linear_gradient(90., linear_color_stop(c.markdown_accent_line, 0.), linear_color_stop(clear(c.markdown_accent_line), 0.72)));
            heading.pb(px(m::SPACE_2)).child(underline)
        })),
        block_quote: Some(Arc::new(move |quote: Div, _| {
            let tint = over(c.markdown_accent_soft, c.bg_sunken);
            let left_bar = div()
                .absolute()
                .left(px(-3.))
                .top(px(-1.))
                .bottom(px(-1.))
                .w(px(3.))
                .rounded_l(px(m::RADIUS_SM))
                .bg(c.markdown_accent);
            quote
                .relative()
                .mt(px(m::SPACE_2))
                .mb(px(m::SPACE_2))
                // v1 `blockquote > :last-child { margin-bottom: 0 }`：段落自带下边距 space-2，此处底部留白由它提供
                .pt(px(m::SPACE_2))
                .pb_0()
                .pr(px(m::SPACE_3))
                .pl(px(m::SPACE_4))
                .border_1()
                .border_l(px(3.))
                .border_color(c.markdown_accent_medium)
                .rounded_tl(px(m::RADIUS_SM))
                .rounded_bl(px(m::RADIUS_SM))
                .rounded_tr(px(m::RADIUS_MD))
                .rounded_br(px(m::RADIUS_MD))
                .bg(linear_gradient(135., linear_color_stop(tint, 0.), linear_color_stop(c.bg_sunken, 0.7)))
                .child(left_bar)
        })),
        list: Some(Arc::new(move |list: Div, top_level, _| {
            let list = list.pl_0();
            if top_level { list.mt(px(m::SPACE_2)).mb(px(m::SPACE_2)) } else { list.mt(px(m::SPACE_1)) }
        })),
        list_bullet: Some(Arc::new(move |kind, cx| bullet(kind, c, cx))),
        table: Some(Arc::new(move |table: Div, _| {
            table
                .w_full()
                .mt(px(m::SPACE_3))
                .mb(px(m::SPACE_3))
                .border_1()
                .border_color(c.border_default)
                .rounded(px(m::RADIUS_MD))
                .overflow_hidden()
                .bg(c.bg_elevated)
                .text_size(px(m::FONT_SIZE_BASE))
        })),
        table_cell: Some(Arc::new(move |cell: Div, info: TableCellInfo, _| {
            let cell = cell.px(px(m::SPACE_3)).py(px(m::SPACE_2)).border_color(c.border_subtle);
            if info.is_header { cell.bg(c.markdown_accent_soft) } else { cell.bg(Hsla::transparent_black()) }
        })),
        rule: Some(Arc::new(move |_| {
            let half = |from: Hsla, to: Hsla| div().flex_1().h_full().bg(linear_gradient(90., linear_color_stop(from, 0.), linear_color_stop(to, 1.)));
            let line: Hsla = c.markdown_accent_line.into();
            div()
                .flex()
                .w_full()
                .h(px(1.))
                .my(px(m::SPACE_4))
                .child(half(clear(c.markdown_accent_line), line))
                .child(half(line, clear(c.markdown_accent_line)))
                .into_any_element()
        })),
        strong: Some(TextStyleRefinement { font_weight: Some(FontWeight(m::FONT_WEIGHT_EMPHASIS)), ..Default::default() }),
        strikethrough: Some(TextStyleRefinement {
            strikethrough: Some(StrikethroughStyle { thickness: px(1.), color: None }),
            // v1 `del { opacity: 0.7 }`：文本运行没有不透明度，改为前景色 70%
            color: Some(Hsla::from(c.text_primary).opacity(0.7)),
            ..Default::default()
        }),
        table_head: Some(TextStyleRefinement {
            font_weight: Some(FontWeight(m::FONT_WEIGHT_EMPHASIS)),
            color: Some(c.markdown_accent_strong.into()),
            ..Default::default()
        }),
        // 与 v1（WebKit）选区纯文本一致，并去掉加粗守卫（S04-09）
        copy_text: Some(Arc::new(super::copy::copy_text)),
        // 流式渐显与星标按帧设置（S04-06，见 markdown::streaming）
        veil: None,
        overlay: None,
    }
}

fn bullet(kind: ListBulletKind, c: &'static Palette, cx: &App) -> AnyElement {
    let line = m::FONT_SIZE_MD; // 1em
    let column = div().flex_none().w(px(BULLET_COLUMN)).relative();
    match kind {
        ListBulletKind::Unordered => column
            .child(
                div()
                    .absolute()
                    .left(px(m::SPACE_6 - m::SPACE_3))
                    .top(px(line * 0.72 - m::SPACE_1 / 2.))
                    .size(px(m::SPACE_1))
                    .rounded(px(m::RADIUS_FULL))
                    .bg(c.markdown_accent)
                    .shadow(vec![BoxShadow {
                        color: c.markdown_accent_soft.into(),
                        offset: point(px(0.), px(0.)),
                        blur_radius: px(0.),
                        spread_radius: px(3.),
                        inset: false,
                    }]),
            )
            .into_any_element(),
        ListBulletKind::Ordered(n) => column
            .flex()
            .justify_end()
            // 徽章右缘距文字 space-2（v1 `right: calc(100% + var(--space-2))`），扣除上游 4px 间距
            .pr(px(m::SPACE_2 - m::SPACE_1))
            .pt(px(line * 0.12))
            .child(
                div()
                    .min_w(px(m::SPACE_4))
                    .h(px(m::SPACE_4))
                    .px(px(3.))
                    .rounded(px(m::RADIUS_FULL))
                    .bg(c.markdown_accent_soft)
                    .text_color(c.markdown_accent_strong)
                    .font_family(fonts::mono_font(cx).family)
                    .font_weight(FontWeight(m::FONT_WEIGHT_EMPHASIS))
                    .text_size(px(m::FONT_SIZE_XS))
                    .line_height(px(m::SPACE_4))
                    .flex()
                    .justify_center()
                    .child(n.to_string()),
            )
            .into_any_element(),
        ListBulletKind::Task { checked } => {
            // v1：`li.task-list-item` 不画圆点，复选框在文字前、右边距 space-2 → 列宽加宽，文字右移
            let size = 13.;
            let boxed = div()
                .size(px(size))
                .mt(px(3.))
                .rounded(px(m::RADIUS_SM))
                .border_1()
                .flex()
                .items_center()
                .justify_center()
                .when_else(
                    checked,
                    |b| b.bg(c.markdown_accent).border_color(c.markdown_accent).text_color(c.text_on_primary).child(icon(IconName::Check, px(10.))),
                    |b| b.bg(c.bg_surface).border_color(c.border_strong),
                );
            div()
                .flex_none()
                .w(px(BULLET_COLUMN + size + m::SPACE_2))
                .pl(px(BULLET_COLUMN + m::SPACE_1))
                .line_height(relative(1.0))
                .child(boxed)
                .into_any_element()
        }
    }
}

/// 链接点击时实际打开的地址（v1 `normalizeHref` + `isExternalUrl`）
///
/// - `#锚点` → `None`（v1 为页内跳转；消息内无锚点目标，不打开任何东西）
/// - 已带 scheme（`https:` / `mailto:` …）→ 原样
/// - `//host` → `https://host`；无 scheme → 补 `https://`
pub fn link_target(href: &str) -> Option<String> {
    if href.starts_with('#') {
        return None;
    }
    let has_scheme = href
        .split_once(':')
        .is_some_and(|(scheme, _)| scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c)));
    Some(if href.is_empty() || has_scheme {
        href.to_string()
    } else if let Some(rest) = href.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        format!("https://{href}")
    })
}

/// markdown 图片的来源（`MarkdownElement::image_resolver`）
///
/// 只加载 `http(s)` 网络图片（需 [`crate::http::install`]），失败时显示上游的失败占位；
/// `data:` 图片由上游自行解码。其他地址（相对路径、`file:`）v1 的 webview 中同样无法显示 → 不加载。
pub fn image_source(url: &str) -> Option<gpui::ImageSource> {
    let lower = url.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://")).then(|| gpui::ImageSource::Resource(gpui::Resource::Uri(url.to_string().into())))
}

/// 用系统默认应用打开链接（v1 经 Tauri shell `open`）
pub fn open_link(href: &str, cx: &mut App) {
    if let Some(url) = link_target(href).filter(|u| !u.is_empty()) {
        cx.open_url(&url);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_target_matches_v1_normalize_href() {
        assert_eq!(link_target("#section"), None);
        assert_eq!(link_target("https://v2.tauri.app/").as_deref(), Some("https://v2.tauri.app/"));
        assert_eq!(link_target("mailto:a@b.c").as_deref(), Some("mailto:a@b.c"));
        assert_eq!(link_target("//cdn.example.com/x").as_deref(), Some("https://cdn.example.com/x"));
        assert_eq!(link_target("example.com/a?b=1").as_deref(), Some("https://example.com/a?b=1"));
        // v1 正则 `^[a-z][a-z0-9+.-]*:`：数字开头不算 scheme
        assert_eq!(link_target("1password:x").as_deref(), Some("https://1password:x"));
        assert_eq!(link_target("localhost:3000").as_deref(), Some("localhost:3000"));
    }

    #[test]
    fn only_network_images_are_resolved() {
        assert!(image_source("https://example.com/a.png").is_some());
        assert!(image_source("HTTP://example.com/a.png").is_some());
        assert!(image_source("file:///etc/passwd").is_none());
        assert!(image_source("images/a.png").is_none());
    }

    #[test]
    fn helpers() {
        assert_eq!(heading_scale(1), 1.25);
        assert_eq!(heading_scale(4), 1.0);
        assert_eq!(heading_weight(3), m::FONT_WEIGHT_HEADING);
        assert_eq!(heading_weight(4), m::FONT_WEIGHT_EMPHASIS);
        let p = Theme::of(crate::theme_system::Appearance::Light).colors;
        let (top, bottom) = (p.markdown_accent_soft, p.bg_sunken);
        assert!(top.a > 0.0 && top.a < 1.0, "soft 应为半透明令牌");
        let c = over(top, bottom);
        let mix = |t: f32, b: f32| t * top.a + b * (1.0 - top.a);
        assert!((c.r - mix(top.r, bottom.r)).abs() < 1e-6 && (c.b - mix(top.b, bottom.b)).abs() < 1e-6 && c.a == 1.0);
    }
}
