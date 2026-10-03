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

use super::ask_card::{self, AnswerFn, AskUserCard, CardInput};
use super::drag::{self, DragSource};
use super::image_gen;
use super::image_gen_state::{
    self, CopyStates, DownloadFn, DownloadState, DownloadStates, ImageLoadStates, RetryStates,
};
use super::message_actions;
use super::message_row;
use super::rows::{self, Row, RowKind};
use super::session::Conversation;
use super::think_block;
use super::tool_card;
use super::web_search::{self, OpenFn};
use crate::icons::{IconName, icon};
use crate::markdown::{
    self, code_block,
    normalize::normalize_markdown,
    streaming,
    zed_markdown::{Markdown, MarkdownElement, MarkdownOptions, syntax::LanguageRegistry},
};
use crate::theme_system::box_shadows;
use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use buddy_engine::models::MessageRole;
use buddy_engine::streaming::ContentBlock;
use gpui::ScrollHandle;
use gpui::{
    AnyElement, App, Context, Entity, FollowMode, Hsla, ListAlignment, ListState, Pixels, Render,
    RetainAllImageCache, SharedString, Subscription, Task, Window, div, list, prelude::*, px,
};
use gpui::MouseButton;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 预渲染区（视口上下额外布局的高度）。取 Comet `OVERDRAW_PX`
pub const OVERDRAW_PX: f32 = 320.0;
/// 距顶多近时加载更早历史（v1 `el.scrollTop <= 56`）
pub const LOAD_OLDER_THRESHOLD_PX: f32 = 56.0;

/// 「正在加载更早消息…」（v1 列表顶部的提示行：space-2 / space-4 内边距、xs 字号、三级文字色、居中）。
/// 叠在列表顶部而非作为一行插入：插成行会被视口保持挪到视口外，看不到
fn loading_older_banner(cx: &App) -> impl IntoElement {
    let c = cx.buddy_theme().colors;
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .px(px(m::SPACE_4))
        .py(px(m::SPACE_2))
        .bg(c.bg_surface)
        .text_size(px(m::FONT_SIZE_XS))
        .text_color(c.text_tertiary)
        .flex()
        .justify_center()
        .child("正在加载更早消息…")
}

/// 消息列表视图
pub struct Transcript {
    conversation: Entity<Conversation>,
    list: ListState,
    rows: Vec<Row>,
    /// 各行的 [`rows::stable_key`]（与 `rows` 一一对应）
    row_keys: Vec<String>,
    /// 上一帧看到的历史起点：刚并入更早一页的那一帧新行尚未测量、滚动位置算作 0，不据此再触发加载
    seen_history_offset: Option<u64>,
    /// 每个正文行一个 markdown 实体（按行 id；行 id 稳定 → 流式结束后实体沿用，不重建）
    markdown: HashMap<String, (Entity<Markdown>, u64)>,
    registry: Arc<LanguageRegistry>,
    /// 行渲染计数（自检用：验证虚拟化）
    pub rendered_rows: usize,
    /// 行重测计数（自检用：验证流式只重测一行）
    pub remeasured_rows: usize,
    /// 单次行同步重测的最大行数（自检用：不依赖采样时机，验证流式只重测一行）
    pub max_remeasured_per_sync: usize,
    /// 提问卡被渲染的次数（自检用：回答之后不应再渲染，应与其他工具一致）
    pub ask_card_renders: usize,
    /// 上一次同步时是否在流式（检测「开始流式」以恢复跟随）
    was_streaming: bool,
    /// 在底部时看到的最后一条可见消息（v1 `lastSeenMessageCountRef` 记条数；
    /// 这里记 id —— 顶部并入更早历史时条数会变，但不算新消息）
    last_seen: Option<String>,
    /// 出现未读新消息的时刻：按钮外圈脉冲一次（v1 `.has-new-message::after`）
    unseen_pulse: Option<Instant>,
    /// 点击「回到底部」后的平滑滚动
    scroll_animation: Option<Task<()>>,
    /// 滚轮平滑滚动（v1 `useSmoothWheelScroll`）
    wheel: WheelScroll,
    /// 用户点过的思考块展开状态（按行 id；未点过则默认折叠，v1 `userToggled`）
    think_expanded: HashMap<String, bool>,
    /// 用户点过的工具卡片展开状态（按行 id；未点过则按 v1 `initialExpanded`）
    tool_expanded: HashMap<String, bool>,
    /// 工具详情的卡内滚动句柄（按详情 id）
    detail_scroll: HashMap<String, ScrollHandle>,
    /// 本帧可见的内层滚动区域：滚轮落在其中且它还能滚时，让给它（v1 `canNestedScrollerConsume`）
    nested_scroll: Rc<RefCell<Vec<ScrollHandle>>>,
    /// 最近一次绘制时各行的屏幕边界（按行 id）。GPUI 贴底列表的 `bounds_for_item`
    /// 对末尾几行恒为 None，自检与定位需要以实际绘制为准
    painted_rows: Rc<RefCell<HashMap<String, gpui::Bounds<Pixels>>>>,
    /// 刚复制过的操作栏（行 id）与恢复计时（v1 1.6 秒）
    copied_actions: Option<(String, Task<()>)>,
    /// ask_user 提问卡（按工具调用 id）；卡内状态变化时重测所在行
    ask_cards: HashMap<String, (Entity<AskUserCard>, Subscription)>,
    /// 回答回调（路由器提供，经 engine 回传）
    answer: Option<AnswerFn>,
    /// Shared by shell edge strips and Markdown blank-body hit testing.
    drag_source: DragSource,
    /// 打开搜索结果链接（默认系统浏览器；自检可替换）
    open_url: OpenFn,
    /// 保存生成图片（默认返回未接入错误；路由器注入 engine 下载动作）
    download_image: DownloadFn,
    /// 资源图片缓存；失败后由卡片“重试”清除对应条目并重新加载。
    image_cache: Entity<RetainAllImageCache>,
    retry_states: RetryStates,
    image_load_states: ImageLoadStates,
    /// 各张生成图片的异步保存状态。
    download_states: DownloadStates,
    /// 复制提示词的短暂反馈状态。
    copy_states: CopyStates,
    _observe: Subscription,
}

