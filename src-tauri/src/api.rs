// HTTP 客户端模块：封装所有与 OpenAI 兼容 API 的通信逻辑
//
// 职责：
// 1. SSE 流式请求 —— stream_chat() 发起流式对话，逐 token 解析 SSE 并发射事件
// 2. 获取模型列表 —— fetch_models() 从 /models 端点拉取可用模型（多候选 URL）
// 3. 测速 —— test_latency() 发送最小请求测量端点延迟

use crate::models::Message;
use futures_util::StreamExt;
use log::{error, info, warn};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tauri::Emitter;
use tokio::sync::watch;
use tokio::time::timeout;

/// API 错误类型枚举
///
/// 对常见的 HTTP 错误码和网络异常进行分类，便于前端展示对应的错误提示。
#[derive(Debug)]
pub enum ApiError {
    /// 401 未授权：API Key 无效或过期
    Unauthorized,
    /// 429 配额超限：请求频率过高或余额不足
    QuotaExceeded,
    /// 5xx 服务端错误（含具体状态码）
    ServerError(u16),
    /// 网络连接错误（DNS、超时、连接拒绝等）
    NetworkError(String),
    /// 流式数据传输错误
    StreamError(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Unauthorized => write!(f, "401"),
            ApiError::QuotaExceeded => write!(f, "429"),
            ApiError::ServerError(code) => write!(f, "server_error({})", code),
            ApiError::NetworkError(msg) => write!(f, "network: {}", msg),
            ApiError::StreamError(msg) => write!(f, "stream: {}", msg),
        }
    }
}

