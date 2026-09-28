//! 回答操作栏（S05-14）—— 对应 v1 `src/components/chat/MessageActions.tsx` 与 `.message-action*` 样式
//!
//! | v1 | 本模块 |
//! |----|------|
//! | 复制回答：正文块以空行连接后去首尾空白（`getAnswerText`）；成功后 1.6 秒显示「已复制」 | [`answer_text`]、[`COPIED_FEEDBACK`] |
//! | 回到问题：滚到本轮用户消息的顶部 | 由列表处理 |
//! | 时间：`YYYY-MM-DD HH:MM`（本地时区，等宽数字） | [`format_time`] |
//! | 外框：圆角 lg、细边框、`--control-surface` 72%、inset 高光 + `--shadow-static`；与正文间距 space-3 | 同 |

use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, box_shadows, fonts, tokens::metrics as m};
use buddy_engine::models::Message;
use buddy_engine::streaming::ContentBlock;
use gpui::{AnyElement, App, BoxShadow, ClickEvent, FontWeight, Hsla, SharedString, Window, div, point, prelude::*, px};
use std::time::Duration;

/// 「已复制」反馈时长（v1 `setTimeout(..., 1600)`）
pub const COPIED_FEEDBACK: Duration = Duration::from_millis(1600);

/// v1 `getAnswerText`：有结构化块时取正文块（思考块不含）以空行连接；否则按 `<think>` 拆分取正文
pub fn answer_text(message: &Message) -> String {
    let blocks = match message.blocks.as_ref().filter(|b| !b.is_empty()) {
        Some(b) => b.clone(),
        None => ContentBlock::parse_from_text(&message.content),
    };
    blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { content } => Some(content.as_str()),
            ContentBlock::Thinking { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
        .trim()
        .to_string()
}

/// v1 `formatMessageTime`（本地时区）
pub fn format_time(unix_seconds: u64) -> String {
    use chrono::{Local, TimeZone};
    Local.timestamp_opt(unix_seconds as i64, 0).single().map(|t| t.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default()
}

fn action_button(id: SharedString, name: IconName, label: &'static str, active: bool, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static, cx: &App) -> AnyElement {
    let theme = cx.buddy_theme();
    let c = theme.colors;
    div()
        .id(id)
        .min_w(px(m::SPACE_6 + m::SPACE_1))
        .h(px(m::SPACE_6))
        .px(px(m::SPACE_2))
        .border_1()
        .rounded(px(m::RADIUS_MD))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(m::SPACE_1))
        .text_size(px(m::FONT_SIZE_SM))
        .font_weight(FontWeight(500.0))
        .cursor_pointer()
        .when_else(
            active,
            // v1 `.is-copied`：品牌色字、淡品牌底与边
            |b| b.text_color(c.buddy_primary).bg(c.primary_tint_soft).border_color(c.primary_tint_strong),
            |b| {
                b.text_color(c.text_muted).border_color(Hsla::transparent_black()).hover(|s| {
                    s.text_color(c.text_primary).bg(c.control_surface).border_color(c.border_default).shadow(box_shadows(theme.shadows.shadow_static))
                })
            },
        )
        .on_click(on_click)
        .child(div().size(px(m::SPACE_4)).flex().items_center().justify_center().child(icon(name, px(13.0))))
        .child(label)
        .into_any_element()
}

/// 渲染操作栏
pub fn message_actions(
    id: &str,
    created_at: u64,
    copied: bool,
    has_question: bool,
    on_copy: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_back: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let theme = cx.buddy_theme();
    let c = theme.colors;
    let mut shadows = vec![BoxShadow { color: c.surface_highlight.into(), offset: point(px(0.), px(1.)), blur_radius: px(0.), spread_radius: px(0.), inset: true }];
    shadows.extend(box_shadows(theme.shadows.shadow_static));
    div()
        .flex()
        .child(
            div()
                .mt(px(m::SPACE_3))
                .min_h(px(m::SPACE_6 + m::SPACE_1))
                .pl(px(m::SPACE_2))
                .pr(px(m::SPACE_1))
                .py(px(m::SPACE_1 / 2.0))
                .border_1()
                .border_color(c.border_subtle)
                .rounded(px(m::RADIUS_LG))
                .flex()
                .items_center()
                .gap(px(m::SPACE_1 / 2.0))
                .text_color(c.text_muted)
                .bg(Hsla::from(c.control_surface).opacity(0.72))
                .shadow(shadows)
                .child(action_button(SharedString::from(format!("{id}-copy")), if copied { IconName::Check } else { IconName::Copy }, if copied { "已复制" } else { "复制" }, copied, on_copy, cx))
                .when(has_question, |d| d.child(action_button(SharedString::from(format!("{id}-back")), IconName::ArrowUp, "回到问题", false, on_back, cx)))
                .child(div().w(px(1.0)).h(px(m::SPACE_4)).mx(px(m::SPACE_1)).flex_none().bg(c.border_default))
                .child(
                    div()
                        .px(px(m::SPACE_1))
                        .text_size(px(m::FONT_SIZE_XS))
                        .font_weight(FontWeight(500.0))
                        .font_family(fonts::mono_font(cx).family)
                        .child(SharedString::from(format_time(created_at))),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use buddy_engine::models::MessageRole;

    fn msg(content: &str, blocks: Option<Vec<ContentBlock>>) -> Message {
        Message {
            id: "a".into(),
            role: MessageRole::Assistant,
            content: content.into(),
            images: Vec::new(),
            blocks,
            model_id: None,
            created_at: 0,
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
            is_error: None,
            parent_message_id: None,
        }
    }

    #[test]
    fn answer_text_matches_v1() {
        let blocks = vec![
            ContentBlock::Thinking { content: "想".into(), is_open: false },
            ContentBlock::Text { content: " 第一段 ".into() },
            ContentBlock::Text { content: "第二段\n".into() },
        ];
        assert_eq!(answer_text(&msg("", Some(blocks))), "第一段 \n\n第二段");
        // 无结构化块：按 <think> 拆分，只取正文
        assert_eq!(answer_text(&msg("<think>想</think>答案", None)), "答案");
    }

    #[test]
    fn time_format_matches_v1() {
        let s = format_time(1_700_000_000);
        assert_eq!(s.len(), "2023-11-15 06:13".len());
        assert!(s.starts_with("2023-11-1"), "{s}");
    }
}
