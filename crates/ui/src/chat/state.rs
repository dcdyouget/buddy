//! 对话状态（S05-17）—— 逐项移植 v1 `src/stores/chatStore.ts` 中与界面相关的部分（`v1-final`）
//!
//! v1 的 1447 行 store 里，调用模型、工具循环、持久化已由 engine（S02-07）承担；
//! 界面只需消费 [`StreamEvent`] 并维护可显示的状态。本模块是纯数据逻辑（无 GPUI），
//! 行为与 v1 逐条对应，v1 的 14 个 store 用例全部移植为本模块测试。
//!
//! # 流式事件的两条路径（v1 `useStreaming.ts` + `queueStreamEvent`）
//!
//! - `Start` 忽略；`Done` / `Error` 为终态（见 [`ChatState::push_event`]）。
//! - 其余事件进入**同一个先进先出队列**：正文增量交给节奏器（[`Pacer`]，S04-06）逐字放出，
//!   **排在未放完正文之后的结构事件必须等待**（否则第二轮的内容会抢在第一轮正文之前出现）。
//!   相邻的正文增量合并，使数千个 1 字片段变成一个可按积压量追赶的缓冲。
//!
//! # 回合与消息（v1 `_ensureStreamingAssistantTurn` / `_commitCurrentAssistantTurn`）
//!
//! engine 每个工具轮持久化一条 assistant 消息，其后是对应的 tool 消息。界面同样如此：
//! 最后一条消息不是 assistant（即工具结果之后）时，下一个内容事件新建一条 assistant。
//! 流式中正文先在 `LiveTurn::blocks` 中累积，**只在结构边界提交**到消息（工具事件、回合结束、完成 / 出错）。

use crate::markdown::streaming::Pacer;
use buddy_engine::models::{ImageAttachment, Message, MessageRole, ToolCall};
use buddy_engine::streaming::{ContentBlock, QuestionOption, StreamEvent};
use std::collections::{HashMap, VecDeque};

/// 工具调用的界面状态（v1 `ToolCallStatus`）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolStatus {
    /// 模型正在生成调用（参数流式到达中）
    Calling,
    /// 后端执行中
    Executing,
    /// 成功
    Done,
    /// 失败
    Error,
    /// 历史中找不到结果：进程退出或主动停止时被中断
    Interrupted,
}

/// 一次工具调用在界面上的全部信息（v1 前端 `ToolCall` 的扩展字段）
#[derive(Clone, Debug, PartialEq)]
pub struct ToolView {
    /// 模型给出的调用 id
    pub id: String,
    /// 工具名
    pub name: String,
    /// 参数（原始 JSON 字符串，流式时可能不完整）
    pub arguments: String,
    /// 状态
    pub status: ToolStatus,
    /// 结果文本
    pub result: Option<String>,
    /// 结果是否为错误
    pub is_error: bool,
    /// 工具产生的展示图片
    pub images: Vec<ImageId>,
    /// 插入位置：在该内容块之后（-1 = 第一个块之前）；历史消息无此信息 → `None`
    pub insert_after: Option<i32>,
}

/// 图片附件在界面状态中的引用（完整附件留在消息里）
pub type ImageId = String;

/// 待审批的工具调用（v1 `toolApproval`）
#[derive(Clone, Debug, PartialEq)]
pub struct Approval {
    /// 调用 id
    pub id: String,
    /// 工具名
    pub name: String,
    /// 参数
    pub arguments: String,
    /// 审批原因
    pub reason: String,
}

/// 等待用户回答的 ask_user 问题（v1 `pendingQuestion`）
#[derive(Clone, Debug)]
pub struct Question {
    /// 调用 id
    pub id: String,
    /// 问题
    pub question: String,
    /// 选项
    pub options: Vec<QuestionOption>,
    /// 是否多选
    pub multi_select: bool,
    /// 短标签
    pub header: String,
}

/// 正在进行的一次对话（v1 `isStreaming` 期间的全部临时状态）
#[derive(Debug)]
pub struct LiveTurn {
    /// 本次使用的模型
    pub model_id: String,
    /// 当前 assistant 的实时内容块（v1 `streamingBlocks`）
    pub blocks: Vec<ContentBlock>,
    /// 本轮的工具调用，按出现顺序（v1 `activeToolCalls`，对象键序即插入序）
    active: Vec<ToolView>,
    /// 尚未应用的事件（v1 `pendingStreamEvents`）
    queue: VecDeque<StreamEvent>,
    /// 正文节奏器；其缓冲即 v1 `pendingTextBuffer`
    pacer: Pacer,
    /// 后端已结束，等队列放完再收尾（v1 `streamDonePending`）
    done_pending: bool,
    /// 最近一批放出的字数（v1 `streamingRevealCount`，S04-06 落定效果用）
    pub reveal_count: usize,
    /// 最近一批放出的时刻（毫秒）
    pub batch_at: f64,
}

/// 聊天界面的状态
#[derive(Debug, Default)]
pub struct ChatState {
    /// 全部消息（含 tool 消息；显示时由行模型过滤）
    pub messages: Vec<Message>,
    /// 工具调用的界面状态，按调用 id（历史水化 + 流式更新）
    pub tools: HashMap<String, ToolView>,
    /// 流式中的临时状态；`None` = 未在流式
    pub live: Option<LiveTurn>,
    /// 待审批
    pub approval: Option<Approval>,
    /// 待回答
    pub question: Option<Question>,
    /// 错误提示（v1 `error`）
    pub error: Option<String>,
    /// 任何可见变化都递增（视图据此决定是否刷新）
    pub revision: u64,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// 新建一条 assistant 消息（v1 `createStreamingAssistantMessage`）
fn assistant_message(model_id: &str) -> Message {
    Message {
        id: format!("live-a-{}", unique_suffix()),
        role: MessageRole::Assistant,
        content: String::new(),
        images: Vec::new(),
        blocks: Some(Vec::new()),
        model_id: Some(model_id.to_string()),
        created_at: now_secs(),
        tool_calls: None,
        tool_call_id: None,
        tool_name: None,
        is_error: None,
        parent_message_id: None,
    }
}

/// 进程内唯一后缀（界面生成的消息 id 用）
pub fn unique_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!("{millis}-{}", SEQ.fetch_add(1, Ordering::Relaxed))
}

