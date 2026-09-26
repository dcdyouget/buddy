// 统一流式事件模块
//
// 定义与 pi-agent 兼容的统一事件协议，所有 Provider 实现都输出此格式的事件。
// 前端只需监听一套事件类型即可支持多种模型提供商。
//
// 事件协议（对应 pi-agent 的 AssistantMessageEvent）：
// - stream-start      → 流式开始，携带初始 partial message
// - stream-text-start  → 文本块开始
// - stream-text-delta  → 文本块增量（单个或少量 token）
// - stream-text-end    → 文本块结束
// - stream-thinking-start → 思考块开始
// - stream-thinking-delta → 思考块增量
// - stream-thinking-end   → 思考块结束
// - stream-done        → 流正常完成，携带完整 assistant message
// - stream-error       → 流出错，携带错误信息

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::models::ImageAttachment;

/// 内容块类型：文本或思考
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentBlock {
    /// 文本内容块
    Text { content: String },
    /// 思考内容块（reasoning/thinking）
    Thinking {
        content: String,
        /// 思考是否仍在进行中（流式期间为 true）
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        is_open: bool,
    },
}

impl ContentBlock {
    /// 从文本中解析 <think>...</think> 标签，转换为 ContentBlock 数组
    ///
    /// 规则：
    /// - <think> 之前的内容 → Text block
    /// - <think>...</think> → Thinking block (is_open=false)
    /// - <think>... (无闭合) → Thinking block (is_open=true)
    /// - 空 Text block 会被过滤
    pub fn parse_from_text(text: &str) -> Vec<Self> {
        let mut blocks: Vec<Self> = Vec::new();
        let think_open = "<think>";
        let think_close = "</think>";
        let mut i = 0;

        while i < text.len() {
            let Some(open_pos) = text[i..].find(think_open) else {
                // 没有更多标签，剩余全是文本
                blocks.push(ContentBlock::Text {
                    content: text[i..].to_string(),
                });
                break;
            };
            let open_pos = i + open_pos;

            // <think> 之前的文本
            if open_pos > i {
                blocks.push(ContentBlock::Text {
                    content: text[i..open_pos].to_string(),
                });
            }

            let think_start = open_pos + think_open.len();
            let Some(close_pos) = text[think_start..].find(think_close) else {
                // 无闭合标签 → 流式进行中
                blocks.push(ContentBlock::Thinking {
                    content: text[think_start..].to_string(),
                    is_open: true,
                });
                break;
            };
            let close_pos = think_start + close_pos;

            // 完整闭合的 think 块
            blocks.push(ContentBlock::Thinking {
                content: text[think_start..close_pos].to_string(),
                is_open: false,
            });
            i = close_pos + think_close.len();
        }

        // 过滤空 text block
        blocks.retain(|b| match b {
            ContentBlock::Text { content } => !content.is_empty(),
            ContentBlock::Thinking { .. } => true,
        });

        blocks
    }
}

