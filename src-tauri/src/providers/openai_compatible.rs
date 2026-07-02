// OpenAI 兼容 API Provider
//
// 实现标准 OpenAI /v1/chat/completions 端点（SSE 流式）的 Provider。
// 支持所有 OpenAI 兼容的 API（OpenAI、DeepSeek、MiniMax、GLM、Kimi 等）。
//
// 将 OpenAI SSE 格式（choices[0].delta.content）转换为统一的 StreamEvent 协议。

use super::{ApiError, LlmProvider};
use crate::models::{get_context_window, CompatConfig, Message, MessageRole, ModelInfo};
use crate::streaming::{StopReason, StreamEventEmitter};
use futures_util::StreamExt;
use log::{error, info, warn};
use reqwest::Client;
use serde_json::Value;
use std::pin::Pin;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::timeout;

/// 单次字节块读取超时
const CHUNK_TIMEOUT: Duration = Duration::from_secs(30);

/// 模型列表获取超时
const FETCH_MODELS_TIMEOUT: Duration = Duration::from_secs(15);

/// OpenAI 兼容 Provider
pub struct OpenAICompatibleProvider;

impl OpenAICompatibleProvider {
    /// 将内部消息格式转换为 OpenAI API 格式
    fn convert_messages(messages: &[Message]) -> Vec<Value> {
        messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": match m.role {
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                    },
                    "content": m.content,
                })
            })
            .collect()
    }

    /// 根据 base_url 构造模型列表候选 URL
    fn build_model_url_candidates(base_url: &str) -> Vec<String> {
        let trimmed = base_url.trim().trim_end_matches('/');
        if trimmed.is_empty() {
            return vec![];
        }

        let mut candidates: Vec<String> = Vec::new();

        let ends_with_version = {
            let last = trimmed.rsplit('/').next().unwrap_or("");
            last.strip_prefix('v')
                .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        };

        if ends_with_version {
            candidates.push(format!("{trimmed}/models"));
            if !trimmed.ends_with("/v1") {
                candidates.push(format!("{trimmed}/v1/models"));
            }
        } else {
            candidates.push(format!("{trimmed}/v1/models"));
        }

        candidates
    }
}

impl LlmProvider for OpenAICompatibleProvider {
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
            let client = Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .map_err(|e| ApiError::NetworkError(e.to_string()))?;

            let chat_messages = Self::convert_messages(messages);

            // 应用 compat 配置构建请求体
            let body = build_openai_request_body(model, &chat_messages, compat);

            let url = format!(
                "{}/chat/completions",
                base_url.trim_end_matches('/')
            );
            info!(
                "[openai::stream_chat] 开始请求: model={}, url={}",
                model, url
            );