/// 参数是否为完整的 JSON 对象（v1 `getCompleteToolCalls` 的判定）
fn is_complete_call(tool: &ToolView) -> bool {
    !tool.id.trim().is_empty()
        && !tool.name.trim().is_empty()
        && serde_json::from_str::<serde_json::Value>(&tool.arguments).is_ok_and(|v| v.is_object())
}

/// 可写入历史的调用：已有结果且参数完整（v1 `getPersistableToolCalls`）
fn is_persistable_call(tool: &ToolView) -> bool {
    matches!(tool.status, ToolStatus::Done | ToolStatus::Error) && is_complete_call(tool)
}

fn to_tool_call(tool: &ToolView) -> ToolCall {
    ToolCall { id: tool.id.clone(), name: tool.name.clone(), arguments: tool.arguments.clone() }
}

/// 关闭思考块并去掉末尾的空文本分隔块（v1 `finalizeStreamingBlocks`）
pub fn finalize_blocks(blocks: &[ContentBlock]) -> Vec<ContentBlock> {
    let mut out: Vec<ContentBlock> = blocks
        .iter()
        .map(|b| match b {
            ContentBlock::Thinking { content, .. } => ContentBlock::Thinking { content: content.clone(), is_open: false },
            other => other.clone(),
        })
        .collect();
    while matches!(out.last(), Some(ContentBlock::Text { content }) if content.is_empty()) {
        out.pop();
    }
    out
}

/// 纯正文（v1 `textContentFromBlocks`）
pub fn text_of(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { content } => Some(content.as_str()),
            ContentBlock::Thinking { .. } => None,
        })
        .collect()
}

fn same_block(a: &ContentBlock, b: &ContentBlock) -> bool {
    match (a, b) {
        (ContentBlock::Text { content: x }, ContentBlock::Text { content: y }) => x == y,
        (ContentBlock::Thinking { content: x, is_open: ox }, ContentBlock::Thinking { content: y, is_open: oy }) => x == y && ox == oy,
        _ => false,
    }
}

/// v1 `endsWithBlocks`
fn ends_with_blocks(blocks: &[ContentBlock], suffix: &[ContentBlock]) -> bool {
    if suffix.is_empty() || suffix.len() > blocks.len() {
        return false;
    }
    let offset = blocks.len() - suffix.len();
    suffix.iter().enumerate().all(|(i, s)| same_block(&blocks[offset + i], s))
}

impl ChatState {
    /// 以历史消息建立状态（v1 `setMessages` + `hydrateHistoryMessages`）
    pub fn from_history(messages: Vec<Message>) -> Self {
        let mut state = Self { messages, ..Default::default() };
        state.hydrate();
        state
    }

    /// 在开头并入更早的历史（v1 `loadOlderMessages`：按 id 去重）
    pub fn prepend_history(&mut self, older: Vec<Message>) {
        let existing: std::collections::HashSet<&str> = self.messages.iter().map(|m| m.id.as_str()).collect();
        let mut merged: Vec<Message> = older.into_iter().filter(|m| !existing.contains(m.id.as_str())).collect();
        merged.append(&mut self.messages);
        self.messages = merged;
        self.hydrate();
        self.touch();
    }

    /// 历史水化：无 `blocks` 的旧消息按 `<think>` 标签拆块；工具调用按 tool 消息恢复状态，
    /// 找不到结果的记为「已中断」（v1 `hydrateHistoryMessages`）
    fn hydrate(&mut self) {
        let results: HashMap<&str, &Message> = self
            .messages
            .iter()
            .filter(|m| m.role == MessageRole::Tool)
            .filter_map(|m| m.tool_call_id.as_deref().map(|id| (id, m)))
            .collect();
        let mut updates = Vec::new();
        for m in self.messages.iter().filter(|m| m.role == MessageRole::Assistant) {
            for call in m.tool_calls.iter().flatten() {
                if let Some(result) = results.get(call.id.as_str()) {
                    let is_error = result.is_error == Some(true);
                    updates.push(ToolView {
                        id: call.id.clone(),
                        name: call.name.clone(),
                        arguments: call.arguments.clone(),
                        status: if is_error { ToolStatus::Error } else { ToolStatus::Done },
                        result: Some(result.content.clone()),
                        is_error,
                        images: result.images.iter().map(|i| i.id.clone()).collect(),
                        insert_after: self.tools.get(&call.id).and_then(|t| t.insert_after),
                    });
                } else if !self.tools.contains_key(&call.id) {
                    updates.push(ToolView {
                        id: call.id.clone(),
                        name: call.name.clone(),
                        arguments: call.arguments.clone(),
                        status: ToolStatus::Interrupted,
                        result: None,
                        is_error: false,
                        images: Vec::new(),
                        insert_after: None,
                    });
                }
            }
        }
        for t in updates {
            self.tools.insert(t.id.clone(), t);
        }
        for m in self.messages.iter_mut().filter(|m| m.role == MessageRole::Assistant) {
            if m.blocks.as_ref().is_none_or(|b| b.is_empty()) && !m.content.is_empty() {
                m.blocks = Some(ContentBlock::parse_from_text(&m.content));
            }
        }
    }

    fn touch(&mut self) {
        self.revision += 1;
    }

    /// 是否在流式中
    pub fn is_streaming(&self) -> bool {
        self.live.is_some()
    }