impl Transcript {
    /// 新建，并立即同步当前会话的行
    pub fn new(conversation: Entity<Conversation>, cx: &mut Context<Self>) -> Self {
        let list = ListState::new(0, ListAlignment::Bottom, px(OVERDRAW_PX)).measure_all();
        list.set_follow_mode(FollowMode::Tail);
        // 陷阱 2：回调内不得访问 ListState（此时它被可变借用）→ 只安排一次延后的重绘，渲染时再读跟随状态
        let weak = cx.entity().downgrade();
        list.set_scroll_handler(move |_, _, cx| {
            let weak = weak.clone();
            cx.defer(move |cx| {
                let _ = weak.update(cx, |_, cx| cx.notify());
            });
        });
        let observe = cx.observe(&conversation, |this: &mut Self, _, cx| this.sync(cx));
        let mut this = Self {
            conversation,
            list,
            rows: Vec::new(),
            row_keys: Vec::new(),
            seen_history_offset: None,
            markdown: HashMap::new(),
            registry: Arc::new(LanguageRegistry::default()),
            rendered_rows: 0,
            remeasured_rows: 0,
            ask_card_renders: 0,
            max_remeasured_per_sync: 0,
            was_streaming: false,
            last_seen: None,
            unseen_pulse: None,
            scroll_animation: None,
            wheel: WheelScroll::default(),
            think_expanded: HashMap::new(),
            tool_expanded: HashMap::new(),
            detail_scroll: HashMap::new(),
            nested_scroll: Rc::default(),
            painted_rows: Rc::default(),
            copied_actions: None,
            ask_cards: HashMap::new(),
            answer: None,
            drag_source: drag::default_drag_source(),
            open_url: Rc::new(|url, cx| markdown::gfm::open_link(url, cx)),
            download_image: Rc::new(image_gen_state::default_download),
            image_cache: RetainAllImageCache::new(cx),
            retry_states: Default::default(),
            image_load_states: Default::default(),
            download_states: Default::default(),
            copy_states: Default::default(),
            _observe: observe,
        };
        this.sync(cx);
        this.list.scroll_to_end();
        this
    }

    /// 替换打开链接的动作（自检用）
    pub fn set_open_handler(&mut self, open: OpenFn) {
        self.open_url = open;
    }

    /// Shared callback source used by ChatPage's edge strips and Markdown
    /// blank-body dragging. Kept stable so self-tests can replace the action.
    pub fn drag_source(&self) -> DragSource {
        self.drag_source.clone()
    }

    /// 替换生成图片的保存动作（生产路由可接入 engine `download_generated_image`）。
    pub fn set_download_handler(&mut self, download: DownloadFn) {
        self.download_image = download;
    }

    /// 读取图片保存状态（预览自测用）。
    pub fn download_state_for_test(&self, image_id: &str) -> Option<DownloadState> {
        self.download_states
            .lock()
            .ok()
            .and_then(|states| states.get(image_id).cloned())
    }

    /// 读取图片重试点击状态（预览自测用）。
    pub fn retry_state_for_test(&self, image_id: &str) -> bool {
        self.retry_states
            .lock()
            .ok()
            .is_some_and(|states| states.contains(image_id))
    }

    /// 读取资源图片缓存数量（预览自测用）。
    pub fn image_cache_len_for_test(&self, cx: &App) -> usize {
        self.image_cache.read(cx).len()
    }

    /// 读取图片加载结果（预览自测用）。
    pub fn image_load_state_for_test(
        &self,
        image_id: &str,
    ) -> Option<image_gen_state::ImageLoadState> {
        self.image_load_states
            .lock()
            .ok()
            .and_then(|states| states.get(image_id).cloned())
    }

    /// 读取复制提示词反馈状态（预览自测用）。
    pub fn copy_state_for_test(&self, row_id: &str) -> bool {
        self.copy_states
            .lock()
            .ok()
            .is_some_and(|states| states.contains(row_id))
    }

    /// 设置回答回调（提问卡的「确认 / 跳过」经它交给 engine）
    pub fn set_answer_fn(&mut self, answer: AnswerFn) {
        self.answer = Some(answer);
    }

    /// 提问卡（自检用）
    pub fn ask_card_for_test(&self, tool_id: &str) -> Option<Entity<AskUserCard>> {
        self.ask_cards.get(tool_id).map(|(card, _)| card.clone())
    }