const THINK_OPEN_TAG: &str = "<think>";
const THINK_CLOSE_TAG: &str = "</think>";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineThinkMode {
    Text,
    Thinking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InlineThinkOutput {
    TextDelta(String),
    ThinkingStart,
    ThinkingDelta(String),
    ThinkingEnd(String),
}

/// 增量解析文本中的 `<think>` 标签。
///
/// 仅保留可能属于标签的短后缀，普通文本 delta 会原样立即输出。这样既能处理
/// 标签横跨多个 SSE chunk 的情况，也不会把整段回复留给 WebView 反复扫描。
#[derive(Debug)]
struct InlineThinkParser {
    mode: InlineThinkMode,
    pending: String,
    thinking_content: String,
}

impl Default for InlineThinkParser {
    fn default() -> Self {
        Self {
            mode: InlineThinkMode::Text,
            pending: String::new(),
            thinking_content: String::new(),
        }
    }
}

impl InlineThinkParser {
    fn push(&mut self, delta: &str) -> Vec<InlineThinkOutput> {
        self.pending.push_str(delta);
        let mut output = Vec::new();

        loop {
            match self.mode {
                InlineThinkMode::Text => {
                    if let Some(open_index) = self.pending.find(THINK_OPEN_TAG) {
                        let text = self.pending[..open_index].to_string();
                        self.pending.drain(..open_index + THINK_OPEN_TAG.len());
                        self.push_text(text, &mut output);
                        self.mode = InlineThinkMode::Thinking;
                        self.thinking_content.clear();
                        output.push(InlineThinkOutput::ThinkingStart);
                        continue;
                    }

                    let retained = possible_tag_prefix_len(&self.pending, THINK_OPEN_TAG);
                    let safe_len = self.pending.len() - retained;
                    if safe_len > 0 {
                        let suffix = self.pending.split_off(safe_len);
                        let text = std::mem::replace(&mut self.pending, suffix);
                        self.push_text(text, &mut output);
                    }
                    break;
                }
                InlineThinkMode::Thinking => {
                    if let Some(close_index) = self.pending.find(THINK_CLOSE_TAG) {
                        let thinking = self.pending[..close_index].to_string();
                        self.pending.drain(..close_index + THINK_CLOSE_TAG.len());
                        self.push_thinking(thinking, &mut output);
                        output.push(InlineThinkOutput::ThinkingEnd(std::mem::take(
                            &mut self.thinking_content,
                        )));
                        self.mode = InlineThinkMode::Text;
                        continue;
                    }

                    let retained = possible_tag_prefix_len(&self.pending, THINK_CLOSE_TAG);
                    let safe_len = self.pending.len() - retained;
                    if safe_len > 0 {
                        let suffix = self.pending.split_off(safe_len);
                        let thinking = std::mem::replace(&mut self.pending, suffix);
                        self.push_thinking(thinking, &mut output);
                    }
                    break;
                }
            }
        }

        output
    }

    fn finish(mut self) -> Vec<InlineThinkOutput> {
        let mut output = Vec::new();
        let pending = std::mem::take(&mut self.pending);
        match self.mode {
            InlineThinkMode::Text => self.push_text(pending, &mut output),
            InlineThinkMode::Thinking => {
                self.push_thinking(pending, &mut output);
                output.push(InlineThinkOutput::ThinkingEnd(std::mem::take(
                    &mut self.thinking_content,
                )));
            }
        }
        output
    }

    fn push_text(&mut self, text: String, output: &mut Vec<InlineThinkOutput>) {
        if text.is_empty() {
            return;
        }
        output.push(InlineThinkOutput::TextDelta(text));
    }

    fn push_thinking(&mut self, thinking: String, output: &mut Vec<InlineThinkOutput>) {
        if thinking.is_empty() {
            return;
        }
        self.thinking_content.push_str(&thinking);
        output.push(InlineThinkOutput::ThinkingDelta(thinking));
    }
}

fn possible_tag_prefix_len(value: &str, tag: &str) -> usize {
    let max_len = value.len().min(tag.len().saturating_sub(1));
    (1..=max_len)
        .rev()
        .find(|&len| value.ends_with(&tag[..len]))
        .unwrap_or(0)
}

/// 停止原因
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StopReason {
    /// 正常结束
    Stop,
    /// 错误终止
    Error,
    /// 用户取消
    Aborted,
}

/// ask_user tool 的选项结构(用于 ToolQuestionRequired 事件 payload)
///
/// `#[serde(rename_all = "camelCase")]` 让 wire 格式 (`requiresInput`、`inputPlaceholder`)
/// 与前端 `QuestionOption` 类型一致 —— 这是修复 camelCase/snake_case 不匹配的根因。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionOption {
    /// 1-5 词的简短标签
    pub label: String,
    /// 可选说明
    #[serde(default)]
    pub description: String,
    /// 此选项是否需要用户补充输入
    #[serde(default)]
    pub requires_input: bool,
    /// 输入框占位符
    #[serde(default)]
    pub input_placeholder: String,
}

/// stream_chat 的返回结果(P4 新增)
///
/// 拆分 full_text + tool_calls,让 P4 的 send_message 知道本轮有没有 tool_call 需要执行
///
/// Provider 的终态错误信息。
///
/// Provider 只负责收集错误；由 `send_message` 在消息持久化、释放生成占用后再统一
/// 发射终态事件，避免前端已经允许下一次发送时后端仍认为上一轮在运行。
#[derive(Debug, Clone)]
pub struct StreamFailure {
    pub reason: StopReason,
    pub message: String,
}

