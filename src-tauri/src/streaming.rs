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

/// 统一流式事件
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
