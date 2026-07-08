// ============================================================================
// Anthropic Messages API Provider
// ============================================================================
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
// ============================================================================

// ─── use 语句（= Java 的 import） ───
// super::* 导入父模块（providers::mod）的所有 pub 项（ApiError, LlmProvider, extract_error_message）
use super::{ApiError, LlmProvider};
use crate::models::{get_context_window, CompatConfig, Message, MessageRole, ModelInfo, ToolCall};
use crate::streaming::{StopReason, StreamEventEmitter, StreamOutcome};
use crate::tools::ToolDefinition;
use futures_util::StreamExt;                          // 为 Stream trait 提供 .next() 等适配器
use log::{error, info, warn};                         // 日志门面（log crate）
use reqwest::Client;                                 // HTTP 客户端
use serde_json::{json, Value};                               // 通用 JSON 值类型
use std::pin::Pin;                                    // 用于 Pin<Box<...>>
use std::time::Duration;                             // 时间段
use tokio::sync::watch;                              // watch channel（取消信号）
use tokio::time::timeout;                             // 给异步操作加超时


/// Anthropic API 版本（固定）
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// 单次字节块读取超时
const CHUNK_TIMEOUT: Duration = Duration::from_secs(30);


// ============================================================================
// AnthropicProvider —— Anthropic 适配器
// ============================================================================
//
// Rust `struct` 没有构造函数（除了字段初始化语法），所有"初始化逻辑"
// 都通过关联函数（associated function，类似 Java 的 static method）实现，
// 或者直接在 impl 块里挂方法。
// ============================================================================
pub struct AnthropicProvider;

impl AnthropicProvider {
    /// 将内部消息格式转换为 Anthropic API 格式
    ///
    /// 入参 `messages: &[Message]` 是 `&[Message]`：不可变借用 Message 切片
    /// ≈ Java 的 `List<Message>` 不可变视图
    fn convert_messages(messages: &[Message]) -> Vec<Value> {
        // 收集所有 assistant 消息中有效的 tool_use id
        let valid_tool_use_ids: std::collections::HashSet<&str> = messages
            .iter()
            .filter(|m| m.role == MessageRole::Assistant)
            .filter_map(|m| m.tool_calls.as_ref())
            .flat_map(|calls| calls.iter().map(|tc| tc.id.as_str()))
            .collect();

        messages
            .iter()
            .filter_map(|m| {
                // 孤儿 tool 消息：有 tool_use_id 但找不到对应的 assistant tool_use
                // 整条过滤掉，不发给 API
                if m.role == MessageRole::Tool {
                    let id = m.tool_call_id.as_deref().unwrap_or("");
                    if !id.is_empty() && !valid_tool_use_ids.contains(id) {
                        warn!(
                            "[anthropic::convert_messages] 过滤孤儿 tool 消息, tool_use_id='{}' 找不到对应的 assistant tool_use",
                            id
                        );
                        return None;
                    }
                }

                let result = match m.role {
                    MessageRole::User => {
                        json!({ "role": "user", "content": m.content })
                    }
                    MessageRole::Assistant => {
                        let mut obj = json!({ "role": "assistant" });
                        if let Some(calls) = &m.tool_calls {
                            let mut content: Vec<Value> = if m.content.is_empty() {
                                vec![]
                            } else {
                                vec![json!({"type": "text", "text": m.content})]
                            };
                            for tc in calls {
                                let args: Value = serde_json::from_str(&tc.arguments).unwrap_or(Value::Null);
                                content.push(json!({
                                    "type": "tool_use",
                                    "id": tc.id,
                                    "name": tc.name,
                                    "input": args,
                                }));
                            }
                            obj["content"] = json!(content);
                        } else {
                            obj["content"] = json!(m.content);
                        }
                        obj
                    }
                    MessageRole::Tool => {
                        let id = m.tool_call_id.as_deref().unwrap_or("");
                        json!({
                            "role": "user",
                            "content": [{
                                "type": "tool_result",
                                "tool_use_id": id,
                                "content": m.content,
                                "is_error": m.is_error.unwrap_or(false),
                            }]
                        })
                    }
                };
                Some(result)
            })
            .collect()
    }