    /// 发送：追加用户消息与空 assistant 占位，进入流式（v1 `sendMessage` 的前半段）
    pub fn begin_send(&mut self, user: Message, model_id: &str) {
        self.messages.push(user);
        self.messages.push(assistant_message(model_id));
        self.live = Some(LiveTurn {
            model_id: model_id.to_string(),
            blocks: Vec::new(),
            active: Vec::new(),
            queue: VecDeque::new(),
            pacer: Pacer::new(),
            done_pending: false,
            reveal_count: 0,
            batch_at: 0.0,
        });
        self.approval = None;
        self.question = None;
        self.error = None;
        self.touch();
    }

    /// 发送在占用生成通道前即被拒绝（engine 返回 `Err`，无任何事件）：移除空占位并报错
    pub fn send_rejected(&mut self, message: String) {
        self.remove_empty_trailing_assistant();
        self.live = None;
        self.error = Some(message);
        self.touch();
    }

    fn remove_empty_trailing_assistant(&mut self) {
        if let Some(last) = self.messages.last()
            && last.role == MessageRole::Assistant
            && last.content.is_empty()
            && last.blocks.as_ref().is_none_or(|b| b.is_empty())
        {
            self.messages.pop();
        }
    }

    /// 接收一个流式事件（v1 `useStreaming.ts` 的分派）
    pub fn push_event(&mut self, event: StreamEvent, now: f64) {
        let Some(live) = self.live.as_mut() else { return };
        match event {
            StreamEvent::Start => {}
            StreamEvent::Done { .. } => {
                live.done_pending = true;
                self.drain(now);
            }
            StreamEvent::Error { message, .. } => self.handle_error(message, now),
            StreamEvent::TextDelta { delta, .. } => {
                // 相邻正文增量合并；正在放出的正文即队首（v1 中缓冲镜像队首）
                match live.queue.back_mut() {
                    Some(StreamEvent::TextDelta { delta: tail, .. }) => tail.push_str(&delta),
                    None if !live.pacer.pending().is_empty() => {
                        if let Some(batch) = live.pacer.push(&delta, now) {
                            self.apply_text(batch, now);
                        }
                    }
                    _ => live.queue.push_back(StreamEvent::TextDelta { content_index: 0, delta }),
                }
                self.drain(now);
            }
            other => {
                live.queue.push_back(other);
                self.drain(now);
            }
        }
    }

    /// 每帧调用：按节奏放出正文，放完后继续处理排队的事件（v1 `smoothTextDelta`）
    pub fn tick(&mut self, now: f64) {
        let Some(live) = self.live.as_mut() else { return };
        if let Some(batch) = live.pacer.tick(now) {
            self.apply_text(batch, now);
            self.drain(now);
        }
    }

    /// 窗口隐藏 / 失焦：立即放出全部（v1 `enterBackgroundMode`）
    pub fn hide(&mut self, now: f64) {
        if let Some(batch) = self.live.as_mut().and_then(|l| l.pacer.hide()) {
            self.apply_text(batch, now);
        }
        self.drain(now);
    }

    /// 窗口重新显示（v1 `leaveBackgroundMode`）
    pub fn show(&mut self, now: f64) {
        if let Some(batch) = self.live.as_mut().and_then(|l| l.pacer.show(now)) {
            self.apply_text(batch, now);
        }
        self.drain(now);
    }

    /// 立即放出全部缓冲并处理完队列（v1 `flushTextBuffer`）
    pub fn flush(&mut self, now: f64) {
        loop {
            let Some(live) = self.live.as_mut() else { return };
            if let Some(batch) = live.pacer.flush() {
                self.apply_text(batch, now);
            }
            let Some(live) = self.live.as_mut() else { return };
            match live.queue.front() {
                Some(StreamEvent::TextDelta { .. }) => {
                    if let Some(StreamEvent::TextDelta { delta, .. }) = live.queue.pop_front() {
                        self.apply_text(delta, now);
                    }
                }
                Some(_) => self.drain(now),
                None => {
                    self.drain(now);
                    return;
                }
            }
        }
    }

    /// 是否仍有待放出的正文或待处理事件
    pub fn has_pending(&self) -> bool {
        self.live.as_ref().is_some_and(|l| !l.queue.is_empty() || !l.pacer.pending().is_empty())
    }

    /// 处理队首事件，直到遇到尚未放完的正文（v1 `_drainStreamEventQueue`）
    fn drain(&mut self, now: f64) {
        loop {
            let Some(live) = self.live.as_mut() else { return };
            if !live.pacer.pending().is_empty() {
                return;
            }
            let Some(next) = live.queue.pop_front() else {
                if live.done_pending {
                    self.finish_done();
                }
                return;
            };
            match next {
                StreamEvent::TextDelta { delta, .. } => {
                    if let Some(batch) = live.pacer.push(&delta, now) {
                        // 立即模式（窗口隐藏）：整段放出，继续处理下一项
                        self.apply_text(batch, now);
                    }
                }
                StreamEvent::TextStart { content_index } => self.text_start(content_index),
                StreamEvent::TextEnd { content_index, content } => self.text_end(content_index, content),
                StreamEvent::ThinkingStart { .. } => self.thinking_start(),
                StreamEvent::ThinkingDelta { delta, .. } => self.thinking_delta(delta),
                StreamEvent::ThinkingEnd { content, .. } => self.thinking_end(content),
                StreamEvent::ToolCallStart { id, name, .. } => self.tool_call_start(id, name),
                StreamEvent::ToolCallDelta { id, arguments_delta } => self.tool_call_delta(id, arguments_delta),
                StreamEvent::ToolCallEnd { id, name, arguments } => self.tool_call_end(id, name, arguments),
                StreamEvent::ToolExecuting { id, name } => {
                    self.ensure_turn();
                    self.ensure_tool(&id, &name, ToolStatus::Executing);
                    self.commit(false);
                }
                StreamEvent::ToolResult { id, name, content, images, is_error } => self.tool_result(id, name, content, images, is_error),
                StreamEvent::ToolApprovalRequired { id, name, arguments, reason } => {
                    self.approval = Some(Approval { id, name, arguments, reason });
                }
                StreamEvent::ToolQuestionRequired { id, question, options, multi_select, header, .. } => {
                    self.question = Some(Question { id, question, options, multi_select, header });
                }
                StreamEvent::TurnEnd { .. } => self.commit(false),
                StreamEvent::Start | StreamEvent::Done { .. } | StreamEvent::Error { .. } => {}
            }
            self.touch();
        }
    }

