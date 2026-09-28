//! 工具调用卡片（S05-10）—— 对应 v1 `ToolSection.tsx` 与 `global.css` 的 `.tool-section*` / `.tool-status-badge*` / `.tool-detail-*`
//!
//! | v1 | 本模块 |
//! |----|------|
//! | 状态：准备中 / 执行中（旋转）/ 已完成 / 失败 / 已中断；等待回答时显示「等待回答」 | [`status_meta`] |
//! | 工具名 → 中文名与图标（读取文件、浏览目录……其余「调用工具」） | [`tool_meta`] |
//! | 折叠态摘要：ask_user 取问题，其余依次取参数 path / command / query / url / name | [`action_summary`] |
//! | 默认展开：等待回答，或流式中且准备中 / 执行中；用户点过则保持用户选择 | [`default_expanded`] |
//! | 展开：「调用参数」（格式化 JSON）+ 有结果时「执行结果」/「执行错误」（无内容时「(无返回内容)」） | 紧凑代码块，最高 180px 卡内滚动 |
//! | 卡片：圆角 lg、左侧 3px 强调色（inset 阴影）、135° 淡强调色渐变叠 `--panel-surface`；悬停 / 展开时边框带强调色 | 同 |
//!
//! 网络搜索（S05-11）、图片生成（S05-12）、提问卡（S05-13）有专门的卡片；未完成前按通用卡片显示。

use super::state::{ToolStatus, ToolView};
use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, box_shadows, fonts, tokens::metrics as m};
use gpui::{
    AnyElement, App, BoxShadow, ClickEvent, FontWeight, Hsla, Rgba, ScrollHandle, SharedString, Transformation, Window, div,
    linear_color_stop, linear_gradient, point, prelude::*, px,
};

/// 状态徽标的色调
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// 中性
    Neutral,
    /// 信息
    Info,
    /// 成功
    Success,
    /// 错误
    Error,
}

/// v1 `getStatusMeta`（等待回答时由调用方覆盖）
pub fn status_meta(status: ToolStatus, awaiting_answer: bool) -> (&'static str, IconName, Tone, bool) {
    if awaiting_answer {
        return ("等待回答", IconName::CircleHelp, Tone::Info, false);
    }
    match status {
        ToolStatus::Executing => ("执行中", IconName::LoaderCircle, Tone::Info, true),
        ToolStatus::Done => ("已完成", IconName::CircleCheck, Tone::Success, false),
        ToolStatus::Error => ("失败", IconName::CircleX, Tone::Error, false),
        ToolStatus::Interrupted => ("已中断", IconName::CircleX, Tone::Neutral, false),
        ToolStatus::Calling => ("准备中", IconName::CircleDashed, Tone::Neutral, false),
    }
}

/// v1 `getToolMeta`
pub fn tool_meta(name: &str) -> (&'static str, IconName) {
    match name {
        "read_file" => ("读取文件", IconName::FileText),
        "list_directory" => ("浏览目录", IconName::FolderTree),
        "search_files" => ("搜索文件", IconName::Search),
        "create_file" => ("创建文件", IconName::FilePlus),
        "overwrite_file" => ("覆盖文件", IconName::FilePenLine),
        "append_file" => ("追加文件", IconName::FileOutput),
        "edit_file" => ("编辑文件", IconName::FileDiff),
        "ask_user" => ("询问用户", IconName::CircleHelp),
        _ => ("调用工具", IconName::Wrench),
    }
}

/// v1 `actionSummary`
pub fn action_summary(name: &str, arguments: &str) -> String {
    let Ok(args) = serde_json::from_str::<serde_json::Value>(arguments) else { return String::new() };
    if name == "ask_user" {
        return args.get("question").and_then(|q| q.as_str()).filter(|q| !q.is_empty()).unwrap_or("等待用户回答").to_string();
    }
    ["path", "command", "query", "url", "name"]
        .iter()
        .find_map(|k| args.get(*k).and_then(|v| v.as_str()).filter(|v| !v.trim().is_empty()))
        .unwrap_or_default()
        .to_string()
}

/// v1 `prettyArgs`：两空格缩进的 JSON；解析失败原样；空参数显示「(空参数)」
pub fn pretty_args(raw: &str) -> String {
    if raw.is_empty() {
        return "(空参数)".into();
    }
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| raw.to_string()),
        Err(_) => raw.to_string(),
    }
}

/// v1 `initialExpanded`
pub fn default_expanded(tool: &ToolView, streaming: bool, awaiting_answer: bool) -> bool {
    awaiting_answer || (streaming && matches!(tool.status, ToolStatus::Calling | ToolStatus::Executing))
}