    /// 取（或建）提问卡；卡内状态变化 → 重测该行
    fn ask_card(
        &mut self,
        tool_id: &str,
        row_id: &str,
        cx: &mut Context<Self>,
    ) -> Entity<AskUserCard> {
        if let Some((card, _)) = self.ask_cards.get(tool_id) {
            return card.clone();
        }
        let answer = self
            .answer
            .clone()
            .unwrap_or_else(|| Rc::new(|_, _, _| Err("未接入回答通道".to_string())));
        let id = tool_id.to_string();
        let card = cx.new(|cx| AskUserCard::new(id, answer, cx));
        let row_id = row_id.to_string();
        let subscription = cx.observe(&card, move |this: &mut Self, _, cx| {
            if let Some(ix) = this.rows.iter().position(|r| r.id == row_id) {
                this.list.remeasure_items(ix..ix + 1);
                this.remeasured_rows += 1;
            }
            cx.notify();
        });
        self.ask_cards
            .insert(tool_id.to_string(), (card.clone(), subscription));
        card
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
        let state = &self.conversation.read(cx).state;
        let streaming = state.is_streaming();
        let new_rows = rows::build_rows(state);
        // v1：流式开始时重置为跟随（`useEffect([isStreaming])`）
        if streaming && !self.was_streaming {
            self.list.set_follow_mode(FollowMode::Tail);
        }
        self.was_streaming = streaming;
        let change = rows::diff(&self.rows, &new_rows);
        if change.is_noop() {
            return;
        }
        let following = self.list.is_following_tail();
        // 视口首行落在被替换的区间内（典型：顶部并入更早历史后，分页边界的行改了锚点），
        // GPUI 会把位置重置到区间起点、行内偏移清零 → 视口跳动。记下它的行键，替换后按键还原
        let top = self.list.logical_scroll_top();
        let keep_top = (!following && change.old_range.contains(&top.item_ix))
            .then(|| self.row_keys.get(top.item_ix).cloned())
            .flatten()
            .map(|key| (key, top.offset_in_item));
        if !change.old_range.is_empty() || change.new_count > 0 {
            self.list.splice(change.old_range.clone(), change.new_count);
        }
        for &ix in &change.remeasure {
            self.list.remeasure_items(ix..ix + 1);
        }
        self.remeasured_rows += change.remeasure.len();
        self.max_remeasured_per_sync = self.max_remeasured_per_sync.max(change.remeasure.len());
        self.row_keys = new_rows
            .iter()
            .map(|r| rows::stable_key(state, r))
            .collect();
        self.rows = new_rows;
        if let Some((key, offset_in_item)) = keep_top
            && let Some(item_ix) = self.row_keys.iter().position(|k| *k == key)
        {
            self.list.scroll_to(gpui::ListOffset {
                item_ix,
                offset_in_item,
            });
        }
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
                    let blocks = if *live {
                        state.live.as_ref().map(|l| l.blocks.as_slice())
                    } else {
                        state.messages[*msg].blocks.as_deref()
                    };
                    match blocks.and_then(|b| b.get(*block)) {
                        // 思考内容同样以 markdown 显示（v1 `ThinkSection` 展开后用 StreamingMarkdown）
                        Some(
                            ContentBlock::Text { content } | ContentBlock::Thinking { content, .. },
                        ) => {
                            wanted.push((
                                row.id.clone(),
                                normalize_markdown(content),
                                row.version,
                                false,
                            ));
                        }
                        None => {}
                    }
                }
                // 工具详情：调用参数 / 执行结果（紧凑代码块）
                RowKind::Tool { call, .. } => {
                    if let Some(tool) = state.tools.get(call) {
                        let (args, result) = tool_card::detail_sources(tool);
                        wanted.push((format!("{}#args", row.id), args, row.version, false));
                        if let Some((result, _)) = result {
                            wanted.push((format!("{}#result", row.id), result, row.version, false));
                        }
                    }
                }
                // 用户消息为纯文本（v1 `white-space: pre-wrap`，不解析 markdown），但要能选择复制 → 同样走 markdown 实体
                RowKind::User { msg } => wanted.push((
                    row.id.clone(),
                    state.messages[*msg].content.clone(),
                    row.version,
                    true,
                )),
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
                    let options = MarkdownOptions {
                        parse_links_only: true,
                        ..Default::default()
                    };
                    (
                        cx.new(|cx| {
                            Markdown::new_with_options(source.into(), None, None, options, cx)
                        }),
                        version,
                    )
                }
                None => {
                    let registry = registry.clone();
                    (
                        cx.new(|cx| Markdown::new(source.into(), Some(registry), None, cx)),
                        version,
                    )
                }
            };
            next.insert(id, entry);
        }
        self.markdown = next;
    }

    fn render_row(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.rendered_rows += 1;
        let Some(row) = self.rows.get(ix).cloned() else {
            return div().into_any_element();
        };
        let painted = self.painted_rows.clone();
        let id = row.id.clone();
        let content = self.render_row_content(row, window, cx);
        div()
            .relative()
            .w_full()
            .child(content)
            .child(
                gpui::canvas(
                    |_, _, _| {},
                    move |bounds, _, _, _| {
                        painted.borrow_mut().insert(id.clone(), bounds);
                    },
                )
                // 须显式 top/left：否则绝对定位元素落在内容之后，边界整体下移一个行高
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .into_any_element()
    }

    /// 某行最近一次绘制的屏幕边界
    pub fn painted_row_bounds(&self, row_id: &str) -> Option<gpui::Bounds<Pixels>> {
        self.painted_rows.borrow().get(row_id).copied()
    }

    fn render_row_content(
        &mut self,
        row: Row,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = *cx.buddy_theme();
        let conversation = self.conversation.read(cx);
        let now = conversation.now_ms();
        let state = &conversation.state;
        let reduce_motion = crate::accessibility::prefers_reduced_motion();
        let drag_source = self.drag_source.clone();
        match &row.kind {
            RowKind::User { msg } => match self.markdown.get(&row.id) {
                Some((md, _)) => message_row::user_row_with_images_drag(
                    // 纯文本：网址不作链接（v1 不识别链接），点击无动作
                    message_row::with_blank_drag(
                        MarkdownElement::new(md.clone(), message_row::user_text_style(window, cx))
                            .on_url_click(|_, _, _| {}),
                        drag_source.clone(),
                    ),
                    &state.messages[*msg].images,
                    !state.messages[*msg].content.trim().is_empty(),
                    window,
                    cx,
                    drag_source.clone(),
                ),
                None => div().into_any_element(),
            },
            RowKind::Block { msg, block, live } => {
                let blocks = if *live {
                    state.live.as_ref().map(|l| l.blocks.clone())
                } else {
                    state.messages[*msg].blocks.clone()
                };
                let content: AnyElement = match blocks.and_then(|b| b.get(*block).cloned()) {
                    Some(ContentBlock::Thinking { content, is_open }) => {
                        // v1：流式思考 = 本消息最后一块、未闭合、且在流式中
                        let thinking_live =
                            *live && is_open && row.pos.last && state.live.is_some();
                        let expanded = self.think_expanded.get(&row.id).copied().unwrap_or(false);
                        let view = expanded.then(|| self.markdown.get(&row.id)).flatten().map(
                            |(md, _)| {
                                message_row::with_blank_drag(
                                    MarkdownElement::new(
                                        md.clone(),
                                        markdown::thinking_style(window, cx),
                                    )
                                    .on_url_click(|url, _, cx| markdown::gfm::open_link(&url, cx)),
                                    drag_source.clone(),
                                )
                                .into_any_element()
                            },
                        );
                        let id = row.id.clone();
                        let weak = cx.entity().downgrade();
                        think_block::think_block(
                            SharedString::from(format!("think-{}", row.id)),
                            &content,
                            thinking_live,
                            expanded,
                            view,
                            now,
                            move |_, _, cx| {
                                let _ = weak.update(cx, |t, cx| {
                                    let e = t.think_expanded.entry(id.clone()).or_insert(false);
                                    *e = !*e;
                                    // 展开 / 折叠改变行高
                                    if let Some(ix) = t.rows.iter().position(|r| r.id == id) {
                                        t.list.remeasure_items(ix..ix + 1);
                                    }
                                    cx.notify();
                                });
                            },
                            window,
                            cx,
                        )
                    }
                    Some(ContentBlock::Text { content }) => match self.markdown.get(&row.id) {
                        Some((md, _)) => {
                            let mut style = markdown::message_style(window, cx);
                            // 流式中的最后一个正文块：落定渐显 + 星标（S04-06）
                            if let Some(live_turn) =
                                state.live.as_ref().filter(|_| *live && row.pos.last)
                            {
                                let tail = streaming::tail(
                                    &normalize_markdown(&content),
                                    true,
                                    live_turn.reveal_count,
                                );
                                if streaming::decorate(
                                    &mut style,
                                    &tail,
                                    now - live_turn.batch_at,
                                    &theme,
                                    reduce_motion,
                                ) {
                                    window.request_animation_frame();
                                }
                            }
                            message_row::with_blank_drag(
                                MarkdownElement::new(md.clone(), style)
                                    .code_block_renderer(code_block::renderer(md.downgrade(), *live))
                                    .on_url_click(|url, _, cx| markdown::gfm::open_link(&url, cx))
                                    .image_resolver(|url, _| markdown::gfm::image_source(url)),
                                drag_source.clone(),
                            )
                            .into_any_element()
                        }
                        None => div().into_any_element(),
                    },
                    None => div().into_any_element(),
                };
                message_row::assistant_row_with_drag(row.pos, content, drag_source.clone()).into_any_element()
            }
            RowKind::Tool { call, msg } => {
                let Some(tool) = state.tools.get(call).cloned() else {
                    return div().into_any_element();
                };
                let awaiting = state.question.as_ref().is_some_and(|q| &q.id == call);
                let live_msg = state.live.is_some()
                    && state
                        .messages
                        .iter()
                        .rposition(|m| m.role == MessageRole::Assistant)
                        == Some(*msg);
                let expanded = if image_gen::is_image_gen(&tool.name) {
                    // v1 GenerateImageSection owns an independent `expanded=false` state.
                    self.tool_expanded.get(&row.id).copied().unwrap_or(false)
                } else {
                    self.tool_expanded
                        .get(&row.id)
                        .copied()
                        .unwrap_or_else(|| default_open(&tool, live_msg, awaiting))
                };
                if image_gen::is_image_gen(&tool.name) {
                    let attachments = state
                        .messages
                        .iter()
                        .find(|message| {
                            message.role == MessageRole::Tool
                                && message.tool_call_id.as_deref() == Some(call.as_str())
                        })
                        .map(|message| message.images.clone())
                        .unwrap_or_default();
                    let id = row.id.clone();
                    let weak = cx.entity().downgrade();
                    let now = self.conversation.read(cx).now_ms();
                    let before_load: Vec<_> = attachments.iter().map(|image| self.image_load_state_for_test(&image.id)).collect();
                    let card = image_gen::block(
                        SharedString::from(format!("image-{}", row.id)),
                        &tool,
                        expanded,
                        now,
                        &attachments,
                        self.image_cache.clone(),
                        self.retry_states.clone(),
                        self.image_load_states.clone(),
                        self.download_states.clone(),
                        self.copy_states.clone(),
                        move |_, _, cx| {
                            let _ =
                                weak.update(cx, |t, cx| t.set_tool_expanded(&id, !expanded, cx));
                        },
                        self.download_image.clone(),
                        window,
                        cx,
                    );
                    let after_load: Vec<_> = attachments.iter().map(|image| self.image_load_state_for_test(&image.id)).collect();
                    if before_load != after_load {
                        // Resource completion can change intrinsic height. ListState
                        // also caches overdraw rows, so invalidate after its layout lock is released.
                        let weak = cx.entity().downgrade();
                        let id = row.id.clone();
                        cx.defer(move |cx| {
                            let _ = weak.update(cx, |this, cx| {
                                if let Some(ix) = this.rows.iter().position(|row| row.id == id) {
                                    this.list.remeasure_items(ix..ix + 1);
                                    this.remeasured_rows += 1;
                                    if this.list.is_following_tail() {
                                        this.list.scroll_to_end();
                                    }
                                }
                                cx.notify();
                            });
                        });
                    }
                    return message_row::assistant_row_with_drag(row.pos, card, drag_source.clone()).into_any_element();
                }
                if web_search::is_web_search(&tool.name) {
                    // v1 `ToolSection`：websearch 走专用卡片（外壳同思考块），默认折叠，可点开看来源
                    let id = row.id.clone();
                    let weak = cx.entity().downgrade();
                    let now = self.conversation.read(cx).now_ms();
                    let scroll = self
                        .detail_scroll
                        .entry(format!("{}#search", row.id))
                        .or_default()
                        .clone();
                    self.nested_scroll.borrow_mut().push(scroll.clone());
                    let card = web_search::block(
                        SharedString::from(format!("search-{}", row.id)),
                        &tool,
                        expanded,
                        now,
                        &scroll,
                        move |_, _, cx| {
                            let _ =
                                weak.update(cx, |t, cx| t.set_tool_expanded(&id, !expanded, cx));
                        },
                        self.open_url.clone(),
                        window,
                        cx,
                    );
                    return message_row::assistant_row_with_drag(row.pos, card, drag_source.clone()).into_any_element();
                }
                let mut details = Vec::new();
                if ask_card::shows_card(&tool.name, awaiting, expanded) {
                    // 等待回答时展开的是提问卡；回答之后与其他工具统一（折叠 + 调用参数 / 执行结果，目检 #19 反馈，偏离 v1 的「用户回应」）
                    let display = state
                        .question
                        .as_ref()
                        .map(ask_card::from_question)
                        .unwrap_or_default();
                    self.ask_card_renders += 1;
                    let card = self.ask_card(&tool.id, &row.id, cx);
                    card.update(cx, |c, cx| c.sync(CardInput { display, awaiting }, cx));
                    details.push(card.into_any_element());
                } else if expanded {
                    let (_, result) = tool_card::detail_sources(&tool);
                    let mut detail = |key: &str,
                                      label: &str,
                                      icon: IconName,
                                      is_error: bool,
                                      this: &mut Self,
                                      window: &mut Window,
                                      cx: &mut Context<Self>| {
                        let id = format!("{}#{key}", row.id);
                        let Some((md, _)) = this.markdown.get(&id).cloned() else {
                            return;
                        };
                        let scroll = this.detail_scroll.entry(id.clone()).or_default().clone();
                        this.nested_scroll.borrow_mut().push(scroll.clone());
                        let content = message_row::with_blank_drag(
                            MarkdownElement::new(md.clone(), markdown::tool_detail_style(window, cx))
                                .code_block_renderer(code_block::compact_renderer(md.downgrade())),
                            drag_source.clone(),
                        )
                        .into_any_element();
                        details.push(tool_card::detail_block(
                            SharedString::from(format!("detail-{id}")),
                            label,
                            icon,
                            is_error,
                            content,
                            &scroll,
                            cx,
                        ));
                    };
                    detail(
                        "args",
                        "调用参数",
                        IconName::Braces,
                        false,
                        self,
                        window,
                        cx,
                    );
                    if let Some((_, is_error)) = result {
                        detail(
                            "result",
                            if is_error {
                                "执行错误"
                            } else {
                                "执行结果"
                            },
                            IconName::FileCheck,
                            is_error,
                            self,
                            window,
                            cx,
                        );
                    }
                }
                let id = row.id.clone();
                let weak = cx.entity().downgrade();
                let now = self.conversation.read(cx).now_ms();
                let card = tool_card::tool_card(
                    SharedString::from(format!("tool-{}", row.id)),
                    &tool,
                    awaiting,
                    expanded,
                    details,
                    now,
                    move |_, _, cx| {
                        let _ = weak.update(cx, |t, cx| t.set_tool_expanded(&id, !expanded, cx));
                    },
                    window,
                    cx,
                );
                message_row::assistant_row_with_drag(row.pos, card, drag_source.clone()).into_any_element()
            }
            RowKind::Actions { msg } => {
                let message = &state.messages[*msg];
                let answer = message_actions::answer_text(message);
                // 本轮用户消息 = 行 id 的锚（S05-02）；它在行集合中才有「回到问题」
                let anchor = row.id.split('#').next().unwrap_or_default().to_string();
                let has_question = self
                    .rows
                    .iter()
                    .any(|r| r.id == anchor && matches!(r.kind, RowKind::User { .. }));
                let copied = self
                    .copied_actions
                    .as_ref()
                    .is_some_and(|(id, _)| *id == row.id);
                let (w1, w2) = (cx.entity().downgrade(), cx.entity().downgrade());
                let id = row.id.clone();
                let bar = message_actions::message_actions(
                    &row.id,
                    message.created_at,
                    copied,
                    has_question,
                    move |_, _, cx| {
                        if answer.is_empty() {
                            return;
                        }
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(answer.clone()));
                        let _ = w1.update(cx, |t, cx| t.mark_copied(id.clone(), cx));
                    },
                    move |_, _, cx| {
                        let _ = w2.update(cx, |t, cx| t.scroll_to_row(&anchor, cx));
                    },
                    cx,
                );
                message_row::assistant_row_with_drag(row.pos, bar, drag_source.clone()).into_any_element()
            }
            // 回答尚无内容：只显示呼吸星标（v1 `StreamingNextStar`）
            RowKind::Pending { .. } => {
                let since = state.live.as_ref().map_or(0.0, |l| now - l.batch_at);
                if !reduce_motion {
                    window.request_animation_frame();
                }
                message_row::assistant_row_with_drag(
                    row.pos,
                    streaming::star_element(&theme, since, reduce_motion),
                    drag_source.clone(),
                )
                .into_any_element()
            }
        }
    }
}