/// SSE 流式对话
///
/// 向 OpenAI 兼容 API 发起流式 POST 请求，逐行解析 SSE 数据，
/// 遇到 delta content → 发射 `stream-token` 事件，
/// 遇到 [DONE] → 发射 `stream-done` 事件，
/// 发生错误 → 发射 `stream-error` 事件。
///
/// 支持通过 `cancel_rx` watch channel 取消生成：检测到取消信号时发射 `stream-cancelled` 并提前返回。
///
/// 返回值：累积的完整 AI 回复文本（即使出错也返回已累积的部分，用于持久化）。
pub async fn stream_chat(
    base_url: &str,
    api_key: &str,
    model: &str,
    messages: &[Message],
    app: &tauri::AppHandle,
    mut cancel_rx: watch::Receiver<bool>,
) -> Result<String, ApiError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| ApiError::NetworkError(e.to_string()))?;

    // 将内部 Message 转换为 API 所需的 JSON 格式
    let chat_messages: Vec<Value> = messages
        .iter()
        .map(|m| {
            serde_json::json!({
                "role": match m.role {
                    crate::models::MessageRole::User => "user",
                    crate::models::MessageRole::Assistant => "assistant",
                },
                "content": m.content,
            })
        })
        .collect();

    // 构造请求体：指定模型、消息列表、开启流式
    let body = serde_json::json!({
        "model": model,
        "messages": chat_messages,
        "stream": true,
    });

    // 拼接 chat/completions 端点 URL
    let url = format!(
        "{}/chat/completions",
        base_url.trim_end_matches('/')
    );
    info!("[stream_chat] 开始请求: model={}, url={}", model, url);

    // 发送 POST 请求
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            // send() 失败只有传输层错误（DNS/connect/timeout/TLS 等），
            // HTTP 状态码错误（4xx/5xx）会以 Ok(Response) 返回，不会进这里。
            if e.is_timeout() {
                ApiError::NetworkError("timeout".into())
            } else if e.is_connect() {
                ApiError::NetworkError(e.to_string())
            } else {
                ApiError::NetworkError(e.to_string())
            }
        })?;

    // 检查 HTTP 状态码：非 2xx 一律返回错误
    let status = response.status();
    info!("[stream_chat] 收到响应: status={}", status.as_u16());
    if !status.is_success() {
        // 尝试读取错误响应体用于日志
        let body_text = response.text().await.unwrap_or_default();
        let preview: String = body_text.chars().take(500).collect();
        warn!("[stream_chat] HTTP {} 错误响应: {}", status.as_u16(), preview);
        match status.as_u16() {
            401 => return Err(ApiError::Unauthorized),
            429 => return Err(ApiError::QuotaExceeded),
            code if code >= 500 => return Err(ApiError::ServerError(code)),
            _ => return Err(ApiError::NetworkError(format!("HTTP {}", status.as_u16()))),
        }
    }

    // 获取字节流，用于逐块读取 SSE 数据
    let mut byte_stream = response.bytes_stream();
    let mut buffer = String::new(); // SSE 行缓冲区，处理不完整行
    let mut full_response = String::new(); // 累积完整 AI 回复，用于持久化

    info!("[stream_chat] 开始接收流式数据...");
    let mut chunk_count: u64 = 0;
    let mut token_count: u64 = 0;

    // 单次字节块读取超时时间：若服务端 30s 无数据，视为连接僵死
    const CHUNK_TIMEOUT: Duration = Duration::from_secs(30);

    loop {
        // tokio::select! 同时监听取消信号和新数据到达
        let chunk = tokio::select! {
            _ = cancel_rx.changed() => {
                // 检测到取消信号（首轮 changed() 会立即返回标记初始值，我们直接 continue 跳过）
                if *cancel_rx.borrow() {
                    info!(
                        "[stream_chat] 被取消: {} chunks, {} tokens",
                        chunk_count, token_count
                    );
                    let _ = app.emit("stream-cancelled", ());
                    return Ok(full_response); // 返回已累积的部分回复
                }
                // 首轮空转：初始值已被标记为"已读"，后续 changed() 只在信号变化时触发
                continue;
            }
            result = timeout(CHUNK_TIMEOUT, byte_stream.next()) => {
                match result {
                    Ok(chunk) => chunk,
                    Err(_elapsed) => {
                        // 读取超时：服务端可能已断连但未正常关闭连接
                        error!(
                            "[stream_chat] 读取超时 (30s 无数据): {} chunks, {} tokens, {} chars",
                            chunk_count, token_count, full_response.len()
                        );
                        let _ = app.emit("stream-error", "读取超时");
                        return Ok(full_response);
                    }
                }
            }
        };

        match chunk {
            Some(Ok(bytes)) => {
                chunk_count += 1;
                let text = String::from_utf8_lossy(&bytes);
                // 前 3 个 chunk 打印原始内容（截断到 500 字符），方便排查 SSE 格式问题
                if chunk_count <= 3 {
                    let preview: String = text.chars().take(500).collect();
                    info!(
                        "[stream_chat] chunk#{}: {} bytes, raw={}",
                        chunk_count,
                        bytes.len(),
                        preview
                    );
                }
                buffer.push_str(&text);

                // 逐行解析 SSE 数据（以 \n 为分隔符）
                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].to_string();
                    buffer = buffer[pos + 1..].to_string();

                    let line = line.trim().to_string();
                    if line.is_empty() {
                        continue; // 跳过空行
                    }

                    // 解析 "data: ..." 行（SSE 规范中冒号后的空格可选）
                    if let Some(data) = line
                        .strip_prefix("data: ")
                        .or_else(|| line.strip_prefix("data:"))
                    {
                        let data = data.trim();
                        if data == "[DONE]" {
                            // 流结束标识
                            info!(
                                "[stream_chat] 完成: {} chunks, {} tokens, {} chars",
                                chunk_count, token_count, full_response.len()
                            );
                            let _ = app.emit("stream-done", ());
                            return Ok(full_response);
                        }
                        if data.is_empty() {
                            continue;
                        }

                        // 解析 JSON，提取 delta content
                        match serde_json::from_str::<Value>(data) {
                            Ok(json) => {
                                let delta = json["choices"]
                                    .get(0) // 第一个 choice
                                    .and_then(|c| c["delta"]["content"].as_str())
                                    .unwrap_or("");
                                if !delta.is_empty() {
                                    token_count += 1;
                                    full_response.push_str(delta);
                                    // 发射每个 token 给前端
                                    let _ = app.emit("stream-token", delta.to_string());
                                }
                            }
                            Err(_) => continue, // JSON 解析失败则跳过该行
                        }
                    }
                }
            }
            Some(Err(e)) => {
                // 流读取错误：已通过事件通知前端，返回已累积的部分回复
                let _ = app.emit("stream-error", format!("流读取错误: {}", e));
                return Ok(full_response);
            }
            None => {
                // 流正常结束（无 [DONE] 标识的情况）
                let _ = app.emit("stream-done", ());
                return Ok(full_response);
            }
        }
    }
}

