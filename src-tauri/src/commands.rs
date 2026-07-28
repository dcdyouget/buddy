// IPC 命令处理模块：定义所有前端可调用的 Tauri 命令
//
// 每个 #[tauri::command] 函数对应一个前端 invoke 调用点，
// 负责参数校验、调用业务逻辑、返回结果或错误。

use crate::models::*;
use crate::providers::{self, ProviderType};
use crate::streaming::{ContentBlock, QuestionOption, StopReason, StreamEventEmitter};
use crate::storage;
use crate::tools::{AskUserAnswer, AskUserArgs, AskUserOption, ToolRegistry};
use log::{info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use parking_lot::Mutex;
use tauri::{Manager, State};
use tokio::sync::{oneshot, watch};
/// 取消状态：跨命令共享的取消通道
///
/// 使用 watch channel 实现：send_message 创建发送端，
/// stop_generation 通过发送端发出取消信号。
/// Mutex 包裹保证线程安全（Tauri 命令可能在不同线程执行）。
pub struct CancelState {
    pub sender: Mutex<Option<watch::Sender<bool>>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tool 审批状态(共享)
// ─────────────────────────────────────────────────────────────────────────────
//
// send_message 遇到 write 类 tool 时:
//   1. 发射 ToolApprovalRequired 事件(前端弹 modal)
//   2. 创建 oneshot channel,把 receiver 存进 ApprovalState
//   3. await receiver → 拿到用户的批准(approved=true)或拒绝(approved=false)
//   4. 前端调用 approve_tool_call(id, approved) → 通过 oneshot sender 推送
//
// "本次都允许" 标志:
//   - ApprovalState 持有 `approve_all_for_turn: AtomicBool`
//   - send_message 在审批循环开头读,如果是 true 就跳过 ToolApprovalRequired
//   - 前端点"本次都允许"时设为 true(P4 通过单独 command 触发;实现见 P4 后端)
// ─────────────────────────────────────────────────────────────────────────────


/// 一次 tool 调用审批的等待槽位
#[allow(dead_code)] // 字段在 send_message 中构造并通过 oneshot 传递
pub struct ApprovalSlot {
    /// tool_call.id
    pub id: String,
    /// tool 名(仅显示)
    pub name: String,
    /// 参数(供 UI 展示)
    pub arguments: String,
    /// 审批原因("write to /path" 等)
    pub reason: String,
    /// oneshot sender:前端 invoke('approve_tool_call', {id, approved}) 通过这里推回结果
    pub tx: Option<oneshot::Sender<bool>>,
}

/// 共享审批状态
pub struct ApprovalState {
    /// 当前挂起待审批的 slot 列表
    pub pending: Mutex<Vec<ApprovalSlot>>,
    /// "本次都允许"标志:send_message 进入审批循环时检查,跳过所有 ToolApprovalRequired
    pub approve_all_for_turn: AtomicBool,
}

// ─────────────────────────────────────────────────────────────────────────────
// ask_user 问题等待状态
// ─────────────────────────────────────────────────────────────────────────────
//
// 当模型调用 ask_user tool 时, send_message 会:
//   1. 解析 arguments(question/options/multi_select/header)
//   2. 发射 ToolQuestionRequired 事件(让前端弹 QuestionModal)
//   3. 创建 oneshot channel, 把 receiver 存进 QuestionState
//   4. await receiver → 拿到用户选择
//   5. 前端调用 answer_tool_question(id, selected, custom) → 通过 sender 推回

/// 一次 ask_user 调用的等待槽位
pub struct QuestionSlot {
    /// tool_call.id
    pub id: String,
    /// 工具名(始终为 "ask_user")
    pub name: String,
    /// 原始参数 JSON 字符串(供 UI 展示)
    pub arguments: String,
    /// 解析后的选项(用于构造 tool result)
    pub options: Vec<AskUserOption>,
    /// oneshot sender:前端 invoke('answer_tool_question', ...) 通过这里推回结果
    pub tx: Option<oneshot::Sender<AskUserAnswer>>,
}

/// 共享问题等待状态
pub struct QuestionState {
    /// 当前挂起待回答的 slot 列表
    pub pending: Mutex<Vec<QuestionSlot>>,
}
// ── 滑动窗口辅助函数 ──

/// 估算文本的 token 数量（字符级启发式算法）
///
/// 公式: `chars().count() / 3`，最少返回 1
///
/// 对于中英文混合文本，这是一个合理的保守估算：
/// - 英文约 4 字符/token → 可能高估约 25%
/// - 中文约 1-2 字符/token → 可能低估约 30%
/// - 30% 的预算余量可以吸收估算误差
fn estimate_tokens(text: &str) -> u32 {
    let count = text.chars().count() as u32;
    if count == 0 {
        return 0;
    }
    (count / 3).max(1)
}

/// 估算消息列表的总 token 数
fn estimate_total_tokens(messages: &[Message]) -> u32 {
    messages.iter().map(|m| estimate_tokens(&m.content)).sum()
}

/// 计算滑动窗口的起始索引
///
/// 从最新消息开始向前累加 token 估算值，预算耗尽时停止。
/// 返回可保留的最老消息的索引，调用方使用 `&messages[start..]`。
///
/// 保证最后一条消息（当前用户输入）永不被丢弃。
fn compute_window_start(messages: &[Message], token_budget: u32) -> usize {
    if messages.is_empty() {
        return 0;
    }

    let mut remaining = token_budget;
    let mut start = messages.len();

    // 从最新到最老遍历
    for i in (0..messages.len()).rev() {
        let cost = estimate_tokens(&messages[i].content);
        if remaining >= cost {
            remaining -= cost;
            start = i;
        } else {
            break;
        }
    }

    // 安全网：永远不丢弃最后一条消息
    if start == messages.len() {
        start = messages.len() - 1;
    }

    start
}

/// 构造 tool 结果消息
fn build_tool_msg(turn: usize, call: &crate::models::ToolCall, content: String, is_error: bool) -> Message {
    Message {
        id: format!("t-{}-{}", turn, chrono::Utc::now().timestamp_millis()),
        role: MessageRole::Tool,
        content,
        blocks: None,
        model_id: None,
        created_at: chrono::Utc::now().timestamp() as u64,
        tool_calls: None,
        tool_call_id: Some(call.id.clone()),
        tool_name: Some(call.name.clone()),
        is_error: Some(is_error),
        parent_message_id: None,
    }
}

/// 将同一次模型调用累积的消息一次性落盘，避免工具循环产生密集的小文件写入。
async fn flush_pending_messages(app: tauri::AppHandle, pending: Vec<Message>) {
    if pending.is_empty() {
        return;
    }

    let count = pending.len();
    match tokio::task::spawn_blocking(move || storage::append_messages(&app, &pending)).await {
        Ok(Ok(())) => info!("[send_message] 本轮 {} 条消息已批量持久化", count),
        Ok(Err(e)) => warn!("[send_message] 批量持久化消息失败: {}", e),
        Err(e) => warn!("[send_message] 批量持久化任务失败: {}", e),
    }
}

/// 发送消息命令
///
/// 前端调用此命令发起 AI 对话。后端根据 Provider 类型分发到对应适配器：
/// 1. 查找模型和 Provider 配置
/// 2. 持久化用户消息到本地存储
/// 3. 创建取消通道并存入共享状态
/// 4. 调用对应 Provider 的 stream_chat 进行流式对话
/// 5. 流式完成后自动持久化 AI 回复
/// 6. 错误时发送对应事件给前端
/// 7. 清理取消通道
#[tauri::command]
pub async fn send_message(
    app: tauri::AppHandle,
    state: State<'_, CancelState>,
    approval: State<'_, ApprovalState>,
    messages: Vec<Message>,
    model_id: String,
) -> Result<(), String> {
    // 从配置中查找模型对应的 Provider
    let config = storage::get_config(&app)?;
    let model = config
        .models
        .iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| "未找到指定的模型".to_string())?;

    let provider = config
        .providers
        .iter()
        .find(|p| p.id == model.provider_id)
        .ok_or_else(|| "未找到对应的 Provider".to_string())?;

    let provider_type = ProviderType::from_str(&provider.provider_type);

    info!(
        "[send_message] model={}, provider={}, type={:?}, history_len={}, est_tokens={}, context_window={}",
        model_id,
        provider.id,
        provider_type,
        messages.len(),
        estimate_total_tokens(&messages),
        model.context_window,
    );

    // ── 滑动窗口：保留 70% context_window 以内的最近消息 ──
    let budget = ((model.context_window as f64) * 0.7_f64) as u32;
    let full_est = estimate_total_tokens(&messages);
    let start_idx = compute_window_start(&messages, budget);
    let windowed_messages = &messages[start_idx..];
    let windowed_est = estimate_total_tokens(windowed_messages);

    if start_idx > 0 {
        warn!(
            "[send_message] 滑动窗口裁剪: 保留 {}/{} 条消息, est_tokens={}/{}, budget={}/{}, dropped={}条, oldest_kept_idx={}",
            windowed_messages.len(),
            messages.len(),
            windowed_est,
            full_est,
            budget,
            model.context_window,
            start_idx,
            start_idx,
        );
    } else {
        info!(
            "[send_message] 滑动窗口: 无需裁剪, est_tokens={}/{}, budget={}, messages={}",
            full_est,
            model.context_window,
            budget,
            messages.len(),
        );
    }

    // 本轮产生的用户、assistant 与工具消息先放在内存，结束时统一批量写入。
    let mut pending_persistence: Vec<Message> = messages.last().cloned().into_iter().collect();

    // 创建取消通道
    let (cancel_tx, mut cancel_rx) = watch::channel(false);
    *state.sender.lock() = Some(cancel_tx);

    // 创建统一事件发射器
    let emitter = StreamEventEmitter::new(app.clone());

    // 根据 Provider 类型创建对应的适配器
    let llm_provider = providers::create_provider(&provider_type);
    let compat = provider.compat.as_ref();

    // ── P4: 构造 ToolRegistry(只包含内置 tool,MCP tool 由 P7 注入) ──
    let registry = ToolRegistry::new(crate::tools::builtin::builtin_tools(
        config.allowed_paths.clone(),
    ));
    // P7 会在此追加 mcp tool

    // 把 messages 拷成可变的 Vec,tool 循环会往里 push assistant(tool_calls) + tool(result)
    let mut conv_messages: Vec<Message> = messages.clone();

    // 每次 send_message 开始时重置"本次都允许"标志，防止上次被中断时残留
    approval.approve_all_for_turn.store(false, Ordering::Relaxed);

    // 工具循环上限(防止 model 死循环):
    // - 硬上限 20 轮(安全网,正常情况下不会触发)
    // - 软上限:连续 3 轮全部 tool 执行失败则中断,避免 model 反复失败同一操作
    const MAX_TOOL_TURNS: usize = 20;
    const MAX_CONSECUTIVE_FAILED_TURNS: usize = 3;
    let mut turn: usize = 0;
    let mut consecutive_failed_turns: usize = 0;

    let outcome: Result<crate::streaming::StreamOutcome, _> = loop {
        turn += 1;
        if turn > MAX_TOOL_TURNS {
            warn!("[send_message] 达到 tool 轮数硬上限 {}", MAX_TOOL_TURNS);
            break Err(crate::providers::ApiError::NetworkError(
                format!("已达到最大工具调用轮数 {}", MAX_TOOL_TURNS),
            ));
        }

        // 滑动窗口(每轮都算)
        let budget = ((model.context_window as f64) * 0.7_f64) as u32;
        let start_idx = compute_window_start(&conv_messages, budget);
        let windowed = &conv_messages[start_idx..];

        let tools = registry.all_definitions();
        let outcome = llm_provider
            .stream_chat(
                &provider.base_url,
                &provider.api_key,
                &model_id,
                windowed,
                &emitter,
                cancel_rx.clone(),
                compat,
                &tools,
            )
            .await;

        let out = match outcome {
            Ok(o) => o,
            Err(e) => break Err(e),
        };

        // 持久化本轮 assistant 消息
        if !out.full_text.is_empty() || !out.tool_calls.is_empty() || !out.thinking_text.is_empty() {
            // 合并思考 + 文本为 blocks
            // - thinking_text 非空时:显式构造 thinking + text 块
            // - thinking_text 为空时:尝试从 full_text 解析 <think> 标签(部分模型把思考包在 text 里)
            let blocks = if !out.thinking_text.is_empty() {
                let mut b = vec![ContentBlock::Thinking {
                    content: out.thinking_text.clone(),
                    is_open: false,
                }];
                if !out.full_text.is_empty() {
                    b.push(ContentBlock::Text { content: out.full_text.clone() });
                }
                b
            } else {
                ContentBlock::parse_from_text(&out.full_text)
            };
            let assistant_msg = Message {
                id: format!("a-{}-{}", turn, chrono::Utc::now().timestamp_millis()),
                role: MessageRole::Assistant,
                content: out.full_text.clone(),
                blocks: Some(blocks),
                model_id: Some(model_id.clone()),
                created_at: chrono::Utc::now().timestamp() as u64,
                tool_calls: if out.tool_calls.is_empty() { None } else { Some(out.tool_calls.clone()) },
                tool_call_id: None,
                tool_name: None,
                is_error: None,
                parent_message_id: None,
            };
            pending_persistence.push(assistant_msg.clone());
            conv_messages.push(assistant_msg.clone());
        }

        // 没 tool_calls → 正常结束
        if out.tool_calls.is_empty() {
            break Ok(out);
        }

        // 有 tool_calls → 顺序执行(并发=1;P4 决定),再调 stream_chat
        info!("[send_message] turn {} 收到 {} 个 tool_call,开始执行",
            turn, out.tool_calls.len());
        // 本轮追踪:是否有至少一个 tool 执行成功(非 is_error)?
        let mut turn_has_success = false;
        for call in &out.tool_calls {
            // ── ask_user 特殊分支:不进入普通 tool.execute,而是弹 QuestionModal 等回答 ──
            if call.name == "ask_user" {
                let args_value: serde_json::Value = match serde_json::from_str(&call.arguments) {
                    Ok(v) => v,
                    Err(e) => {
                        let content = format!("ask_user 参数解析失败: {}", e);
                        emitter.tool_result(&call.id, "ask_user", &content, true);
                        let tool_msg = build_tool_msg(turn, call, content, true);
                        pending_persistence.push(tool_msg.clone());
                        conv_messages.push(tool_msg.clone());
                        continue;
                    }
                };
                let parsed: AskUserArgs = match serde_json::from_value(args_value) {
                    Ok(a) => a,
                    Err(e) => {
                        let content = format!("ask_user 参数校验失败: {}", e);
                        emitter.tool_result(&call.id, "ask_user", &content, true);
                        let tool_msg = build_tool_msg(turn, call, content, true);
                        pending_persistence.push(tool_msg.clone());
                        conv_messages.push(tool_msg.clone());
                        continue;
                    }
                };

                // 发射 ToolQuestionRequired,前端弹 QuestionModal
                let q_options: Vec<QuestionOption> = parsed.options.iter().map(|o| QuestionOption {
                    label: o.label.clone(),
                    description: o.description.clone(),
                    requires_input: o.requires_input,
                    input_placeholder: o.input_placeholder.clone(),
                }).collect();
                emitter.tool_question_required(
                    &call.id,
                    "ask_user",
                    &parsed.question,
                    q_options,
                    parsed.multi_select,
                    &parsed.header,
                );

                // 阻塞等待用户回答
                let (tx, rx) = oneshot::channel::<AskUserAnswer>();
                let q_state = app.state::<QuestionState>();
                q_state.pending.lock().push(QuestionSlot {
                    id: call.id.clone(),
                    name: "ask_user".to_string(),
                    arguments: call.arguments.clone(),
                    options: parsed.options.clone(),
                    tx: Some(tx),
                });

                let answer = tokio::select! {
                    _ = cancel_rx.changed() => {
                        // 取消时移除 slot,按 "用户跳过" 处理
                        // 注意: position + remove 必须在同一持锁作用域内完成,
                        // 否则并发 answer_tool_question 可能在两次 lock 之间改动 Vec,
                        // 让 idx 失效导致越界 panic 或误删其他 slot。
                        let removed = {
                            let mut pending = q_state.pending.lock();
                            if let Some(idx) = pending.iter().position(|s| s.id == call.id) {
                                pending.remove(idx);
                                true
                            } else {
                                false
                            }
                        };
                        if !removed {
                            warn!("[send_message] ask_user 取消时找不到 slot id={} (可能已答完)", call.id);
                        } else {
                            warn!("[send_message] ask_user 等待被用户取消");
                        }
                        AskUserAnswer { selected: vec![], inputs: vec![], custom: Some("(已取消)".to_string()) }
                    }
                    result = rx => {
                        match result {
                            Ok(a) => a,
                            Err(_) => {
                                warn!("[send_message] ask_user oneshot 失败");
                                AskUserAnswer { selected: vec![], inputs: vec![], custom: Some("(无响应)".to_string()) }
                            }
                        }
                    }
                };

                // 把答案转成 tool result 文本
                let content = format_ask_user_answer(&parsed.options, &answer);
                turn_has_success = true; // 用户回答了就是成功
                emitter.tool_result(&call.id, "ask_user", &content, false);
                let tool_msg = build_tool_msg(turn, call, content, false);
                pending_persistence.push(tool_msg.clone());
                conv_messages.push(tool_msg.clone());
                continue;
            }

            let tool = match registry.get(&call.name) {
                Some(t) => t,
                None => {
                    let content = format!("tool '{}' 未在 ToolRegistry 中注册", call.name);
                    emitter.tool_result(&call.id, &call.name, &content, true);
                    let tool_msg = build_tool_msg(turn, call, content, true);
                    pending_persistence.push(tool_msg.clone());
                    conv_messages.push(tool_msg.clone());
                    continue;
                }
            };

            // Write 类且本轮未"本次都允许":弹审批
            if tool.safety() == crate::tools::ToolSafety::Write
                && !approval.approve_all_for_turn.load(Ordering::Relaxed)
            {
                let reason = format!("{} 调用 {}", call.name, call.arguments);
                emitter.tool_approval_required(&call.id, &call.name, &call.arguments, &reason);
                let (tx, rx) = oneshot::channel::<bool>();
                approval.pending.lock().push(ApprovalSlot {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    arguments: call.arguments.clone(),
                    reason: reason.clone(),
                    tx: Some(tx),
                });
                let approved = tokio::select! {
                    _ = cancel_rx.changed() => {
                        // User cancelled during approval wait — remove the slot from pending
                        if let Some(idx) = approval.pending.lock().iter().position(|s| s.id == call.id) {
                            approval.pending.lock().remove(idx);
                        }
                        warn!("[send_message] 审批被用户取消");
                        false
                    }
                    result = rx => {
                        match result {
                            Ok(b) => b,
                            Err(_) => {
                                warn!("[send_message] 审批 oneshot 失败,按拒绝处理");
                                false
                            }
                        }
                    }
                };
                if !approved {
                    let content = "用户拒绝执行".to_string();
                    emitter.tool_result(&call.id, &call.name, &content, true);
                    let tool_msg = build_tool_msg(turn, call, content, true);
                    pending_persistence.push(tool_msg.clone());
                    conv_messages.push(tool_msg.clone());
                    continue;
                }
            }

            // 执行
            emitter.tool_executing(&call.id, &call.name);
            let args_value: serde_json::Value = match serde_json::from_str(&call.arguments) {
                Ok(v) => v,
                Err(e) => {
                    let content = format!("参数解析失败: {}", e);
                    emitter.tool_result(&call.id, &call.name, &content, true);
                    let tool_msg = build_tool_msg(turn, call, content, true);
                    pending_persistence.push(tool_msg.clone());
                    conv_messages.push(tool_msg.clone());
                    continue;
                }
            };
            let ctx = crate::tools::ToolContext {
                approve_all_for_turn: approval.approve_all_for_turn.load(Ordering::Relaxed),
            };
            let result = tool.execute(args_value, ctx).await;
            let (content, is_error) = match &result {
                Ok(o) => (o.content.clone(), o.is_error),
                Err(e) => (format!("执行失败: {}", e), true),
            };
            info!("[send_message] turn {} tool '{}' 执行结果: is_error={}, content_len={}, id={}",
                turn, call.name, is_error, content.len(), call.id);
            if !is_error {
                turn_has_success = true;
            }
            emitter.tool_result(&call.id, &call.name, &content, is_error);
            info!("[send_message] turn {} tool '{}' tool_result 事件已发射 (id={})",
                turn, call.name, call.id);
            let tool_msg = build_tool_msg(turn, call, content, is_error);
            pending_persistence.push(tool_msg.clone());
            conv_messages.push(tool_msg.clone());
        }
        // ── 本轮总结:如果所有 tool 都失败了,累计连续失败计数 ──
        if out.tool_calls.is_empty() {
            // 没有 tool_call,谈不上失败
        } else if turn_has_success {
            consecutive_failed_turns = 0;
        } else {
            consecutive_failed_turns += 1;
            warn!(
                "[send_message] turn {} 全部 tool 执行失败 (连续 {}/{} 轮)",
                turn, consecutive_failed_turns, MAX_CONSECUTIVE_FAILED_TURNS
            );
            if consecutive_failed_turns >= MAX_CONSECUTIVE_FAILED_TURNS {
                break Err(crate::providers::ApiError::NetworkError(
                    format!(
                        "连续 {} 轮工具调用全部失败,已中断对话",
                        consecutive_failed_turns
                    ),
                ));
            }
        }
        // 继续下一轮:重新调 stream_chat(把 tool result 给 model)
    };

    // 成功时发射 done 事件（仅当 provider 内部未发射过 error 时）
    // done 事件从 provider 移到这里发射，保证 tool 循环中的多轮 stream_chat
    // 不会触发前端过早调用 handleStreamDone()
    match &outcome {
        Ok(out) if !out.had_stream_error => {
            emitter.done(StopReason::Stop, &out.full_text);
        }
        Ok(_) => {
            // provider 已发射过 error 事件，不再发 done
        }
        Err(e) => {
            warn!("[send_message] 最终错误: {}", e);
            let error_emitter = StreamEventEmitter::new(app.clone());
            // 若在 tool 循环中失败(turn>1) → 附加提示信息
            let error_msg = if turn > 1 {
                format!(
                    "{} (工具调用后请求失败, 可能是 Provider 不支持 tool 回传格式)",
                    e
                )
            } else {
                e.to_string()
            };
            error_emitter.error(StopReason::Error, &error_msg, "");
        }
    }

    // 不论正常完成、取消还是工具循环报错，均保存本轮已经生成的完整记录。
    flush_pending_messages(app.clone(), pending_persistence).await;

    // 清理:取消通道 + 重置 "本次都允许" 标志(P5 一次性,下次 send_message 重新开始)
    state.sender.lock().take();
    approval.approve_all_for_turn.store(false, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn stop_generation(state: State<'_, CancelState>) -> Result<(), String> {
    if let Some(sender) = state.sender.lock().as_ref() {
        info!("[stop_generation] 用户取消生成");
        sender.send(true).ok();
    }
    Ok(())
}

/// Tool 审批命令
///
/// 前端在用户点击"允许"/"拒绝"后调用。
/// - approve_all=true → 本轮剩余所有 write tool 都自动批准
/// - approve_all=false → 只批准这一个(approved=true)或拒绝这一个(approved=false)
///
/// 后端通过 oneshot::Sender 把结果推回 send_message 的 await 点。
#[tauri::command]
pub async fn approve_tool_call(
    approval: State<'_, ApprovalState>,
    id: String,
    approved: bool,
    approve_all: bool,
) -> Result<(), String> {
    info!(
        "[approve_tool_call] id={}, approved={}, approve_all={}",
        id, approved, approve_all
    );

    if approve_all && approved {
        // 设置"本次都允许"标志,后续 write tool 自动放行
        approval.approve_all_for_turn.store(true, Ordering::Relaxed);
    }

    // 找到对应 id 的 slot,取走它的 sender,推送结果
    let mut pending = approval.pending.lock();
    let idx = match pending.iter().position(|s| s.id == id) {
        Some(i) => i,
        None => {
            warn!("[approve_tool_call] 找不到 id={} 的 slot(可能已超时)", id);
            return Err(format!("approval slot {} not found", id));
        }
    };
    let mut slot = pending.remove(idx);
    if let Some(tx) = slot.tx.take() {
        // 忽略 send 错误:意味着 receiver 已被 drop(send_message 中途崩了)
        let _ = tx.send(approved);
    }
    Ok(())
}

/// 把 AskUserAnswer 格式化成给 model 看的 tool result 文本
///
/// 格式:
/// - 单选带输入:`User selected: 选其他文件\nUser input: /Users/me/foo.txt`
/// - 多选:`User selected:\n- 选项A (input: foo)\n- 选项B`
/// - 自定义:`User responded: <text>`
/// - 跳过:`User skipped the question`
fn format_ask_user_answer(options: &[AskUserOption], answer: &AskUserAnswer) -> String {
    // 优先用 selected 索引
    if !answer.selected.is_empty() {
        // 收集 (label, optional_input) 对
        let pairs: Vec<(String, String)> = answer
            .selected
            .iter()
            .enumerate()
            .filter_map(|(i, idx)| {
                options.get(*idx).map(|o| {
                    let input = answer.inputs.get(i).cloned().unwrap_or_default();
                    (o.label.clone(), input)
                })
            })
            .collect();

        if !pairs.is_empty() {
            // 单选 + 带输入
            if pairs.len() == 1 {
                let (label, input) = &pairs[0];
                if !input.trim().is_empty() {
                    return format!("User selected: {}\nUser input: {}", label, input.trim());
                }
                return format!("User selected: {}", label);
            }
            // 多选
            let lines: Vec<String> = pairs
                .iter()
                .map(|(label, input)| {
                    if !input.trim().is_empty() {
                        format!("- {} (input: {})", label, input.trim())
                    } else {
                        format!("- {}", label)
                    }
                })
                .collect();
            return format!("User selected:\n{}", lines.join("\n"));
        }
    }
    // 回退到自定义文本
    if let Some(custom) = &answer.custom {
        if !custom.trim().is_empty() {
            return format!("User responded: {}", custom.trim());
        }
    }
    "User skipped the question".to_string()
}

/// 用户回答 ask_user 的命令
///
/// 前端在 QuestionModal 中用户做出选择后调用。
/// 把 AskUserAnswer 通过 oneshot 推回 send_message 的 await 点。
#[tauri::command]
pub async fn answer_tool_question(
    question: State<'_, QuestionState>,
    id: String,
    selected: Vec<usize>,
    inputs: Option<Vec<String>>,
    custom: Option<String>,
) -> Result<(), String> {
    info!(
        "[answer_tool_question] id={}, selected={:?}, inputs={:?}, custom={:?}",
        id, selected, inputs, custom
    );

    let mut pending = question.pending.lock();
    let idx = match pending.iter().position(|s| s.id == id) {
        Some(i) => i,
        None => {
            warn!("[answer_tool_question] 找不到 id={} 的 slot(可能已超时)", id);
            return Err(format!("question slot {} not found", id));
        }
    };
    let mut slot = pending.remove(idx);
    if let Some(tx) = slot.tx.take() {
        let _ = tx.send(AskUserAnswer { selected, inputs: inputs.unwrap_or_default(), custom });
    }
    Ok(())
}

/// 获取应用配置命令
///
/// 从磁盘读取完整的 AppConfig 返回给前端。
#[tauri::command]
pub async fn get_config(app: tauri::AppHandle) -> Result<AppConfig, String> {
    storage::get_config(&app)
}

/// 保存应用配置命令
///
/// 在写入磁盘之前进行基本校验：
/// - 如果 selected_model_id 非空，确保该模型在 models 列表中存在
#[tauri::command]
pub async fn save_config(app: tauri::AppHandle, config: AppConfig) -> Result<(), String> {
    // 校验：selected_model_id 必须存在于 models 列表中
    if !config.selected_model_id.is_empty() {
        let model_exists = config
            .models
            .iter()
            .any(|m| m.id == config.selected_model_id);
        if !model_exists {
            return Err("选择的模型不在可用模型列表中".to_string());
        }
    }

    storage::save_config(&app, &config)?;

    // 热键配置变更后，重新注册全局快捷键
    crate::hotkey::update_hotkey(&app, &config.hotkey);

    Ok(())
}

/// 获取模型列表命令
///
/// 根据 Provider 类型调用对应适配器的 fetch_models。
/// context_window 通过已知模型映射表获取，未知模型默认 128000。
#[tauri::command]
pub async fn fetch_models(
    base_url: String,
    api_key: String,
    provider_type: Option<String>,
) -> Result<Vec<ModelInfo>, String> {
    let pt = ProviderType::from_str(&provider_type.unwrap_or_default());
    let llm_provider = providers::create_provider(&pt);
    llm_provider.fetch_models(&base_url, &api_key).await
}

/// 测试延迟命令
///
/// 测量指定模型端点的响应延迟，返回毫秒数。
#[tauri::command]
pub async fn test_latency(
    base_url: String,
    api_key: String,
    model_id: String,
    provider_type: Option<String>,
) -> Result<u32, String> {
    let pt = ProviderType::from_str(&provider_type.unwrap_or_default());
    let llm_provider = providers::create_provider(&pt);
    llm_provider.test_latency(&base_url, &api_key, &model_id).await
}

/// 加载历史消息命令
///
/// 分页加载历史消息，offset 为偏移量（跳过的消息数），limit 为最大返回数。
#[tauri::command]
pub async fn load_messages(
    app: tauri::AppHandle,
    offset: u64,
    limit: u64,
) -> Result<Vec<Message>, String> {
    storage::load_messages(&app, offset, limit)
}

/// 返回本地历史消息总数，供前端计算分页起点。
#[tauri::command]
pub async fn get_message_count(app: tauri::AppHandle) -> Result<u64, String> {
    storage::message_count(&app)
}

/// 保存消息命令
///
/// 将单条消息追加到本地存储分块文件中。
/// 若当前分块已满则自动创建新分块，并更新 manifest 计数。
#[tauri::command]
pub async fn save_message(app: tauri::AppHandle, message: Message) -> Result<(), String> {
    storage::append_message(&app, &message).map_err(|e| format!("保存消息失败: {}", e))
}

// ── 测试 ──

#[cfg(test)]
mod tests {
    use super::*;

    fn make_msg(content: &str) -> Message {
        Message {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::User,
            content: content.to_string(),
            blocks: None,
            model_id: None,
            created_at: 0,
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
            is_error: None,
            parent_message_id: None,
        }
    }

    // ── estimate_tokens ──

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_single_char() {
        assert_eq!(estimate_tokens("a"), 1); // 1/3=0, .max(1)=1
        assert_eq!(estimate_tokens("中"), 1);
    }

    #[test]
    fn test_estimate_tokens_english() {
        // "Hello world" = 11 chars → 11/3 = 3
        assert_eq!(estimate_tokens("Hello world"), 3);
        // 30 chars → 10 tokens
        assert_eq!(estimate_tokens("abcdefghijklmnopqrstuvwxyzabcd"), 10);
    }

    #[test]
    fn test_estimate_tokens_chinese() {
        // "你好世界" = 4 chars → 4/3 = 1
        assert_eq!(estimate_tokens("你好世界"), 1);
        // 12 Chinese chars → 4 tokens
        assert_eq!(estimate_tokens("这是一段比较长的中文文本内容"), 4);
    }

    // ── compute_window_start ──

    #[test]
    fn test_window_empty() {
        assert_eq!(compute_window_start(&[], 1000), 0);
    }

    #[test]
    fn test_window_all_fit() {
        let msgs = vec![make_msg("hi"), make_msg("hello"), make_msg("hey")];
        // 3 messages × ~1 token each → well within 100 budget
        assert_eq!(compute_window_start(&msgs, 100), 0);
    }

    #[test]
    fn test_window_trims_from_front() {
        let msgs: Vec<Message> = (0..12).map(|i| make_msg(&format!("msg{}", i))).collect();
        // Each msg: "msg0"=4 chars → 1 token. 12 msgs = ~12 tokens.
        // Budget of 5 → should keep ~5 most recent messages.
        let start = compute_window_start(&msgs, 5);
        assert!(start > 0, "should have dropped some messages");
        assert!(start <= 7, "should keep at least 5 messages");
        // Verify last message is always included
        assert!(start < msgs.len());
    }

    #[test]
    fn test_window_keeps_last_message_when_over_budget() {
        let msgs = vec![
            make_msg("short"),
            make_msg("a very long message that exceeds the budget all by itself"),
        ];
        // Budget is tiny, even the last message exceeds it
        let start = compute_window_start(&msgs, 1);
        // Should keep the last message (index 1)
        assert_eq!(start, 1);
    }

    #[test]
    fn test_window_budget_zero() {
        let msgs = vec![make_msg("a"), make_msg("b"), make_msg("c")];
        let start = compute_window_start(&msgs, 0);
        // Budget zero → only the last message is kept
        assert_eq!(start, msgs.len() - 1);
    }

    #[test]
    fn test_window_single_message() {
        let msgs = vec![make_msg("solo")];
        assert_eq!(compute_window_start(&msgs, 1), 0);
        assert_eq!(compute_window_start(&msgs, 0), 0); // kept anyway
    }
}
