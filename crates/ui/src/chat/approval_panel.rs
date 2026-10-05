//! 工具审批浮层—— 对应 v1 `ApprovalModal.tsx` 与 `.tool-interaction-*` / `.tool-action-button` 样式
//!
//! | v1 | 本模块 |
//! |----|------|
//! | 浮在输入区上方居中的面板：宽 440（最大 100%），圆角 xl、`--bg-elevated` 叠 145° 强调色渐变、`--shadow-floating-md` | [`approval_panel`] |
//! | 头部：盾牌图标 + 「工具调用审批」+ 工具名（等宽） | 同 |
//! | 参数预览：`reason` 文本，等宽小字，最高 100px 内部滚动 | 同 |
//! | 三个并排按钮：「拒绝」（悬停变红）/「本次都允许」/「允许」（实心）；底部提示「Esc · 拒绝」 | [`Decision`] |
//! | Esc = 拒绝 | 由对话页处理（[`super::chat_page::ChatPage`]） |
//!
//! 决定经回调交给路由器 → `ChatEngine::approve_tool_call`（按调用 id 配对）。

use super::state::Approval;
use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, box_shadows, fonts, tokens::metrics as m};
use gpui::{AnyElement, App, FontWeight, Hsla, SharedString, Window, div, linear_color_stop, linear_gradient, prelude::*, px};
use std::rc::Rc;

/// 用户的决定
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// 拒绝这一个
    Deny,
    /// 允许这一个
    Allow,
    /// 本轮剩余的写入类调用都允许
    AllowAll,
}

impl Decision {
    /// v1 `resolveApproval(approved, approveAll)` 的参数
    pub fn flags(self) -> (bool, bool) {
        match self {
            Decision::Deny => (false, false),
            Decision::Allow => (true, false),
            Decision::AllowAll => (true, true),
        }
    }
}

/// 审批回调：`(调用 id, 决定)`
pub type DecideFn = Rc<dyn Fn(&str, Decision, &mut App)>;

/// 面板宽度（v1 `width: 440`）
pub const PANEL_WIDTH: f32 = 440.0;
/// 面板底边距输入区上缘（v1 `bottom: space-12 + space-4`）
pub const BOTTOM_OFFSET: f32 = m::SPACE_12 + m::SPACE_4;