    fn live(&mut self) -> &mut LiveTurn {
        self.live.as_mut().expect("仅在流式中调用")
    }

    /// 放出的正文追加到当前 assistant（v1 `appendStreamingText`）
    fn apply_text(&mut self, text: String, now: f64) {
        self.ensure_turn();
        let live = self.live();
        live.reveal_count = text.chars().count();
        live.batch_at = now;
        if let Some(ContentBlock::Thinking { is_open, .. }) = live.blocks.last_mut() {
            *is_open = false;
        }
        match live.blocks.last_mut() {
            Some(ContentBlock::Text { content }) => content.push_str(&text),
            _ => live.blocks.push(ContentBlock::Text { content: text }),
        }
        self.touch();
    }

    /// 最后一条消息不是 assistant 时新建一条（v1 `_ensureStreamingAssistantTurn`）
    fn ensure_turn(&mut self) {
        if self.messages.last().is_some_and(|m| m.role == MessageRole::Assistant) {
            return;
        }
        let model = self.live().model_id.clone();
        self.messages.push(assistant_message(&model));
        let live = self.live();
        live.blocks.clear();
        live.active.clear();
        live.reveal_count = 0;
    }

    fn last_assistant(&mut self) -> Option<&mut Message> {
        self.messages.iter_mut().rev().find(|m| m.role == MessageRole::Assistant)
    }

    /// 把实时内容写入当前 assistant（v1 `_commitCurrentAssistantTurn`）
    fn commit(&mut self, persistable_only: bool) {
        let live = self.live();
        let blocks = finalize_blocks(&live.blocks);
        let calls: Vec<ToolCall> = live
            .active
            .iter()
            .filter(|t| if persistable_only { is_persistable_call(t) } else { is_complete_call(t) })
            .map(to_tool_call)
            .collect();
        if let Some(a) = self.last_assistant() {
            a.content = text_of(&blocks);
            a.blocks = Some(blocks);
            a.tool_calls = (!calls.is_empty()).then_some(calls);
        }
    }

    fn text_start(&mut self, index: usize) {
        self.ensure_turn();
        let blocks = &mut self.live().blocks;
        while blocks.len() <= index {
            blocks.push(ContentBlock::Text { content: String::new() });
        }
    }

    /// v1 `handleTextEnd`：索引 0 为 OpenAI 单块模式（整段文本可能含 `<think>`），其余为 Anthropic 多块模式
    fn text_end(&mut self, index: usize, content: String) {
        let live = self.live();
        if index != 0 {
            while live.blocks.len() <= index {
                live.blocks.push(ContentBlock::Text { content: String::new() });
            }
            live.blocks[index] = ContentBlock::Text { content };
            return;
        }
        let blocks = &mut live.blocks;
        if !content.is_empty() {
            let parsed = ContentBlock::parse_from_text(&content);
            if ends_with_blocks(blocks, &parsed) {
                blocks.push(ContentBlock::Text { content: String::new() });
                return;
            }
            // 过期快照：多轮时本轮 text_end 可能晚于后续轮的正文到达，只是累积正文的严格前缀
            let accumulated = text_of(blocks);
            let stale = accumulated.len() > content.len() && accumulated.starts_with(&content);
            let last = blocks.len() as isize - 1;
            let trailing_separator = matches!(blocks.last(), Some(ContentBlock::Text { content }) if content.is_empty());
            let target = if trailing_separator { last - 1 } else { last };
            if target >= 0 {
                let target = target as usize;
                let replacement = match (&blocks[target], stale) {
                    (ContentBlock::Text { content: current }, true) => ContentBlock::parse_from_text(current),
                    _ => parsed,
                };
                blocks.splice(target..=target, replacement);
            } else {
                blocks.extend(parsed);
            }
        }
        blocks.push(ContentBlock::Text { content: String::new() });
    }

    fn thinking_start(&mut self) {
        self.ensure_turn();
        let blocks = &mut self.live().blocks;
        // text_start 先建的空占位：内联 <think> 位于正文开头时移除，与 engine 持久化结构一致
        if matches!(blocks.last(), Some(ContentBlock::Text { content }) if content.is_empty()) {
            blocks.pop();
        }
        match blocks.last_mut() {
            Some(ContentBlock::Thinking { content, is_open: true }) => content.clear(),
            _ => blocks.push(ContentBlock::Thinking { content: String::new(), is_open: true }),
        }
    }

    fn thinking_delta(&mut self, delta: String) {
        self.ensure_turn();
        let blocks = &mut self.live().blocks;
        match blocks.iter_mut().rev().find(|b| matches!(b, ContentBlock::Thinking { is_open: true, .. })) {
            Some(ContentBlock::Thinking { content, .. }) => content.push_str(&delta),
            _ => blocks.push(ContentBlock::Thinking { content: delta, is_open: true }),
        }
    }

    fn thinking_end(&mut self, full: String) {
        self.ensure_turn();
        let blocks = &mut self.live().blocks;
        match blocks.iter_mut().rev().find(|b| matches!(b, ContentBlock::Thinking { is_open: true, .. })) {
            Some(b) => *b = ContentBlock::Thinking { content: full, is_open: false },
            None => blocks.push(ContentBlock::Thinking { content: full, is_open: false }),
        }
    }

    /// v1 `_ensureToolCallEntry`
    fn ensure_tool(&mut self, id: &str, name: &str, status: ToolStatus) {
        let live = self.live();
        match live.active.iter_mut().find(|t| t.id == id) {
            Some(t) => {
                t.name = name.to_string();
                t.status = status;
            }
            None => live.active.push(ToolView {
                id: id.to_string(),
                name: name.to_string(),
                arguments: String::new(),
                status,
                result: None,
                is_error: false,
                images: Vec::new(),
                insert_after: None,
            }),
        }
        self.sync_tool(id);
    }

