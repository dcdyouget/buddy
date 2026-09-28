//! 消息列表（S05-01）：GPUI `ListState` 真虚拟化
//!
//! v1 `ChatPage.tsx` 把全部消息渲染成 DOM（无虚拟化，只靠手动分页控制数量）。这里：
//!
//! - 行 = [`rows::build_rows`] 的块粒度行；行集合变化按 [`rows::diff`] 做一次最小 `splice`，
//!   内容变化的行只 `remeasure_items` 那一行（流式放字 → 只重测最后一行）；
//! - 每帧只布局视口内 + 预渲染区（overdraw）的行。
//!
//! # `ListState` 的四个陷阱（research-log §16.3，S00-07 实测）
//!
//! 1. `list()` 必须自己 `flex_grow`，否则**静默渲染 0 行**；
//! 2. `set_scroll_handler` 回调内不得访问 `ListState`（会 panic），只读事件字段；
//! 3. `ListAlignment::Bottom` 必须 `measure_all()`，否则行高未知、视口空白；
//! 4. `FollowMode::Tail` 只记状态，贴底须显式 `scroll_to_end()`。
//!
//! 行外观见 [`message_row`]（S05-08）；思考块 / 工具行的完整外观由 S05-09 / S05-10 完成。

use super::message_row;
use super::rows::{self, Row, RowKind};
use super::session::Conversation;
use crate::markdown::{
    self, code_block,
    normalize::normalize_markdown,
    streaming,
    zed_markdown::{Markdown, MarkdownElement, MarkdownOptions, syntax::LanguageRegistry},
};
use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use buddy_engine::streaming::ContentBlock;
use gpui::{
    AnyElement, Context, Entity, FollowMode, ListAlignment, ListState, Render, SharedString, Subscription, Window, div, list,
    prelude::*, px,
};
use std::collections::HashMap;
use std::sync::Arc;

/// 预渲染区（视口上下额外布局的高度）。取 Comet `OVERDRAW_PX`
pub const OVERDRAW_PX: f32 = 320.0;

/// 消息列表视图
pub struct Transcript {
    conversation: Entity<Conversation>,
    list: ListState,
    rows: Vec<Row>,
    /// 每个正文行一个 markdown 实体（按行 id；行 id 稳定 → 流式结束后实体沿用，不重建）
    markdown: HashMap<String, (Entity<Markdown>, u64)>,
    registry: Arc<LanguageRegistry>,
    /// 行渲染计数（自检用：验证虚拟化）
    pub rendered_rows: usize,
    /// 行重测计数（自检用：验证流式只重测一行）
    pub remeasured_rows: usize,
    _observe: Subscription,
}

impl Transcript {
    /// 新建，并立即同步当前会话的行
    pub fn new(conversation: Entity<Conversation>, cx: &mut Context<Self>) -> Self {
        let list = ListState::new(0, ListAlignment::Bottom, px(OVERDRAW_PX)).measure_all();
        list.set_follow_mode(FollowMode::Tail);
        let observe = cx.observe(&conversation, |this: &mut Self, _, cx| this.sync(cx));
        let mut this = Self {
            conversation,
            list,
            rows: Vec::new(),
            markdown: HashMap::new(),
            registry: Arc::new(LanguageRegistry::default()),
            rendered_rows: 0,
            remeasured_rows: 0,
            _observe: observe,
        };
        this.sync(cx);
        this.list.scroll_to_end();
        this
    }

    /// 当前行（自检与测试用）
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// 列表状态（自检用）
    pub fn list_state(&self) -> &ListState {
        &self.list
    }

    /// 按会话状态更新行：一次最小 splice + 只重测内容变化的行
    fn sync(&mut self, cx: &mut Context<Self>) {
        let new_rows = rows::build_rows(&self.conversation.read(cx).state);
        let change = rows::diff(&self.rows, &new_rows);
        if change.is_noop() {
            return;
        }
        let following = self.list.is_following_tail();
        if !change.old_range.is_empty() || change.new_count > 0 {
            self.list.splice(change.old_range.clone(), change.new_count);
        }
        for &ix in &change.remeasure {
            self.list.remeasure_items(ix..ix + 1);
        }
        self.remeasured_rows += change.remeasure.len();
        self.rows = new_rows;
        self.update_markdown(cx);
        if following {
            self.list.scroll_to_end();
        }
        cx.notify();
    }

