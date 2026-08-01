// ============================================================================
// OpenAI 兼容 API Provider
// ============================================================================
//
// 实现标准 OpenAI /v1/chat/completions 端点（SSE 流式）的 Provider。
// 支持所有 OpenAI 兼容的 API（OpenAI、DeepSeek、MiniMax、GLM、Kimi 等）。
//
// 将 OpenAI SSE 格式（choices[0].delta.content）转换为统一的 StreamEvent 协议。
//
// 与 anthropic.rs 的对比：
//   - 协议不同：OpenAI 的 SSE 数据是单行 `data: {...}`，Anthropic 是 `event:` + `data:`
//   - 结束标志：OpenAI 用 `data: [DONE]`；Anthropic 用 `event: message_stop`
//   - 增量结构：OpenAI 的 delta.content 是单个字符串；Anthropic 还要区分 text/thinking
// ============================================================================

use super::{ApiError, LlmProvider};
use crate::models::{get_context_window, CompatConfig, Message, MessageRole, ModelInfo, ToolCall};
use crate::streaming::{StopReason, StreamEventEmitter, StreamOutcome};
use crate::tools::ToolDefinition;
use futures_util::StreamExt; // Stream trait 扩展（提供 .next() 等）
use log::{error, info, warn}; // 日志门面
use reqwest::Client; // 异步 HTTP 客户端
use serde_json::{json, Value}; // 通用 JSON 值
use std::pin::Pin; // 自引用指针固定
use std::time::{Duration, Instant}; // 时间段与耗时统计
use tokio::sync::watch; // watch channel（取消信号）
use tokio::time::timeout; // 异步超时

/// 单次字节块读取超时
const CHUNK_TIMEOUT: Duration = Duration::from_secs(30);

/// SSE 单行数据上限：正常文本增量远小于此；防止异常/恶意响应把行缓冲撑爆。
const MAX_SSE_LINE_BYTES: usize = 1 << 20;

/// 模型列表获取超时
const FETCH_MODELS_TIMEOUT: Duration = Duration::from_secs(15);

// ============================================================================
// OpenAICompatibleProvider —— OpenAI 协议族适配器
// ============================================================================
pub struct OpenAICompatibleProvider;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PromptCacheUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    cache_hit_tokens: Option<u64>,
    cache_miss_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
}

impl PromptCacheUsage {
    fn hit_rate_percent(&self) -> Option<f64> {
        let hit = self.cache_hit_tokens?;
        (self.prompt_tokens > 0).then(|| hit as f64 * 100.0 / self.prompt_tokens as f64)
    }
}

fn parse_prompt_cache_usage(json: &Value) -> Option<PromptCacheUsage> {
    let usage = json.get("usage")?.as_object()?;
    let prompt_tokens = usage.get("prompt_tokens")?.as_u64()?;
    let completion_tokens = usage
        .get("completion_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(prompt_tokens.saturating_add(completion_tokens));
    let prompt_details = usage
        .get("prompt_tokens_details")
        .and_then(Value::as_object);
    let cache_hit_tokens = usage
        .get("prompt_cache_hit_tokens")
        .and_then(Value::as_u64)
        .or_else(|| {
            prompt_details
                .and_then(|details| details.get("cached_tokens"))
                .and_then(Value::as_u64)
        });
    let cache_miss_tokens = usage
        .get("prompt_cache_miss_tokens")
        .and_then(Value::as_u64)
        .or_else(|| cache_hit_tokens.map(|hit| prompt_tokens.saturating_sub(hit)));
    let cache_write_tokens = prompt_details
        .and_then(|details| details.get("cache_write_tokens"))
        .and_then(Value::as_u64);

    Some(PromptCacheUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cache_hit_tokens,
        cache_miss_tokens,
        cache_write_tokens,
    })
}

fn usage_log_summary(usage: Option<&PromptCacheUsage>) -> String {
    let Some(usage) = usage else {
        return "tokens=Provider未返回".to_string();
    };
    let cache = match (usage.cache_hit_tokens, usage.hit_rate_percent()) {
        (Some(hit), Some(rate)) => format!(
            "cache_hit={}, cache_miss={}, cache_write={}, cache_rate={:.2}%",
            hit,
            usage
                .cache_miss_tokens
                .unwrap_or_else(|| usage.prompt_tokens.saturating_sub(hit)),
            usage.cache_write_tokens.unwrap_or(0),
            rate,
        ),
        _ => "cache=Provider未返回".to_string(),
    };
    format!(
        "input_tokens={}, output_tokens={}, total_tokens={}, {}",
        usage.prompt_tokens, usage.completion_tokens, usage.total_tokens, cache
    )
}

fn has_valid_tool_arguments(tool_call: &ToolCall) -> bool {
    !tool_call.id.trim().is_empty()
        && !tool_call.name.trim().is_empty()
        && matches!(
            serde_json::from_str::<Value>(&tool_call.arguments),
            Ok(Value::Object(_))
        )
}

