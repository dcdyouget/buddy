// Anthropic Messages API Provider
//
// 实现 Anthropic Messages API 的流式对话，将 Anthropic 特有的 SSE 事件格式
// 转换为统一的 StreamEvent 协议。
//
// Anthropic SSE 事件类型：
// - message_start       → 消息开始，含 usage 和 message.id
// - content_block_start → 内容块开始（text/thinking/tool_use）
// - content_block_delta → 内容块增量（text_delta/thinking_delta/input_json_delta）
// - content_block_stop  → 内容块结束
// - message_delta       → 消息增量，含 stop_reason 和最终 usage
// - message_stop        → 消息结束
//
// 参考：https://docs.anthropic.com/en/api/messages-streaming

use super::{ApiError, LlmProvider};
use crate::models::{CompatConfig, Message, MessageRole, ModelInfo};
use crate::streaming::{StopReason, StreamEventEmitter};
use futures_util::StreamExt;
use log::{error, info, warn};
use reqwest::Client;
use serde_json::Value;
use std::pin::Pin;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::timeout;

/// Anthropic API 版本（固定）
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// 单次字节块读取超时
const CHUNK_TIMEOUT: Duration = Duration::from_secs(30);

/// Anthropic Provider
pub struct AnthropicProvider;

impl AnthropicProvider {
    /// 将内部消息格式转换为 Anthropic API 格式
    fn convert_messages(messages: &[Message]) -> Vec<Value> {
        messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                };
                serde_json::json!({
                    "role": role,
                    "content": m.content,
                })
            })
            .collect()
    }

    /// 解析 Anthropic SSE 事件行
    ///
    /// 返回 (event_type, data) 或 None（非完整事件行）。
    fn parse_sse_line(line: &str) -> Option<(String, String)> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        // 注释行（以 : 开头）
        if line.starts_with(':') {
            return None;
        }

        // 解析 "field: value" 格式
        if let Some(colon_pos) = line.find(':') {
            let field = line[..colon_pos].trim();
            let value = line[colon_pos + 1..].trim();
            Some((field.to_string(), value.to_string()))
        } else {
            None
        }
    }
}