    /// 正文行的 markdown 实体：新行创建，内容变化时替换（规范化见 S04-05）
    fn update_markdown(&mut self, cx: &mut Context<Self>) {
        let state = &self.conversation.read(cx).state;
        // (行 id, 源文本, 版本, 是否纯文本)
        let mut wanted: Vec<(String, String, u64, bool)> = Vec::new();
        for row in &self.rows {
            match &row.kind {
                RowKind::Block { msg, block, live } => {
                    let blocks = if *live { state.live.as_ref().map(|l| l.blocks.as_slice()) } else { state.messages[*msg].blocks.as_deref() };
                    if let Some(ContentBlock::Text { content }) = blocks.and_then(|b| b.get(*block)) {
                        wanted.push((row.id.clone(), normalize_markdown(content), row.version, false));
                    }
                }
                // 用户消息为纯文本（v1 `white-space: pre-wrap`，不解析 markdown），但要能选择复制 → 同样走 markdown 实体
                RowKind::User { msg } => wanted.push((row.id.clone(), state.messages[*msg].content.clone(), row.version, true)),
                _ => {}
            }
        }
        let registry = self.registry.clone();
        let mut next = HashMap::new();
        for (id, source, version, plain) in wanted {
            let entry = match self.markdown.remove(&id) {
                Some((md, v)) if v == version => (md, v),
                Some((md, _)) => {
                    md.update(cx, |m, cx| m.replace(source, cx));
                    (md, version)
                }
                None if plain => {
                    let options = MarkdownOptions { parse_links_only: true, ..Default::default() };
                    (cx.new(|cx| Markdown::new_with_options(source.into(), None, None, options, cx)), version)
                }
                None => {
                    let registry = registry.clone();
                    (cx.new(|cx| Markdown::new(source.into(), Some(registry), None, cx)), version)
                }
            };
            next.insert(id, entry);
        }
        self.markdown = next;
    }

    fn render_row(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.rendered_rows += 1;
        let Some(row) = self.rows.get(ix).cloned() else { return div().into_any_element() };
        let theme = *cx.buddy_theme();
        let c = theme.colors;
        let conversation = self.conversation.read(cx);
        let now = conversation.now_ms();
        let state = &conversation.state;
        let reduce_motion = crate::accessibility::prefers_reduced_motion();
        match &row.kind {
            RowKind::User { .. } => match self.markdown.get(&row.id) {
                Some((md, _)) => message_row::user_row(
                    // 纯文本：网址不作链接（v1 不识别链接），点击无动作
                    MarkdownElement::new(md.clone(), message_row::user_text_style(window, cx)).on_url_click(|_, _, _| {}),
                    cx,
                ),
                None => div().into_any_element(),
            },
            RowKind::Block { msg, block, live } => {
                let blocks = if *live { state.live.as_ref().map(|l| l.blocks.clone()) } else { state.messages[*msg].blocks.clone() };
                let content: AnyElement = match blocks.and_then(|b| b.get(*block).cloned()) {
                    // 思考块外观由 S05-09 完成
                    Some(ContentBlock::Thinking { content, .. }) => div()
                        .text_color(c.text_muted)
                        .text_size(px(m::FONT_SIZE_BASE))
                        .child(SharedString::from(format!("思考：{}", content.chars().take(80).collect::<String>())))
                        .into_any_element(),
                    Some(ContentBlock::Text { content }) => match self.markdown.get(&row.id) {
                        Some((md, _)) => {
                            let mut style = markdown::message_style(window, cx);
                            // 流式中的最后一个正文块：落定渐显 + 星标（S04-06）
                            if let Some(live_turn) = state.live.as_ref().filter(|_| *live && row.pos.last) {
                                let tail = streaming::tail(&normalize_markdown(&content), true, live_turn.reveal_count);
                                if streaming::decorate(&mut style, &tail, now - live_turn.batch_at, &theme, reduce_motion) {
                                    window.request_animation_frame();
                                }
                            }
                            MarkdownElement::new(md.clone(), style)
                                .code_block_renderer(code_block::renderer(md.downgrade(), *live))
                                .on_url_click(|url, _, cx| markdown::gfm::open_link(&url, cx))
                                .image_resolver(|url, _| markdown::gfm::image_source(url))
                                .into_any_element()
                        }
                        None => div().into_any_element(),
                    },
                    None => div().into_any_element(),
                };
                message_row::assistant_row(row.pos, content).into_any_element()
            }
            // 工具行外观由 S05-10 完成
            RowKind::Tool { call, .. } => {
                let label = state.tools.get(call).map_or_else(|| call.clone(), |t| format!("工具 {}（{:?}）", t.name, t.status));
                message_row::assistant_row(row.pos, div().text_color(c.text_muted).text_size(px(m::FONT_SIZE_SM)).child(SharedString::from(label)))
                    .into_any_element()
            }
            // 回答尚无内容：只显示呼吸星标（v1 `StreamingNextStar`）
            RowKind::Pending { .. } => {
                let since = state.live.as_ref().map_or(0.0, |l| now - l.batch_at);
                if !reduce_motion {
                    window.request_animation_frame();
                }
                message_row::assistant_row(row.pos, streaming::star_element(&theme, since, reduce_motion)).into_any_element()
            }
        }
    }
}

impl Render for Transcript {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 陷阱 1：`list()` 自己 flex_grow；v1 列表上下内边距 space-3 / space-2
        div().size_full().flex().flex_col().pt(px(m::SPACE_3)).pb(px(m::SPACE_2)).child(
            list(self.list.clone(), cx.processor(|this, ix, window, cx| this.render_row(ix, window, cx))).flex_grow(1.).size_full(),
        )
    }
}
