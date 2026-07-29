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

use serde::{Deserialize, Serialize};
use tauri::Emitter;

/// 内容块类型：文本或思考
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentBlock {
    /// 文本内容块
    Text {
        content: String,
    },
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
/// `had_stream_error`: provider 内部是否已发射过 error 事件。
/// 为 true 时 commands.rs 不应再发 done，避免前端收到 error + done 的组合。
#[derive(Debug, Clone, Default)]
pub struct StreamOutcome {
    /// 累积的完整文本(用于持久化 + UI 展示)
    pub full_text: String,
    /// 累积的思考文本(DeepSeek reasoning_content 等)
    /// 持久化时会被合并进 assistant 消息的 blocks 中
    pub thinking_text: String,
    /// 本轮产生的 tool_calls(可能为空,表示纯文本回复)
    pub tool_calls: Vec<crate::models::ToolCall>,
    /// provider 内部是否已通过 emitter 发射过 error 事件
    pub had_stream_error: bool,
}
///
/// 所有 Provider 实现都输出此枚举的事件。
/// 前端通过监听 Tauri 事件来处理流式响应。
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
    ThinkingStart {
        content_index: usize,
    },
    /// 思考增量
    ThinkingDelta {
        content_index: usize,
        /// 增量思考文本
        delta: String,
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
    ToolExecuting {
        id: String,
        name: String,
    },
    /// Tool 执行结果
    /// OpenAI 协议以 role:"tool" 消息塞回 messages;这里的事件用于前端实时显示
    ToolResult {
        id: String,
        name: String,
        /// 结果内容
        content: String,
        /// true=执行出错(让 model 看到错误并重试)
        is_error: bool,
    },
    /// 需要用户审批(只对 Write 类 tool 触发)
    /// 前端弹 ApprovalModal,点击后 invoke('approve_tool_call', {id, approved})
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
    /// invoke('answer_tool_question', {id, selected, custom})
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
    TurnEnd {
        tool_calls_pending: usize,
    },
}

/// 流式事件发射器
///
/// 封装 Tauri 事件发射逻辑，将 StreamEvent 发送到前端。
/// 通过单一事件名 `stream-event` 发送 JSON 序列化的事件，
/// 前端通过 event.event 字段区分事件类型。
pub struct StreamEventEmitter {
    app: tauri::AppHandle,
    event_name: String,
}

impl StreamEventEmitter {
    /// 创建新的事件发射器
    pub fn new(app: tauri::AppHandle) -> Self {
        Self {
            app,
            event_name: "stream-event".to_string(),
        }
    }

    /// 发射一个流式事件
    pub fn emit(&self, event: &StreamEvent) {
        let _ = self.app.emit(&self.event_name, event);
    }

    /// 便捷方法：发射 Start 事件
    pub fn start(&self) {
        self.emit(&StreamEvent::Start);
    }

    /// 便捷方法：发射 TextStart 事件
    pub fn text_start(&self, content_index: usize) {
        self.emit(&StreamEvent::TextStart { content_index });
    }

    /// 便捷方法：发射 TextDelta 事件
    pub fn text_delta(&self, content_index: usize, delta: &str) {
        self.emit(&StreamEvent::TextDelta {
            content_index,
            delta: delta.to_string(),
        });
    }

    /// 便捷方法：发射 TextEnd 事件
    pub fn text_end(&self, content_index: usize, content: &str) {
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

    /// 便捷方法：发射 Done 事件
    pub fn done(&self, reason: StopReason, full_text: &str) {
        self.emit(&StreamEvent::Done {
            reason,
            full_text: full_text.to_string(),
        });
    }

    /// 便捷方法：发射 Error 事件
    pub fn error(&self, reason: StopReason, message: &str, partial_text: &str) {
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

    pub fn tool_result(&self, id: &str, name: &str, content: &str, is_error: bool) {
        self.emit(&StreamEvent::ToolResult {
            id: id.to_string(),
            name: name.to_string(),
            content: content.to_string(),
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
        self.emit(&StreamEvent::TurnEnd { tool_calls_pending });
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
}
