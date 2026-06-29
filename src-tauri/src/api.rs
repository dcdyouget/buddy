// HTTP 客户端模块：封装所有与 OpenAI 兼容 API 的通信逻辑
//
// 职责：
// 1. SSE 流式请求 —— stream_chat() 发起流式对话，逐 token 解析 SSE 并发射事件
// 2. 获取模型列表 —— fetch_models() 从 /models 端点拉取可用模型
// 3. 测速 —— test_latency() 发送最小请求测量端点延迟

use crate::models::Message;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tauri::Emitter;
use tokio::sync::watch;

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
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    // 发送 POST 请求，区分各种错误类型
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
            } else if let Some(status) = e.status() {
                match status.as_u16() {
                    401 => ApiError::Unauthorized,
                    429 => ApiError::QuotaExceeded,
                    code if code >= 500 => ApiError::ServerError(code),
                    _ => ApiError::NetworkError(e.to_string()),
                }
            } else {
                ApiError::NetworkError(e.to_string())
            }
        })?;

    // 二次检查 HTTP 状态码（部分代理可能返回 200 包装的错误）
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(ApiError::Unauthorized);
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(ApiError::QuotaExceeded);
    }
    if status.is_server_error() {
        return Err(ApiError::ServerError(status.as_u16()));
    }

    // 获取字节流，用于逐块读取 SSE 数据
    let mut byte_stream = response.bytes_stream();
    let mut buffer = String::new(); // SSE 行缓冲区，处理不完整行
    let mut full_response = String::new(); // 累积完整 AI 回复，用于持久化

    loop {
        // tokio::select! 同时监听取消信号和新数据到达
        let chunk = tokio::select! {
            _ = cancel_rx.changed() => {
                // 检测到取消信号
                if *cancel_rx.borrow() {
                    let _ = app.emit("stream-cancelled", ());
                    return Ok(full_response); // 返回已累积的部分回复
                }
                continue;
            }
            chunk = byte_stream.next() => chunk,
        };

        match chunk {
            Some(Ok(bytes)) => {
                let text = String::from_utf8_lossy(&bytes);
                buffer.push_str(&text);

                // 逐行解析 SSE 数据（以 \n 为分隔符）
                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].to_string();
                    buffer = buffer[pos + 1..].to_string();

                    let line = line.trim().to_string();
                    if line.is_empty() {
                        continue; // 跳过空行
                    }

                    // 解析 "data: ..." 行
                    if let Some(data) = line.strip_prefix("data: ") {
                        let data = data.trim();
                        if data == "[DONE]" {
                            // 流结束标识
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

/// 从 OpenAI 兼容 API 获取可用模型列表
///
/// 调用 GET /models 端点，返回原始 JSON 数组（data 字段），
/// 由上层 commands 模块转换为内部 ModelInfo 结构。
pub async fn fetch_models(base_url: &str, api_key: &str) -> Result<Vec<Value>, String> {
    let client = Client::new();
    let url = format!("{}/models", base_url.trim_end_matches('/'));

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("请求模型列表失败: {}", e))?;

    let json: Value = response
        .json()
        .await
        .map_err(|e| format!("解析模型列表失败: {}", e))?;

    // 提取 data 数组，不存在则返回空数组
    let models = json["data"].as_array().cloned().unwrap_or_default();

    Ok(models)
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
    let client = Client::new();
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

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