/// `had_stream_error` 表示 Provider 流未正常结束。为 true 时 `terminal_error` 必须存在，
/// 供编排层（`chat::ChatEngine::send_message`）在收尾阶段发射唯一的 error 事件。
#[derive(Debug, Clone, Default)]
pub struct StreamOutcome {
    /// 累积的完整文本(用于持久化 + UI 展示)
    pub full_text: String,
    /// 累积的思考文本(DeepSeek reasoning_content 等)
    /// 持久化时会被合并进 assistant 消息的 blocks 中
    pub thinking_text: String,
    /// 本轮产生的 tool_calls(可能为空,表示纯文本回复)
    pub tool_calls: Vec<crate::models::ToolCall>,
    /// Provider 是否遇到流错误或取消
    pub had_stream_error: bool,
    /// 延后到 command 层发射的终态错误
    pub terminal_error: Option<StreamFailure>,
}

impl StreamOutcome {
    pub fn completed(
        full_text: String,
        thinking_text: String,
        tool_calls: Vec<crate::models::ToolCall>,
    ) -> Self {
        Self {
            full_text,
            thinking_text,
            tool_calls,
            had_stream_error: false,
            terminal_error: None,
        }
    }

    pub fn failed(
        full_text: String,
        thinking_text: String,
        reason: StopReason,
        message: impl Into<String>,
    ) -> Self {
        Self {
            full_text,
            thinking_text,
            tool_calls: Vec::new(),
            had_stream_error: true,
            terminal_error: Some(StreamFailure {
                reason,
                message: message.into(),
            }),
        }
    }
}
///
/// 所有 Provider 实现都输出此枚举的事件。
/// UI 通过 `StreamEventEmitter::channel()` 的接收端消费（v1 为监听 Tauri 事件）。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum StreamEvent {
    /// 流式开始
    Start,
    /// 文本块开始
    TextStart {
        /// 内容块索引（从 0 开始）
        content_index: usize,
    },
    /// 文本增量
    TextDelta {
        content_index: usize,
        /// 增量文本
        delta: String,
    },
    /// 文本块结束
    TextEnd {
        content_index: usize,
        /// 完整文本内容
        content: String,
    },
    /// 思考块开始
    ThinkingStart { content_index: usize },
    /// 思考增量
    ThinkingDelta {
        content_index: usize,
        /// 增量思考文本
        delta: String,
    },
    /// 思考块结束
    ThinkingEnd {
        content_index: usize,
        /// 完整思考内容
        content: String,
    },
    /// 流式完成
    Done {
        /// 停止原因
        reason: StopReason,
        /// 累积的完整回复文本（用于持久化）
        full_text: String,
    },
    /// 流式错误
    Error {
        /// 错误原因
        reason: StopReason,
        /// 错误信息
        message: String,
        /// 已累积的部分回复文本（用于持久化）
        partial_text: String,
    },

    // ── Tool 调用相关事件（tool_calls 协议） ──
    // 一个 assistant 响应可以包含 0~N 个 tool_call,每个 tool_call 都有
    // start / delta / end 三个事件;同一轮(turn)内可能多 tool_call 并行拼接
    /// Tool 调用开始
    /// 前端收到后应创建占位 UI,显示 "正在调用 tool_name"
    ToolCallStart {
        /// 模型提供的 tool_call id (OpenAI: call_xxx, Anthropic: toolu_xxx)
        id: String,
        /// 工具名
        name: String,
        /// 同一 assistant 消息内的 tool_call 索引(0-based)
        content_index: usize,
    },
    /// Tool 调用参数增量
    /// OpenAI 是 partial JSON,Anthropic 是 partial input_json
    /// 拼装到 ToolCall.arguments 原始字符串
    ToolCallDelta {
        id: String,
        /// 增量(原始 JSON 片段)
        arguments_delta: String,
    },
    /// Tool 调用参数完整
    ToolCallEnd {
        id: String,
        name: String,
        /// 完整参数(JSON 字符串)
        arguments: String,
    },
    /// 后端开始执行 tool(已通过审批,或 read-only 不需审批)
    ToolExecuting { id: String, name: String },
    /// Tool 执行结果
    /// OpenAI 协议以 role:"tool" 消息塞回 messages;这里的事件用于前端实时显示
    ToolResult {
        id: String,
        name: String,
        /// 结果内容
        content: String,
        /// 工具产生的展示图片，不会被拼入 tool result 文本。
        images: Vec<ImageAttachment>,
        /// true=执行出错(让 model 看到错误并重试)
        is_error: bool,
    },
    /// 需要用户审批(只对 Write 类 tool 触发)
    /// UI 弹审批卡,点击后调用 `ChatEngine::approve_tool_call(id, approved, approve_all)`
    ToolApprovalRequired {
        id: String,
        name: String,
        /// 完整参数(供 UI 展示)
        arguments: String,
        /// 审批原因(写文件时是 "write to <path>")
        reason: String,
    },
    /// 模型调用了 ask_user tool — 需要用户在内联工具卡片中做出选择
    /// 前端在 AskUserCard 点击选项/输入自定义答案后
    /// `ChatEngine::answer_tool_question(id, selected, inputs, custom)`
    ToolQuestionRequired {
        id: String,
        /// 始终是 "ask_user",保留字段便于前端过滤
        name: String,
        /// 问题文本
        question: String,
        /// 2-4 个选项
        options: Vec<QuestionOption>,
        /// 是否允许多选
        multi_select: bool,
        /// 短标签(chip)
        header: String,
    },
    /// 一轮 assistant 完成,统计待处理 tool_call 数
    /// tool_calls_pending == 0 时整次 send_message 也将结束
    TurnEnd { tool_calls_pending: usize },
}