/// 详情块的 markdown 源：围栏代码块（语言 json / text），用紧凑代码块渲染
pub fn detail_sources(tool: &ToolView) -> (String, Option<(String, bool)>) {
    let fence = |lang: &str, body: &str| {
        // 围栏须长于内容中最长的连续反引号（否则内容里的 ``` 会提前闭合代码块）
        let longest = body.split(|c| c != '`').map(str::len).max().unwrap_or(0);
        let ticks = "`".repeat(longest.max(2) + 1);
        format!("{ticks}{lang}\n{body}\n{ticks}")
    };
    let args = fence("json", &pretty_args(&tool.arguments));
    let result = matches!(tool.status, ToolStatus::Done | ToolStatus::Error).then(|| {
        let text = tool.result.clone().filter(|r| !r.is_empty()).unwrap_or_else(|| "(无返回内容)".into());
        (fence("text", &text), tool.is_error)
    });
    (args, result)
}

fn mix(c: Rgba, alpha: f32) -> Hsla {
    Hsla::from(c).opacity(alpha)
}

/// 详情块：标签 + 内容（紧凑代码块，最高 180px 卡内滚动）
pub fn detail_block(id: SharedString, label: &str, icon_name: IconName, is_error: bool, content: AnyElement, scroll: &ScrollHandle, cx: &App) -> AnyElement {
    let c = cx.buddy_theme().colors;
    div()
        .min_w_0()
        .child(
            div()
                .mb(px(m::SPACE_2))
                .flex()
                .items_center()
                .gap(px(m::SPACE_1))
                .text_color(if is_error { c.state_error } else { c.text_tertiary })
                .text_size(px(m::FONT_SIZE_XS))
                .font_weight(FontWeight(600.0))
                .child(icon(icon_name, px(12.0)))
                .child(SharedString::from(label.to_string())),
        )
        .child(div().id(id).max_h(px(180.0 + m::SPACE_6 + m::SPACE_1)).overflow_y_scroll().track_scroll(scroll).child(content))
        .into_any_element()
}