/// 滚轮平滑滚动的参数（v1 `useSmoothWheelScroll.ts`）
const WHEEL_LINE_PX: f32 = 20.0;
const WHEEL_EASING: f32 = 0.28;
const WHEEL_SETTLE_PX: f32 = 0.5;
const WHEEL_EXTERNAL_TOLERANCE_PX: f32 = 1.0;

/// 滚轮平滑滚动状态：连续滚轮累加目标位置，每帧走剩余距离的 28%（v1 同）
#[derive(Default)]
struct WheelScroll {
    /// 目标滚动位置（距顶部像素）
    target: Pixels,
    /// 上一帧动画写入后的位置；与当前位置不符 = 外部改动了滚动（历史补位、跟随等）→ 放弃旧目标
    last_animated: Option<Pixels>,
    task: Option<Task<()>>,
}

/// v1 `normalizeWheelDelta`：行 × 20px；返回「向下为正」的像素（DOM 约定）。
/// GPUI 只有像素 / 行两种增量（v1 的「页」模式无对应）。
pub fn wheel_delta_px(delta: &gpui::ScrollDelta) -> Pixels {
    match delta {
        // GPUI 的 y 向上为正（内容下移），DOM 的 deltaY 向下为正
        gpui::ScrollDelta::Pixels(p) => -p.y,
        gpui::ScrollDelta::Lines(l) => px(-l.y * WHEEL_LINE_PX),
    }
}

