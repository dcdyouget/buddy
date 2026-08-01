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
use futures_util::StreamExt; // 为 Stream trait 提供 .next() 等适配器
use log::{error, info, warn}; // 日志门面（log crate）
use reqwest::Client; // HTTP 客户端
use serde_json::{json, Value}; // 通用 JSON 值类型
use std::pin::Pin; // 用于 Pin<Box<...>>
use std::time::{Duration, Instant}; // 时间段与耗时统计
use tokio::sync::watch; // watch channel（取消信号）
use tokio::time::timeout; // 给异步操作加超时

/// Anthropic API 版本（固定）
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// 单次字节块读取超时
const CHUNK_TIMEOUT: Duration = Duration::from_secs(30);

/// SSE 单行数据上限：正常文本增量远小于此；防止异常/恶意响应把行缓冲撑爆。
const MAX_SSE_LINE_BYTES: usize = 1 << 20;

// ============================================================================
// AnthropicProvider —— Anthropic 适配器
// ============================================================================
//
// Rust `struct` 没有构造函数（除了字段初始化语法），所有"初始化逻辑"
// 都通过关联函数（associated function，类似 Java 的 static method）实现，
// 或者直接在 impl 块里挂方法。
// ============================================================================
pub struct AnthropicProvider;

fn has_valid_tool_arguments(tool_call: &ToolCall) -> bool {
    !tool_call.id.trim().is_empty()
        && !tool_call.name.trim().is_empty()
        && matches!(
            serde_json::from_str::<Value>(&tool_call.arguments),
            Ok(Value::Object(_))
        )
}