/// 流式事件发射器
///
/// 把 StreamEvent 发送到 channel，由 UI 侧直接消费（无 IPC、无 JSON 序列化）。
/// v1 经 Tauri 事件 `stream-event` 发给 webview；v2 UI 与 engine 同进程，改用 channel
/// （S00-08 实测，docs/evidence/s00-08/engine-integration.md §1.1）。
pub struct StreamEventEmitter {
    tx: UnboundedSender<StreamEvent>,
    inline_think_parsers: Mutex<HashMap<usize, InlineThinkParser>>,
}

impl StreamEventEmitter {
    /// 创建新的事件发射器
    pub fn new(tx: UnboundedSender<StreamEvent>) -> Self {
        Self {
            tx,
            inline_think_parsers: Mutex::new(HashMap::new()),
        }
    }

    /// 发射一个流式事件
    pub fn emit(&self, event: &StreamEvent) {
        // 接收端已 drop（UI 放弃了本次流）时静默丢弃，与 v1 忽略 emit 错误一致
        let _ = self.tx.send(event.clone());
    }

    /// 创建发射器及其配对的接收端
    pub fn channel() -> (Self, UnboundedReceiver<StreamEvent>) {
        let (tx, rx) = unbounded_channel();
        (Self::new(tx), rx)
    }

    /// 便捷方法：发射 Start 事件
    pub fn start(&self) {
        self.finish_all_inline_thinking();
        self.emit(&StreamEvent::Start);
    }

    /// 便捷方法：发射 TextStart 事件
    pub fn text_start(&self, content_index: usize) {
        self.finish_inline_thinking(content_index);
        self.inline_think_parsers
            .lock()
            .insert(content_index, InlineThinkParser::default());
        self.emit(&StreamEvent::TextStart { content_index });
    }

    /// 便捷方法：发射 TextDelta 事件
    pub fn text_delta(&self, content_index: usize, delta: &str) {
        let output = self
            .inline_think_parsers
            .lock()
            .entry(content_index)
            .or_default()
            .push(delta);
        self.emit_inline_think_output(content_index, output);
    }

    /// 便捷方法：发射 TextEnd 事件
    pub fn text_end(&self, content_index: usize, content: &str) {
        self.finish_inline_thinking(content_index);
        self.emit(&StreamEvent::TextEnd {
            content_index,
            content: content.to_string(),
        });
    }

    /// 便捷方法：发射 ThinkingStart 事件
    pub fn thinking_start(&self, content_index: usize) {
        self.emit(&StreamEvent::ThinkingStart { content_index });
    }