    /// 把实时调用同步到全局工具表（行渲染只读 `tools`）
    fn sync_tool(&mut self, id: &str) {
        if let Some(t) = self.live.as_ref().and_then(|l| l.active.iter().find(|t| t.id == id)).cloned() {
            self.tools.insert(t.id.clone(), t);
        }
    }

    /// v1 `_computeInsertAfterBlockIndex`：最后一个非空块（跳过末尾的空分隔块）
    fn insert_after_index(&mut self) -> i32 {
        let blocks = &self.live().blocks;
        blocks
            .iter()
            .rposition(|b| match b {
                ContentBlock::Text { content } | ContentBlock::Thinking { content, .. } => !content.is_empty(),
            })
            .map_or(-1, |i| i as i32)
    }

    fn tool_call_start(&mut self, id: String, name: String) {
        self.ensure_turn();
        self.ensure_tool(&id, &name, ToolStatus::Calling);
        let index = self.insert_after_index();
        if let Some(t) = self.live().active.iter_mut().find(|t| t.id == id) {
            t.insert_after = Some(index);
        }
        self.sync_tool(&id);
        self.commit(false);
    }

    fn tool_call_delta(&mut self, id: String, delta: String) {
        let Some(t) = self.live().active.iter_mut().find(|t| t.id == id) else {
            return; // start 缺失时忽略（v1 同）
        };
        t.arguments.push_str(&delta);
        self.sync_tool(&id);
        self.commit(false);
    }

    fn tool_call_end(&mut self, id: String, name: String, arguments: String) {
        self.ensure_turn();
        let live = self.live();
        match live.active.iter_mut().find(|t| t.id == id) {
            Some(t) => {
                t.name = name;
                t.arguments = arguments;
                t.status = ToolStatus::Calling;
            }
            None => live.active.push(ToolView {
                id: id.clone(),
                name,
                arguments,
                status: ToolStatus::Calling,
                result: None,
                is_error: false,
                images: Vec::new(),
                insert_after: None,
            }),
        }
        self.sync_tool(&id);
        self.commit(false);
    }

    fn tool_result(&mut self, id: String, name: String, content: String, images: Vec<ImageAttachment>, is_error: bool) {
        let status = if is_error { ToolStatus::Error } else { ToolStatus::Done };
        let image_ids: Vec<ImageId> = images.iter().map(|i| i.id.clone()).collect();
        let live = self.live();
        match live.active.iter_mut().find(|t| t.id == id) {
            Some(t) => {
                t.name = name.clone();
                t.status = status;
                t.result = Some(content.clone());
                t.is_error = is_error;
                t.images = image_ids;
            }
            None => live.active.push(ToolView {
                id: id.clone(),
                name: name.clone(),
                arguments: String::new(),
                status,
                result: Some(content.clone()),
                is_error,
                images: image_ids,
                insert_after: None,
            }),
        }
        self.sync_tool(&id);
        let calls: Vec<ToolCall> = self.live().active.iter().filter(|t| is_complete_call(t)).map(to_tool_call).collect();
        if let Some(a) = self.last_assistant() {
            a.tool_calls = (!calls.is_empty()).then_some(calls);
        }
        self.messages.push(Message {
            id: format!("live-t-{}", unique_suffix()),
            role: MessageRole::Tool,
            content,
            images,
            blocks: None,
            model_id: None,
            created_at: now_secs(),
            tool_calls: None,
            tool_call_id: Some(id),
            tool_name: Some(name),
            is_error: Some(is_error),
            parent_message_id: None,
        });
    }

    /// 完成：只保留已有结果且参数完整的调用（v1 `handleStreamDone` 的收尾）
    fn finish_done(&mut self) {
        self.commit(true);
        self.end_live();
    }

    /// 出错：先放出全部已到正文，只保留可写入历史的调用；什么都没产出时移除空占位（v1 `handleStreamError`）
    fn handle_error(&mut self, message: String, now: f64) {
        if let Some(live) = self.live.as_mut() {
            live.done_pending = false;
        }
        // 放出全部正文并处理完排队事件，但不在其中触发「完成」收尾
        loop {
            let Some(live) = self.live.as_mut() else { break };
            if let Some(batch) = live.pacer.flush() {
                self.apply_text(batch, now);
            }
            let Some(live) = self.live.as_mut() else { break };
            if live.queue.is_empty() {
                break;
            }
            match live.queue.pop_front() {
                Some(StreamEvent::TextDelta { delta, .. }) => self.apply_text(delta, now),
                Some(other) => {
                    live.queue.push_front(other);
                    self.drain(now);
                }
                None => break,
            }
        }
        if self.live.is_some() {
            let blocks = finalize_blocks(&self.live().blocks);
            let calls: Vec<ToolCall> = self.live().active.iter().filter(|t| is_persistable_call(t)).map(to_tool_call).collect();
            let is_last = self.messages.last().is_some_and(|m| m.role == MessageRole::Assistant);
            if let Some(a) = self.last_assistant() {
                if !blocks.is_empty() {
                    a.content = text_of(&blocks);
                    a.blocks = Some(blocks);
                }
                let empty = a.content.is_empty() && a.blocks.as_ref().is_none_or(|b| b.is_empty());
                if empty && is_last {
                    self.messages.pop();
                } else if let Some(a) = self.last_assistant() {
                    a.tool_calls = (!calls.is_empty()).then_some(calls);
                }
            }
        }
        self.end_live();
        self.error = Some(message);
    }