impl OpenAICompatibleProvider {
    /// 将内部消息格式转换为 OpenAI API 格式
    ///
    /// 与 Anthropic 转换器几乎一致（都是 role + content 数组）
    fn convert_messages(messages: &[Message]) -> Vec<Value> {
        // 系统提示保持静态；每条用户消息根据固定 created_at 重建相同的时间后缀，
        // 让对话增长时仍保持此前请求的精确前缀。
        let mut result: Vec<Value> = vec![json!({
            "role": "system",
            "content": super::BUDDY_SYSTEM_PROMPT,
        })];
        // 只有参数完整、且存在对应 tool_result 的调用才允许进入下一次请求。
        // 主动停止流式输出时，assistant 中可能留下半截 arguments；MiniMax 会直接以
        // invalid function arguments json string (2013) 拒绝整次请求。
        let tool_result_ids: std::collections::HashSet<&str> = messages
            .iter()
            .filter(|m| m.role == MessageRole::Tool)
            .filter_map(|m| m.tool_call_id.as_deref())
            .filter(|id| !id.trim().is_empty())
            .collect();

        let valid_tool_call_ids: std::collections::HashSet<&str> = messages
            .iter()
            .filter(|m| m.role == MessageRole::Assistant)
            .filter_map(|m| m.tool_calls.as_ref())
            .flat_map(|calls| calls.iter())
            .filter(|tc| has_valid_tool_arguments(tc) && tool_result_ids.contains(tc.id.as_str()))
            .map(|tc| tc.id.as_str())
            .collect();

        let converted: Vec<Value> = messages
            .iter()
            .filter_map(|m| {
                // 孤儿或未完成配对的 tool 消息整条过滤掉。
                if m.role == MessageRole::Tool {
                    let tc_id = m.tool_call_id.as_deref().unwrap_or("");
                    if tc_id.is_empty() || !valid_tool_call_ids.contains(tc_id) {
                        warn!(
                            "[openai::convert_messages] 过滤无效 tool 消息, tool_call_id='{}'",
                            tc_id
                        );
                        return None;
                    }
                }

                let original_has_tool_calls = m.role == MessageRole::Assistant
                    && m.tool_calls.as_ref().is_some_and(|calls| !calls.is_empty());
                let valid_calls: Vec<&ToolCall> = if m.role == MessageRole::Assistant {
                    m.tool_calls
                        .iter()
                        .flatten()
                        .filter(|tc| {
                            if !has_valid_tool_arguments(tc) {
                                warn!(
                                    "[openai::convert_messages] 丢弃参数不完整的 tool_call, id='{}', name='{}'",
                                    tc.id, tc.name
                                );
                                return false;
                            }
                            if !tool_result_ids.contains(tc.id.as_str()) {
                                warn!(
                                    "[openai::convert_messages] 丢弃没有结果的 tool_call, id='{}', name='{}'",
                                    tc.id, tc.name
                                );
                                return false;
                            }
                            true
                        })
                        .collect()
                } else {
                    Vec::new()
                };

                // 纯工具调用消息在中断后若没有任何可发送的调用，就不能留下空 assistant。
                if original_has_tool_calls
                    && valid_calls.is_empty()
                    && m.content.trim().is_empty()
                {
                    return None;
                }

                // content: assistant 消息有 tool_calls 时 content 必须为 null
                // (OpenAI 协议规范；MiniMax 等 Provider 严格校验此字段)
                let has_tool_calls = !valid_calls.is_empty();
                let content_val = if has_tool_calls {
                    json!(null)
                } else if m.role == MessageRole::User && !m.images.is_empty() {
                    let mut content = Vec::with_capacity(m.images.len() + 1);
                    if !m.content.is_empty() {
                        content.push(json!({
                            "type": "text",
                            "text": m.content,
                        }));
                    }
                    content.extend(m.images.iter().map(|image| {
                        json!({
                            "type": "image_url",
                            "image_url": {
                                "url": image.data_url,
                            },
                        })
                    }));
                    let context = super::runtime_time_context_for_message(m.created_at);
                    content.push(json!({
                        "type": "text",
                        "text": context,
                    }));
                    json!(content)
                } else if m.role == MessageRole::User {
                    let context = super::runtime_time_context_for_message(m.created_at);
                    json!(super::append_runtime_time_context(
                        &m.content,
                        &context,
                    ))
                } else {
                    json!(m.content)
                };

                let mut obj = json!({
                    "role": match m.role {
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                        MessageRole::Tool => "tool",
                    },
                    "content": content_val,
                });
                if matches!(m.role, MessageRole::Tool) {
                    let tc_id = m.tool_call_id.as_deref().unwrap_or("");
                    if !tc_id.is_empty() {
                        obj["tool_call_id"] = json!(tc_id);
                        if let Some(ref name) = m.tool_name {
                            obj["name"] = json!(name);
                        }
                    }
                }
                if matches!(m.role, MessageRole::Assistant) {
                    if !valid_calls.is_empty() {
                        // 转换为 OpenAI 兼容的 tool_calls 格式：
                        //   [{id, type:"function", function:{name, arguments}}]
                        let openai_calls: Vec<Value> = valid_calls
                            .iter()
                            .map(|tc| {
                                json!({
                                    "id": tc.id,
                                    "type": "function",
                                    "function": {
                                        "name": tc.name,
                                        "arguments": tc.arguments,
                                    }
                                })
                            })
                            .collect();
                        obj["tool_calls"] = json!(openai_calls);
                    }
                }
                Some(obj)
            })
            .collect();

        // 把 system 消息放到最前
        result.extend(converted);
        result
    }

