//! 对话页（S05-18）—— 对应 v1 `src/pages/ChatPage.tsx` 的外壳（`streaming` 与 `conversation` 共用）
//!
//! | v1 | 本模块 |
//! |----|------|
//! | `GlassPanel.buddy-shell`：`--bg-surface` 叠 145° 的 `--surface-highlight` 渐变、`--window-outline` 边、内侧 `--window-inner-highlight` 描边、圆角 xl | 同 |
//! | 消息列表（`flex: 1; min-height: 0`）| [`Transcript`] |
//! | 列表下方：错误条（可关闭）→ 输入区 | [`error_banner`] → [`Composer`] |
//! | 审批弹窗 `ApprovalModal`（浮在输入区上方，Esc 拒绝） | [`approval_panel`] |
//! | ask_user 提问卡 | 在消息列表的工具行内（[`super::ask_card`]） |
//!
//! # 窗口拖动
//!
//! v1 的规则是「空白区域和玻璃边缘都能拖，文本 / 按钮 / 输入框不能」（`useDragHandle`）。窗口本身没有标题栏
//! （`titlebar: None` 时 macOS 仍保留一条透明标题栏，只有最上面那一条能原生拖动），所以在四周放
//! 不可见的拖动条：顶部 16px、左右与底部各 8px（正是输入区的外边距，即「玻璃边缘」），按下即 `start_window_move`。
//! 消息正文空白由 Markdown 字形命中判定与行 padding 区域处理，文字与控件保留各自输入。
//!
//! 页面本身无状态：错误来自 [`Conversation`]，输入区与空态页共用（草稿两页共享）。

use super::approval_panel::{DecideFn, Decision, approval_panel};
use super::ask_card::AnswerFn;
use super::composer::Composer;
pub use super::drag::DragFn;
use super::drag::DragSource;
use super::image_gen_state::DownloadFn;
use super::message_row::error_banner;
use super::session::Conversation;
use super::transcript::Transcript;
use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use gpui::{BoxShadow, Context, Div, Entity, FocusHandle, Focusable, Hsla, KeyDownEvent, MouseButton, Window, div, linear_color_stop, linear_gradient, point, prelude::*, px};

/// 工具交互的回调（路由器提供，经 engine 回传）
#[derive(Clone)]
pub struct ToolActions {
    /// 审批决定
    pub decide: DecideFn,
    /// ask_user 回答
    pub answer: AnswerFn,
    /// 生图保存动作（engine 异步下载）。
    pub download: DownloadFn,
}

/// 顶部拖动条高度（v1 空态页 `.empty-drag-region` 为 `--space-3`；对话页整块面板都可拖，这里取 `--space-4`）
const DRAG_TOP: f32 = m::SPACE_4;
/// 左 / 右 / 底的「玻璃边缘」拖动条宽度（= 输入区的外边距）
const DRAG_EDGE: f32 = m::SPACE_2;

/// 对话页
pub struct ChatPage {
    conversation: Entity<Conversation>,
    transcript: Entity<Transcript>,
    composer: Entity<Composer>,
    actions: ToolActions,
    drag: DragFn,
    drag_source: DragSource,
    /// 有审批时页面持有焦点，Esc 才能拒绝（v1 是 window 级 keydown）
    focus: FocusHandle,
}

impl Focusable for ChatPage {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ChatPage {
    /// 新建；`composer` 与空态页共用
    pub fn new(conversation: Entity<Conversation>, composer: Entity<Composer>, actions: ToolActions, cx: &mut Context<Self>) -> Self {
        let transcript = cx.new(|cx| Transcript::new(conversation.clone(), cx));
        let drag_source = transcript.read(cx).drag_source();
        let answer = actions.answer.clone();
        let download = actions.download.clone();
        transcript.update(cx, |t, _| {
            t.set_answer_fn(answer);
            t.set_download_handler(download);
        });
        // 错误条随会话状态出现 / 消失
        cx.observe(&conversation, |_, _, cx| cx.notify()).detach();
        let drag = drag_source.borrow().clone();
        Self { conversation, transcript, composer, actions, drag, drag_source, focus: cx.focus_handle() }
    }

    /// 消息列表（自检用）
    pub fn transcript(&self) -> &Entity<Transcript> {
        &self.transcript
    }
}

impl Render for ChatPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.buddy_theme().colors;
        let drag_source = self.drag_source.clone();
        self.composer.update(cx, |composer, _| {
            composer.set_drag_source(drag_source.clone());
        });
        let error = self.conversation.read(cx).state.error.clone();
        let approval = self.conversation.read(cx).state.approval.clone();
        if approval.is_some() && !self.focus.is_focused(window) {
            window.focus(&self.focus, cx);
        }
        let conversation = self.conversation.clone();
        let approval_id = approval.as_ref().map(|a| a.id.clone());
        let decide = self.actions.decide.clone();
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .on_key_down(move |event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape"
                    && let Some(id) = approval_id.as_deref()
                {
                    cx.stop_propagation();
                    decide(id, Decision::Deny, cx);
                }
            })
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
            .children(drag_strips(&self.drag))
            .children(approval.map(|a| approval_panel(&a, self.actions.decide.clone(), window, cx)))
    }
}

/// 四条不可见的拖动条（顶 / 左 / 右 / 底）
fn drag_strips(drag: &DragFn) -> Vec<Div> {
    let strip = |drag: &DragFn| {
        let drag = drag.clone();
        div().absolute().on_mouse_down(MouseButton::Left, move |_, window, cx| {
            cx.stop_propagation();
            drag(window);
        })
    };
    vec![
        strip(drag).top_0().left_0().right_0().h(px(DRAG_TOP)),
        strip(drag).top(px(DRAG_TOP)).bottom_0().left_0().w(px(DRAG_EDGE)),
        strip(drag).top(px(DRAG_TOP)).bottom_0().right_0().w(px(DRAG_EDGE)),
        strip(drag).bottom_0().left(px(DRAG_EDGE)).right(px(DRAG_EDGE)).h(px(DRAG_EDGE)),
    ]
}