impl LlmProvider for AnthropicProvider {
    fn stream_chat<'a>(
        &'a self,
        base_url: &'a str,
        api_key: &'a str,
        model: &'a str,
        messages: &'a [Message],
        emitter: &'a StreamEventEmitter,
        mut cancel_rx: watch::Receiver<bool>,
        compat: Option<&'a CompatConfig>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String, ApiError>> + Send + 'a>> {
        Box::pin(async move {
            // 解析 compat 配置
            let _supports_temperature = compat.map(|c| c.supports_temperature()).unwrap_or(true);
            // 注意: Anthropic API 默认不发送 temperature 字段。
            // 当 supports_temperature=false (如 Claude Opus 4.7+)，保持不发送即可。

            let client = Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .map_err(|e| ApiError::NetworkError(e.to_string()))?;

            let anthropic_messages = Self::convert_messages(messages);

            let body = serde_json::json!({
                "model": model,
                "max_tokens": 4096,
                "messages": anthropic_messages,
                "stream": true,
            });

            let url = format!(
                "{}/v1/messages",
                base_url.trim_end_matches('/')
            );
            info!("[anthropic::stream_chat] 开始请求: model={}, url={}", model, url);

            let response = client
                .post(&url)
                .header("x-api-key", api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| {
                    if e.is_timeout() {
                        ApiError::NetworkError("timeout".into())
                    } else if e.is_connect() {
                        ApiError::NetworkError(e.to_string())
                    } else {
                        ApiError::NetworkError(e.to_string())
                    }
                })?;

            let status = response.status();
            info!("[anthropic::stream_chat] 收到响应: status={}", status.as_u16());
            if !status.is_success() {
                let body_text = response.text().await.unwrap_or_default();
                let preview: String = body_text.chars().take(500).collect();
                warn!(
                    "[anthropic::stream_chat] HTTP {} 错误响应: {}",
                    status.as_u16(),
                    preview
                );
                match status.as_u16() {
                    401 => return Err(ApiError::Unauthorized),
                    429 => return Err(ApiError::QuotaExceeded),
                    code if code >= 500 => return Err(ApiError::ServerError(code)),
                    _ => {
                        return Err(ApiError::NetworkError(format!(
                            "HTTP {}",
                            status.as_u16()
                        )))
                    }
                }
            }

            // 开始解析 SSE 流
            let mut byte_stream = response.bytes_stream();
            let mut buffer = String::new();

            // 流式状态追踪
            let mut full_response = String::new(); // 累积完整回复文本
            let mut current_event: Option<String> = None;
            let mut content_index: usize = 0; // 内容块索引
            let mut _has_started = false;

            let mut chunk_count: u64 = 0;
            let mut token_count: u64 = 0;

            emitter.start();

            loop {
                let chunk = tokio::select! {
                    _ = cancel_rx.changed() => {
                        if *cancel_rx.borrow() {
                            info!(
                                "[anthropic::stream_chat] 被取消: {} chunks, {} tokens",
                                chunk_count, token_count
                            );
                            emitter.error(
                                StopReason::Aborted,
                                "用户取消",
                                &full_response,
                            );
                            return Ok(full_response);
                        }
                        continue;
                    }
                    result = timeout(CHUNK_TIMEOUT, byte_stream.next()) => {
                        match result {
                            Ok(chunk) => chunk,
                            Err(_elapsed) => {
                                error!(
                                    "[anthropic::stream_chat] 读取超时: {} chunks, {} tokens",
                                    chunk_count, token_count
                                );
                                emitter.error(
                                    StopReason::Error,
                                    "读取超时",
                                    &full_response,
                                );
                                return Ok(full_response);
                            }
                        }
                    }
                };

                match chunk {
                    Some(Ok(bytes)) => {
                        chunk_count += 1;
                        let text = String::from_utf8_lossy(&bytes);
                        if chunk_count <= 3 {
                            let preview: String = text.chars().take(500).collect();
                            info!(
                                "[anthropic::stream_chat] chunk#{}: {} bytes, raw={}",
                                chunk_count,
                                bytes.len(),
                                preview
                            );
                        }
                        buffer.push_str(&text);

                        // 逐行解析 SSE
                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].to_string();
                            buffer = buffer[pos + 1..].to_string();

                            // 尝试解析 SSE 行
                            if let Some((field, value)) = Self::parse_sse_line(&line) {
                                match field.as_str() {
                                    "event" => {
                                        current_event = Some(value);
                                    }
                                    "data" => {
                                        let event_type =
                                            current_event.as_deref().unwrap_or("");
                                        // 解析 data JSON
                                        match serde_json::from_str::<Value>(&value) {
                                            Ok(json) => {
                                                match event_type {
                                                    "message_start" => {
                                                        _has_started = true;
                                                        // 提取 input_tokens（usage 在 message.message.usage 或直接 message.usage）
                                                        let usage = json["message"]["usage"].as_object()
                                                            .or_else(|| json["usage"].as_object());
                                                        if let Some(u) = usage {
                                                            let input = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                                                            info!("[anthropic] message_start: input_tokens={}", input);
                                                        }
                                                    }
                                                    "content_block_start" => {
                                                        let block = &json["content_block"];
                                                        let block_type =
                                                            block["type"].as_str().unwrap_or("");
                                                        match block_type {
                                                            "text" => {
                                                                emitter.text_start(content_index);
                                                            }
                                                            "thinking" | "redacted_thinking" => {
                                                                emitter.thinking_start(content_index);
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                    "content_block_delta" => {
                                                        let delta = &json["delta"];
                                                        let delta_type =
                                                            delta["type"].as_str().unwrap_or("");
                                                        match delta_type {
                                                            "text_delta" => {
                                                                let text =
                                                                    delta["text"].as_str().unwrap_or("");
                                                                if !text.is_empty() {
                                                                    token_count += 1;
                                                                    full_response.push_str(text);
                                                                    emitter.text_delta(
                                                                        content_index,
                                                                        text,
                                                                    );
                                                                }
                                                            }
                                                            "thinking_delta" => {
                                                                let thinking =
                                                                    delta["thinking"].as_str().unwrap_or("");
                                                                if !thinking.is_empty() {
                                                                    emitter.thinking_delta(
                                                                        content_index,
                                                                        thinking,
                                                                    );
                                                                }
                                                            }
                                                            "signature_delta" => {
                                                                // 思考签名增量，暂不处理（用于多轮对话连续性）
                                                            }
                                                            "input_json_delta" => {
                                                                // 工具调用 JSON，暂不处理
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                    "content_block_stop" => {
                                                        let _block = &json.get("content_block");
                                                        // content_block_stop 中可能包含 content_block 信息
                                                        // 也可能直接通过 index 知道是哪个块
                                                        let index = json["index"].as_u64().unwrap_or(content_index as u64) as usize;

                                                        // 根据之前记录的 block 类型发送 end 事件
                                                        // 简化处理：检查是否有 text 或 thinking 内容
                                                        // 实际上我们需要追踪 block 类型，这里简化逻辑
                                                        content_index = index + 1;
                                                    }
                                                    "message_delta" => {
                                                        let delta = &json["delta"];
                                                        let stop_reason =
                                                            delta["stop_reason"].as_str().unwrap_or("");
                                                        let usage = &json["usage"];
                                                        let output_tokens =
                                                            usage["output_tokens"].as_u64().unwrap_or(0);
                                                        info!(
                                                            "[anthropic] message_delta: stop_reason={}, output_tokens={}",
                                                            stop_reason, output_tokens
                                                        );
                                                    }
                                                    "message_stop" => {
                                                        // 消息流结束，但我们需要确保所有 content_blocK_stop 都已处理
                                                        info!(
                                                            "[anthropic::stream_chat] message_stop: {} chunks, {} tokens, {} chars",
                                                            chunk_count, token_count, full_response.len()
                                                        );
                                                    }
                                                    "error" => {
                                                        let error_msg = json["error"]["message"]
                                                            .as_str()
                                                            .unwrap_or("未知错误");
                                                        error!("[anthropic] error 事件: {}", error_msg);
                                                        emitter.error(
                                                            StopReason::Error,
                                                            error_msg,
                                                            &full_response,
                                                        );
                                                        return Ok(full_response);
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            Err(e) => {
                                                warn!(
                                                    "[anthropic] JSON 解析失败: {}, data={}",
                                                    e,
                                                    &value.chars().take(200).collect::<String>()
                                                );
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            // 空行表示一个 SSE 事件的结束，重置 event 类型
                            if line.trim().is_empty() {
                                current_event = None;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        emitter.error(
                            StopReason::Error,
                            &format!("流读取错误: {}", e),
                            &full_response,
                        );
                        return Ok(full_response);
                    }
                    None => {
                        // 流正常结束
                        emitter.done(StopReason::Stop, &full_response);
                        return Ok(full_response);
                    }
                }
            }
        })
    }

    fn fetch_models<'a>(
        &'a self,
        base_url: &'a str,
        api_key: &'a str,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<ModelInfo>, String>> + Send + 'a>> {
        Box::pin(async move {
            // Anthropic 没有公开的模型列表 API，返回已知模型
            // 实际应用中可以从配置文件或硬编码列表获取
            let url = format!(
                "{}/v1/models",
                base_url.trim_end_matches('/')
            );

            let client = Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(15))
                .build()
                .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

            let response = client
                .get(&url)
                .header("x-api-key", api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .send()
                .await
                .map_err(|e| format!("请求模型列表失败: {}", e))?;

            if !response.status().is_success() {
                // 如果 /v1/models 端点不可用，返回硬编码的 Anthropic 模型列表
                info!(
                    "[anthropic::fetch_models] /v1/models 不可用 ({}), 使用内置模型列表",
                    response.status().as_u16()
                );
                return Ok(builtin_anthropic_models());
            }

            let body_text = response.text().await.map_err(|e| e.to_string())?;
            let json: Value =
                serde_json::from_str(&body_text).map_err(|e| format!("JSON 解析失败: {}", e))?;

            let models: Vec<ModelInfo> = json["data"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|m| {
                    let id = m["id"].as_str()?;
                    Some(ModelInfo {
                        id: id.to_string(),
                        provider_id: String::new(),
                        display_name: id.to_string(),
                        context_window: 200000,
                        latency_ms: None,
                    })
                })
                .collect();

            if models.is_empty() {
                Ok(builtin_anthropic_models())
            } else {
                Ok(models)
            }
        })
    }

    fn test_latency<'a>(
        &'a self,
        base_url: &'a str,
        api_key: &'a str,
        model_id: &'a str,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<u32, String>> + Send + 'a>> {
        Box::pin(async move {
            let client = Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

            let url = format!(
                "{}/v1/messages",
                base_url.trim_end_matches('/')
            );

            let body = serde_json::json!({
                "model": model_id,
                "max_tokens": 1,
                "messages": [{ "role": "user", "content": "hi" }],
                "stream": false,
            });

            let start = std::time::Instant::now();

            let response = client
                .post(&url)
                .header("x-api-key", api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("测速请求失败: {}", e))?;

            if !response.status().is_success() {
                return Err(format!("API returned error status: {}", response.status()));
            }

            let elapsed = start.elapsed().as_millis() as u32;
            Ok(elapsed)
        })
    }
}

/// 内置 Anthropic 模型列表
///
/// 当 /v1/models 端点不可用时使用此列表。
fn builtin_anthropic_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "claude-sonnet-4-6".to_string(),
            provider_id: String::new(),
            display_name: "Claude Sonnet 4.6".to_string(),
            context_window: 200000,
            latency_ms: None,
        },
        ModelInfo {
            id: "claude-opus-4-8".to_string(),
            provider_id: String::new(),
            display_name: "Claude Opus 4.8".to_string(),
            context_window: 200000,
            latency_ms: None,
        },
        ModelInfo {
            id: "claude-haiku-4-5".to_string(),
            provider_id: String::new(),
            display_name: "Claude Haiku 4.5".to_string(),
            context_window: 200000,
            latency_ms: None,
        },
        ModelInfo {
            id: "claude-fable-5".to_string(),
            provider_id: String::new(),
            display_name: "Claude Fable 5".to_string(),
            context_window: 200000,
            latency_ms: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sse_line_event() {
        assert_eq!(
            AnthropicProvider::parse_sse_line("event: message_start"),
            Some(("event".to_string(), "message_start".to_string()))
        );
    }

    #[test]
    fn test_parse_sse_line_data() {
        assert_eq!(
            AnthropicProvider::parse_sse_line("data: {\"type\":\"text\"}"),
            Some(("data".to_string(), "{\"type\":\"text\"}".to_string()))
        );
    }

    #[test]
    fn test_parse_sse_line_empty() {
        assert_eq!(AnthropicProvider::parse_sse_line(""), None);
    }

    #[test]
    fn test_parse_sse_line_comment() {
        assert_eq!(AnthropicProvider::parse_sse_line(": this is a comment"), None);
    }
}