impl AnthropicProvider {
    /// 将内部消息格式转换为 Anthropic API 格式
    ///
    /// 入参 `messages: &[Message]` 是 `&[Message]`：不可变借用 Message 切片
    /// ≈ Java 的 `List<Message>` 不可变视图
    fn convert_messages(messages: &[Message]) -> Vec<Value> {
        // 收集所有 assistant 消息中有效的 tool_use id
        // (此函数只负责 messages 数组；系统提示通过请求体顶层 `system` 字段注入,见 stream_chat)
        // 只有参数完整、且存在对应 tool_result 的调用才允许进入下一次请求。
        // 主动停止流式输出时，assistant 中可能留下半截 arguments（input_json_delta
        // 未收完）；Anthropic API 要求 tool_use 的 input 是 JSON 对象，否则整次请求 400。
        let tool_result_ids: std::collections::HashSet<&str> = messages
            .iter()
            .filter(|m| m.role == MessageRole::Tool)
            .filter_map(|m| m.tool_call_id.as_deref())
            .filter(|id| !id.trim().is_empty())
            .collect();

        let valid_tool_use_ids: std::collections::HashSet<&str> = messages
            .iter()
            .filter(|m| m.role == MessageRole::Assistant)
            .filter_map(|m| m.tool_calls.as_ref())
            .flat_map(|calls| calls.iter())
            .filter(|tc| has_valid_tool_arguments(tc) && tool_result_ids.contains(tc.id.as_str()))
            .map(|tc| tc.id.as_str())
            .collect();

        messages
            .iter()
            .filter_map(|m| {
                // 孤儿 tool 消息：有 tool_use_id 但找不到对应的 assistant tool_use
                // 整条过滤掉，不发给 API
                if m.role == MessageRole::Tool {
                    let id = m.tool_call_id.as_deref().unwrap_or("");
                    if id.is_empty() || !valid_tool_use_ids.contains(id) {
                        warn!(
                            "[anthropic::convert_messages] 过滤孤儿 tool 消息, tool_use_id='{}' 找不到有效的 assistant tool_use",
                            id
                        );
                        return None;
                    }
                }

                let result = match m.role {
                    MessageRole::User => {
                        if m.images.is_empty() {
                            let context = super::runtime_time_context_for_message(m.created_at);
                            let content =
                                super::append_runtime_time_context(&m.content, &context);
                            json!({ "role": "user", "content": content })
                        } else {
                            let mut content = m
                                .images
                                .iter()
                                .filter_map(|image| {
                                    let (_, data) = image.data_url.split_once(";base64,")?;
                                    Some(json!({
                                        "type": "image",
                                        "source": {
                                            "type": "base64",
                                            "media_type": image.media_type,
                                            "data": data,
                                        }
                                    }))
                                })
                                .collect::<Vec<_>>();
                            if !m.content.is_empty() {
                                content.push(json!({
                                    "type": "text",
                                    "text": m.content,
                                }));
                            }
                            let context = super::runtime_time_context_for_message(m.created_at);
                            content.push(json!({
                                "type": "text",
                                "text": context,
                            }));
                            json!({ "role": "user", "content": content })
                        }
                    }
                    MessageRole::Assistant => {
                        let original_has_tool_calls =
                            m.tool_calls.as_ref().is_some_and(|calls| !calls.is_empty());
                        // 只保留参数完整、且存在对应 tool_result 的调用；
                        // 主动停止/中断流时可能留下孤儿 tool_use 或半截 arguments。
                        let valid_calls: Vec<&ToolCall> = m
                            .tool_calls
                            .iter()
                            .flatten()
                            .filter(|tc| {
                                if !has_valid_tool_arguments(tc) {
                                    warn!(
                                        "[anthropic::convert_messages] 丢弃参数不完整的 tool_use, id='{}', name='{}'",
                                        tc.id, tc.name
                                    );
                                    return false;
                                }
                                if !tool_result_ids.contains(tc.id.as_str()) {
                                    warn!(
                                        "[anthropic::convert_messages] 丢弃没有结果的 tool_use, id='{}', name='{}'",
                                        tc.id, tc.name
                                    );
                                    return false;
                                }
                                true
                            })
                            .collect();

                        // 纯工具调用消息在中断后若没有任何可发送的调用，就不能留下空 assistant
                        if original_has_tool_calls
                            && valid_calls.is_empty()
                            && m.content.trim().is_empty()
                        {
                            return None;
                        }

                        let mut obj = json!({ "role": "assistant" });
                        let mut content: Vec<Value> = if m.content.is_empty() {
                            vec![]
                        } else {
                            vec![json!({"type": "text", "text": m.content})]
                        };
                        for tc in valid_calls {
                            // has_valid_tool_arguments 已保证 arguments 可解析为 JSON 对象
                            let args: Value =
                                serde_json::from_str(&tc.arguments).unwrap_or(Value::Null);
                            content.push(json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.name,
                                "input": args,
                            }));
                        }
                        obj["content"] = json!(content);
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
        let line = line.trim(); // 去掉首尾空白
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
            let field = line[..colon_pos].trim(); // &str
            let value = line[colon_pos + 1..].trim(); // &str
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
        request_id: &'a str,
        messages: &'a [Message],
        emitter: &'a StreamEventEmitter,
        mut cancel_rx: watch::Receiver<bool>,
        compat: Option<&'a CompatConfig>,
        tools: &'a [ToolDefinition],
    ) -> Pin<Box<dyn std::future::Future<Output = Result<StreamOutcome, ApiError>> + Send + 'a>>
    {
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
            // 注意: Anthropic 用顶层 `system` 字段(不是 messages 里的 system 消息)
            // 系统提示保持静态，每条用户消息的固定时间后缀已在转换阶段追加。
            // Anthropic 要求 max_tokens 不能超过模型的 context_window；
            // 固定 4096 在小窗口模型上会被 API 拒绝。取 min(4096, context_window 的一半)，
            // 大窗口模型保持 4096 不变。
            let max_tokens =
                std::cmp::min(4096u32, get_context_window(model).saturating_div(2).max(1));
            let mut body = serde_json::json!({
                "model": model,
                "max_tokens": max_tokens,
                "system": super::BUDDY_SYSTEM_PROMPT,
                "messages": anthropic_messages,
                "stream": true,
            });
            if !tools.is_empty() {
                let anthropic_tools: Vec<Value> = tools
                    .iter()
                    .map(|t| {
                        json!({
                            "name": t.name,
                            "description": t.description,
                            // 与 OpenAI provider 保持一致：确保 input_schema 是标准对象
                            "input_schema": super::openai_compatible::ensure_object_schema(&t.parameters),
                        })
                    })
                    .collect();
                body["tools"] = json!(anthropic_tools);
                body["tool_choice"] = json!({"type": "auto"});
            }

            // ── 4. 拼接 URL ──
            // format!("{}/v1/messages", ...) ≈ Java 的 String.format("%s/v1/messages", baseUrl)
            // trim_end_matches('/') 移除结尾的 '/'，防止出现 "//v1"
            let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));
            let request_bytes = serde_json::to_vec(&body)
                .map(|serialized| serialized.len())
                .unwrap_or_else(|_| body.to_string().len());
            let tool_names: Vec<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
            info!(
                "[llm][{}] 请求摘要: protocol=anthropic_messages, model={}, url={}, body_bytes={}, system_prompt_bytes={}, runtime_contexts={}, {}, available_tools={} [{}]",
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
            let request_started = Instant::now();
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
            // buffer 累积"未完整行"，SSE 按行解析，必须自己组行。
            // 注意：用字节缓冲而不是 String——按网络 chunk 解码会把跨 chunk 的
            // 多字节字符（中文/emoji）永久损坏，必须在完整行处才解码。
            let mut buffer: Vec<u8> = Vec::new();

            // 流式状态追踪变量（mut 表示可变）
            let mut full_response = String::new(); // 累积完整回复文本
            let mut thinking_response = String::new(); // 累积思考文本
            let mut current_event: Option<String> = None; // 当前事件类型
            let mut content_index: usize = 0; // 内容块索引
            let mut _has_started = false; // 下划线前缀：未使用变量（Rust 会警告）

            let mut chunk_count: u64 = 0; // 接收到的字节块数
            let mut token_count: u64 = 0; // 已发出的文本 token 数
            let mut response_bytes = 0usize;
            let mut first_output_ms: Option<u64> = None;
            let mut input_tokens = 0u64;
            let mut output_tokens = 0u64;
            let mut cache_hit_tokens = 0u64;
            let mut cache_write_tokens = 0u64;
            let mut has_usage = false;

            // P5: tool_call 流式追踪 (key=index, Anthropic 用 index 区分 block)
            use std::collections::HashMap;
            let mut tool_calls: HashMap<usize, ToolCall> = HashMap::new();
            let flush_tool_calls =
                |tc: &mut HashMap<usize, ToolCall>, em: &StreamEventEmitter| -> Vec<ToolCall> {
                    let mut ordered: Vec<(usize, ToolCall)> = tc.drain().collect();
                    ordered.sort_by_key(|(i, _)| *i);
                    // 丢弃从未收到 input_json_delta 的不完整 tool_use（如流被中断/提前结束），
                    // 避免把半截调用当作可执行工具。空参数说明该块从未真正开始执行。
                    ordered.retain(|(_, t)| {
                        if t.arguments.trim().is_empty() {
                            warn!(
                                "[anthropic::stream_chat] 丢弃参数为空的不完整 tool_use, id='{}', name='{}'",
                                t.id, t.name
                            );
                            false
                        } else {
                            true
                        }
                    });
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
                        response_bytes += bytes.len();
                        // 累积原始字节而不是按 chunk 解码：若多字节字符被 TCP 分块
                        // 从中间切开，逐 chunk 用 from_utf8_lossy 会永久损坏该字符。
                        // 改为按字节缓冲，遇到完整行（以 0x0A 结尾）才整行解码。
                        buffer.extend_from_slice(&bytes);
                        // 单行数据上限：防止异常/恶意响应把行缓冲撑爆
                        if buffer.len() > MAX_SSE_LINE_BYTES {
                            warn!(
                                "[anthropic::stream_chat] SSE 行数据超过 {} 字节，终止解析",
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

                        // 逐行解析：SSE 是行分隔的协议
                        // 注意：0x0A 不可能是多字节字符的续字节，所以按字节找 \n 切出的
                        // 完整行保证是合法 UTF-8，可安全解码，不会损坏跨 chunk 的字符。
                        while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                            let line = String::from_utf8_lossy(&buffer[..pos]).to_string();
                            buffer.drain(..pos + 1);

                            // 尝试解析 SSE 字段行（"field: value"）
                            if let Some((field, value)) = Self::parse_sse_line(&line) {
                                match field.as_str() {
                                    "event" => {
                                        // 记录事件类型，下一行 data 会用到
                                        current_event = Some(value);
                                    }
                                    "data" => {
                                        // event_type 取 current_event，没设过就给空串
                                        let event_type = current_event.as_deref().unwrap_or("");
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
                                                        let usage = json["message"]["usage"]
                                                            .as_object()
                                                            .or_else(|| json["usage"].as_object());
                                                        if let Some(u) = usage {
                                                            let uncached_input = u
                                                                .get("input_tokens")
                                                                .and_then(|v| v.as_u64())
                                                                .unwrap_or(0);
                                                            cache_hit_tokens = u
                                                                .get("cache_read_input_tokens")
                                                                .and_then(|v| v.as_u64())
                                                                .unwrap_or(0);
                                                            cache_write_tokens = u
                                                                .get("cache_creation_input_tokens")
                                                                .and_then(|v| v.as_u64())
                                                                .unwrap_or(0);
                                                            input_tokens = uncached_input
                                                                .saturating_add(cache_hit_tokens)
                                                                .saturating_add(cache_write_tokens);
                                                            has_usage = true;
                                                        }
                                                    }
                                                    "content_block_start" => {
                                                        // 内容块开始，区分 text/thinking 块
                                                        let block = &json["content_block"];
                                                        let block_type =
                                                            block["type"].as_str().unwrap_or("");
                                                        let idx =
                                                            json["index"].as_u64().unwrap_or(0)
                                                                as usize;
                                                        match block_type {
                                                            "text" => {
                                                                emitter.text_start(idx);
                                                            }
                                                            "thinking" | "redacted_thinking" => {
                                                                emitter.thinking_start(idx);
                                                            }
                                                            "tool_use" => {
                                                                first_output_ms.get_or_insert_with(|| {
                                                                    request_started.elapsed().as_millis()
                                                                        as u64
                                                                });
                                                                let block = &json["content_block"];
                                                                let id = block["id"]
                                                                    .as_str()
                                                                    .unwrap_or("");
                                                                let n = block["name"]
                                                                    .as_str()
                                                                    .unwrap_or("");
                                                                tool_calls.insert(
                                                                    idx,
                                                                    ToolCall {
                                                                        id: id.to_string(),
                                                                        name: n.to_string(),
                                                                        arguments: String::new(),
                                                                    },
                                                                );
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
                                                                let text = delta["text"]
                                                                    .as_str()
                                                                    .unwrap_or("");
                                                                if !text.is_empty() {
                                                                    first_output_ms.get_or_insert_with(|| {
                                                                        request_started
                                                                            .elapsed()
                                                                            .as_millis()
                                                                            as u64
                                                                    });
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
                                                                let thinking = delta["thinking"]
                                                                    .as_str()
                                                                    .unwrap_or("");
                                                                if !thinking.is_empty() {
                                                                    first_output_ms.get_or_insert_with(|| {
                                                                        request_started
                                                                            .elapsed()
                                                                            .as_millis()
                                                                            as u64
                                                                    });
                                                                    emitter.thinking_delta(
                                                                        content_index,
                                                                        thinking,
                                                                    );
                                                                    thinking_response
                                                                        .push_str(thinking);
                                                                }
                                                            }
                                                            "signature_delta" => {
                                                                // 思考签名增量，暂不处理（用于多轮对话连续性）
                                                            }
                                                            "input_json_delta" => {
                                                                let partial = delta["partial_json"]
                                                                    .as_str()
                                                                    .unwrap_or("");
                                                                let ij_idx = json["index"]
                                                                    .as_u64()
                                                                    .unwrap_or(0)
                                                                    as usize;
                                                                if !partial.is_empty() {
                                                                    if let Some(tc) =
                                                                        tool_calls.get_mut(&ij_idx)
                                                                    {
                                                                        tc.arguments
                                                                            .push_str(partial);
                                                                    }
                                                                    let fid = tool_calls
                                                                        .get(&ij_idx)
                                                                        .map(|tc| tc.id.clone())
                                                                        .unwrap_or_default();
                                                                    if !fid.is_empty() {
                                                                        emitter.tool_call_delta(
                                                                            &fid, partial,
                                                                        );
                                                                    }
                                                                }
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                    "content_block_stop" => {
                                                        // 块结束：递增索引
                                                        let _block = &json.get("content_block");
                                                        let index = json["index"]
                                                            .as_u64()
                                                            .unwrap_or(content_index as u64)
                                                            as usize;
                                                        content_index = index + 1;
                                                    }
                                                    "message_delta" => {
                                                        // 消息级增量（含 stop_reason）
                                                        let delta = &json["delta"];
                                                        let stop_reason = delta["stop_reason"]
                                                            .as_str()
                                                            .unwrap_or("");
                                                        let usage = &json["usage"];
                                                        output_tokens = usage["output_tokens"]
                                                            .as_u64()
                                                            .unwrap_or(0);
                                                        has_usage = has_usage || output_tokens > 0;
                                                        info!("[llm][{}] stop_reason={}", request_id, stop_reason);
                                                    }
                                                    "message_stop" => {
                                                        let calls = flush_tool_calls(
                                                            &mut tool_calls,
                                                            emitter,
                                                        );
                                                        let usage_summary = if has_usage {
                                                            let total_tokens = input_tokens
                                                                .saturating_add(output_tokens);
                                                            let cache_rate = if input_tokens > 0 {
                                                                cache_hit_tokens as f64 * 100.0
                                                                    / input_tokens as f64
                                                            } else {
                                                                0.0
                                                            };
                                                            format!(
                                                                "input_tokens={}, output_tokens={}, total_tokens={}, cache_hit={}, cache_miss={}, cache_write={}, cache_rate={:.2}%",
                                                                input_tokens,
                                                                output_tokens,
                                                                total_tokens,
                                                                cache_hit_tokens,
                                                                input_tokens.saturating_sub(cache_hit_tokens),
                                                                cache_write_tokens,
                                                                cache_rate,
                                                            )
                                                        } else {
                                                            "tokens=Provider未返回".to_string()
                                                        };
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
                                                            usage_summary,
                                                        );
                                                        // done 事件由 commands.rs 在整轮 tool 循环结束时统一发射
                                                        return Ok(StreamOutcome {
                                                            full_text: full_response,
                                                            thinking_text: thinking_response,
                                                            tool_calls: calls,
                                                            had_stream_error: false,
                                                        });
                                                    }
                                                    "error" => {
                                                        // 服务端推送的 error 事件
                                                        let error_msg = json["error"]["message"]
                                                            .as_str()
                                                            .unwrap_or("未知错误");
                                                        error!(
                                                            "[anthropic] error 事件: {}",
                                                            error_msg
                                                        );
                                                        emitter.error(
                                                            StopReason::Error,
                                                            error_msg,
                                                            &full_response,
                                                        );
                                                        return Ok(StreamOutcome {
                                                            full_text: full_response,
                                                            thinking_text: thinking_response,
                                                            tool_calls: vec![],
                                                            had_stream_error: true,
                                                        });
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
                                    _ => {} // 其他字段（id: 等）忽略
                                }
                            }
                            // 空行表示一个 SSE 事件的结束，重置 event 类型
                            if line.trim().is_empty() {
                                current_event = None;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        // 网络错误：保留已累积的完整工具调用（不完整调用在 flush 内被丢弃），
                        // 避免整轮工具结果因网络错误而丢失。
                        emitter.error(
                            StopReason::Error,
                            &format!("流读取错误: {}", e),
                            &full_response,
                        );
                        let calls = flush_tool_calls(&mut tool_calls, emitter);
                        return Ok(StreamOutcome {
                            full_text: full_response,
                            thinking_text: thinking_response,
                            tool_calls: calls,
                            had_stream_error: true,
                        });
                    }
                    None => {
                        // 流结束但没收到 message_stop：连接被提前切断（异常结束）。
                        // Anthropic 协议保证正常完成的响应必然以 message_stop 收尾，
                        // 走到这里说明消息不完整——按网络错误处理，不执行未完成工具调用。
                        warn!(
                            "[anthropic::stream_chat] 流提前结束(未收到 message_stop)，视为异常"
                        );
                        emitter.error(
                            StopReason::Error,
                            "流提前结束(连接中断)",
                            &full_response,
                        );
                        return Ok(StreamOutcome {
                            full_text: full_response,
                            thinking_text: thinking_response,
                            tool_calls: vec![],
                            had_stream_error: true,
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
            // Anthropic 没有公开的模型列表 API，返回已知模型
            // 实际应用中可以从配置文件或硬编码列表获取
            let url = format!("{}/v1/models", base_url.trim_end_matches('/'));

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
                        supports_vision: false,
                        supports_image_generation: false,
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

            let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

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
            supports_vision: false,
            supports_image_generation: false,
        },
        ModelInfo {
            id: "claude-opus-4-8".to_string(),
            provider_id: String::new(),
            display_name: "Claude Opus 4.8".to_string(),
            context_window: get_context_window("claude-opus-4-8"),
            latency_ms: None,
            supports_vision: false,
            supports_image_generation: false,
        },
        ModelInfo {
            id: "claude-haiku-4-5".to_string(),
            provider_id: String::new(),
            display_name: "Claude Haiku 4.5".to_string(),
            context_window: get_context_window("claude-haiku-4-5"),
            latency_ms: None,
            supports_vision: false,
            supports_image_generation: false,
        },
        ModelInfo {
            id: "claude-fable-5".to_string(),
            provider_id: String::new(),
            display_name: "Claude Fable 5".to_string(),
            context_window: get_context_window("claude-fable-5"),
            latency_ms: None,
            supports_vision: false,
            supports_image_generation: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ImageAttachment, MessageRole};

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
        assert_eq!(
            AnthropicProvider::parse_sse_line(": this is a comment"),
            None
        );
    }

    #[test]
    fn test_convert_user_images_to_anthropic_content_blocks() {
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

        let output = AnthropicProvider::convert_messages(&[message]);
        let content = output[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "image");
        assert_eq!(content[0]["source"]["type"], "base64");
        assert_eq!(content[0]["source"]["media_type"], "image/png");
        assert_eq!(content[0]["source"]["data"], "aGVsbG8=");
        assert_eq!(content[1]["type"], "text");
        assert_eq!(content[2]["type"], "text");
        assert!(content[2]["text"]
            .as_str()
            .unwrap()
            .contains("<buddy_runtime_context>"));
    }

    #[test]
    fn test_each_user_message_keeps_its_stable_runtime_time() {
        let output = AnthropicProvider::convert_messages(&[
            make_user_message("u1", "旧问题", 1_700_000_000),
            make_user_message("u2", "新问题", 1_800_000_000),
        ]);

        let previous = output[0]["content"].as_str().unwrap();
        assert!(previous.starts_with("旧问题\n\n<buddy_runtime_context>"));
        let latest = output[1]["content"].as_str().unwrap();
        assert!(latest.starts_with("新问题\n\n<buddy_runtime_context>"));
        assert_ne!(previous, latest);
        assert_eq!(
            output,
            AnthropicProvider::convert_messages(&[
                make_user_message("u1", "旧问题", 1_700_000_000),
                make_user_message("u2", "新问题", 1_800_000_000),
            ])
        );
    }
}
