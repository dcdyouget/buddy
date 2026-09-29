//! 对话页（S05-18）—— 对应 v1 `src/pages/ChatPage.tsx` 的外壳（`streaming` 与 `conversation` 共用）
//!
//! | v1 | 本模块 |
//! |----|------|
//! | `GlassPanel.buddy-shell`：`--bg-surface` 叠 145° 的 `--surface-highlight` 渐变、`--window-outline` 边、内侧 `--window-inner-highlight` 描边、圆角 xl | 同 |
//! | 消息列表（`flex: 1; min-height: 0`）| [`Transcript`] |
//! | 列表下方：错误条（可关闭）→ 输入区 | [`error_banner`] → [`Composer`] |
//! | 审批弹窗 `ApprovalModal` / 模型下拉 `ModelDropdown` | 归 S05-13 / S05-15 |
//!
//! 页面本身无状态：错误来自 [`Conversation`]，输入区与空态页共用（草稿两页共享）。

use super::composer::Composer;
use super::message_row::error_banner;
use super::session::Conversation;
use super::transcript::Transcript;
use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use gpui::{BoxShadow, Context, Entity, Hsla, Window, div, linear_color_stop, linear_gradient, point, prelude::*, px};

/// 对话页
pub struct ChatPage {
    conversation: Entity<Conversation>,
    transcript: Entity<Transcript>,
    composer: Entity<Composer>,
}

impl ChatPage {
    /// 新建；`composer` 与空态页共用
    pub fn new(conversation: Entity<Conversation>, composer: Entity<Composer>, cx: &mut Context<Self>) -> Self {
        let transcript = cx.new(|cx| Transcript::new(conversation.clone(), cx));
        // 错误条随会话状态出现 / 消失
        cx.observe(&conversation, |_, _, cx| cx.notify()).detach();
        Self { conversation, transcript, composer }
    }

    /// 消息列表（自检用）
    pub fn transcript(&self) -> &Entity<Transcript> {
        &self.transcript
    }
}

impl Render for ChatPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.buddy_theme().colors;
        let error = self.conversation.read(cx).state.error.clone();
        let conversation = self.conversation.clone();
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(m::RADIUS_XL))
            .border_1()
            .border_color(c.window_outline)
            .bg(c.bg_surface)
            .shadow(vec![BoxShadow { color: c.window_inner_highlight.into(), offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(1.), inset: true }])
            // v1 `linear-gradient(145deg, --surface-highlight, transparent 38%)`
            .child(div().absolute().top_0().left_0().size_full().bg(linear_gradient(
                145.,
                linear_color_stop(c.surface_highlight, 0.),
                linear_color_stop(Hsla::from(c.surface_highlight).opacity(0.), 0.38),
            )))
            .child(div().flex_1().min_h_0().child(self.transcript.clone()))
            .when_some(error, |d, error| d.child(error_banner(&error, move |_, cx| conversation.update(cx, |c, cx| c.dismiss_error(cx)), cx)))
            .child(self.composer.clone())
    }
}