// ── 模型列表获取 ──────────────────────────────────────────────

/// 模型列表获取超时
const FETCH_MODELS_TIMEOUT: Duration = Duration::from_secs(15);

/// 404/405 响应体最大字符数
const ERROR_BODY_MAX_CHARS: usize = 512;

/// 根据 base_url 构造模型列表候选 URL（多候选举，按优先级排列）
///
/// 处理多种 base_url 模式：
/// 1. 普通地址（如 `https://api.deepseek.com`）→ `/v1/models`
/// 2. 已含版本段（如 `https://open.bigmodel.cn/api/paas/v4`）→ `{base}/models`
/// 3. 已含 /v1（如 `https://api.moonshot.cn/v1`）→ `{base}/models`
fn build_model_url_candidates(base_url: &str) -> Vec<String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return vec![];
    }

    let mut candidates: Vec<String> = Vec::new();

    // 判断 base_url 是否以版本段 /v{N} 结尾（如 /v1, /v4）
    let ends_with_version = {
        let last = trimmed.rsplit('/').next().unwrap_or("");
        last.strip_prefix('v')
            .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
    };

    if ends_with_version {
        // 已含版本段：{base}/models 是首要候选
        candidates.push(format!("{trimmed}/models"));
        // 如果版本段不是 /v1，补一个 /v1/models 作为兜底
        if !trimmed.ends_with("/v1") {
            candidates.push(format!("{trimmed}/v1/models"));
        }
    } else {
        // 不带版本段：标准 OpenAI 路径
        candidates.push(format!("{trimmed}/v1/models"));
    }

    candidates
}