impl Transcript {
    fn scroll_offset(&self) -> Pixels {
        -self.list.scroll_px_offset_for_scrollbar().y
    }

    /// GPUI 列表贴底时以「最后一项之后、偏移 0」表示位置，`scroll_by` 会把它当作「内容总高」
    /// 而非「总高 − 视口」来计算 → 从贴底上滑不足一屏时被夹回底部、毫无反应（T18 发现）。
    /// 先换成等价的显式位置（从第 0 项起算当前像素位置），再做增量滚动。
    fn make_position_explicit(&self, current: Pixels) {
        if self.list.logical_scroll_top().item_ix >= self.list.item_count() {
            let following = self.list.is_following_tail();
            self.list.scroll_to(gpui::ListOffset {
                item_ix: 0,
                offset_in_item: px(0.),
            });
            self.list.scroll_by(current);
            if !following {
                self.list.pause_following_tail();
            }
        }
    }

    /// 接管列表区域的滚轮：换算成像素、累加目标、逐帧缓动（v1 `useSmoothWheelScroll`）
    fn on_wheel(&mut self, delta: Pixels, cx: &mut Context<Self>) {
        let current = self.scroll_offset();
        let max = self.list.max_offset_for_scrollbar().y;
        // 已到边缘且继续同向：停止动画（v1）
        if (delta < px(0.) && current <= px(0.)) || (delta > px(0.) && current >= max) {
            self.wheel.task = None;
            self.wheel.last_animated = None;
            return;
        }
        // v1 `onUserScrollIntent`：向上滚立即脱离跟随，不等首个动画帧（否则流式跟随会在这一帧把列表拉回）
        if delta < px(0.) {
            self.list.pause_following_tail();
        }
        self.make_position_explicit(current);
        let animating = self.wheel.task.is_some();
        if animating
            && self
                .wheel
                .last_animated
                .is_some_and(|last| (current - last).abs() > px(WHEEL_EXTERNAL_TOLERANCE_PX))
        {
            self.wheel.task = None;
        }
        if self.wheel.task.is_none() {
            self.wheel.target = current;
        }
        self.wheel.target = (self.wheel.target + delta).clamp(px(0.), max);
        if self.wheel.task.is_some() {
            return;
        }
        // 立即走第一步：否则下一次布局时视口仍在底部，列表会重新进入跟随并贴底，
        // 随后被动画当作「外部改动」而放弃 —— 流式中上滑就脱离不了（T18 发现）
        let first = (self.wheel.target - current) * WHEEL_EASING;
        self.list.scroll_by(first);
        self.wheel.last_animated = Some(self.scroll_offset());
        cx.notify();
        self.wheel.task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let keep = this.update(cx, |t, cx| {
                    let current = t.scroll_offset();
                    if t.wheel.last_animated.is_some_and(|last| {
                        (current - last).abs() > px(WHEEL_EXTERNAL_TOLERANCE_PX)
                    }) {
                        t.wheel.last_animated = None;
                        return false;
                    }
                    let max = t.list.max_offset_for_scrollbar().y;
                    t.wheel.target = t.wheel.target.clamp(px(0.), max);
                    let distance = t.wheel.target - current;
                    let step = if distance.abs() <= px(WHEEL_SETTLE_PX) {
                        distance
                    } else {
                        distance * WHEEL_EASING
                    };
                    t.list.scroll_by(step);
                    t.wheel.last_animated = Some(t.scroll_offset());
                    cx.notify();
                    distance.abs() > px(WHEEL_SETTLE_PX)
                });
                if !matches!(keep, Ok(true)) {
                    let _ = this.update(cx, |t, _| {
                        t.wheel.task = None;
                        t.wheel.last_animated = None;
                    });
                    break;
                }
            }
        }));
    }

    /// 在捕获阶段接管列表区域的纵向滚轮（先于列表自身的冒泡处理），交给平滑滚动。
    /// Ctrl / Cmd / Shift 或横向为主的滚动不接管（v1 同），代码块的横向滚动照常。
    fn wheel_interceptor(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let weak = cx.entity().downgrade();
        let nested = self.nested_scroll.clone();
        gpui::canvas(
            |bounds, window, _| window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal),
            move |bounds, hitbox, window, _| {
                let weak = weak.clone();
                let nested = nested.clone();
                let hitbox = hitbox.clone();
                window.on_mouse_event(move |event: &gpui::ScrollWheelEvent, phase, window, cx| {
                    // 捕获阶段也必须尊重设置等前景覆盖层；仅检查 bounds 会吞掉其滚轮。
                    if phase != gpui::DispatchPhase::Capture
                        || !bounds.contains(&event.position)
                        || !hitbox.should_handle_scroll(window)
                    {
                        return;
                    }
                    let m = event.modifiers;
                    let (dx, dy) = match event.delta {
                        gpui::ScrollDelta::Pixels(p) => (p.x.abs(), p.y.abs()),
                        gpui::ScrollDelta::Lines(l) => (px(l.x.abs()), px(l.y.abs())),
                    };
                    if m.control || m.platform || m.shift || dx > dy {
                        return;
                    }
                    let delta = wheel_delta_px(&event.delta);
                    if delta == px(0.) {
                        return;
                    }
                    // 落在还能继续滚的内层区域（工具详情）：让给它
                    let yield_to_inner = nested.borrow().iter().any(|h| {
                        h.bounds().contains(&event.position)
                            && if delta < px(0.) {
                                h.offset().y < px(0.)
                            } else {
                                -h.offset().y < h.max_offset().y
                            }
                    });
                    if yield_to_inner {
                        let _ = weak.update(cx, |t, _| t.wheel.task = None);
                        return;
                    }
                    let _ = weak.update(cx, |t, cx| t.on_wheel(delta, cx));
                    cx.stop_propagation();
                });
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }

    /// 会话状态（自检用）
    pub fn conversation_state<'a>(&self, cx: &'a App) -> &'a super::state::ChatState {
        &self.conversation.read(cx).state
    }

    /// 切换工具卡片展开（自检用，与点击标题行同一路径）
    pub fn toggle_tool_for_test(&mut self, row_id: &str, cx: &mut Context<Self>) {
        let state = &self.conversation.read(cx).state;
        let current = self.tool_expanded.get(row_id).copied().unwrap_or_else(|| {
            self.rows
                .iter()
                .find(|r| r.id == row_id)
                .and_then(|r| match &r.kind {
                    RowKind::Tool { call, .. } => {
                        state.tools.get(call).map(|t| default_open(t, false, false))
                    }
                    _ => None,
                })
                .unwrap_or(false)
        });
        self.set_tool_expanded(row_id, !current, cx);
    }

    /// 操作栏「已复制」反馈，1.6 秒后恢复（v1）
    fn mark_copied(&mut self, row_id: String, cx: &mut Context<Self>) {
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(message_actions::COPIED_FEEDBACK)
                .await;
            let _ = this.update(cx, |t, cx| {
                t.copied_actions = None;
                cx.notify();
            });
        });
        self.copied_actions = Some((row_id, task));
        cx.notify();
    }

    /// 回到问题：把该行滚到视口顶部，并脱离跟随（v1 `scrollIntoView({ block: 'start' })`）
    pub fn scroll_to_row(&mut self, row_id: &str, cx: &mut Context<Self>) {
        if let Some(ix) = self.rows.iter().position(|r| r.id == row_id) {
            self.list.scroll_to(gpui::ListOffset {
                item_ix: ix,
                offset_in_item: px(0.),
            });
            cx.notify();
        }
    }

    /// 预览中定位到一张卡片时避免尾部跟随把目标行挤出视口。
    pub fn stop_following_for_test(&mut self, cx: &mut Context<Self>) {
        self.list.set_follow_mode(FollowMode::Normal);
        cx.notify();
    }

    /// 设置工具卡片展开（用户点击）：记住用户选择并重测该行
    fn set_tool_expanded(&mut self, row_id: &str, expanded: bool, cx: &mut Context<Self>) {
        self.tool_expanded.insert(row_id.to_string(), expanded);
        if let Some(ix) = self.rows.iter().position(|r| r.id == row_id) {
            self.list.remeasure_items(ix..ix + 1);
            self.remeasured_rows += 1;
        }
        cx.notify();
    }

    /// 当前处于「已复制」的操作栏行（自检用）
    pub fn copied_row_for_test(&self) -> Option<String> {
        self.copied_actions.as_ref().map(|(id, _)| id.clone())
    }

    /// 工具详情的卡内滚动句柄（自检用）
    pub fn detail_scroll_for_test(&self, detail_id: &str) -> Option<ScrollHandle> {
        self.detail_scroll.get(detail_id).cloned()
    }

    /// 最后一条可见消息（tool 消息不显示）
    fn last_visible_message(&self, cx: &App) -> Option<String> {
        self.conversation
            .read(cx)
            .state
            .messages
            .iter()
            .rev()
            .find(|m| m.role != MessageRole::Tool)
            .map(|m| m.id.clone())
    }

    /// 是否显示「滚动到底部」按钮（v1：离开底部、不在流式、且有消息）
    pub fn scroll_button_visible(&self, cx: &App) -> bool {
        !self.list.is_following_tail()
            && !self.conversation.read(cx).state.is_streaming()
            && !self.rows.is_empty()
    }

    /// 是否有未读新消息（v1 `hasUnseenMessages`）
    pub fn has_unseen(&self, cx: &App) -> bool {
        !self.list.is_following_tail() && self.last_visible_message(cx) != self.last_seen
    }

    /// 平滑滚到底部后恢复跟随（v1 `scrollTo({ behavior: 'smooth' })`）
    pub fn scroll_to_bottom(&mut self, cx: &mut Context<Self>) {
        if crate::accessibility::prefers_reduced_motion() {
            self.list.set_follow_mode(FollowMode::Tail);
            cx.notify();
            return;
        }
        let list = self.list.clone();
        self.scroll_animation = Some(cx.spawn(async move |this, cx| {
            // 每帧走剩余距离的一部分，约 250ms 内到底（缓动由距离递减自然形成）
            for _ in 0..16 {
                let remaining = list.max_offset_for_scrollbar().y
                    - list.scroll_px_offset_for_scrollbar().y.abs();
                if remaining <= px(1.0) {
                    break;
                }
                list.scroll_by(remaining * 0.3);
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    return;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
            }
            let _ = this.update(cx, |this, cx| {
                this.list.set_follow_mode(FollowMode::Tail);
                this.scroll_animation = None;
                cx.notify();
            });
        }));
    }

    /// v1 `.scroll-to-bottom-button`：32px 圆、`--bg-elevated`、`--border-default`、`--shadow-floating-sm`、ChevronDown 16px
    fn scroll_button(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.buddy_theme();
        let c = theme.colors;
        let pulse = self
            .unseen_pulse
            .map(|t| t.elapsed().as_secs_f64() * 1000.0);
        // 新消息脉冲：v1 `scroll-new-message-pulse`（--duration-slow，0%→45%→100%：不透明 0→0.55→0、缩放 0.88→1.08）
        let ring = pulse
            .filter(|ms| *ms < f64::from(crate::theme_system::tokens::motion::DURATION_SLOW))
            .map(|ms| {
                let t = (ms / f64::from(crate::theme_system::tokens::motion::DURATION_SLOW)) as f32;
                let ease = crate::theme_system::easing::cubic_bezier(
                    crate::theme_system::tokens::motion::EASE_STANDARD,
                );
                let opacity = if t < 0.45 {
                    0.55 * ease(t / 0.45)
                } else {
                    0.55 * (1.0 - ease((t - 0.45) / 0.55))
                };
                let grow = (m::SPACE_1) * (0.88 + 0.2 * ease(t));
                div()
                    .absolute()
                    .top(px(-grow))
                    .left(px(-grow))
                    .size(px(m::SPACE_8 + 2.0 * grow))
                    .rounded(px(m::RADIUS_FULL))
                    .border_1()
                    .border_color(Hsla::from(c.buddy_primary).opacity(opacity))
            });
        if ring.is_some() && !crate::accessibility::prefers_reduced_motion() {
            window.request_animation_frame();
        }
        div()
            .absolute()
            .bottom(px(m::SPACE_4))
            .right(px(m::SPACE_4))
            .child(
                div()
                    .id("scroll-to-bottom")
                    .relative()
                    .size(px(m::SPACE_8))
                    .rounded(px(m::RADIUS_FULL))
                    .bg(c.bg_elevated)
                    .border_1()
                    .border_color(c.border_default)
                    .shadow(box_shadows(theme.shadows.shadow_floating_sm))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(c.text_muted)
                    .hover(|s| s.text_color(c.text_primary).bg(c.bg_surface))
                    .children(ring)
                    .child(icon(IconName::ChevronDown, px(16.0)))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _, cx| this.scroll_to_bottom(cx))),
            )
    }
}

