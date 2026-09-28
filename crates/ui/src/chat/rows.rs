//! 块粒度行模型（S05-02）
//!
//! 虚拟列表的一行 = 一条用户消息 / 助手的一个内容块（正文或思考）/ 一次工具调用，**不是一条消息**
//! （长回答被拆成多行，虚拟化只布局可见的块；流式时只有最后一行在变）。
//!
//! # 行 id：以本轮的用户消息为锚
//!
//! engine 在回合结束时才以自己生成的 id 持久化 assistant / tool 消息（流式事件不带这些 id）。
//! 若行 id 取消息 id，落盘后重新加载时 id 全变，列表会整体重建、闪烁。因此：
//!
//! | 行 | id |
//! |----|----|
//! | 用户消息 | `{用户消息 id}`（界面生成，发送与持久化相同） |
//! | 助手内容块 | `{锚}#{m}.{i}`：本轮第 m 条 assistant 消息的第 i 个块 |
//! | 工具调用 | `{锚}#{m}.t.{调用 id}`（调用 id 由模型给出，流式与持久化相同） |
//!
//! engine 每个工具轮写一条 assistant（S05-17），界面按同一规则拆分 → 流式与持久化的 (m, i) 一致。
//! 锚为本轮用户消息 id；最前面没有用户消息的助手消息以 `head` 为锚。
//!
//! # 工具调用的位置（v1 `MessageBubble.tsx` 分桶规则）
//!
//! 按 `insert_after` 插在对应块之后（-1 = 第一个块之前）；历史消息没有该信息 → 放在最后一个块之后；
//! 同一条消息里只要有一个调用带位置信息，其余无位置的放到全部块之后。

use super::state::{ChatState, ToolView};
use buddy_engine::models::{Message, MessageRole};
use buddy_engine::streaming::ContentBlock;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;

/// 行的内容指向（下标指向 [`ChatState::messages`]）
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// 用户消息
    User {
        /// 消息下标
        msg: usize,
    },
    /// 助手的一个内容块
    Block {
        /// 消息下标
        msg: usize,
        /// 块下标
        block: usize,
        /// 来自流式中的实时内容（`LiveTurn::blocks`）
        live: bool,
    },
    /// 一次工具调用
    Tool {
        /// 所属 assistant 消息下标
        msg: usize,
        /// 调用 id
        call: String,
    },
    /// 回答下方的操作栏（复制 / 回到问题 / 时间，S05-14）。v1：非流式、不是工具循环的中间段、且有正文时显示
    Actions {
        /// 所属 assistant 消息下标
        msg: usize,
    },
    /// 流式中的回答尚无任何可见内容：占位行（v1 此时只显示呼吸星标）。
    /// 只在流式中存在，出现首个内容后即消失。
    Pending {
        /// 所属 assistant 消息下标
        msg: usize,
    },
}

/// 行在所属消息中的位置（决定上下留白，v1 的留白按整条消息计算）
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RowPos {
    /// 本消息的第一行
    pub first: bool,
    /// 本消息的最后一行
    pub last: bool,
    /// 助手消息紧接在另一条助手消息之后（v1 `isContinuation`：工具循环的续段）
    pub continuation: bool,
    /// 下一条可见消息仍是助手消息（v1 `continuesToNext`）
    pub continues: bool,
}

/// 一行
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// 稳定 id
    pub id: String,
    /// 内容指向
    pub kind: RowKind,
    /// 内容指纹：内容或位置变化时改变（决定是否重新测量这一行）
    pub version: u64,
    /// 在所属消息中的位置
    pub pos: RowPos,
}

fn msg_of(kind: &RowKind) -> usize {
    match kind {
        RowKind::User { msg } | RowKind::Block { msg, .. } | RowKind::Tool { msg, .. } | RowKind::Pending { msg } | RowKind::Actions { msg } => *msg,
    }
}

fn fingerprint(parts: impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    parts.hash(&mut h);
    h.finish()
}

fn block_fingerprint(block: &ContentBlock) -> u64 {
    match block {
        ContentBlock::Text { content } => fingerprint(("t", content)),
        ContentBlock::Thinking { content, is_open } => fingerprint(("k", content, is_open)),
    }
}

