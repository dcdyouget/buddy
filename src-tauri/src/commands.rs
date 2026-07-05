// IPC 命令处理模块：定义所有前端可调用的 Tauri 命令
//
// 每个 #[tauri::command] 函数对应一个前端 invoke 调用点，
// 负责参数校验、调用业务逻辑、返回结果或错误。

use crate::models::*;
use crate::providers::{self, ProviderType};
use crate::streaming::{ContentBlock, StopReason, StreamEventEmitter};
use crate::storage;
use log::{info, warn};
use std::sync::Mutex;
use tauri::State;
use tokio::sync::watch;

/// 取消状态：跨命令共享的取消通道
///
/// 使用 watch channel 实现：send_message 创建发送端，
/// stop_generation 通过发送端发出取消信号。
/// Mutex 包裹保证线程安全（Tauri 命令可能在不同线程执行）。
pub struct CancelState {
    pub sender: Mutex<Option<watch::Sender<bool>>>,
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

    // 持久化用户消息（messages 最后一条是新发送的用户消息）
    if let Some(user_msg) = messages.last() {
        let msg = user_msg.clone();
        let app_handle = app.clone();
        tokio::task::spawn_blocking(move || {
            storage::append_message(&app_handle, &msg).ok();
        });
    }

    // 创建取消通道
    let (cancel_tx, cancel_rx) = watch::channel(false);
    *state.sender.lock().unwrap() = Some(cancel_tx);

    // 创建统一事件发射器
    let emitter = StreamEventEmitter::new(app.clone());

    // 根据 Provider 类型创建对应的适配器
    let llm_provider = providers::create_provider(&provider_type);

    // 获取 compat 配置引用
    let compat = provider.compat.as_ref();

    // 发起流式请求
    match llm_provider
        .stream_chat(
            &provider.base_url,
            &provider.api_key,
            &model_id,
            windowed_messages,
            &emitter,
            cancel_rx,
            compat,
        )
        .await
    {
        Ok(full_response) => {
            info!(
                "[send_message] 流式结束，回复长度={} chars",
                full_response.len()
            );
            if !full_response.is_empty() {
                // 解析 <think> 标签为结构化 blocks
                let blocks = ContentBlock::parse_from_text(&full_response);
                let assistant_msg = Message {
                    id: format!("a-{}", chrono::Utc::now().timestamp_millis()),
                    role: MessageRole::Assistant,
                    content: full_response,
                    blocks: Some(blocks),
                    model_id: Some(model_id),
                    created_at: chrono::Utc::now().timestamp() as u64,
                };
                let app_handle = app.clone();
                tokio::task::spawn_blocking(move || {
                    storage::append_message(&app_handle, &assistant_msg).ok();
                });
            }
        }
        Err(e) => {
            warn!("[send_message] 请求失败: {}", e);
            // 使用统一事件发射器发送错误（前端监听 stream-event）
            // 当前所有 ApiError 变体均映射为 StopReason::Error —— 类型穷举性在编译期保证
            let error_emitter = StreamEventEmitter::new(app.clone());
            let msg = e.to_string();
            error_emitter.error(StopReason::Error, &msg, "");
        }
    }

    // 清理取消通道
    state.sender.lock().unwrap().take();
    Ok(())
}

/// 停止生成命令
///
/// 设置取消信号为 true，触发 stream_chat 中的 cancel_rx 检测，
/// 从而实现中途停止 AI 回复。
#[tauri::command]
pub async fn stop_generation(state: State<'_, CancelState>) -> Result<(), String> {
    if let Some(sender) = state.sender.lock().unwrap().as_ref() {
        info!("[stop_generation] 用户取消生成");
        sender.send(true).ok();
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