            let response = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
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
            info!(
                "[openai::stream_chat] 收到响应: status={}",
                status.as_u16()
            );
            if !status.is_success() {
                let body_text = response.text().await.unwrap_or_default();
                let preview: String = body_text.chars().take(500).collect();
                warn!(
                    "[openai::stream_chat] HTTP {} 错误响应: {}",
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

            // 发送 Start 事件
            emitter.start();

            let mut byte_stream = response.bytes_stream();
            let mut buffer = String::new();
            let mut full_response = String::new();

            info!("[openai::stream_chat] 开始接收流式数据...");
            let mut chunk_count: u64 = 0;
            let mut token_count: u64 = 0;

            // OpenAI 兼容格式只有一个文本块（content_index = 0）
            let content_index: usize = 0;
            let mut text_started = false;

            loop {
                let chunk = tokio::select! {
                    _ = cancel_rx.changed() => {
                        if *cancel_rx.borrow() {
                            info!(
                                "[openai::stream_chat] 被取消: {} chunks, {} tokens",
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
                                    "[openai::stream_chat] 读取超时: {} chunks, {} tokens, {} chars",
                                    chunk_count, token_count, full_response.len()
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
                                "[openai::stream_chat] chunk#{}: {} bytes, raw={}",
                                chunk_count,
                                bytes.len(),
                                preview
                            );
                        }
                        buffer.push_str(&text);

                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].to_string();
                            buffer = buffer[pos + 1..].to_string();

                            let line = line.trim().to_string();
                            if line.is_empty() {
                                continue;
                            }

                            if let Some(data) = line
                                .strip_prefix("data: ")
                                .or_else(|| line.strip_prefix("data:"))
                            {
                                let data = data.trim();
                                if data == "[DONE]" {
                                    info!(
                                        "[openai::stream_chat] 完成: {} chunks, {} tokens, {} chars",
                                        chunk_count, token_count, full_response.len()
                                    );
                                    if text_started {
                                        emitter.text_end(content_index, &full_response);
                                    }
                                    emitter.done(StopReason::Stop, &full_response);
                                    return Ok(full_response);
                                }
                                if data.is_empty() {
                                    continue;
                                }

                                match serde_json::from_str::<Value>(data) {
                                    Ok(json) => {
                                        // 检查是否有 reasoning_content（DeepSeek 等支持思考的模型）
                                        let reasoning = json["choices"]
                                            .get(0)
                                            .and_then(|c| c["delta"]["reasoning_content"].as_str())
                                            .unwrap_or("");

                                        if !reasoning.is_empty() {
                                            // reasoning_content 作为思考块发射
                                            if !text_started {
                                                // 如果之前有文本输出，先结束文本块
                                                // 简单起见，reasoning 总是在文本之前
                                            }
                                            emitter.thinking_delta(content_index, reasoning);
                                            continue;
                                        }

                                        let delta = json["choices"]
                                            .get(0)
                                            .and_then(|c| c["delta"]["content"].as_str())
                                            .unwrap_or("");

                                        if !delta.is_empty() {
                                            if !text_started {
                                                text_started = true;
                                                emitter.text_start(content_index);
                                            }
                                            token_count += 1;
                                            full_response.push_str(delta);
                                            emitter.text_delta(content_index, delta);
                                        }
                                    }
                                    Err(_) => continue,
                                }
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
                        if text_started {
                            emitter.text_end(content_index, &full_response);
                        }
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
            let candidates = Self::build_model_url_candidates(base_url);
            if candidates.is_empty() {
                return Err("Base URL 为空".to_string());
            }

            let client = Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(FETCH_MODELS_TIMEOUT)
                .tcp_keepalive(Duration::from_secs(60))
                .build()
                .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

            let mut last_err: Option<String> = None;

            for url in &candidates {
                let response = match client
                    .get(url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        error!("[openai::fetch_models] 网络错误: url={}, err={:?}", url, e);
                        last_err = Some(format!("请求失败: {}", e));
                        continue;
                    }
                };

                let status = response.status();
                let body_text = match response.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        last_err = Some(format!("读取失败: {}", e));
                        continue;
                    }
                };

                if status == reqwest::StatusCode::NOT_FOUND
                    || status == reqwest::StatusCode::METHOD_NOT_ALLOWED
                {
                    warn!(
                        "[openai::fetch_models] 端点不存在 ({}), 尝试下一个候选",
                        status.as_u16()
                    );
                    last_err = Some(format!("HTTP {}: endpoint 不存在", status.as_u16()));
                    continue;
                }

                if !status.is_success() {
                    return Err(format!("HTTP {}: {:.500}", status.as_u16(), body_text));
                }

                let json: Value = match serde_json::from_str(&body_text) {
                    Ok(j) => j,
                    Err(e) => return Err(format!("解析失败: {}", e)),
                };

                // OpenAI 格式：{ "data": [...] }
                if let Some(arr) = json["data"].as_array() {
                    let models: Vec<ModelInfo> = arr
                        .iter()
                        .filter_map(|m| {
                            let id = m["id"].as_str()?;
                            Some(ModelInfo {
                                id: id.to_string(),
                                provider_id: String::new(),
                                display_name: id.to_string(),
                                context_window: get_context_window(id),
                                latency_ms: None,
                            })
                        })
                        .collect();
                    info!(
                        "[openai::fetch_models] 成功: {} 个模型, url={}",
                        models.len(),
                        url
                    );
                    return Ok(models);
                }

                // { "models": [...] } 格式
                if let Some(arr) = json["models"].as_array() {
                    let models: Vec<ModelInfo> = arr
                        .iter()
                        .filter_map(|m| {
                            let id = m["id"].as_str()?;
                            Some(ModelInfo {
                                id: id.to_string(),
                                provider_id: String::new(),
                                display_name: id.to_string(),
                                context_window: get_context_window(id),
                                latency_ms: None,
                            })
                        })
                        .collect();
                    info!(
                        "[openai::fetch_models] 成功: {} 个模型, url={}",
                        models.len(),
                        url
                    );
                    return Ok(models);
                }

                warn!("[openai::fetch_models] 无法识别的响应格式: url={}", url);
                last_err = Some("响应格式无法识别".to_string());
            }

            Err(last_err.unwrap_or_else(|| "所有候选 URL 均失败".to_string()))
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
                "{}/chat/completions",
                base_url.trim_end_matches('/')
            );

            let body = serde_json::json!({
                "model": model_id,
                "messages": [{ "role": "user", "content": "hi" }],
                "max_tokens": 1,
                "stream": false,
            });

            let start = std::time::Instant::now();

            let response = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
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

/// 根据 compat 配置构建 OpenAI 兼容 API 请求体
///
/// 参考 pi-agent buildParams() 中的 thinkingFormat 分支处理。
/// 不同厂商即使使用 OpenAI 兼容格式，thinking/reasoning 参数的发送方式也不同。
fn build_openai_request_body(
    model: &str,
    messages: &[serde_json::Value],
    compat: Option<&CompatConfig>,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });

    // 解析 compat 配置（使用默认值回退）
    let thinking_format = compat.map(|c| c.thinking_format()).unwrap_or("openai");
    let max_tokens_field = compat.map(|c| c.max_tokens_field()).unwrap_or("max_tokens");
    let supports_stream_usage = compat.map(|c| c.supports_stream_options_usage()).unwrap_or(true);
    let _supports_reasoning_effort = compat.map(|c| c.supports_reasoning_effort()).unwrap_or(true);

    // 使用正确的 max_tokens 字段名
    match max_tokens_field {
        "max_completion_tokens" => {
            body["max_completion_tokens"] = serde_json::json!(4096);
        }
        _ => {
            body["max_tokens"] = serde_json::json!(4096);
        }
    }

    // stream_options: include_usage（仅在支持时发送）
    if supports_stream_usage {
        body["stream_options"] = serde_json::json!({
            "include_usage": true,
        });
    }

    // 根据 thinking_format 发送不同格式的 reasoning/thinking 参数
    match thinking_format {
        "deepseek" => {
            // DeepSeek: thinking: { type: "enabled" } 格式
            // 注意：DeepSeek v4 模型总是会思考，不需要显式发送 thinking 参数
            // 但保留此格式以便 R1 等旧模型使用
            log::info!("[openai] using deepseek thinking format");
        }
        "openrouter" => {
            // OpenRouter: reasoning: { effort: "..." } 格式
            log::info!("[openai] using openrouter thinking format");
        }
        "qwen" => {
            // Qwen: enable_thinking: true/false
            log::info!("[openai] using qwen thinking format");
        }
        "together" => {
            // Together: reasoning: { enabled: true/false }
            log::info!("[openai] using together thinking format");
        }
        "zai" => {
            // Z.AI: thinking: { type: "enabled"/"disabled" }
            log::info!("[openai] using zai thinking format");
        }
        _ => {
            // 默认 openai 格式: reasoning_effort 字段
            // 不在此处发送，由调用方根据模型能力决定
        }
    }

    body
}