    fn end_live(&mut self) {
        // 流式结束：未完成的调用不再显示为进行中（v1 清空 activeToolCalls，历史水化记为已中断）
        if let Some(live) = self.live.take() {
            for t in live.active {
                if matches!(t.status, ToolStatus::Calling | ToolStatus::Executing)
                    && let Some(view) = self.tools.get_mut(&t.id)
                {
                    view.status = ToolStatus::Interrupted;
                }
            }
        }
        self.approval = None;
        self.question = None;
        self.touch();
    }
}

#[cfg(test)]
mod tests {
    //! v1 `src/stores/chatStore.test.ts`（14 例）逐一移植；每个测试名后注明对应的 v1 用例
    use super::*;
    use buddy_engine::streaming::StopReason;

    fn text(s: &str) -> ContentBlock {
        ContentBlock::Text { content: s.into() }
    }
    fn think(s: &str, open: bool) -> ContentBlock {
        ContentBlock::Thinking { content: s.into(), is_open: open }
    }
    fn eq(a: &[ContentBlock], b: &[ContentBlock]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same_block(x, y))
    }
    #[track_caller]
    fn assert_blocks(a: &[ContentBlock], b: &[ContentBlock]) {
        assert!(eq(a, b), "\n实际 {a:?}\n期望 {b:?}");
    }

    /// v1 `beforeEach`：一条空 assistant、处于流式中
    fn streaming() -> ChatState {
        let mut s = ChatState::default();
        s.messages.push(assistant_message("test-model"));
        s.live = Some(LiveTurn {
            model_id: "test-model".into(),
            blocks: Vec::new(),
            active: Vec::new(),
            queue: VecDeque::new(),
            pacer: Pacer::new(),
            done_pending: false,
            reveal_count: 0,
            batch_at: 0.0,
        });
        s
    }

    fn blocks(s: &ChatState) -> &[ContentBlock] {
        &s.live.as_ref().unwrap().blocks
    }

    fn tool(s: &ChatState, id: &str) -> ToolView {
        s.live.as_ref().unwrap().active.iter().find(|t| t.id == id).unwrap().clone()
    }

    fn delta(s: &str) -> StreamEvent {
        StreamEvent::TextDelta { content_index: 0, delta: s.into() }
    }

    /// 以 v1 的「每次放出 n 字」直接驱动节奏器，避免依赖时间
    fn reveal(s: &mut ChatState, n: usize) {
        let live = s.live.as_mut().unwrap();
        let pending = live.pacer.pending().to_string();
        let split = pending.char_indices().nth(n).map_or(pending.len(), |(i, _)| i);
        let (batch, rest) = pending.split_at(split);
        let (batch, rest) = (batch.to_string(), rest.to_string());
        live.pacer = Pacer::new();
        if !rest.is_empty() {
            live.pacer.push(&rest, 0.0);
        }
        s.apply_text(batch, 0.0);
        s.drain(0.0);
    }

    #[test]
    fn tool_before_content_is_placed_before_first_block() {
        // v1「将内容出现前的工具调用记录在第一个 block 之前」
        let mut s = streaming();
        s.tool_call_start("call-before-content".into(), "websearch".into());
        assert_eq!(tool(&s, "call-before-content").insert_after, Some(-1));
        s.live.as_mut().unwrap().blocks = vec![think("后续思考", true)];
        s.tool_call_start("call-after-thinking".into(), "websearch".into());
        assert_eq!(tool(&s, "call-after-thinking").insert_after, Some(0));
    }

    #[test]
    fn structured_thinking_streams_incrementally() {
        // v1「按 Rust 输出的结构化事件增量展示思考内容」
        let mut s = streaming();
        s.text_start(0);
        s.thinking_start();
        assert_blocks(blocks(&s), &[think("", true)]);
        s.thinking_delta("正在".into());
        s.thinking_delta("分析".into());
        assert_blocks(blocks(&s), &[think("正在分析", true)]);
    }

    #[test]
    fn text_after_thinking_and_text_end_does_not_duplicate() {
        // v1「思考结束后切回正文，并且 text_end 不重复思考块」
        let mut s = streaming();
        s.text_start(0);
        s.thinking_start();
        s.thinking_delta("分析过程".into());
        s.thinking_end("分析过程".into());
        s.push_event(delta("最终答案"), 0.0);
        reveal(&mut s, 4);
        assert_blocks(blocks(&s), &[think("分析过程", false), text("最终答案")]);
        s.text_end(0, "<think>分析过程</think>最终答案".into());
        assert_blocks(blocks(&s), &[think("分析过程", false), text("最终答案"), text("")]);
    }

    #[test]
    fn text_delta_after_structured_thinking_is_body() {
        // v1「结构化思考块结束后，普通 text_delta 仍作为正文处理」
        let mut s = streaming();
        s.live.as_mut().unwrap().blocks = vec![think("结构化思考", true)];
        s.push_event(delta("正文"), 0.0);
        reveal(&mut s, 2);
        assert_blocks(blocks(&s), &[think("结构化思考", false), text("正文")]);
    }

    #[test]
    fn done_waits_for_paced_text_and_text_end() {
        // v1「等待逐字队列消费完毕后再提交 text_end 和 done」
        let mut s = streaming();
        s.push_event(delta("答案"), 0.0);
        s.push_event(StreamEvent::TextEnd { content_index: 0, content: "答案".into() }, 0.0);
        s.push_event(StreamEvent::Done { reason: StopReason::Stop, full_text: "答案".into() }, 0.0);
        assert!(s.is_streaming());
        assert_eq!(s.live.as_ref().unwrap().pacer.pending(), "答案");
        reveal(&mut s, 1);
        assert_eq!(s.messages[0].content, "");
        assert_blocks(blocks(&s), &[text("答")]);
        assert!(s.is_streaming());
        reveal(&mut s, 1);
        assert_eq!(s.messages[0].content, "答案");
        assert_blocks(s.messages[0].blocks.as_deref().unwrap(), &[text("答案")]);
        assert!(!s.is_streaming());
    }

    #[test]
    fn reveal_never_splits_surrogate_pairs() {
        // v1「逐字消费时不会拆开 Unicode 代理对字符」（v1 用 emoji；按硬约束 4 改用同为代理对的「𠮷」）
        let mut s = streaming();
        s.push_event(delta("A𠮷"), 0.0);
        reveal(&mut s, 1);
        assert_eq!(s.messages[0].content, "");
        assert_blocks(blocks(&s), &[text("A")]);
        reveal(&mut s, 1);
        assert_blocks(blocks(&s), &[text("A𠮷")]);
        assert_eq!(s.live.as_ref().unwrap().pacer.pending(), "");
    }

    #[test]
    fn error_commits_revealed_text() {
        // v1「流式错误时将已显示正文一次性提交到消息」
        let mut s = streaming();
        s.push_event(delta("已生成内容"), 0.0);
        s.flush(0.0);
        assert_eq!(s.messages[0].content, "");
        s.push_event(StreamEvent::Error { reason: StopReason::Error, message: "网络错误".into(), partial_text: String::new() }, 0.0);
        assert_eq!(s.messages[0].content, "已生成内容");
        assert_blocks(s.messages[0].blocks.as_deref().unwrap(), &[text("已生成内容")]);
    }

    fn with_active(s: &mut ChatState, t: ToolView) {
        s.tools.insert(t.id.clone(), t.clone());
        s.live.as_mut().unwrap().active.push(t);
    }

    fn view(id: &str, name: &str, args: &str, status: ToolStatus) -> ToolView {
        ToolView { id: id.into(), name: name.into(), arguments: args.into(), status, result: None, is_error: false, images: Vec::new(), insert_after: None }
    }

    #[test]
    fn done_drops_partial_json_calls_and_clears_interactions() {
        // v1「丢弃尚未完成且参数只有半截 JSON 的工具调用」
        let mut s = streaming();
        with_active(&mut s, view("partial", "ask_user", "{\"question\":\"尚未生成完", ToolStatus::Calling));
        s.question = Some(Question { id: "partial".into(), question: "等待回答".into(), options: Vec::new(), multi_select: false, header: "询问用户".into() });
        s.approval = Some(Approval { id: "partial".into(), name: "ask_user".into(), arguments: "{}".into(), reason: "测试".into() });
        s.push_event(StreamEvent::Done { reason: StopReason::Stop, full_text: String::new() }, 0.0);
        assert!(s.messages[0].tool_calls.is_none());
        assert!(s.live.is_none());
        assert!(s.question.is_none() && s.approval.is_none());
    }

    #[test]
    fn done_keeps_completed_json_calls() {
        // v1「保留已经完成且参数为 JSON 对象的工具调用」
        let mut s = streaming();
        let mut t = view("completed", "read_file", "{\"path\":\"/tmp/a.txt\"}", ToolStatus::Done);
        t.result = Some("ok".into());
        with_active(&mut s, t);
        s.push_event(StreamEvent::Done { reason: StopReason::Stop, full_text: String::new() }, 0.0);
        let calls = s.messages[0].tool_calls.as_ref().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].arguments, "{\"path\":\"/tmp/a.txt\"}");
        assert_eq!(s.tools["completed"].status, ToolStatus::Done);
    }

    #[test]
    fn error_does_not_persist_unfinished_calls() {
        // v1「流式报错时不会把未完成调用伪装成可复用的错误调用」
        let mut s = streaming();
        with_active(&mut s, view("partial", "ask_user", "{\"question\":", ToolStatus::Executing));
        s.push_event(StreamEvent::Error { reason: StopReason::Error, message: "网络错误".into(), partial_text: String::new() }, 0.0);
        assert!(s.messages.is_empty(), "空占位应被移除");
        assert!(s.live.is_none());
        assert_eq!(s.error.as_deref(), Some("网络错误"));
    }

    #[test]
    fn adjacent_text_deltas_merge_for_catch_up() {
        // v1「合并相邻小文本增量，让平滑渲染按积压量追赶」：100 个 1 字增量合并为一个缓冲
        let mut s = streaming();
        for _ in 0..100 {
            s.push_event(delta("字"), 0.0);
        }
        let live = s.live.as_ref().unwrap();
        assert_eq!(live.pacer.pending(), "字".repeat(100));
        assert!(live.queue.is_empty());
        reveal(&mut s, 16);
        assert_blocks(blocks(&s), &[text(&"字".repeat(16))]);
    }

    #[test]
    fn deltas_queued_behind_structure_event_merge() {
        // 正文未放完时到达的结构事件排队；其后的相邻正文增量在队列中合并为一项
        let mut s = streaming();
        s.push_event(delta("前"), 0.0);
        s.push_event(StreamEvent::ThinkingStart { content_index: 0 }, 0.0);
        for _ in 0..3 {
            s.push_event(delta("字"), 0.0);
        }
        let queue: Vec<_> = s.live.as_ref().unwrap().queue.iter().collect();
        assert_eq!(queue.len(), 2, "{queue:?}");
        assert!(matches!(queue[1], StreamEvent::TextDelta { delta, .. } if delta == "字字字"));
    }

    #[test]
    fn multi_turn_tool_flow_keeps_order_while_text_pending() {
        // v1「在首轮正文尚未渲染完时，按顺序保留工具轮与最终答案」
        let mut s = streaming();
        let events = vec![
            StreamEvent::TextStart { content_index: 0 },
            delta("先检索资料。"),
            StreamEvent::TextEnd { content_index: 0, content: "先检索资料。".into() },
            StreamEvent::ToolCallStart { id: "call-search".into(), name: "websearch".into(), content_index: 0 },
            StreamEvent::ToolCallDelta { id: "call-search".into(), arguments_delta: "{\"query\":\"Buddy\"}".into() },
            StreamEvent::ToolCallEnd { id: "call-search".into(), name: "websearch".into(), arguments: "{\"query\":\"Buddy\"}".into() },
            StreamEvent::TurnEnd { tool_calls_pending: 1 },
            StreamEvent::ToolExecuting { id: "call-search".into(), name: "websearch".into() },
            StreamEvent::ToolResult { id: "call-search".into(), name: "websearch".into(), content: "找到一条资料".into(), images: Vec::new(), is_error: false },
            StreamEvent::TextStart { content_index: 0 },
            delta("最终答案。"),
            StreamEvent::TextEnd { content_index: 0, content: "最终答案。".into() },
            StreamEvent::TurnEnd { tool_calls_pending: 0 },
            StreamEvent::Done { reason: StopReason::Stop, full_text: String::new() },
        ];
        for e in events {
            s.push_event(e, 0.0);
        }
        // 第一段正文仍在节奏器中，后续边界不能抢先写入第二轮
        assert_eq!(s.live.as_ref().unwrap().pacer.pending(), "先检索资料。");
        assert_eq!(s.messages.len(), 1);
        s.flush(0.0);
        assert!(!s.is_streaming());
        let roles: Vec<MessageRole> = s.messages.iter().map(|m| m.role.clone()).collect();
        assert_eq!(roles, vec![MessageRole::Assistant, MessageRole::Tool, MessageRole::Assistant]);
        assert_eq!(s.messages[0].content, "先检索资料。");
        let calls = s.messages[0].tool_calls.as_ref().unwrap();
        assert_eq!((calls[0].id.as_str(), calls[0].arguments.as_str()), ("call-search", "{\"query\":\"Buddy\"}"));
        assert_eq!(s.tools["call-search"].status, ToolStatus::Done);
        assert_eq!(s.tools["call-search"].result.as_deref(), Some("找到一条资料"));
        assert_eq!(s.messages[1].tool_call_id.as_deref(), Some("call-search"));
        assert_eq!(s.messages[1].content, "找到一条资料");
        assert_eq!(s.messages[2].content, "最终答案。");
        assert!(s.messages[2].tool_calls.is_none());
    }

    fn history_assistant(calls: Vec<ToolCall>) -> Message {
        let mut m = assistant_message("test-model");
        m.id = "assistant-test".into();
        m.tool_calls = Some(calls);
        m
    }

    fn tool_msg(id: &str, call: &str, name: &str, content: &str, is_error: bool, images: Vec<ImageAttachment>) -> Message {
        Message {
            id: id.into(),
            role: MessageRole::Tool,
            content: content.into(),
            images,
            blocks: None,
            model_id: None,
            created_at: 1,
            tool_calls: None,
            tool_call_id: Some(call.into()),
            tool_name: Some(name.into()),
            is_error: Some(is_error),
            parent_message_id: None,
        }
    }

    #[test]
    fn history_restores_done_with_images() {
        // v1「根据 tool result 将历史调用与展示图片恢复为已完成」
        let image = ImageAttachment { id: "generated-1".into(), name: "generated.png".into(), media_type: "image/png".into(), path: String::new(), data_url: "data:image/png;base64,aGVsbG8=".into() };
        let s = ChatState::from_history(vec![
            history_assistant(vec![ToolCall { id: "call-success".into(), name: "ask_user".into(), arguments: "{\"question\":\"选哪个？\"}".into() }]),
            tool_msg("tool-success", "call-success", "ask_user", "用户选择：继续", false, vec![image]),
        ]);
        let t = &s.tools["call-success"];
        assert_eq!(t.status, ToolStatus::Done);
        assert_eq!(t.result.as_deref(), Some("用户选择：继续"));
        assert!(!t.is_error);
        assert_eq!(t.images, vec!["generated-1".to_string()]);
    }

    #[test]
    fn history_restores_error_and_interrupted() {
        // v1「根据错误结果恢复为失败，没有结果的调用恢复为已中断」
        let s = ChatState::from_history(vec![
            history_assistant(vec![
                ToolCall { id: "call-error".into(), name: "read_file".into(), arguments: "{\"path\":\"/missing\"}".into() },
                ToolCall { id: "call-interrupted".into(), name: "ask_user".into(), arguments: "{\"question\":\"未完成\"}".into() },
            ]),
            tool_msg("tool-error", "call-error", "read_file", "文件不存在", true, Vec::new()),
        ]);
        assert_eq!(s.tools["call-error"].status, ToolStatus::Error);
        assert_eq!(s.tools["call-error"].result.as_deref(), Some("文件不存在"));
        assert!(s.tools["call-error"].is_error);
        assert_eq!(s.tools["call-interrupted"].status, ToolStatus::Interrupted);
    }

    #[test]
    fn history_without_blocks_parses_think_tags() {
        let mut m = assistant_message("m");
        m.blocks = None;
        m.content = "<think>想</think>答".into();
        let s = ChatState::from_history(vec![m]);
        assert_blocks(s.messages[0].blocks.as_deref().unwrap(), &[think("想", false), text("答")]);
    }

    #[test]
    fn hidden_window_reveals_immediately_and_drains() {
        let mut s = streaming();
        s.hide(0.0);
        s.push_event(delta("后台继续"), 0.0);
        s.push_event(StreamEvent::TextEnd { content_index: 0, content: "后台继续".into() }, 0.0);
        s.push_event(StreamEvent::Done { reason: StopReason::Stop, full_text: String::new() }, 0.0);
        assert!(!s.is_streaming());
        assert_eq!(s.messages[0].content, "后台继续");
    }

    #[test]
    fn paced_by_real_clock() {
        let mut s = streaming();
        s.push_event(delta(&"字".repeat(10)), 0.0);
        s.tick(0.0);
        assert_eq!(text_of(blocks(&s)).chars().count(), 2);
        s.tick(20.0);
        assert_eq!(text_of(blocks(&s)).chars().count(), 2, "未到 40ms 不放出");
        s.tick(40.0);
        assert_eq!(text_of(blocks(&s)).chars().count(), 4);
        assert_eq!(s.live.as_ref().unwrap().reveal_count, 2);
    }
}