/// 渲染浮层（绝对定位铺满父元素，只有面板本身接收鼠标）
pub fn approval_panel(approval: &Approval, on_decide: DecideFn, _window: &Window, cx: &App) -> AnyElement {
    let theme = cx.buddy_theme();
    let c = theme.colors;
    let mono = fonts::mono_font(cx).family;
    let id = approval.id.clone();
    let decide = move |decision: Decision| {
        let (on_decide, id) = (on_decide.clone(), id.clone());
        move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| on_decide(&id, decision, cx)
    };
    let button = |key: &'static str, label: &'static str, name: IconName, grow: f32, weight: f32| {
        div()
            .id(key)
            .flex_grow(grow)
            .flex_basis(px(0.0))
            .flex_shrink(1.0)
            .min_h(px(m::SPACE_8))
            .py(px(m::SPACE_2))
            .rounded(px(m::RADIUS_MD))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(5.0))
            .text_size(px(m::FONT_SIZE_SM))
            .font_weight(FontWeight(weight))
            .line_height(px(m::FONT_SIZE_SM))
            .cursor_pointer()
            .child(icon(name, px(13.0)))
            .child(label)
    };

    let shell = div()
        .w(px(PANEL_WIDTH))
        .max_w_full()
        .max_h_full()
        .relative()
        .overflow_hidden()
        .border_1()
        .border_color(c.border_default)
        .rounded(px(m::RADIUS_XL))
        .bg(c.bg_elevated)
        .shadow(box_shadows(theme.shadows.shadow_floating_md))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(linear_gradient(145., linear_color_stop(Hsla::from(c.tool_ui_accent).opacity(0.06), 0.), linear_color_stop(Hsla::from(c.tool_ui_accent).opacity(0.0), 0.38))),
        )
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .child(
                    div()
                        .min_h(px(m::SPACE_12))
                        .px(px(m::SPACE_4))
                        .pt(px(m::SPACE_3))
                        .pb(px(m::SPACE_2))
                        .flex()
                        .items_center()
                        .gap(px(m::SPACE_3))
                        .child(
                            div()
                                .flex_none()
                                .size(px(m::SPACE_6 + m::SPACE_1))
                                .border_1()
                                .border_color(c.tool_ui_accent_medium)
                                .rounded(px(m::RADIUS_MD))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(c.tool_ui_accent_strong)
                                .bg(c.tool_ui_accent_soft)
                                .child(icon(IconName::Shield, px(16.0))),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_size(px(m::FONT_SIZE_MD)).font_weight(FontWeight(600.0)).text_color(c.text_primary).child("工具调用审批"))
                                .child(div().mt(px(1.0)).text_size(px(m::FONT_SIZE_SM)).font_family(mono.clone()).text_color(c.text_muted).child(SharedString::from(approval.name.clone()))),
                        ),
                )
                .child(
                    div()
                        .id("approval-preview")
                        .mx(px(m::SPACE_4))
                        .mb(px(m::SPACE_3))
                        .px(px(m::SPACE_3))
                        .py(px(m::SPACE_2))
                        .max_h(px(100.0))
                        .overflow_y_scroll()
                        .border_1()
                        .border_color(c.border_subtle)
                        .rounded(px(m::RADIUS_MD))
                        .bg(c.bg_sunken)
                        .text_size(px(m::FONT_SIZE_XS))
                        .font_family(mono)
                        .line_height(px(m::FONT_SIZE_XS * 1.5))
                        .text_color(c.text_muted)
                        .child(SharedString::from(approval.reason.clone())),
                )
                .child(
                    div()
                        .px(px(m::SPACE_4))
                        .pb(px(m::SPACE_4))
                        .flex()
                        .gap(px(m::SPACE_2))
                        .child(
                            button("approval-deny", "拒绝", IconName::ShieldX, 1.0, 500.0)
                                .border_1()
                                .border_color(c.border_default)
                                .text_color(c.text_muted)
                                .hover(|s| s.bg(c.bg_sunken).border_color(c.state_error).text_color(c.state_error))
                                .on_click(decide(Decision::Deny)),
                        )
                        .child(
                            button("approval-allow-all", "本次都允许", IconName::ShieldCheck, 1.0, 500.0)
                                .border_1()
                                .border_color(c.tool_ui_accent)
                                .text_color(c.tool_ui_accent_strong)
                                .hover(|s| s.bg(c.tool_ui_accent_soft))
                                .on_click(decide(Decision::AllowAll)),
                        )
                        .child(
                            button("approval-allow", "允许", IconName::ShieldCheck, 1.2, 600.0)
                                .bg(c.tool_ui_action)
                                .text_color(c.text_on_primary)
                                .hover(|s| s.opacity(0.92))
                                .on_click(decide(Decision::Allow)),
                        ),
                )
                .child(div().px(px(m::SPACE_4)).pb(px(m::SPACE_3)).flex().justify_center().text_size(px(m::FONT_SIZE_XS)).text_color(c.text_tertiary).child("Esc · 拒绝")),
        );

    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom(px(BOTTOM_OFFSET))
        .top(px(m::SPACE_12))
        .px(px(m::SPACE_4))
        .flex()
        .flex_col()
        .justify_end()
        .items_center()
        // 只有面板接收鼠标（v1 `pointer-events: none` 的层）
        .child(shell)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions_map_to_v1_resolve_approval_flags() {
        assert_eq!(Decision::Deny.flags(), (false, false));
        assert_eq!(Decision::Allow.flags(), (true, false));
        assert_eq!(Decision::AllowAll.flags(), (true, true), "「本次都允许」= 批准并对本轮后续写入调用放行");
    }
}