    /// 根据 base_url 构造模型列表候选 URL
    ///
    /// 不同 Provider 的 /models 端点位置不同：
    ///   - 有些直接挂 /models（base_url 已经是 https://host/v1）
    ///   - 有些挂 /v1/models（base_url 只是 https://host）
    ///
    /// 函数同时生成两种候选 URL，逐一尝试
    fn build_model_url_candidates(base_url: &str) -> Vec<String> {
        // trim() 去掉首尾空白，trim_end_matches('/') 去掉结尾的 '/'
        let trimmed = base_url.trim().trim_end_matches('/');
        if trimmed.is_empty() {
            return vec![]; // 提前返回空 Vec
        }

        let mut candidates: Vec<String> = Vec::new();

        // ── 块表达式 `{...}` 的返回值 ──
        // 整个 `let ends_with_version = { ... };` 把块最后一个表达式作为值
        // 块里：trimmed.rsplit('/').next().unwrap_or("") 取最后一个 '/ ' 后面的段
        //   .strip_prefix('v')  去掉开头的 'v'，返回 Option<&str>
        //   .is_some_and(|d| ...)  Option 上挂的便捷方法：
        //     Some(x) -> f(x)
        //     None    -> false
        //   闭包里：!d.is_empty() && d.bytes().all(|b| b.is_ascii_digit())
        //     判断去掉 v 后剩余是否都是数字（如 "v1", "v2"）
        let ends_with_version = {
            let last = trimmed.rsplit('/').next().unwrap_or("");
            last.strip_prefix('v')
                .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        };

        if ends_with_version {
            // base_url 已经带 /v{数字}，直接加 /models
            candidates.push(format!("{trimmed}/models"));
            // 注意：`{trimmed}` 是 Rust 1.58+ 的"捕获标识符"语法糖
            // 等价于 format!("{}", trimmed) —— 变量名直接当占位符
            if !trimmed.ends_with("/v1") {
                candidates.push(format!("{trimmed}/v1/models"));
            }
        } else {
            // base_url 没有版本段，约定加 /v1/models
            candidates.push(format!("{trimmed}/v1/models"));
        }

        candidates
    }
}

