//! 思考块—— 对应 v1 `src/components/chat/ThinkSection.tsx` 与 `global.css` `.think-section*`
//!
//! | v1 | 本模块 |
//! |----|------|
//! | 默认折叠；用户点过后保持用户的选择 | 展开状态按行 id 记在列表中 |
//! | 流式中：「正在思考」+ 三点加载 + 最新思考的末尾 96 字；完成后：「思考过程」+ 第一行（最多 80 字） | [`preview`] |
//! | 左 2px 强调色边、其余 1px 边框、右侧圆角 md、`--panel-surface` 底；标题栏最小高 28px、悬停凹陷底 | 同 |
//! | 流式中一道光泽 3.4s 循环扫过；加载点 1.2s 依次跳动 | 同；减弱动效时静止 |
//! | 展开后 markdown 显示思考内容（13px、弱化色）；无内容时「等待思考内容...」 | 同 |

use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, easing::cubic_bezier, tokens::{metrics as m, motion}};
use gpui::{AnyElement, App, ClickEvent, FontWeight, Hsla, SharedString, Window, div, linear_color_stop, linear_gradient, prelude::*, px, relative};

/// 折叠态预览（v1 `firstLinePreview` / `latestContentPreview`）
pub fn preview(content: &str, streaming: bool) -> String {
    if streaming {
        let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
        let chars: Vec<char> = normalized.chars().collect();
        if chars.len() <= 96 { normalized } else { format!("…{}", chars[chars.len() - 96..].iter().collect::<String>()) }
    } else {
        let line = content.split('\n').next().unwrap_or_default();
        // v1 按 UTF-16 长度截 80；这里按字符（中文场景一致，仅代理对字符有差异）
        let chars: Vec<char> = line.chars().collect();
        if chars.len() > 80 { format!("{}…", chars[..80].iter().collect::<String>()) } else { line.to_string() }
    }
}

/// 三点加载第 `i` 个点在 `ms` 时刻的（不透明度, 上移像素）：v1 `think-loading-dot`，第 i 个延迟 i × 周期 / 6
fn dot(ms: f64, i: usize) -> (f32, f32) {
    let period = f64::from(motion::DURATION_THINKING_LOADER);
    let t = (((ms - i as f64 * period / 6.0).rem_euclid(period)) / period) as f32;
    let ease = cubic_bezier(motion::EASE_STANDARD);
    // 0% / 60% / 100%：0.3、0；30%：1、-space-1/2
    let k = if t < 0.3 { ease(t / 0.3) } else if t < 0.6 { 1.0 - ease((t - 0.3) / 0.3) } else { 0.0 };
    (0.3 + 0.7 * k, -(m::SPACE_1 / 2.0) * k)
}

/// 光泽位置（相对容器宽度的横向偏移比例）：v1 `think-section-sheen` 0–18% 在 -120%，72–100% 在 120%
fn sheen(ms: f64) -> f32 {
    let period = f64::from(motion::DURATION_THINKING_SHEEN);
    let t = ((ms.rem_euclid(period)) / period) as f32;
    let ease = cubic_bezier(motion::EASE_STANDARD);
    let k = if t <= 0.18 { 0.0 } else if t >= 0.72 { 1.0 } else { ease((t - 0.18) / 0.54) };
    -1.2 + 2.4 * k
}

/// 三点加载（思考块与网络搜索块共用）
pub(crate) fn loader(accent: gpui::Rgba, now_ms: f64, reduce: bool) -> gpui::Div {
    div().min_w(px(m::SPACE_4)).h(px(m::SPACE_3)).flex().items_center().justify_center().gap(px(m::SPACE_1 / 2.0)).children((0..3).map(|i| {
        let (opacity, dy) = if reduce { (0.32, 0.0) } else { dot(now_ms, i) };
        div().size(px(m::SPACE_1 / 1.5)).mt(px(dy)).rounded(px(m::RADIUS_FULL)).bg(Hsla::from(accent).opacity(opacity))
    }))
}

/// 流式光泽层：两段渐变拼成「透明 → 高光 → 透明」（GPUI 渐变只有两个色标）
pub(crate) fn sheen_layer(highlight: gpui::Rgba, now_ms: f64) -> gpui::Div {
    let hi = Hsla::from(highlight).opacity(0.34);
    let clear = Hsla::from(highlight).opacity(0.0);
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .w_full()
        .left(relative(sheen(now_ms)))
        .flex()
        .child(div().w(relative(0.22)))
        .child(div().w(relative(0.26)).h_full().bg(linear_gradient(105., linear_color_stop(clear, 0.), linear_color_stop(hi, 1.))))
        .child(div().w(relative(0.30)).h_full().bg(linear_gradient(105., linear_color_stop(hi, 0.), linear_color_stop(clear, 1.))))
}