fn tool_fingerprint(tool: Option<&ToolView>) -> u64 {
    match tool {
        Some(t) => fingerprint((&t.name, t.arguments.len(), t.status as u8, &t.result, t.is_error, &t.images)),
        None => 0,
    }
}

/// 某条 assistant 消息当前应显示的块：流式中的最后一条 assistant 取实时块
fn blocks_of<'a>(state: &'a ChatState, msg: usize) -> (&'a [ContentBlock], bool) {
    let live_ix = state.live.as_ref().and_then(|_| state.messages.iter().rposition(|m| m.role == MessageRole::Assistant));
    match (&state.live, live_ix) {
        (Some(live), Some(ix)) if ix == msg => (&live.blocks, true),
        _ => (state.messages[msg].blocks.as_deref().unwrap_or(&[]), false),
    }
}

/// 该消息应显示的工具调用 id（v1：实时优先、按 id 去重，其后接已持久化的）
fn calls_of(state: &ChatState, msg: usize, live: bool) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    if live {
        ids.extend(state.live.as_ref().map(|l| l.active_ids()).unwrap_or_default());
    }
    for call in state.messages[msg].tool_calls.iter().flatten() {
        if !ids.contains(&call.id) {
            ids.push(call.id.clone());
        }
    }
    ids
}

/// 分桶：-1..=n（v1 `toolCallsByIndex`）
fn bucket(state: &ChatState, calls: &[String], block_count: usize) -> HashMap<i32, Vec<String>> {
    let has_any = calls.iter().any(|c| state.tools.get(c).and_then(|t| t.insert_after).is_some());
    let fallback = block_count as i32 - 1; // 无块时即 -1
    let mut out: HashMap<i32, Vec<String>> = HashMap::new();
    for c in calls {
        let requested = match state.tools.get(c).and_then(|t| t.insert_after) {
            Some(i) => i,
            None if has_any => block_count as i32,
            None => fallback,
        };
        out.entry(requested.clamp(-1, block_count as i32)).or_default().push(c.clone());
    }
    out
}

/// 由对话状态生成全部行（纯函数）
pub fn build_rows(state: &ChatState) -> Vec<Row> {
    let mut rows = Vec::new();
    // 开头没有用户消息的助手行（分页边界）锚在首条消息上：各页开头的 `head` 行 id 不能相同，
    // 否则并入更早一页时 diff 会把两页的开头行误认作同一行（S05-05 T22 发现）
    let mut anchor = format!("head-{}", state.messages.first().map_or("", |m| m.id.as_str()));
    let mut assistant_ordinal = 0usize;
    for (ix, message) in state.messages.iter().enumerate() {
        match message.role {
            MessageRole::User => {
                anchor = message.id.clone();
                assistant_ordinal = 0;
                rows.push(Row {
                    id: message.id.clone(),
                    kind: RowKind::User { msg: ix },
                    version: fingerprint((&message.content, message.images.iter().map(|i| &i.id).collect::<Vec<_>>())),
                    pos: RowPos::default(),
                });
            }
            MessageRole::Tool => {}
            MessageRole::Assistant => {
                let m = assistant_ordinal;
                assistant_ordinal += 1;
                let (blocks, live) = blocks_of(state, ix);
                let calls = calls_of(state, ix, live);
                let buckets = bucket(state, &calls, blocks.len());
                let push_tools = |rows: &mut Vec<Row>, at: i32| {
                    for call in buckets.get(&at).into_iter().flatten() {
                        rows.push(Row {
                            id: format!("{anchor}#{m}.t.{call}"),
                            kind: RowKind::Tool { msg: ix, call: call.clone() },
                            version: tool_fingerprint(state.tools.get(call)),
                            pos: RowPos::default(),
                        });
                    }
                };
                let rows_before = rows.len();
                push_tools(&mut rows, -1);
                for (i, block) in blocks.iter().enumerate() {
                    // 空正文块（流式轮次分隔）不成行：它会在收尾时被去掉，成行则结束时闪一下
                    let empty_text = matches!(block, ContentBlock::Text { content } if content.is_empty());
                    if !empty_text {
                        rows.push(Row {
                            id: format!("{anchor}#{m}.{i}"),
                            kind: RowKind::Block { msg: ix, block: i, live },
                            version: block_fingerprint(block),
                            pos: RowPos::default(),
                        });
                    }
                    push_tools(&mut rows, i as i32);
                }
                if !blocks.is_empty() {
                    push_tools(&mut rows, blocks.len() as i32);
                }
                if live && rows.len() == rows_before {
                    rows.push(Row { id: format!("{anchor}#{m}.pending"), kind: RowKind::Pending { msg: ix }, version: 0, pos: RowPos::default() });
                }
                if !live && !next_visible_is_assistant(state, ix) && has_answer_text(message) {
                    rows.push(Row { id: format!("{anchor}#{m}.actions"), kind: RowKind::Actions { msg: ix }, version: 0, pos: RowPos::default() });
                }
            }
        }
    }
    assign_positions(state, &mut rows);
    rows
}