/// 从 OpenAI 兼容 API 获取可用模型列表
///
/// 尝试多个候选 URL（由 build_model_url_candidates 生成），
/// 404/405 时顺延到下一个候选，其他错误直接返回。
/// 日志打印完整请求链路便于排错。
pub async fn fetch_models(base_url: &str, api_key: &str) -> Result<Vec<Value>, String> {
    let candidates = build_model_url_candidates(base_url);
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

    info!(
        "[fetch_models] base_url={}, candidates={:?}",
        base_url, candidates
    );

    for url in &candidates {
        let masked_key = if api_key.len() > 10 {
            format!("{}...{}", &api_key[..6], &api_key[api_key.len()-4..])
        } else {
            "***".to_string()
        };
        let request_json = serde_json::json!({
            "url": url,
            "method": "GET",
            "headers": {
                "Authorization": format!("Bearer {}", masked_key),
            }
        });
        info!("[fetch_models] 请求: {}", serde_json::to_string_pretty(&request_json).unwrap_or_default());

        let response = match client
            .get(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!(
                    "[fetch_models] ❌ 网络错误: url={}, err={:?}",
                    url, e
                );
                if e.is_timeout() {
                    error!("[fetch_models]   原因: 请求超时");
                } else if e.is_connect() {
                    error!("[fetch_models]   原因: 连接失败 (DNS/TLS/网络不通)");
                } else if e.is_request() {
                    error!("[fetch_models]   原因: 请求发送失败");
                }
                last_err = Some(format!("请求失败: {}", e));
                continue;
            }
        };

        let status = response.status();

        let body_text = match response.text().await {
            Ok(t) => t,
            Err(e) => {
                error!("[fetch_models] ❌ 读取响应体失败: url={}, err={}", url, e);
                last_err = Some(format!("读取失败: {}", e));
                continue;
            }
        };

        // 用 JSON 格式打印响应摘要
        let resp_summary: Value = serde_json::from_str(&body_text).unwrap_or(Value::String(
            body_text.chars().take(ERROR_BODY_MAX_CHARS).collect(),
        ));
        let response_json = serde_json::json!({
            "status": status.as_u16(),
            "body": resp_summary,
        });
        info!("[fetch_models] 响应: {}", serde_json::to_string_pretty(&response_json).unwrap_or_default());

        // 404/405 → 尝试下一个候选
        if status == reqwest::StatusCode::NOT_FOUND
            || status == reqwest::StatusCode::METHOD_NOT_ALLOWED
        {
            warn!("[fetch_models] ⚠ 端点不存在 ({}), 尝试下一个候选", status.as_u16());
            last_err = Some(format!("HTTP {}: endpoint 不存在", status.as_u16()));
            continue;
        }

        // 其他错误状态码
        if !status.is_success() {
            error!(
                "[fetch_models] ❌ HTTP {} for url={}",
                status.as_u16(),
                url
            );
            return Err(format!("HTTP {}: {:.500}", status.as_u16(), body_text));
        }

        // 解析 JSON
        let json: Value = match serde_json::from_str(&body_text) {
            Ok(j) => j,
            Err(e) => {
                error!(
                    "[fetch_models] ❌ JSON 解析失败: url={}, err={}",
                    url, e
                );
                return Err(format!("解析失败: {}", e));
            }
        };

        // 尝试多种响应格式
        // 1. OpenAI 格式：{ "data": [...] }
        if let Some(arr) = json["data"].as_array() {
            let ids: Vec<&str> = arr.iter().filter_map(|m| m["id"].as_str()).collect();
            info!(
                "[fetch_models] ✅ 成功: {} 个模型, ids={:?}, url={}",
                arr.len(),
                ids,
                url
            );
            return Ok(arr.clone());
        }

        // 2. { "models": [...] } 格式
        if let Some(arr) = json["models"].as_array() {
            let ids: Vec<&str> = arr.iter().filter_map(|m| m["id"].as_str()).collect();
            info!(
                "[fetch_models] ✅ 成功: {} 个模型 (models 格式), ids={:?}, url={}",
                arr.len(),
                ids,
                url
            );
            return Ok(arr.clone());
        }

        // 返回了 JSON 但格式不匹配，继续尝试下一个候选
        warn!(
            "[fetch_models] ⚠ 无法识别的响应格式: url={}, 继续下一个候选",
            url
        );
        last_err = Some("响应格式无法识别（非 data 也非 models 数组）".to_string());
    }

    Err(last_err.unwrap_or_else(|| "所有候选 URL 均失败".to_string()))
}

/// 测试模型端点的响应延迟
///
/// 发送一个最小请求（单 token 回复），测量从请求发送到收到完整响应的时间。
/// 返回延迟毫秒数（u32）。
pub async fn test_latency(
    base_url: &str,
    api_key: &str,
    model_id: &str,
) -> Result<u32, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;
    let url = format!(
        "{}/chat/completions",
        base_url.trim_end_matches('/')
    );

    // 最小请求体：单条 "hi" 消息，max_tokens=1，非流式
    let body = serde_json::json!({
        "model": model_id,
        "messages": [{ "role": "user", "content": "hi" }],
        "max_tokens": 1,
        "stream": false,
    });

    let start = std::time::Instant::now();

    // 发送请求（不关心响应内容，只关心耗时）
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
}