impl Render for Transcript {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 到达底部即把现有消息标为已看（v1）；离开底部后新消息到达 → 外圈脉冲一次
        let visible = self.last_visible_message(cx);
        if self.list.is_following_tail() {
            self.last_seen = visible;
            self.unseen_pulse = None;
        } else if visible != self.last_seen && self.unseen_pulse.is_none() {
            self.unseen_pulse = Some(Instant::now());
        }
        let show_button = self.scroll_button_visible(cx);
        // 距顶 ≤56px 时加载更早一页（v1 `handleScroll`）。v1 只在滚动事件里判断，内容不足一屏时永远不加载；
        // 这里每帧判断 → 不足一屏会自动补到一屏以上
        let history = self.conversation.read(cx).state.history;
        let just_prepended = self
            .seen_history_offset
            .is_some_and(|o| o != history.offset);
        self.seen_history_offset = Some(history.offset);
        if history.has_more
            && !history.loading
            && !just_prepended
            && self.scroll_offset() <= px(LOAD_OLDER_THRESHOLD_PX)
        {
            // v1 `loadOlderHistory` 先取消平滑滚轮（并入后位置会整体变化）
            self.wheel.task = None;
            self.wheel.last_animated = None;
            let conversation = self.conversation.clone();
            cx.defer(move |cx| conversation.update(cx, |c, cx| c.load_older(cx)));
        }
        let loading_older = history.has_more && history.loading;
        // 本帧重新登记可见的内层滚动区域（行渲染时写入）
        self.nested_scroll.borrow_mut().clear();
        // 行绘制边界同样只保留本帧（否则未重绘的行留着过期位置）
        self.painted_rows.borrow_mut().clear();
        let painted_rows = self.painted_rows.clone();
        let drag_source = self.drag_source.clone();
        // 陷阱 1：`list()` 自己 flex_grow；v1 列表上下内边距 space-3 / space-2
        div()
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                // A row's measured screen bounds are the exclusion zone for
                // glyph selection, links, and row controls. Those events may
                // bubble to this root without becoming a window drag.
                if painted_rows
                    .borrow()
                    .values()
                    .any(|bounds| bounds.contains(&event.position))
                {
                    return;
                }
                cx.stop_propagation();
                drag::invoke(&drag_source, window);
            })
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .pt(px(m::SPACE_3))
            .pb(px(m::SPACE_2))
            .child(
                list(
                    self.list.clone(),
                    cx.processor(|this, ix, window, cx| this.render_row(ix, window, cx)),
                )
                .flex_grow(1.)
                .size_full(),
            )
            .child(self.wheel_interceptor(cx))
            .when(loading_older, |d| d.child(loading_older_banner(cx)))
            .when(show_button, |d| d.child(self.scroll_button(window, cx)))
    }
}

/// 工具卡的默认展开：网络搜索与生图卡片默认折叠（v1 `useState(false)`），其余按通用规则
fn default_open(tool: &super::state::ToolView, streaming: bool, awaiting: bool) -> bool {
    !web_search::is_web_search(&tool.name)
        && !image_gen::is_image_gen(&tool.name)
        && tool_card::default_expanded(tool, streaming, awaiting)
}