/// 下一条可见消息（跳过 tool）是否为助手（v1 `continuesToNext`）
fn next_visible_is_assistant(state: &ChatState, ix: usize) -> bool {
    state.messages[ix + 1..].iter().find(|m| m.role != MessageRole::Tool).is_some_and(|m| m.role == MessageRole::Assistant)
}

/// v1 `hasAnswerText`：正文或任一正文块非空白
pub fn has_answer_text(message: &Message) -> bool {
    !message.content.trim().is_empty()
        || message.blocks.iter().flatten().any(|b| matches!(b, ContentBlock::Text { content } if !content.trim().is_empty()))
}

/// 第二遍：按消息分组标出首 / 末行与续段关系，并把位置并入版本
fn assign_positions(state: &ChatState, rows: &mut [Row]) {
    // 可见消息序列（tool 消息不显示）中，每条消息前后是否为助手
    let visible: Vec<usize> = (0..state.messages.len()).filter(|&i| state.messages[i].role != MessageRole::Tool).collect();
    let neighbor = |msg: usize, delta: isize| -> bool {
        visible
            .iter()
            .position(|&v| v == msg)
            .and_then(|p| visible.get(p.checked_add_signed(delta)?))
            .is_some_and(|&n| state.messages[n].role == MessageRole::Assistant)
    };
    let n = rows.len();
    for i in 0..n {
        let msg = msg_of(&rows[i].kind);
        let assistant = state.messages[msg].role == MessageRole::Assistant;
        let pos = RowPos {
            first: i == 0 || msg_of(&rows[i - 1].kind) != msg,
            last: i + 1 == n || msg_of(&rows[i + 1].kind) != msg,
            continuation: assistant && neighbor(msg, -1),
            continues: assistant && neighbor(msg, 1),
        };
        rows[i].version = fingerprint((rows[i].version, pos));
        rows[i].pos = pos;
    }
}

/// 把行集合从 `old` 变为 `new` 的最小改动
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Splice {
    /// 被替换的旧区间
    pub old_range: Range<usize>,
    /// 替换进来的新行数
    pub new_count: usize,
    /// 在前后公共段中 id 相同但内容变化、需要重新测量的行（新下标）
    pub remeasure: Vec<usize>,
}

impl Splice {
    /// 无任何变化
    pub fn is_noop(&self) -> bool {
        self.old_range.is_empty() && self.new_count == 0 && self.remeasure.is_empty()
    }
}

/// 按 id 求公共前缀与后缀，中间整体替换（Comet 做法：一次最小 `splice`）
pub fn diff(old: &[Row], new: &[Row]) -> Splice {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a.id == b.id).count();
    let max_suffix = old.len().min(new.len()) - prefix;
    let suffix = old.iter().rev().zip(new.iter().rev()).take(max_suffix).take_while(|(a, b)| a.id == b.id).count();
    let mut remeasure: Vec<usize> = (0..prefix).filter(|&i| old[i].version != new[i].version).collect();
    for k in 0..suffix {
        let (o, n) = (old.len() - 1 - k, new.len() - 1 - k);
        if old[o].version != new[n].version {
            remeasure.push(n);
        }
    }
    remeasure.sort_unstable();
    Splice { old_range: prefix..old.len() - suffix, new_count: new.len() - suffix - prefix, remeasure }
}

