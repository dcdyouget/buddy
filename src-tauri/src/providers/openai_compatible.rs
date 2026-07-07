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
use crate::models::{get_context_window, CompatConfig, Message, MessageRole, ModelInfo};
use crate::streaming::{StopReason, StreamEventEmitter};
use futures_util::StreamExt;                    // Stream trait 扩展（提供 .next() 等）
use log::{error, info, warn};                   // 日志门面
use reqwest::Client;                           // 异步 HTTP 客户端
use serde_json::Value;                         // 通用 JSON 值
use std::pin::Pin;                             // 自引用指针固定
use std::time::Duration;                       // 时间段
use tokio::sync::watch;                        // watch channel（取消信号）
use tokio::time::timeout;                       // 异步超时


/// 单次字节块读取超时
const CHUNK_TIMEOUT: Duration = Duration::from_secs(30);

/// 模型列表获取超时
const FETCH_MODELS_TIMEOUT: Duration = Duration::from_secs(15);


// ============================================================================
// OpenAICompatibleProvider —— OpenAI 协议族适配器
// ============================================================================
pub struct OpenAICompatibleProvider;

impl OpenAICompatibleProvider {
    /// 将内部消息格式转换为 OpenAI API 格式
    ///
    /// 与 Anthropic 转换器几乎一致（都是 role + content 数组）
    fn convert_messages(messages: &[Message]) -> Vec<Value> {
        messages
            .iter()
            .map(|m| {
                // 注意：这里 match 直接写进 json!({...}) 里，作为表达式的值
                // Rust 的 match 是表达式（每个分支返回同一类型的值）
                // ≈ Java 14+ 的 switch expression
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
            return vec![];     // 提前返回空 Vec
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
        messages: &'a [Message],
        emitter: &'a StreamEventEmitter,
        mut cancel_rx: watch::Receiver<bool>,
        compat: Option<&'a CompatConfig>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String, ApiError>> + Send + 'a>> {
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
            let body = build_openai_request_body(model, &chat_messages, compat);

            // ── 4. 拼接 URL（OpenAI 兼容都是 /chat/completions） ──
            let url = format!(
                "{}/chat/completions",
                base_url.trim_end_matches('/')
            );
            info!(
                "[openai::stream_chat] 开始请求: model={}, url={}",
                model, url
            );

            // ── 5. 发送请求 ──
            // 与 Anthropic 不同：OpenAI 用 Authorization: Bearer <key>
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
            let mut buffer = String::new();
            let mut full_response = String::new();

            info!("[openai::stream_chat] 开始接收流式数据...");
            let mut chunk_count: u64 = 0;
            let mut token_count: u64 = 0;

            // OpenAI 兼容格式只有一个文本块（content_index = 0）
            // 与 Anthropic 的多 content_block 不同
            let content_index: usize = 0;
            let mut text_started = false;

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
                            return Ok(full_response);
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

                        // ── 逐行解析（OpenAI 协议比 Anthropic 简单） ──
                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].to_string();
                            buffer = buffer[pos + 1..].to_string();

                            let line = line.trim().to_string();
                            if line.is_empty() {
                                continue;   // 跳过空行
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

                                // 解析 JSON
                                match serde_json::from_str::<Value>(data) {
                                    Ok(json) => {
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
                                            // reasoning_content 作为思考块发射
                                            emitter.thinking_delta(content_index, reasoning);
                                            continue;   // 跳过本行的 content 处理
                                        }

                                        // 正常的文本增量
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
                                    Err(_) => continue,   // 解析失败：跳过
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
                        // 流正常结束
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
            // 生成多个候选 URL
            let candidates = Self::build_model_url_candidates(base_url);
            if candidates.is_empty() {
                return Err("Base URL 为空".to_string());
            }

            let client = Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(FETCH_MODELS_TIMEOUT)
                .tcp_keepalive(Duration::from_secs(60))   // 启用 TCP keepalive
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
                        continue;   // continue 跳过本轮循环剩余部分，进入下一个候选
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

            let url = format!(
                "{}/chat/completions",
                base_url.trim_end_matches('/')
            );

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
) -> serde_json::Value {
    // 用 json!({...}) 创建初始 body
    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });

    // 解析 compat 配置（使用默认值回退）
    // 同样的 Option::map(...).unwrap_or(default) 套路
    let thinking_format = compat.map(|c| c.thinking_format()).unwrap_or("openai");
    let max_tokens_field = compat.map(|c| c.max_tokens_field()).unwrap_or("max_tokens");
    let supports_stream_usage = compat.map(|c| c.supports_stream_options_usage()).unwrap_or(true);
    let _supports_reasoning_effort = compat.map(|c| c.supports_reasoning_effort()).unwrap_or(true);

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

    body   // 函数末尾的表达式作为返回值（无分号）
}