    /// 便捷方法：发射 ThinkingDelta 事件
    pub fn thinking_delta(&self, content_index: usize, delta: &str) {
        self.emit(&StreamEvent::ThinkingDelta {
            content_index,
            delta: delta.to_string(),
        });
    }

    /// 便捷方法：发射 ThinkingEnd 事件
    pub fn thinking_end(&self, content_index: usize, content: &str) {
        self.emit(&StreamEvent::ThinkingEnd {
            content_index,
            content: content.to_string(),
        });
    }

    /// 便捷方法：发射 Done 事件
    pub fn done(&self, reason: StopReason, full_text: &str) {
        self.finish_all_inline_thinking();
        self.emit(&StreamEvent::Done {
            reason,
            full_text: full_text.to_string(),
        });
    }

    /// 便捷方法：发射 Error 事件
    pub fn error(&self, reason: StopReason, message: &str, partial_text: &str) {
        self.finish_all_inline_thinking();
        self.emit(&StreamEvent::Error {
            reason,
            message: message.to_string(),
            partial_text: partial_text.to_string(),
        });
    }

    // ── Tool 事件便捷方法 ──

    pub fn tool_call_start(&self, id: &str, name: &str, content_index: usize) {
        self.emit(&StreamEvent::ToolCallStart {
            id: id.to_string(),
            name: name.to_string(),
            content_index,
        });
    }

    pub fn tool_call_delta(&self, id: &str, arguments_delta: &str) {
        self.emit(&StreamEvent::ToolCallDelta {
            id: id.to_string(),
            arguments_delta: arguments_delta.to_string(),
        });
    }

    pub fn tool_call_end(&self, id: &str, name: &str, arguments: &str) {
        self.emit(&StreamEvent::ToolCallEnd {
            id: id.to_string(),
            name: name.to_string(),
            arguments: arguments.to_string(),
        });
    }

    pub fn tool_executing(&self, id: &str, name: &str) {
        self.emit(&StreamEvent::ToolExecuting {
            id: id.to_string(),
            name: name.to_string(),
        });
    }

    pub fn tool_result(
        &self,
        id: &str,
        name: &str,
        content: &str,
        images: Vec<ImageAttachment>,
        is_error: bool,
    ) {
        self.emit(&StreamEvent::ToolResult {
            id: id.to_string(),
            name: name.to_string(),
            content: content.to_string(),
            images,
            is_error,
        });
    }

    pub fn tool_approval_required(&self, id: &str, name: &str, arguments: &str, reason: &str) {
        self.emit(&StreamEvent::ToolApprovalRequired {
            id: id.to_string(),
            name: name.to_string(),
            arguments: arguments.to_string(),
            reason: reason.to_string(),
        });
    }

    /// 发射 ToolQuestionRequired 事件(模型调用了 ask_user tool)
    pub fn tool_question_required(
        &self,
        id: &str,
        name: &str,
        question: &str,
        options: Vec<QuestionOption>,
        multi_select: bool,
        header: &str,
    ) {
        self.emit(&StreamEvent::ToolQuestionRequired {
            id: id.to_string(),
            name: name.to_string(),
            question: question.to_string(),
            options,
            multi_select,
            header: header.to_string(),
        });
    }

    pub fn turn_end(&self, tool_calls_pending: usize) {
        self.finish_all_inline_thinking();
        self.emit(&StreamEvent::TurnEnd { tool_calls_pending });
    }

    fn take_inline_thinking(&self, content_index: usize) -> Option<Vec<InlineThinkOutput>> {
        self.inline_think_parsers
            .lock()
            .remove(&content_index)
            .map(InlineThinkParser::finish)
    }

    fn finish_inline_thinking(&self, content_index: usize) {
        if let Some(output) = self.take_inline_thinking(content_index) {
            self.emit_inline_think_output(content_index, output);
        }
    }

    fn finish_all_inline_thinking(&self) {
        let mut parsers = self.inline_think_parsers.lock();
        let mut pending: Vec<_> = parsers.drain().collect();
        drop(parsers);
        pending.sort_by_key(|(content_index, _)| *content_index);
        for (content_index, parser) in pending {
            let output = parser.finish();
            self.emit_inline_think_output(content_index, output);
        }
    }