/// 渲染思考块。`content_view` 为展开时的内容（markdown 元素）；`since_ms` 为动画时钟。
#[allow(clippy::too_many_arguments)]
pub fn think_block(
    id: SharedString,
    content: &str,
    streaming: bool,
    expanded: bool,
    content_view: Option<AnyElement>,
    now_ms: f64,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &App,
) -> AnyElement {
    let c = cx.buddy_theme().colors;
    let reduce = crate::accessibility::prefers_reduced_motion();
    if streaming && !reduce {
        window.request_animation_frame();
    }
    let preview_text = preview(content, streaming);
    let has_preview = !preview_text.is_empty();
    let loader = streaming.then(|| loader(c.tool_ui_accent, now_ms, reduce));
    let header = div()
        .id(id.clone())
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .w_full()
        .px(px(m::SPACE_2))
        .py(px(m::SPACE_1))
        .min_h(px(28.0))
        .text_size(px(m::FONT_SIZE_SM))
        .text_color(c.text_muted)
        .cursor_pointer()
        .hover(|s| s.bg(c.bg_sunken))
        .on_click(on_toggle)
        .child(div().flex_none().text_color(c.tool_ui_accent).child(icon(IconName::Brain, px(14.0))))
        .child(div().flex_none().font_weight(FontWeight(600.0)).child(if streaming { "正在思考" } else { "思考过程" }))
        .children(loader)
        .when(!expanded && has_preview, |d| {
            d.child(div().flex_1().min_w_0().ml(px(4.0)).truncate().text_size(px(m::FONT_SIZE_XS)).child(SharedString::from(preview_text)))
        })
        .when(expanded || !has_preview, |d| d.child(div().flex_1()))
        .child(div().flex_none().child(icon(if expanded { IconName::ChevronDown } else { IconName::ChevronRight }, px(13.0))));

    let body = expanded.then(|| {
        div()
            .w_full()
            .px(px(m::SPACE_2))
            .pb(px(m::SPACE_2))
            .border_t_1()
            .border_color(c.border_subtle)
            .text_size(px(m::FONT_SIZE_BASE))
            .text_color(c.text_muted)
            .child(match content_view {
                Some(view) if !content.is_empty() => view,
                _ => div().italic().child("等待思考内容...").into_any_element(),
            })
    });

    let sheen_layer = (streaming && !reduce).then(|| sheen_layer(c.tool_ui_flow_highlight, now_ms));

    div()
        .relative()
        .w_full()
        .overflow_hidden()
        .border_1()
        .border_color(c.border_default)
        .rounded_tr(px(m::RADIUS_MD))
        .rounded_br(px(m::RADIUS_MD))
        .bg(c.panel_surface)
        .children(sheen_layer)
        // 左侧 2px 强调色边（GPUI 边框四边同色 → 覆盖在左边框上的竖条）
        .child(div().absolute().left_0().top_0().bottom_0().w(px(2.0)).bg(c.tool_ui_accent))
        .child(div().relative().child(header).children(body))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_match_v1() {
        assert_eq!(preview("第一行\n第二行", false), "第一行");
        let long: String = "字".repeat(90);
        assert_eq!(preview(&long, false), format!("{}…", "字".repeat(80)));
        assert_eq!(preview("  a \n b  ", true), "a b");
        let stream: String = (0..100).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
        let p = preview(&stream, true);
        assert!(p.starts_with('…') && p.chars().count() == 97);
    }

    #[test]
    fn animations_follow_v1_keyframes() {
        // 加载点：0 时刻第 0 个点在谷底，30% 处到顶
        assert_eq!(dot(0.0, 0), (0.3, 0.0));
        let (o, dy) = dot(f64::from(motion::DURATION_THINKING_LOADER) * 0.3, 0);
        assert!((o - 1.0).abs() < 1e-4 && (dy + m::SPACE_1 / 2.0).abs() < 1e-4);
        // 光泽：18% 前停在左外，72% 后停在右外
        assert_eq!(sheen(0.0), -1.2);
        assert_eq!(sheen(f64::from(motion::DURATION_THINKING_SHEEN) * 0.8), 1.2);
    }
}