/// 与轮次锚点无关的行键（消息 id + 行内位置）。行 id 以本轮用户消息为锚；分页边界处的
/// 助手消息在更早一页并入前锚为 `head`，并入后改锚 → 行 id 变了但行键不变（S05-05 保持视口用）
pub fn stable_key(state: &ChatState, row: &Row) -> String {
    let id = &state.messages[msg_of(&row.kind)].id;
    match &row.kind {
        RowKind::User { .. } => id.clone(),
        RowKind::Block { block, .. } => format!("{id}.{block}"),
        RowKind::Tool { call, .. } => format!("{id}.t.{call}"),
        RowKind::Actions { .. } => format!("{id}.actions"),
        RowKind::Pending { .. } => format!("{id}.pending"),
    }
}

/// 该行所属消息（行渲染取数用）
pub fn message<'a>(state: &'a ChatState, row: &Row) -> &'a Message {
    &state.messages[msg_of(&row.kind)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::state::ToolStatus;
    use buddy_engine::models::ToolCall;
    use buddy_engine::streaming::{StopReason, StreamEvent};

    fn user(id: &str, text: &str) -> Message {
        Message {
            id: id.into(),
            role: MessageRole::User,
            content: text.into(),
            images: Vec::new(),
            blocks: None,
            model_id: None,
            created_at: 0,
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
            is_error: None,
            parent_message_id: None,
        }
    }

    fn assistant(id: &str, blocks: Vec<ContentBlock>, calls: Vec<&str>) -> Message {
        Message {
            id: id.into(),
            role: MessageRole::Assistant,
            content: crate::chat::state::text_of(&blocks),
            images: Vec::new(),
            blocks: Some(blocks),
            model_id: Some("m".into()),
            created_at: 0,
            tool_calls: (!calls.is_empty()).then(|| calls.into_iter().map(|c| ToolCall { id: c.into(), name: "websearch".into(), arguments: "{}".into() }).collect()),
            tool_call_id: None,
            tool_name: None,
            is_error: None,
            parent_message_id: None,
        }
    }

    fn tool(id: &str, call: &str) -> Message {
        Message {
            id: id.into(),
            role: MessageRole::Tool,
            content: "结果".into(),
            images: Vec::new(),
            blocks: None,
            model_id: None,
            created_at: 0,
            tool_calls: None,
            tool_call_id: Some(call.into()),
            tool_name: Some("websearch".into()),
            is_error: Some(false),
            parent_message_id: None,
        }
    }

    fn text(s: &str) -> ContentBlock {
        ContentBlock::Text { content: s.into() }
    }

    fn ids(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|r| r.id.as_str()).collect()
    }

    #[test]
    fn history_rows_follow_v1_order() {
        // 用户 → 助手（思考 + 正文，调用在最后一块之后）→ tool（不显示）→ 助手续段
        let state = ChatState::from_history(vec![
            user("u1", "问"),
            assistant("a1", vec![ContentBlock::Thinking { content: "想".into(), is_open: false }, text("先查")], vec!["c1"]),
            tool("t1", "c1"),
            assistant("a2", vec![text("答")], vec![]),
        ]);
        assert_eq!(ids(&build_rows(&state)), vec!["u1", "u1#0.0", "u1#0.1", "u1#0.t.c1", "u1#1.0", "u1#1.actions"]);
    }

    #[test]
    fn positions_and_continuations() {
        let state = ChatState::from_history(vec![
            user("u1", "问"),
            assistant("a1", vec![text("一"), text("二")], vec!["c1"]),
            tool("t1", "c1"),
            assistant("a2", vec![text("答")], vec![]),
        ]);
        let rows = build_rows(&state);
        let pos: Vec<(bool, bool, bool, bool)> = rows.iter().map(|r| (r.pos.first, r.pos.last, r.pos.continuation, r.pos.continues)).collect();
        assert_eq!(
            pos,
            vec![
                (true, true, false, false),  // u1
                (true, false, false, true),  // a1 第一块：后面还有助手续段
                (false, false, false, true), // a1 第二块
                (false, true, false, true),  // a1 的工具行（末行）
                (true, false, true, false),  // a2：续段（隔着 tool 消息仍算紧接）
                (false, true, true, false),  // a2 的操作栏（末行）
            ]
        );
    }

    #[test]
    fn prepending_history_keeps_stable_keys() {
        let text = |s: &str| vec![ContentBlock::Text { content: s.into() }];
        // 最新一页以助手消息开头（它的提问在更早一页）
        let mut s = ChatState::from_history(vec![assistant("a3", text("答二"), vec![]), user("u4", "问三"), assistant("a5", text("答三"), vec![])]);
        let old = build_rows(&s);
        assert_eq!(old[0].id, "head-a3#0.0");
        let old_top_key = stable_key(&s, &old[0]);
        // 更早一页同样以助手消息开头：两页的开头行 id 不能相同
        s.prepend_history(vec![assistant("a1", text("答一"), vec![]), user("u2", "问二")]);
        let new = build_rows(&s);
        assert_eq!(new[0].id, "head-a1#0.0");
        let change = diff(&old, &new);
        assert_eq!(change.old_range.start, 0, "边界行改锚 → 在替换区间内");
        // 改锚后的边界行可按行键找回
        let found = new.iter().position(|r| stable_key(&s, r) == old_top_key).unwrap();
        assert_eq!(new[found].id, "u2#0.0");
    }

    #[test]
    fn actions_row_rules_match_v1() {
        // 工具循环：中间段无操作栏，最后一段有；纯思考无正文的消息无操作栏
        let state = ChatState::from_history(vec![
            user("u1", "问"),
            assistant("a1", vec![text("先查")], vec!["c1"]),
            tool("t1", "c1"),
            assistant("a2", vec![text("答")], vec![]),
            user("u2", "再问"),
            assistant("a3", vec![ContentBlock::Thinking { content: "想".into(), is_open: false }], vec![]),
        ]);
        let ids: Vec<String> = build_rows(&state).into_iter().map(|r| r.id).collect();
        assert!(ids.contains(&"u1#1.actions".to_string()) && !ids.contains(&"u1#0.actions".to_string()), "{ids:?}");
        assert!(!ids.iter().any(|i| i.starts_with("u2#") && i.ends_with(".actions")), "{ids:?}");
        // 流式中不显示；结束后出现（只插入一行，其余行 id 不变）
        let mut live = ChatState::from_history(Vec::new());
        live.begin_send(user("u", "问"), "m");
        live.push_event(StreamEvent::TextDelta { content_index: 0, delta: "答".into() }, 0.0);
        live.flush(0.0);
        let during: Vec<String> = build_rows(&live).into_iter().map(|r| r.id).collect();
        assert!(!during.iter().any(|i| i.ends_with(".actions")));
        live.push_event(StreamEvent::Done { reason: StopReason::Stop, full_text: String::new() }, 0.0);
        let after: Vec<String> = build_rows(&live).into_iter().map(|r| r.id).collect();
        assert_eq!(after, vec!["u".to_string(), "u#0.0".into(), "u#0.actions".into()]);
    }

    #[test]
    fn insert_after_buckets_match_v1() {
        let mut state = ChatState::from_history(vec![user("u", "q"), assistant("a", vec![text("一"), text("二")], vec!["before", "mid", "unplaced"])]);
        state.tools.get_mut("before").unwrap().insert_after = Some(-1);
        state.tools.get_mut("mid").unwrap().insert_after = Some(0);
        // 同一消息中有调用带位置 → 无位置者放到全部块之后
        assert_eq!(ids(&build_rows(&state)), vec!["u", "u#0.t.before", "u#0.0", "u#0.t.mid", "u#0.1", "u#0.t.unplaced", "u#0.actions"]);
    }

    #[test]
    fn live_and_persisted_rows_share_ids() {
        // 流式中逐事件构建的行 id，与结束后（及重新从历史加载）一致 → 不闪烁
        let mut live = ChatState::from_history(Vec::new());
        live.begin_send(user("u1", "问"), "m");
        let events = vec![
            StreamEvent::ThinkingStart { content_index: 0 },
            StreamEvent::ThinkingDelta { content_index: 0, delta: "想".into() },
            StreamEvent::ThinkingEnd { content_index: 0, content: "想".into() },
            StreamEvent::TextStart { content_index: 0 },
            StreamEvent::TextDelta { content_index: 0, delta: "先查".into() },
            StreamEvent::TextEnd { content_index: 0, content: "先查".into() },
            StreamEvent::ToolCallStart { id: "c1".into(), name: "websearch".into(), content_index: 0 },
            StreamEvent::ToolCallEnd { id: "c1".into(), name: "websearch".into(), arguments: "{}".into() },
            StreamEvent::TurnEnd { tool_calls_pending: 1 },
            StreamEvent::ToolResult { id: "c1".into(), name: "websearch".into(), content: "结果".into(), images: Vec::new(), is_error: false },
            StreamEvent::TextStart { content_index: 0 },
            StreamEvent::TextDelta { content_index: 0, delta: "答".into() },
            StreamEvent::TextEnd { content_index: 0, content: "答".into() },
            StreamEvent::TurnEnd { tool_calls_pending: 0 },
        ];
        let mut seen: Vec<String> = build_rows(&live).into_iter().map(|r| r.id).collect();
        for e in events {
            live.push_event(e, 0.0);
            live.flush(0.0);
            for r in build_rows(&live) {
                if !seen.contains(&r.id) {
                    seen.push(r.id);
                }
            }
        }
        live.push_event(StreamEvent::Done { reason: StopReason::Stop, full_text: String::new() }, 0.0);
        let finished = ids(&build_rows(&live)).into_iter().map(String::from).collect::<Vec<_>>();
        // engine 持久化的形态（id 不同，结构相同）
        let reloaded = ChatState::from_history(vec![
            user("u1", "问"),
            assistant("a-0-1", vec![ContentBlock::Thinking { content: "想".into(), is_open: false }, text("先查")], vec!["c1"]),
            tool("t-0-2", "c1"),
            assistant("a-1-3", vec![text("答")], vec![]),
        ]);
        assert_eq!(finished, vec!["u1", "u1#0.0", "u1#0.1", "u1#0.t.c1", "u1#1.0", "u1#1.actions"]);
        assert_eq!(ids(&build_rows(&reloaded)), finished.iter().map(String::as_str).collect::<Vec<_>>());
        // 流式过程中出现过的 id 都在最终集合里（没有只在流式中存在、结束时消失的行）；
        // 例外只有「尚无内容」占位行，它按设计只在流式中存在
        assert!(seen.iter().filter(|id| !id.ends_with(".pending")).all(|id| finished.contains(id)), "{seen:?}");
        assert!(seen.iter().any(|id| id == "u1#0.pending"), "发送后、首个内容前应有占位行：{seen:?}");
        assert_eq!(live.tools["c1"].status, ToolStatus::Done);
    }

    #[test]
    fn streaming_changes_only_last_row() {
        let mut s = ChatState::from_history(vec![user("u0", "旧问"), assistant("a0", vec![text("旧答")], vec![])]);
        s.begin_send(user("u1", "问"), "m");
        s.push_event(StreamEvent::TextDelta { content_index: 0, delta: "答".repeat(40) }, 0.0);
        s.tick(0.0);
        let before = build_rows(&s);
        s.tick(40.0);
        let after = build_rows(&s);
        let d = diff(&before, &after);
        assert!(d.old_range.is_empty() && d.new_count == 0, "行集合不变：{d:?}");
        assert_eq!(d.remeasure, vec![after.len() - 1], "只有最后一行需要重新测量");
    }

    #[test]
    fn diff_is_minimal_splice() {
        let r = |id: &str, v: u64| Row { id: id.into(), kind: RowKind::User { msg: 0 }, version: v, pos: RowPos::default() };
        let old = vec![r("a", 1), r("b", 1), r("c", 1)];
        let new = vec![r("a", 1), r("b", 2), r("x", 1), r("c", 1)];
        assert_eq!(diff(&old, &new), Splice { old_range: 2..2, new_count: 1, remeasure: vec![1] });
        // 顶部插入更早历史
        let new = vec![r("z", 1), r("a", 1), r("b", 1), r("c", 1)];
        assert_eq!(diff(&old, &new), Splice { old_range: 0..0, new_count: 1, remeasure: vec![] });
        assert!(diff(&old, &old).is_noop());
    }
}