    fn emit_inline_think_output(&self, content_index: usize, output: Vec<InlineThinkOutput>) {
        for event in output {
            match event {
                InlineThinkOutput::TextDelta(delta) => self.emit(&StreamEvent::TextDelta {
                    content_index,
                    delta,
                }),
                InlineThinkOutput::ThinkingStart => {
                    self.emit(&StreamEvent::ThinkingStart { content_index });
                }
                InlineThinkOutput::ThinkingDelta(delta) => {
                    self.emit(&StreamEvent::ThinkingDelta {
                        content_index,
                        delta,
                    });
                }
                InlineThinkOutput::ThinkingEnd(content) => {
                    self.thinking_end(content_index, &content);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_event_serialization() {
        let event = StreamEvent::TextDelta {
            content_index: 0,
            delta: "hello".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"text_delta\""));
        assert!(json.contains("\"delta\":\"hello\""));
    }

    #[test]
    fn test_done_event_serialization() {
        let event = StreamEvent::Done {
            reason: StopReason::Stop,
            full_text: "Hello, world!".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"done\""));
        assert!(json.contains("\"reason\":\"stop\""));
    }

    #[test]
    fn inline_think_parser_forwards_plain_delta_without_batching() {
        let mut parser = InlineThinkParser::default();

        assert_eq!(
            parser.push("普通回复"),
            vec![InlineThinkOutput::TextDelta("普通回复".to_string())]
        );
    }

    #[test]
    fn inline_think_parser_handles_tags_split_across_deltas() {
        let mut parser = InlineThinkParser::default();

        assert_eq!(
            parser.push("前缀<thi"),
            vec![InlineThinkOutput::TextDelta("前缀".to_string())]
        );
        assert_eq!(
            parser.push("nk>思"),
            vec![
                InlineThinkOutput::ThinkingStart,
                InlineThinkOutput::ThinkingDelta("思".to_string()),
            ]
        );
        assert_eq!(
            parser.push("考</thi"),
            vec![InlineThinkOutput::ThinkingDelta("考".to_string())]
        );
        assert_eq!(
            parser.push("nk>结论"),
            vec![
                InlineThinkOutput::ThinkingEnd("思考".to_string()),
                InlineThinkOutput::TextDelta("结论".to_string()),
            ]
        );

        let output = parser.finish();
        assert!(output.is_empty());
    }

    #[test]
    fn inline_think_parser_keeps_nested_open_tag_as_thinking_text() {
        let mut parser = InlineThinkParser::default();

        assert_eq!(
            parser.push("<think>A<think>B</think>C"),
            vec![
                InlineThinkOutput::ThinkingStart,
                InlineThinkOutput::ThinkingDelta("A<think>B".to_string()),
                InlineThinkOutput::ThinkingEnd("A<think>B".to_string()),
                InlineThinkOutput::TextDelta("C".to_string()),
            ]
        );
    }

    #[test]
    fn inline_think_parser_closes_unfinished_thinking_on_finish() {
        let mut parser = InlineThinkParser::default();
        assert_eq!(
            parser.push("<think>尚未结束"),
            vec![
                InlineThinkOutput::ThinkingStart,
                InlineThinkOutput::ThinkingDelta("尚未结束".to_string()),
            ]
        );

        let output = parser.finish();
        assert_eq!(
            output,
            vec![InlineThinkOutput::ThinkingEnd("尚未结束".to_string())]
        );
    }

    #[test]
    fn inline_think_parser_releases_false_tag_prefix() {
        let mut parser = InlineThinkParser::default();
        assert!(parser.push("<thi").is_empty());
        assert_eq!(
            parser.push("s is text"),
            vec![InlineThinkOutput::TextDelta("<this is text".to_string())]
        );
    }

    #[test]
    fn thinking_end_event_serializes_for_frontend_protocol() {
        let event = StreamEvent::ThinkingEnd {
            content_index: 2,
            content: "分析完成".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"thinking_end\""));
        assert!(json.contains("\"content_index\":2"));
        assert!(json.contains("\"content\":\"分析完成\""));
    }
}