// ============================================================================
// impl LlmProvider for OpenAICompatibleProvider
// ============================================================================
impl LlmProvider for OpenAICompatibleProvider {
    fn stream_chat<'a>(
        &'a self,
        base_url: &'a str,
        api_key: &'a str,
        model: &'a str,
        request_id: &'a str,
        messages: &'a [Message],
        emitter: &'a StreamEventEmitter,
        mut cancel_rx: watch::Receiver<bool>,
        compat: Option<&'a CompatConfig>,
        tools: &'a [ToolDefinition],
    ) -> Pin<Box<dyn std::future::Future<Output = Result<StreamOutcome, ApiError>> + Send + 'a>>
    {
        Box::pin(async move {
            // ── 1. 创建 HTTP 客户端 ──
            let client = Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .map_err(|e| ApiError::NetworkError(e.to_string()))?;

            // ── 2. 转换消息 ──
            let chat_messages = Self::convert_messages(messages);

            // ── 3. 应用 compat 配置构造请求体 ──
            // 这个函数在文件末尾定义，处理不同厂商的差异（max_tokens 字段名、thinking 参数等）
            let body = build_openai_request_body(
                model,
                &chat_messages,
                compat,
                if tools.is_empty() { None } else { Some(tools) },
            );

            // ── 4. 拼接 URL（OpenAI 兼容都是 /chat/completions） ──
            let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
            let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
            let request_bytes = serde_json::to_vec(&body)
                .map(|serialized| serialized.len())
                .unwrap_or_else(|_| body.to_string().len());
            info!(
                "[llm][{}] 请求摘要: protocol=openai_chat_completions, model={}, url={}, body_bytes={}, system_prompt_bytes={}, runtime_contexts={}, {}, available_tools={} [{}]",
                request_id,
                model,
                url,
                request_bytes,
                super::BUDDY_SYSTEM_PROMPT.len(),
                messages
                    .iter()
                    .filter(|message| message.role == MessageRole::User)
                    .count(),
                super::summarize_messages_for_log(messages),
                tools.len(),
                tool_names.join(", ")
            );

            // ── 5. 发送请求 ──
            // 与 Anthropic 不同：OpenAI 用 Authorization: Bearer <key>
            let request_started = Instant::now();
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

            // ── 6. 检查 HTTP 状态 ──
            let status = response.status();
            let headers_ms = request_started.elapsed().as_millis() as u64;
            info!(
                "[llm][{}] 响应头: status={}, headers_ms={}",
                request_id,
                status.as_u16(),
                headers_ms
            );
            if !status.is_success() {
                let body_text = response.text().await.unwrap_or_default();
                warn!(
                    "[openai::stream_chat] HTTP {} 完整错误响应: {}",
                    status.as_u16(),
                    body_text,
                );
                let api_msg = super::extract_error_message(&body_text);
                match status.as_u16() {
                    401 => return Err(ApiError::Unauthorized),
                    429 => return Err(ApiError::QuotaExceeded),
                    code if code >= 500 => return Err(ApiError::ServerError(code, api_msg)),
                    _ => {
                        return Err(ApiError::NetworkError(format!(
                            "HTTP {}: {}",
                            status.as_u16(),
                            api_msg
                        )))
                    }
                }
            }

            // 发送 Start 事件
            emitter.start();

            // ── 7. 启动字节流读取循环 ──
            let mut byte_stream = response.bytes_stream();
            // 用字节缓冲而不是 String：按网络 chunk 解码会把跨 chunk 的多字节字符
            // （中文/emoji）永久损坏，必须在完整行处才解码。
            let mut buffer: Vec<u8> = Vec::new();
            let mut full_response = String::new();
            let mut thinking_response = String::new();

            info!("[openai::stream_chat] 开始接收流式数据...");
            let mut chunk_count: u64 = 0;
            let mut token_count: u64 = 0;
            let mut response_bytes = 0usize;
            let mut first_output_ms: Option<u64> = None;
            let mut final_usage: Option<PromptCacheUsage> = None;

            let content_index: usize = 0;
            let mut text_started = false;

            use std::collections::HashMap;
            let mut tool_calls: HashMap<String, ToolCall> = HashMap::new();
            let mut tool_call_indexes: HashMap<String, usize> = HashMap::new();

            let flush_tool_calls = |tc: &mut HashMap<String, ToolCall>,
                                    tci: &HashMap<String, usize>,
                                    em: &StreamEventEmitter|
             -> Vec<ToolCall> {
                let mut ordered: Vec<(usize, ToolCall)> = tc
                    .drain()
                    .map(|(id, mut t)| {
                        let idx = tci.get(&id).copied().unwrap_or(0);
                        if t.arguments.is_empty() {
                            t.arguments = "{}".to_string();
                        }
                        (idx, t)
                    })
                    .collect();
                ordered.sort_by_key(|(i, _)| *i);
                let p = ordered.len();
                for (_, t) in &ordered {
                    em.tool_call_end(&t.id, &t.name, &t.arguments);
                }
                em.turn_end(p);
                ordered.into_iter().map(|(_, t)| t).collect()
            };

            loop {
                let chunk = tokio::select! {
                    // 取消分支
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
                            return Ok(StreamOutcome { full_text: full_response, thinking_text: thinking_response, tool_calls: vec![], had_stream_error: true });
                        }
                        continue;
                    }
                    // 超时分支
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
                                return Ok(StreamOutcome { full_text: full_response, thinking_text: thinking_response, tool_calls: vec![], had_stream_error: true });
                            }
                        }
                    }
                };

                match chunk {
                    Some(Ok(bytes)) => {
                        chunk_count += 1;
                        response_bytes += bytes.len();
                        // 累积原始字节而不是按 chunk 解码：若多字节字符（如中文）被 TCP
                        // 分块从中间切开，逐 chunk 用 from_utf8_lossy 会永久损坏该字符。
                        // 改为按字节缓冲，遇到完整行（以 0x0A 结尾）才整行解码。
                        buffer.extend_from_slice(&bytes);
                        // 单行数据上限：防止异常/恶意响应把行缓冲撑爆
                        if buffer.len() > MAX_SSE_LINE_BYTES {
                            warn!(
                                "[openai::stream_chat] SSE 行数据超过 {} 字节，终止解析",
                                MAX_SSE_LINE_BYTES
                            );
                            emitter.error(
                                StopReason::Error,
                                "响应数据行超过大小上限",
                                &full_response,
                            );
                            return Ok(StreamOutcome {
                                full_text: full_response,
                                thinking_text: thinking_response,
                                tool_calls: vec![],
                                had_stream_error: true,
                            });
                        }

                        // ── 逐行解析（OpenAI 协议比 Anthropic 简单） ──
                        // 0x0A 不可能是多字节字符的续字节，按字节切出的完整行保证是合法 UTF-8
                        while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                            let line = String::from_utf8_lossy(&buffer[..pos]).to_string();
                            buffer.drain(..pos + 1);

                            let line = line.trim().to_string();
                            if line.is_empty() {
                                continue; // 跳过空行
                            }

                            // OpenAI SSE 格式：每行就是 `data: {...}`
                            // strip_prefix("data: ") 返回 Option<&str>（剥去前缀后的剩余）
                            // .or_else(...) 提供"备用方案"：若没有前缀带空格的形式，再试不带空格
                            // ≈ Java: Optional.orElse(other)
                            if let Some(data) = line
                                .strip_prefix("data: ")
                                .or_else(|| line.strip_prefix("data:"))
                            {
                                let data = data.trim();
                                // OpenAI 流结束标志
                                if data == "[DONE]" {
                                    if text_started {
                                        emitter.text_end(content_index, &full_response);
                                    }
                                    let calls = flush_tool_calls(
                                        &mut tool_calls,
                                        &tool_call_indexes,
                                        emitter,
                                    );
                                    info!(
                                        "[llm][{}] 响应摘要: status=success, total_ms={}, headers_ms={}, first_output_ms={}, sse_bytes={}, chunks={}, text_delta_events={}, answer_bytes={}, answer_chars={}, thinking_bytes={}, thinking_chars={}, tool_calls={}, {}",
                                        request_id,
                                        request_started.elapsed().as_millis(),
                                        headers_ms,
                                        first_output_ms
                                            .map(|value| value.to_string())
                                            .unwrap_or_else(|| "无输出".to_string()),
                                        response_bytes,
                                        chunk_count,
                                        token_count,
                                        full_response.len(),
                                        full_response.chars().count(),
                                        thinking_response.len(),
                                        thinking_response.chars().count(),
                                        calls.len(),
                                        usage_log_summary(final_usage.as_ref()),
                                    );
                                    // done 事件由 commands.rs 在整轮 tool 循环结束时统一发射
                                    return Ok(StreamOutcome {
                                        full_text: full_response,
                                        thinking_text: thinking_response,
                                        tool_calls: calls,
                                        had_stream_error: false,
                                    });
                                }
                                if data.is_empty() {
                                    continue;
                                }

                                // 解析 JSON
                                match serde_json::from_str::<Value>(data) {
                                    Ok(json) => {
                                        if let Some(usage) = parse_prompt_cache_usage(&json) {
                                            final_usage = Some(usage);
                                        }
                                        // 检查是否有 reasoning_content（DeepSeek 等支持思考的模型）
                                        // 链式 .get(0).and_then(...).unwrap_or("")：
                                        //   .get(0)         json["choices"] 是数组，.get(0) 拿 Option<&Value>
                                        //   .and_then(|c| c["delta"]["reasoning_content"].as_str())
                                        //                   若 c 是 Some 则继续；为 None 时整体返回 None
                                        //   .unwrap_or("")  None 时给空串
                                        let reasoning = json["choices"]
                                            .get(0)
                                            .and_then(|c| c["delta"]["reasoning_content"].as_str())
                                            .unwrap_or("");

                                        if !reasoning.is_empty() {
                                            first_output_ms.get_or_insert_with(|| {
                                                request_started.elapsed().as_millis() as u64
                                            });
                                            // reasoning_content 作为思考块发射
                                            emitter.thinking_delta(content_index, reasoning);
                                            thinking_response.push_str(reasoning);
                                            continue; // 跳过本行的 content 处理
                                        }

                                        // 正常的文本增量
                                        let delta = json["choices"]
                                            .get(0)
                                            .and_then(|c| c["delta"]["content"].as_str())
                                            .unwrap_or("");

                                        if !delta.is_empty() {
                                            first_output_ms.get_or_insert_with(|| {
                                                request_started.elapsed().as_millis() as u64
                                            });
                                            if !text_started {
                                                text_started = true;
                                                emitter.text_start(content_index);
                                            }
                                            token_count += 1;
                                            full_response.push_str(delta);
                                            emitter.text_delta(content_index, delta);
                                        }

                                        if let Some(calls) = json["choices"]
                                            .get(0)
                                            .and_then(|c| c["delta"]["tool_calls"].as_array())
                                        {
                                            if !calls.is_empty() {
                                                first_output_ms.get_or_insert_with(|| {
                                                    request_started.elapsed().as_millis() as u64
                                                });
                                            }
                                            for tc in calls {
                                                let idx = tc
                                                    .get("index")
                                                    .and_then(|v| v.as_u64())
                                                    .unwrap_or(0)
                                                    as usize;
                                                let id = tc
                                                    .get("id")
                                                    .and_then(|v| v.as_str())
                                                    .map(String::from);
                                                let name = tc
                                                    .get("function")
                                                    .and_then(|f| f.get("name"))
                                                    .and_then(|v| v.as_str())
                                                    .map(String::from);
                                                let args_delta = tc
                                                    .get("function")
                                                    .and_then(|f| f.get("arguments"))
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("");

                                                // 该 index 是否已有条目（可能是缺 id 时创建的占位键）
                                                let existing_key_for_idx = tool_call_indexes
                                                    .iter()
                                                    .find(|(_, &v)| v == idx)
                                                    .map(|(k, _)| k.clone());

                                                // 真实 id 到达：把该 index 的占位键升级为真实 id，
                                                // 避免同一工具调用生成两条记录
                                                if let (Some(new_id), Some(prev_key)) =
                                                    (id.as_ref(), existing_key_for_idx.as_ref())
                                                {
                                                    if new_id != prev_key {
                                                        if let Some(t) =
                                                            tool_calls.remove(prev_key)
                                                        {
                                                            tool_call_indexes.remove(prev_key);
                                                            tool_call_indexes
                                                                .insert(new_id.clone(), idx);
                                                            tool_calls.insert(
                                                                new_id.clone(),
                                                                ToolCall {
                                                                    id: new_id.clone(),
                                                                    ..t
                                                                },
                                                            );
                                                        }
                                                    }
                                                }

                                                // 工具条目键：优先用流式 id；缺 id 时用 index 生成
                                                // 稳定键，保证后续只带 index 的 arguments 增量不会丢
                                                let key = id
                                                    .clone()
                                                    .or(existing_key_for_idx)
                                                    .unwrap_or_else(|| format!("call_{}", idx));

                                                if !tool_calls.contains_key(&key) {
                                                    let effective_id =
                                                        id.clone().unwrap_or_else(|| key.clone());
                                                    tool_calls.insert(
                                                        key.clone(),
                                                        ToolCall {
                                                            id: effective_id,
                                                            name: name
                                                                .clone()
                                                                .unwrap_or_default(),
                                                            arguments: String::new(),
                                                        },
                                                    );
                                                    tool_call_indexes.insert(key.clone(), idx);
                                                    emitter.tool_call_start(
                                                        &key,
                                                        name.as_deref().unwrap_or(""),
                                                        idx,
                                                    );
                                                }

                                                // Name update: 更新已有条目的 name
                                                if let Some(ref new_name) = name {
                                                    if let Some(e) = tool_calls.get_mut(&key) {
                                                        e.name = new_name.clone();
                                                    }
                                                }

                                                // Arguments delta: 追加到对应条目
                                                if !args_delta.is_empty() {
                                                    if let Some(e) = tool_calls.get_mut(&key) {
                                                        e.arguments.push_str(args_delta);
                                                    }
                                                    emitter.tool_call_delta(&key, args_delta);
                                                }
                                            }
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
                        // 保留已累积的完整工具调用，避免网络错误时整轮工具结果丢失
                        let calls = flush_tool_calls(&mut tool_calls, &tool_call_indexes, emitter);
                        return Ok(StreamOutcome {
                            full_text: full_response,
                            thinking_text: thinking_response,
                            tool_calls: calls,
                            had_stream_error: true,
                        });
                    }
                    None => {
                        // 走到这里说明从未收到 [DONE] 终止标记（[DONE] 分支会直接 return）。
                        // 部分 OpenAI 兼容 provider 确实不发送该标记，因此不改变成功语义
                        // （避免回归），但记录告警便于排查截断。
                        warn!(
                            "[openai::stream_chat] 流结束但未收到 [DONE] 标记，已收到 {} chunks / {} tokens",
                            chunk_count, token_count
                        );
                        if text_started {
                            emitter.text_end(content_index, &full_response);
                        }
                        let calls = flush_tool_calls(&mut tool_calls, &tool_call_indexes, emitter);
                        info!(
                            "[llm][{}] 响应摘要: status=eof_without_done, total_ms={}, headers_ms={}, first_output_ms={}, sse_bytes={}, chunks={}, text_delta_events={}, answer_bytes={}, answer_chars={}, thinking_bytes={}, thinking_chars={}, tool_calls={}, {}",
                            request_id,
                            request_started.elapsed().as_millis(),
                            headers_ms,
                            first_output_ms
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "无输出".to_string()),
                            response_bytes,
                            chunk_count,
                            token_count,
                            full_response.len(),
                            full_response.chars().count(),
                            thinking_response.len(),
                            thinking_response.chars().count(),
                            calls.len(),
                            usage_log_summary(final_usage.as_ref()),
                        );
                        // done 事件由 commands.rs 在整轮 tool 循环结束时统一发射
                        return Ok(StreamOutcome {
                            full_text: full_response,
                            thinking_text: thinking_response,
                            tool_calls: calls,
                            had_stream_error: false,
                        });
                    }
                }
            }
        })
    }

    fn fetch_models<'a>(
        &'a self,
        base_url: &'a str,
        api_key: &'a str,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<ModelInfo>, String>> + Send + 'a>>
    {
        Box::pin(async move {
            // 生成多个候选 URL
            let candidates = Self::build_model_url_candidates(base_url);
            if candidates.is_empty() {
                return Err("Base URL 为空".to_string());
            }

            let client = Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(FETCH_MODELS_TIMEOUT)
                .tcp_keepalive(Duration::from_secs(60)) // 启用 TCP keepalive
                .build()
                .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

            // Option<String>：累积最后一次失败原因
            //   None 表示还没有失败过
            //   Some(msg) 表示记录最后一次错误
            let mut last_err: Option<String> = None;

            // 遍历候选 URL，每个都试一次
            for url in &candidates {
                // match 一个 Result，把 Ok(r) 绑定给 r，Err(e) 跳过本次循环
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
                        continue; // continue 跳过本轮循环剩余部分，进入下一个候选
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

                // 404 或 405（method not allowed）说明该端点不存在 —— 继续试下一个候选
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

                // 其他非 2xx：直接返回错误（不是端点缺失，而是真的出错）
                if !status.is_success() {
                    return Err(format!("HTTP {}: {:.500}", status.as_u16(), body_text));
                    // ↑ {:.500} 是 format 语法糖：最多取 500 字符（避免错误消息过长）
                }

                // 解析 JSON
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
                                supports_vision: false,
                                supports_image_generation: false,
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

                // { "models": [...] } 格式（部分厂商用这个）
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
                                supports_vision: false,
                                supports_image_generation: false,
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

            // 所有候选都失败了：返回最后一次错误
            // unwrap_or_else(|| ...) ：若 last_err 是 None（不可能走到这里），用闭包生成默认错误
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

            let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

            // 测速请求：max_tokens=1, 非流式, 一条 "hi"
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

// ============================================================================
// build_openai_request_body —— 根据 compat 配置构建 OpenAI 兼容 API 请求体
// ============================================================================
//
// 参考 pi-agent buildParams() 中的 thinkingFormat 分支处理。
// 不同厂商即使使用 OpenAI 兼容格式，thinking/reasoning 参数的发送方式也不同。
//
// 函数签名：
//   model: &str                              模型 ID
//   messages: &[serde_json::Value]           已经转换好的 OpenAI 格式消息数组
//   compat: Option<&CompatConfig>            可选兼容性配置
//   -> serde_json::Value                     返回构造好的请求体（JSON）
fn build_openai_request_body(
    model: &str,
    messages: &[serde_json::Value],
    compat: Option<&CompatConfig>,
    tools: Option<&[ToolDefinition]>,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });

    let send_tools = compat.map(|c| c.supports_tools()).unwrap_or(true);
    if send_tools {
        if let Some(tools) = tools {
            if !tools.is_empty() {
                let openai_tools: Vec<Value> = tools
                    .iter()
                    .map(|t| {
                        json!({
                            "type": "function",
                            "function": {
                                "name": t.name,
                                "description": t.description,
                                "parameters": ensure_object_schema(&t.parameters),
                            }
                        })
                    })
                    .collect();
                body["tools"] = json!(openai_tools);
                body["tool_choice"] = json!("auto");
            }
        }
    }
    let thinking_format = compat.map(|c| c.thinking_format()).unwrap_or("openai");
    let max_tokens_field = compat.map(|c| c.max_tokens_field()).unwrap_or("max_tokens");
    let supports_stream_usage = compat
        .map(|c| c.supports_stream_options_usage())
        .unwrap_or(true);
    // ── 根据字段名配置决定用哪个 key ──
    // OpenAI 新模型用 max_completion_tokens，老模型用 max_tokens
    match max_tokens_field {
        "max_completion_tokens" => {
            body["max_completion_tokens"] = serde_json::json!(4096);
            // ↑ `body["key"] = value` 语法是 serde_json::Value 的运算符重载（IndexMut）
            //   类似 Java 的 body.put("max_completion_tokens", 4096)
        }
        _ => {
            body["max_tokens"] = serde_json::json!(4096);
        }
    }

    // ── stream_options: include_usage（仅在支持时发送） ──
    // 让 OpenAI 在流末尾额外发一个 usage chunk，告诉前端 token 数
    if supports_stream_usage {
        body["stream_options"] = serde_json::json!({
            "include_usage": true,
        });
    }

    // ── 根据 thinking_format 发送不同格式的 reasoning/thinking 参数 ──
    // 这是一个"声明式路由表"：不同的厂商用不同的 JSON 字段名表达"我要思考"
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

    body // 函数末尾的表达式作为返回值（无分号）
}

pub(crate) fn ensure_object_schema(schema: &Value) -> Value {
    if schema.is_object() {
        let mut s = schema.clone();
        if let Some(obj) = s.as_object_mut() {
            if !obj.contains_key("type") {
                obj.insert("type".to_string(), json!("object"));
            }
            if !obj.contains_key("properties") {
                obj.insert("properties".to_string(), json!({}));
            }
        }
        s
    } else {
        json!({ "type": "object", "properties": {} })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ImageAttachment, MessageRole};
    use crate::tools::ToolSafety;

    fn make_user_message(id: &str, content: &str, created_at: u64) -> Message {
        Message {
            id: id.to_string(),
            role: MessageRole::User,
            content: content.to_string(),
            images: Vec::new(),
            blocks: None,
            model_id: None,
            created_at,
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
            is_error: None,
            parent_message_id: None,
        }
    }

    fn dummy_tool(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: format!("test {name}"),
            parameters: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            safety: ToolSafety::Write,
        }
    }

    #[test]
    fn test_parses_deepseek_prompt_cache_usage() {
        let usage = parse_prompt_cache_usage(&json!({
            "usage": {
                "prompt_tokens": 10_000,
                "completion_tokens": 800,
                "total_tokens": 10_800,
                "prompt_cache_hit_tokens": 7_500,
                "prompt_cache_miss_tokens": 2_500
            }
        }))
        .unwrap();

        assert_eq!(usage.cache_hit_tokens, Some(7_500));
        assert_eq!(usage.cache_miss_tokens, Some(2_500));
        assert_eq!(usage.completion_tokens, 800);
        assert_eq!(usage.total_tokens, 10_800);
        assert_eq!(usage.hit_rate_percent(), Some(75.0));
    }

    #[test]
    fn test_parses_openai_prompt_cache_usage() {
        let usage = parse_prompt_cache_usage(&json!({
            "usage": {
                "prompt_tokens": 2_000,
                "completion_tokens": 300,
                "total_tokens": 2_300,
                "prompt_tokens_details": {
                    "cached_tokens": 1_500,
                    "cache_write_tokens": 256
                }
            }
        }))
        .unwrap();

        assert_eq!(usage.cache_hit_tokens, Some(1_500));
        assert_eq!(usage.cache_miss_tokens, Some(500));
        assert_eq!(usage.cache_write_tokens, Some(256));
        assert_eq!(usage.completion_tokens, 300);
        assert_eq!(usage.total_tokens, 2_300);
        assert_eq!(usage.hit_rate_percent(), Some(75.0));
    }

    #[test]
    fn test_build_request_no_tools_omits_field() {
        let body = build_openai_request_body("gpt-4", &[], None, None);
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn test_build_request_with_tools() {
        let tools = vec![dummy_tool("read_file"), dummy_tool("create_file")];
        let body = build_openai_request_body("gpt-4", &[], None, Some(&tools));
        assert_eq!(body["tools"].as_array().unwrap().len(), 2);
        assert_eq!(body["tool_choice"], "auto");
    }

    fn make_tool_msg(id: &str, tc_id: &str, content: &str) -> Message {
        Message {
            id: id.to_string(),
            role: MessageRole::Tool,
            content: content.to_string(),
            images: Vec::new(),
            blocks: None,
            model_id: None,
            created_at: 0,
            tool_calls: None,
            tool_call_id: Some(tc_id.to_string()),
            tool_name: Some("read_file".to_string()),
            is_error: Some(false),
            parent_message_id: None,
        }
    }

    fn make_assistant_with_tool_call(id: &str, tc_id: &str, tc_name: &str) -> Message {
        make_assistant_with_tool_arguments(id, tc_id, tc_name, "{}")
    }

    #[test]
    fn test_convert_user_images_to_openai_content_parts() {
        let message = Message {
            id: "u1".to_string(),
            role: MessageRole::User,
            content: "描述图片".to_string(),
            images: vec![ImageAttachment {
                id: "img1".to_string(),
                name: "sample.png".to_string(),
                media_type: "image/png".to_string(),
                path: String::new(),
                data_url: "data:image/png;base64,aGVsbG8=".to_string(),
            }],
            blocks: None,
            model_id: None,
            created_at: 0,
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
            is_error: None,
            parent_message_id: None,
        };

        let output = OpenAICompatibleProvider::convert_messages(&[message]);
        let content = output[1]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[1]["type"], "image_url");
        assert_eq!(
            content[1]["image_url"]["url"],
            "data:image/png;base64,aGVsbG8="
        );
        assert_eq!(content[2]["type"], "text");
        assert!(content[2]["text"]
            .as_str()
            .unwrap()
            .contains("<buddy_runtime_context>"));
    }

    #[test]
    fn test_each_user_message_keeps_its_stable_runtime_time() {
        let output = OpenAICompatibleProvider::convert_messages(&[
            make_user_message("u1", "旧问题", 1_700_000_000),
            make_user_message("u2", "新问题", 1_800_000_000),
        ]);

        assert_eq!(output[0]["content"], super::super::BUDDY_SYSTEM_PROMPT);
        let previous = output[1]["content"].as_str().unwrap();
        assert!(previous.starts_with("旧问题\n\n<buddy_runtime_context>"));
        let latest = output[2]["content"].as_str().unwrap();
        assert!(latest.starts_with("新问题\n\n<buddy_runtime_context>"));
        assert_ne!(previous, latest);
        assert_eq!(
            output,
            OpenAICompatibleProvider::convert_messages(&[
                make_user_message("u1", "旧问题", 1_700_000_000),
                make_user_message("u2", "新问题", 1_800_000_000),
            ])
        );
    }

    fn make_assistant_with_tool_arguments(
        id: &str,
        tc_id: &str,
        tc_name: &str,
        arguments: &str,
    ) -> Message {
        Message {
            id: id.to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            images: Vec::new(),
            blocks: None,
            model_id: None,
            created_at: 0,
            tool_calls: Some(vec![ToolCall {
                id: tc_id.to_string(),
                name: tc_name.to_string(),
                arguments: arguments.to_string(),
            }]),
            tool_call_id: None,
            tool_name: None,
            is_error: None,
            parent_message_id: None,
        }
    }

    #[test]
    fn test_convert_tool_msg_has_tool_call_id() {
        // 需要 assistant 消息中有匹配的 tool_call，否则会被校验过滤
        // 注意: convert_messages 会在最前面注入 BUDDY_SYSTEM_PROMPT,索引 0 是 system
        let msgs = [
            make_assistant_with_tool_call("a1", "call_x", "read_file"),
            make_tool_msg("t1", "call_x", "hi"),
        ];
        let out = OpenAICompatibleProvider::convert_messages(&msgs);
        assert_eq!(out[0]["role"], "system");
        assert_eq!(out[1]["role"], "assistant");
        assert_eq!(out[2]["role"], "tool");
        assert_eq!(out[2]["tool_call_id"], "call_x");
    }

    #[test]
    fn test_convert_tool_msg_orphan_id_stripped() {
        // 孤儿的 tool 消息（没有匹配的 assistant tool_call）应被整条过滤掉
        // 转换后只剩 system prompt 注入(无其他对话消息)
        let out =
            OpenAICompatibleProvider::convert_messages(&[make_tool_msg("t1", "orphan_id", "hi")]);
        assert_eq!(out.len(), 1, "只剩 system 注入,孤儿 tool 应被过滤");
        assert_eq!(out[0]["role"], "system");
    }

    #[test]
    fn test_convert_discards_partial_tool_arguments_after_abort() {
        let msgs = [
            make_assistant_with_tool_arguments(
                "a1",
                "call_partial",
                "ask_user",
                r#"{"question":"未完成"#,
            ),
            make_tool_msg("t1", "call_partial", "cancelled"),
        ];

        let out = OpenAICompatibleProvider::convert_messages(&msgs);

        assert_eq!(out.len(), 1, "半截参数及其 tool_result 都不应发给 Provider");
        assert_eq!(out[0]["role"], "system");
    }

    #[test]
    fn test_convert_discards_tool_call_without_result() {
        let out = OpenAICompatibleProvider::convert_messages(&[make_assistant_with_tool_call(
            "a1",
            "call_unfinished",
            "read_file",
        )]);

        assert_eq!(out.len(), 1, "未完成配对的纯工具 assistant 消息应被过滤");
        assert_eq!(out[0]["role"], "system");
    }

    #[test]
    fn test_ensure_object_schema() {
        let s = ensure_object_schema(&json!({"properties": {}}));
        assert_eq!(s["type"], "object");
    }
}