/// 渲染卡片。`details` 为展开时的详情块（由调用方按实体构造）。
#[allow(clippy::too_many_arguments)]
pub fn tool_card(
    id: SharedString,
    tool: &ToolView,
    awaiting_answer: bool,
    expanded: bool,
    details: Vec<AnyElement>,
    now_ms: f64,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &App,
) -> AnyElement {
    let theme = cx.buddy_theme();
    let c = theme.colors;
    let accent = c.tool_ui_accent;
    let (status_label, status_icon, tone, spin) = status_meta(tool.status, awaiting_answer);
    let (label, tool_icon) = tool_meta(&tool.name);
    let summary = action_summary(&tool.name, &tool.arguments);
    let reduce = crate::accessibility::prefers_reduced_motion();

    let (badge_fg, badge_bg, badge_border): (Hsla, Hsla, Hsla) = match tone {
        Tone::Neutral => (c.text_muted.into(), c.bg_sunken.into(), c.border_subtle.into()),
        Tone::Info => (c.tool_ui_accent_strong.into(), c.tool_ui_accent_soft.into(), c.tool_ui_accent_medium.into()),
        Tone::Success => (c.state_success.into(), mix(c.state_success, 0.09), mix(c.state_success, 0.24)),
        Tone::Error => (c.state_error.into(), mix(c.state_error, 0.09), mix(c.state_error, 0.24)),
    };
    // v1 `.buddy-spin`：1.2s 匀速旋转
    let status_svg = icon(status_icon, px(11.0));
    let status_svg = if spin && !reduce {
        window.request_animation_frame();
        let angle = ((now_ms % 1200.0) / 1200.0) as f32 * std::f32::consts::TAU;
        status_svg.with_transformation(Transformation::rotate(gpui::radians(angle)))
    } else {
        status_svg
    };
    let badge = div()
        .flex_none()
        .min_h(px(m::SPACE_5))
        .px(px(m::SPACE_2))
        .py(px(1.0))
        .border_1()
        .border_color(badge_border)
        .rounded(px(m::RADIUS_FULL))
        .flex()
        .items_center()
        .gap(px(m::SPACE_1))
        .text_size(px(m::FONT_SIZE_XS))
        .font_weight(FontWeight(600.0))
        .line_height(px(m::FONT_SIZE_XS))
        .text_color(badge_fg)
        .bg(badge_bg)
        .child(status_svg)
        .child(status_label);

    let mono = fonts::mono_font(cx).family;
    let main = div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_baseline()
        .gap(px(m::SPACE_2))
        .child(
            div()
                .flex_none()
                .flex()
                .items_baseline()
                .gap(px(m::SPACE_2))
                .child(div().text_size(px(m::FONT_SIZE_SM)).font_weight(FontWeight(600.0)).text_color(c.text_primary).child(label))
                .child(div().text_size(px(m::FONT_SIZE_XS)).font_weight(FontWeight(500.0)).font_family(mono.clone()).text_color(c.text_tertiary).child(SharedString::from(tool.name.clone()))),
        )
        .when(!expanded && !summary.is_empty(), |d| {
            d.child(
                div()
                    .min_w_0()
                    .flex()
                    .text_size(px(m::FONT_SIZE_XS))
                    .font_family(mono)
                    .text_color(c.text_muted)
                    .child(div().flex_none().mr(px(m::SPACE_2)).text_color(accent).child("·"))
                    .child(div().min_w_0().truncate().child(SharedString::from(summary))),
            )
        });

    let trigger = div()
        .id(id.clone())
        .w_full()
        .min_h(px(m::SPACE_10))
        .px(px(m::SPACE_3))
        .py(px(m::SPACE_2))
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .cursor_pointer()
        .hover(|s| s.bg(c.bg_sunken))
        .on_click(on_toggle)
        .child(
            div()
                .flex_none()
                .size(px(m::SPACE_6))
                .border_1()
                .border_color(mix(accent, 0.18))
                .rounded(px(m::RADIUS_MD))
                .flex()
                .items_center()
                .justify_center()
                .text_color(accent)
                .bg(mix(accent, 0.10))
                .child(icon(tool_icon, px(14.0))),
        )
        .child(main)
        .child(badge)
        .child(div().flex_none().text_color(c.text_tertiary).child(icon(if expanded { IconName::ChevronDown } else { IconName::ChevronRight }, px(14.0))));

    let body = (expanded && !details.is_empty()).then(|| {
        div()
            .w_full()
            .p(px(m::SPACE_3))
            .border_t_1()
            .border_color(c.border_subtle)
            .flex()
            .flex_col()
            .gap(px(m::SPACE_3))
            .bg(mix(c.bg_sunken, 0.62))
            .children(details)
    });

    let mut shadows = vec![BoxShadow { color: accent.into(), offset: point(px(3.0), px(0.)), blur_radius: px(0.), spread_radius: px(0.), inset: true }];
    shadows.extend(box_shadows(theme.shadows.shadow_static));
    div()
        .id(SharedString::from(format!("{id}-card")))
        .w_full()
        .my(px(m::SPACE_2))
        .overflow_hidden()
        .border_1()
        .border_color(if expanded { mix(accent, 0.32) } else { c.border_default.into() })
        .hover(|s| s.border_color(mix(accent, 0.32)))
        .rounded(px(m::RADIUS_LG))
        .bg(c.panel_surface)
        .shadow(shadows)
        .child(
            div()
                .w_full()
                .bg(linear_gradient(135., linear_color_stop(mix(accent, 0.07), 0.), linear_color_stop(mix(accent, 0.0), 0.44)))
                .child(trigger)
                .children(body),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(name: &str, args: &str, status: ToolStatus) -> ToolView {
        ToolView { id: "c".into(), name: name.into(), arguments: args.into(), status, result: None, is_error: false, images: Vec::new(), insert_after: None }
    }

    #[test]
    fn metas_match_v1() {
        assert_eq!(status_meta(ToolStatus::Executing, false).0, "执行中");
        assert!(status_meta(ToolStatus::Executing, false).3, "执行中旋转");
        assert_eq!(status_meta(ToolStatus::Done, true).0, "等待回答");
        assert_eq!(status_meta(ToolStatus::Interrupted, false).2, Tone::Neutral);
        assert_eq!(tool_meta("edit_file").0, "编辑文件");
        assert_eq!(tool_meta("unknown").0, "调用工具");
    }

    #[test]
    fn summary_and_args_match_v1() {
        assert_eq!(action_summary("read_file", r#"{"path":"/tmp/a.txt"}"#), "/tmp/a.txt");
        assert_eq!(action_summary("x", r#"{"path":"  ","query":"buddy"}"#), "buddy");
        assert_eq!(action_summary("ask_user", r#"{"question":"选哪个？"}"#), "选哪个？");
        assert_eq!(action_summary("ask_user", r#"{}"#), "等待用户回答");
        assert_eq!(action_summary("x", "{半截"), "");
        assert_eq!(pretty_args(""), "(空参数)");
        assert_eq!(pretty_args(r#"{"a":1}"#), "{\n  \"a\": 1\n}");
        assert_eq!(pretty_args("{半截"), "{半截");
    }

    #[test]
    fn default_expansion_matches_v1() {
        assert!(default_expanded(&view("x", "{}", ToolStatus::Executing), true, false));
        assert!(!default_expanded(&view("x", "{}", ToolStatus::Executing), false, false));
        assert!(!default_expanded(&view("x", "{}", ToolStatus::Done), true, false));
        assert!(default_expanded(&view("ask_user", "{}", ToolStatus::Done), false, true));
    }

    #[test]
    fn details_use_longer_fence_when_needed() {
        let mut t = view("x", "{}", ToolStatus::Done);
        t.result = Some("含有 ``` 的结果".into());
        let (args, result) = detail_sources(&t);
        assert!(args.starts_with("```json\n{}"));
        let (result, is_error) = result.unwrap();
        assert!(result.starts_with("````text\n") && !is_error, "{result}");
        t.result = Some(String::new());
        assert!(detail_sources(&t).1.unwrap().0.contains("(无返回内容)"));
        assert!(detail_sources(&view("x", "{}", ToolStatus::Calling)).1.is_none());
    }
}