    /// 解析 Anthropic SSE 事件行
    ///
    /// 返回 (event_type, data) 或 None（非完整事件行）。
    ///
    /// 关键 Rust 语法点：
    /// - `Option<T>` = "可能没有值" 的容器；Some(x) / None
    /// - 函数返回 `Option<(String, String)>`：可能没有值，或返回二元组
    fn parse_sse_line(line: &str) -> Option<(String, String)> {
        let line = line.trim();        // 去掉首尾空白
        if line.is_empty() {
            return None;
        }
        // 注释行（以 : 开头）—— SSE 协议规定的心跳
        if line.starts_with(':') {
            return None;
        }

        // 解析 "field: value" 格式
        // `if let Some(...) = ... { ... }` 是模式匹配，绑定内部值给代码块
        if let Some(colon_pos) = line.find(':') {
            // 字符串切片 line[..colon_pos]：从开头切到冒号位置
            // 注意：字符串切片按字节边界；安全的前提是冒号是 ASCII
            let field = line[..colon_pos].trim();         // &str
            let value = line[colon_pos + 1..].trim();     // &str
            // Some((s1.to_string(), s2.to_string()))：把 &str 转为堆分配的 String
            Some((field.to_string(), value.to_string()))
        } else {
            None
        }
    }
}


// ============================================================================
// impl LlmProvider for AnthropicProvider
// ============================================================================
//
// 这是 trait 实现块（类似 Java 的 `class AnthropicProvider implements LlmProvider`）
// 在块内必须实现 trait 中声明的所有方法。
// ============================================================================
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
        tools: &'a [ToolDefinition],
    ) -> Pin<Box<dyn std::future::Future<Output = Result<StreamOutcome, ApiError>> + Send + 'a>> {
        // Box::pin(async move { ... })：
        //   - async move {...} 创建一个 async 块（Future 实现）
        //     - `move` 关键字把块内用到的外部变量"move 进"块，使其所有权转移到 Future 内
        //     - 不加 move 的话，闭包/Future 会"借用"外部变量；但 async 块的存活期与签名 'a 一致，
        //       可能比外部变量久，所以必须 move
        //   - Box::pin 把 Future 装箱到堆并 Pin 住（自引用安全）
        //     ≈ Java 的 CompletableFuture.supplyAsync(...).get() 返回的对象
        Box::pin(async move {
            // 解析 compat 配置
            // Option<&T>::map(...)   = Option 上挂的链式方法
            //   Some(x) → Some(f(x))
            //   None    → None
            // .unwrap_or(true)        = None 时返回默认值
            let _supports_temperature = compat.map(|c| c.supports_temperature()).unwrap_or(true);
            // 注意: Anthropic API 默认不发送 temperature 字段。
            // 当 supports_temperature=false (如 Claude Opus 4.7+)，保持不发送即可。

            // ── 1. 创建 HTTP 客户端 ──
            // Client::builder()  构造器模式（Builder pattern），每一步返回 self
            // .timeout(...).build() 链式调用
            // .map_err(|e| ApiError::NetworkError(e.to_string()))?
            //   - Client::build() 返回 Result<Client, reqwest::Error>
            //   - map_err 把错误类型转成 ApiError（Java 没有此语法）
            //   - `?` 是"早返回"运算符：
            //       * 若 Result 是 Ok(v)，解包出 v
            //       * 若 Result 是 Err(e)，从当前函数返回 Err(转换后的 e)
            //     ≈ Java 的 throws 机制但写法更紧凑
            let client = Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .map_err(|e| ApiError::NetworkError(e.to_string()))?;

            // ── 2. 转换消息格式 ──
            let anthropic_messages = Self::convert_messages(messages);

            // ── 3. 构造请求体 JSON ──
            // serde_json::json!({...}) 宏：写起来像 JSON，编译后是 Value
            // 与 Python 中 f-string 不同，它是静态字面量 + 占位符的混合体
            // P5: request body with tools (Anthropic format)
            let mut body = serde_json::json!({
                "model": model,
                "max_tokens": 4096,
                "messages": anthropic_messages,
                "stream": true,
            });
            if !tools.is_empty() {
                let anthropic_tools: Vec<Value> = tools
                    .iter()
                    .map(|t| json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters,
                    }))
                    .collect();
                body["tools"] = json!(anthropic_tools);
                body["tool_choice"] = json!({"type": "auto"});
            }

            // ── 4. 拼接 URL ──
            // format!("{}/v1/messages", ...) ≈ Java 的 String.format("%s/v1/messages", baseUrl)
            // trim_end_matches('/') 移除结尾的 '/'，防止出现 "//v1"
            let url = format!(
                "{}/v1/messages",
                base_url.trim_end_matches('/')
            );
            info!("[anthropic::stream_chat] 开始请求: model={}, url={}", model, url);

            // ── DEBUG: 打印完整请求体(用于排查 tool_use_id 问题) ──
            for (i, m) in anthropic_messages.iter().enumerate() {
                let role = m["role"].as_str().unwrap_or("?");
                let content_info = if m["content"].is_array() {
                    let types: Vec<&str> = m["content"]
                        .as_array()
                        .map(|arr| arr.iter().filter_map(|c| c["type"].as_str()).collect())
                        .unwrap_or_default();
                    format!(" content_types={:?}", types)
                } else {
                    let preview: String = m["content"].as_str().unwrap_or("?").chars().take(80).collect();
                    format!(" content='{}'", preview)
                };
                info!(
                    "[anthropic::debug] msg[{}] role={}{}",
                    i, role, content_info,
                );
            }
            info!(
                "[anthropic::debug] 总计 {} 条消息, body 大小≈{} bytes",
                anthropic_messages.len(),
                body.to_string().len(),
            );

            // ── 5. 发送 POST 请求 ──
            // client.post(&url)          链式调用，返回 RequestBuilder
            // .header("x-api-key", api_key)  添加请求头
            // .header("anthropic-version", ANTHROPIC_VERSION)
            // .header("Content-Type", "application/json")
            // .json(&body)              设置 JSON body
            // .send().await              真正发起请求
            //   - send() 返回 Future<Result<Response, Error>>
            //   - .await 是 Rust 异步的关键字：
            //       挂起当前 Future，等 send 的 Future 完成
            //       完成后取回 Result
            //     ≈ Java 的 CompletableFuture.get() 或 Kotlin 的 suspend fun
            let response = client
                .post(&url)
                .header("x-api-key", api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| {
                    // `e.is_timeout()` / `e.is_connect()` 是 reqwest::Error 的判定方法
                    // 这里把所有错误都映射成 NetworkError（简化处理）
                    if e.is_timeout() {
                        ApiError::NetworkError("timeout".into())
                    } else if e.is_connect() {
                        ApiError::NetworkError(e.to_string())
                    } else {
                        ApiError::NetworkError(e.to_string())
                    }
                })?;

            // ── 6. 检查 HTTP 状态码 ──
            let status = response.status();
            info!("[anthropic::stream_chat] 收到响应: status={}", status.as_u16());
            if !status.is_success() {
                let body_text = response.text().await.unwrap_or_default();
                warn!(
                    "[anthropic::stream_chat] HTTP {} 完整错误响应: {}",
                    status.as_u16(),
                    body_text,
                );
                let api_msg = super::extract_error_message(&body_text);
                // match 表达式按 HTTP 状态码分类：
                //   401 → 未授权
                //   429 → 配额
                //   >=500 → 服务端错误
                //   其他 → 网络错误
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

            // ── 7. 开始解析 SSE 流 ──
            // response.bytes_stream() 把 HTTP 响应体转成字节流（Stream）
            //   - Stream<Item = Result<Bytes, Error>>
            //   - 类似 Java 的 InputStream，但包装成 Future/Stream API
            let mut byte_stream = response.bytes_stream();
            // buffer 累积"未完整行"，SSE 按行解析，必须自己组行
            let mut buffer = String::new();

            // 流式状态追踪变量（mut 表示可变）
            let mut full_response = String::new();       // 累积完整回复文本
            let mut thinking_response = String::new(); // 累积思考文本
            let mut current_event: Option<String> = None;// 当前事件类型
            let mut content_index: usize = 0;            // 内容块索引
            let mut _has_started = false;                // 下划线前缀：未使用变量（Rust 会警告）

            let mut chunk_count: u64 = 0;                // 接收到的字节块数
            let mut token_count: u64 = 0;                // 已发出的文本 token 数

            // P5: tool_call 流式追踪 (key=index, Anthropic 用 index 区分 block)
            use std::collections::HashMap;
            let mut tool_calls: HashMap<usize, ToolCall> = HashMap::new();
            let flush_tool_calls = |tc: &mut HashMap<usize, ToolCall>,
                                   em: &StreamEventEmitter| -> Vec<ToolCall> {
                let mut ordered: Vec<(usize, ToolCall)> = tc.drain().collect();
                ordered.sort_by_key(|(i, _)| *i);
                let pending = ordered.len();
                for (_, t) in &ordered {
                    em.tool_call_end(&t.id, &t.name, &t.arguments);
                }
                em.turn_end(pending);
                ordered.into_iter().map(|(_, t)| t).collect()
            };

            // 通知前端：流式开始
            emitter.start();
            // ── 8. 主循环：读取流并解析 ──
            // `loop { ... }` 是 Rust 的无限循环，Java 用 `while(true)` 或 `for(;;)`
            loop {
                // tokio::select! 是 tokio 提供的并发等待宏：
                //   同时等待多个异步分支，哪个先完成就跑哪个，其余取消
                // ≈ Java 的 `CompletableFuture.anyOf(...)` 或 Kotlin 的 select 表达式
                //
                // 语法：
                //   tokio::select! {
                //     分支1 => 处理1,
                //     分支2 => 处理2,
                //   }
                let chunk = tokio::select! {
                    // 取消信号通道变化（cancel_rx 是 watch::Receiver<bool>）
                    _ = cancel_rx.changed() => {
                        // changed() 返回 Future<()>，完成后调用 .borrow() 拿到当前值
                        if *cancel_rx.borrow() {     // `*` 解引用，拿到 bool
                            info!(
                                "[anthropic::stream_chat] 被取消: {} chunks, {} tokens",
                                chunk_count, token_count
                            );
                            emitter.error(
                                StopReason::Aborted,
                                "用户取消",
                                &full_response,
                            );
                            return Ok(StreamOutcome { full_text: full_response, thinking_text: thinking_response, tool_calls: vec![], had_stream_error: true });
                        }
                        continue;  // 还没取消，继续下一轮循环
                    }
                    // 读取下一个字节块，带 30s 超时
                    result = timeout(CHUNK_TIMEOUT, byte_stream.next()) => {
                        // timeout(d, future) 返回 Result<T, Elapsed>
                        // byte_stream.next() 返回 Option<Result<Bytes, Error>>
                        // 套在一起：Result<Option<Result<Bytes, Error>>, Elapsed>
                        match result {
                            Ok(chunk) => chunk,        // 超时未发生，转发 chunk
                            Err(_elapsed) => {          // 超时了（_ 开头表示未使用变量）
                                error!(
                                    "[anthropic::stream_chat] 读取超时: {} chunks, {} tokens",
                                    chunk_count, token_count
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

                // 匹配 chunk 的三种可能：
                //   Some(Ok(bytes))   正常收到一段字节
                //   Some(Err(e))      网络错误
                //   None              流正常结束
                match chunk {
                    Some(Ok(bytes)) => {
                        chunk_count += 1;
                        // from_utf8_lossy 把字节转成 &str，UTF-8 非法字符替换为 �
                        let text = String::from_utf8_lossy(&bytes);
                        // 前 3 个块做预览日志
                        if chunk_count <= 3 {
                            let preview: String = text.chars().take(500).collect();
                            info!(
                                "[anthropic::stream_chat] chunk#{}: {} bytes, raw={}",
                                chunk_count,
                                bytes.len(),
                                preview
                            );
                        }
                        // 把新字节追加到 buffer
                        buffer.push_str(&text);

                        // 逐行解析：SSE 是行分隔的协议
                        // while let Some(pos) = buffer.find('\n') { ... }
                        //   - 找换行位置
                        //   - 切出一行
                        //   - 处理完，把 buffer 切成剩余部分
                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].to_string();
                            buffer = buffer[pos + 1..].to_string();

                            // 尝试解析 SSE 字段行（"field: value"）
                            if let Some((field, value)) = Self::parse_sse_line(&line) {
                                match field.as_str() {
                                    "event" => {
                                        // 记录事件类型，下一行 data 会用到
                                        current_event = Some(value);
                                    }
                                    "data" => {
                                        // event_type 取 current_event，没设过就给空串
                                        let event_type =
                                            current_event.as_deref().unwrap_or("");
                                        // 解析 data 字段为 JSON
                                        match serde_json::from_str::<Value>(&value) {
                                            Ok(json) => {
                                                // 按事件类型分派处理
                                                match event_type {
                                                    "message_start" => {
                                                        _has_started = true;
                                                        // 提取 input_tokens
                                                        // .as_object()    Option<&Map>
                                                        // .or_else(...)   若为 None，用后备分支
                                                        //   类似 Java Optional.orElse
                                                        let usage = json["message"]["usage"].as_object()
                                                            .or_else(|| json["usage"].as_object());
                                                        if let Some(u) = usage {
                                                            // u.get("input_tokens") 拿 Option<&Value>
                                                            // .and_then(|v| v.as_u64())  Option<&Value> → Option<u64>
                                                            // .unwrap_or(0)             None 时给默认值 0
                                                            let input = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                                                            info!("[anthropic] message_start: input_tokens={}", input);
                                                        }
                                                    }
                                                    "content_block_start" => {
                                                        // 内容块开始，区分 text/thinking 块
                                                        let block = &json["content_block"];
                                                        let block_type =
                                                            block["type"].as_str().unwrap_or("");
                                                        let idx = json["index"].as_u64().unwrap_or(0) as usize;
                                                        match block_type {
                                                            "text" => {
                                                                emitter.text_start(idx);
                                                            }
                                                            "thinking" | "redacted_thinking" => {
                                                                emitter.thinking_start(idx);
                                                            }
                                                            "tool_use" => {
                                                                let block = &json["content_block"];
                                                                let id = block["id"].as_str().unwrap_or("");
                                                                let n = block["name"].as_str().unwrap_or("");
                                                                tool_calls.insert(idx, ToolCall {
                                                                    id: id.to_string(),
                                                                    name: n.to_string(),
                                                                    arguments: String::new(),
                                                                });
                                                                emitter.tool_call_start(id, n, idx);
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                    "content_block_delta" => {
                                                        // 增量内容（最常见的事件）
                                                        let delta = &json["delta"];
                                                        let delta_type =
                                                            delta["type"].as_str().unwrap_or("");
                                                        match delta_type {
                                                            "text_delta" => {
                                                                // 文本增量
                                                                let text =
                                                                    delta["text"].as_str().unwrap_or("");
                                                                if !text.is_empty() {
                                                                    token_count += 1;
                                                                    // push_str(&str) 追加字符串
                                                                    full_response.push_str(text);
                                                                    emitter.text_delta(
                                                                        content_index,
                                                                        text,
                                                                    );
                                                                }
                                                            }
                                                            "thinking_delta" => {
                                                                // 思考增量
                                                                let thinking =
                                                                    delta["thinking"].as_str().unwrap_or("");
                                                                if !thinking.is_empty() {
                                                                    emitter.thinking_delta(
                                                                        content_index,
                                                                        thinking,
                                                                    );
                                                                    thinking_response.push_str(thinking);
                                                                }
                                                            }
                                                            "signature_delta" => {
                                                                // 思考签名增量，暂不处理（用于多轮对话连续性）
                                                            }
                                                            "input_json_delta" => {
                                                                let partial = delta["partial_json"].as_str().unwrap_or("");
                                                                let ij_idx = json["index"].as_u64().unwrap_or(0) as usize;
                                                                if !partial.is_empty() {
                                                                    if let Some(tc) = tool_calls.get_mut(&ij_idx) {
                                                                        tc.arguments.push_str(partial);
                                                                    }
                                                                    let fid = tool_calls.get(&ij_idx)
                                                                        .map(|tc| tc.id.clone())
                                                                        .unwrap_or_default();
                                                                    if !fid.is_empty() {
                                                                        emitter.tool_call_delta(&fid, partial);
                                                                    }
                                                                }
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                    "content_block_stop" => {
                                                        // 块结束：递增索引
                                                        let _block = &json.get("content_block");
                                                        let index = json["index"].as_u64().unwrap_or(content_index as u64) as usize;
                                                        content_index = index + 1;
                                                    }
                                                    "message_delta" => {
                                                        // 消息级增量（含 stop_reason）
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
                                                        info!(
                                                            "[anthropic::stream_chat] message_stop: {} chunks, {} tokens, {} chars",
                                                            chunk_count, token_count, full_response.len()
                                                        );
                                                        let calls = flush_tool_calls(&mut tool_calls, emitter);
                                                        // done 事件由 commands.rs 在整轮 tool 循环结束时统一发射
                                                        return Ok(StreamOutcome { full_text: full_response, thinking_text: thinking_response, tool_calls: calls, had_stream_error: false });
                                                    }
                                                    "error" => {
                                                        // 服务端推送的 error 事件
                                                        let error_msg = json["error"]["message"]
                                                            .as_str()
                                                            .unwrap_or("未知错误");
                                                        error!("[anthropic] error 事件: {}", error_msg);
                                                        emitter.error(
                                                            StopReason::Error,
                                                            error_msg,
                                                            &full_response,
                                                        );
                                                        return Ok(StreamOutcome { full_text: full_response, thinking_text: thinking_response, tool_calls: vec![], had_stream_error: true });
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            Err(e) => {
                                                // JSON 解析失败：日志告警，但继续（可能是 partial chunk）
                                                warn!(
                                                    "[anthropic] JSON 解析失败: {}, data={}",
                                                    e,
                                                    &value.chars().take(200).collect::<String>()
                                                );
                                            }
                                        }
                                    }
                                    _ => {}  // 其他字段（id: 等）忽略
                                }
                            }
                            // 空行表示一个 SSE 事件的结束，重置 event 类型
                            if line.trim().is_empty() {
                                current_event = None;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        // 网络错误
                        emitter.error(
                            StopReason::Error,
                            &format!("流读取错误: {}", e),
                            &full_response,
                        );
                        return Ok(StreamOutcome { full_text: full_response, thinking_text: thinking_response, tool_calls: vec![], had_stream_error: true });
                    }
                    None => {
                        let calls = flush_tool_calls(&mut tool_calls, emitter);
                        // done 事件由 commands.rs 在整轮 tool 循环结束时统一发射
                        return Ok(StreamOutcome { full_text: full_response, thinking_text: thinking_response, tool_calls: calls, had_stream_error: false });
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

            // 超时设置更短（10s 连接，15s 总超时）—— 列表请求不应该阻塞太久
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

            // 如果 /v1/models 端点不可用（Anthropic 通常如此），回退到内置列表
            if !response.status().is_success() {
                info!(
                    "[anthropic::fetch_models] /v1/models 不可用 ({}), 使用内置模型列表",
                    response.status().as_u16()
                );
                return Ok(builtin_anthropic_models());
            }

            // 解析 JSON 响应
            let body_text = response.text().await.map_err(|e| e.to_string())?;
            let json: Value =
                serde_json::from_str(&body_text).map_err(|e| format!("JSON 解析失败: {}", e))?;

            // 遍历 json["data"] 数组，把每项转成 ModelInfo
            // .as_array()                Option<&Vec<Value>>
            // .unwrap_or(&vec![])        None 时给空数组引用
            // .iter()                    Iterator<Item = &Value>
            // .filter_map(|m| {...})     闭包返回 Option<T>：None 过滤掉，Some 保留
            //   类似 Java Stream 的 .filter(...).map(...).collect()
            //   但 filter_map 把两步合并成"既能过滤又能转换"
            let models: Vec<ModelInfo> = json["data"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|m| {
                    // `m["id"].as_str()?`  ? 是"早返回 None"运算符：
                    //   若 id 字段不是字符串，整个闭包返回 None（被 filter_map 过滤掉）
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

            // 没解析到模型也用内置列表兜底
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

            // 测速请求：max_tokens=1, 非流式, 内容 "hi"
            // 跑通这条 = 端到端通；耗时即"延迟"
            let body = serde_json::json!({
                "model": model_id,
                "max_tokens": 1,
                "messages": [{ "role": "user", "content": "hi" }],
                "stream": false,
            });

            // Instant::now() ≈ Java 的 System.nanoTime()
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

            // start.elapsed() 返回 Duration；as_millis() 得到 u128；强转 u32
            let elapsed = start.elapsed().as_millis() as u32;
            Ok(elapsed)
        })
    }
}


/// 内置 Anthropic 模型列表
///
/// 当 /v1/models 端点不可用时使用此列表。
fn builtin_anthropic_models() -> Vec<ModelInfo> {
    // vec![ ... ] 宏：构造 Vec<T>
    // 每个 ModelInfo 的字段按 struct 定义顺序赋值（Rust 不支持按名字赋值在 vec! 中，
    // 必须写 ModelInfo { ... } 字面量）
    vec![
        ModelInfo {
            id: "claude-sonnet-4-6".to_string(),
            provider_id: String::new(),
            display_name: "Claude Sonnet 4.6".to_string(),
            context_window: get_context_window("claude-sonnet-4-6"),
            latency_ms: None,
        },
        ModelInfo {
            id: "claude-opus-4-8".to_string(),
            provider_id: String::new(),
            display_name: "Claude Opus 4.8".to_string(),
            context_window: get_context_window("claude-opus-4-8"),
            latency_ms: None,
        },
        ModelInfo {
            id: "claude-haiku-4-5".to_string(),
            provider_id: String::new(),
            display_name: "Claude Haiku 4.5".to_string(),
            context_window: get_context_window("claude-haiku-4-5"),
            latency_ms: None,
        },
        ModelInfo {
            id: "claude-fable-5".to_string(),
            provider_id: String::new(),
            display_name: "Claude Fable 5".to_string(),
            context_window: get_context_window("claude-fable-5"),
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