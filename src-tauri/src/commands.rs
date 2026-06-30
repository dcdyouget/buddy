// IPC 命令处理模块：定义所有前端可调用的 Tauri 命令
//
// 每个 #[tauri::command] 函数对应一个前端 invoke 调用点，
// 负责参数校验、调用业务逻辑、返回结果或错误。

use crate::api;
use crate::models::*;
use crate::storage;
use log::{info, warn};
use std::sync::Mutex;
use tauri::Emitter;
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

/// 发送消息命令
///
/// 前端调用此命令发起 AI 对话。后端自动处理持久化：
/// 1. 查找模型和 Provider 配置
/// 2. 持久化用户消息到本地存储
/// 3. 创建取消通道并存入共享状态
/// 4. 调用 api::stream_chat 进行流式对话（token 实时推送到前端）
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

    info!(
        "[send_message] model={}, provider={}, history_len={}",
        model_id,
        provider.id,
        messages.len()
    );

    // 持久化用户消息（messages 最后一条是新发送的用户消息）
    // 使用 spawn_blocking 避免同步 I/O 阻塞异步运行时线程
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

    // 发起流式请求，返回累积的完整 AI 回复
    match api::stream_chat(
        &provider.base_url,
        &provider.api_key,
        &model_id,
        &messages,
        &app,
        cancel_rx,
    )
    .await
    {
        Ok(full_response) => {
            // 流式完成（含正常结束、取消、流中断）：持久化 AI 回复
            info!(
                "[send_message] 流式结束，回复长度={} chars",
                full_response.len()
            );
            if !full_response.is_empty() {
                let assistant_msg = Message {
                    id: format!("a-{}", chrono::Utc::now().timestamp_millis()),
                    role: MessageRole::Assistant,
                    content: full_response,
                    model_id: Some(model_id),
                    created_at: chrono::Utc::now().timestamp() as u64,
                };
                // 使用 spawn_blocking 避免同步 I/O 阻塞异步线程
                let app_handle = app.clone();
                tokio::task::spawn_blocking(move || {
                    storage::append_message(&app_handle, &assistant_msg).ok();
                });
            }
        }
        Err(e) => {
            // 请求级别错误（401/429/网络等，流尚未开始）：发送错误事件
            warn!("[send_message] 请求失败: {}", e);
            let err_str = e.to_string();
            match e {
                api::ApiError::Unauthorized => {
                    let _ = app.emit("stream-error", "401");
                }
                api::ApiError::QuotaExceeded => {
                    let _ = app.emit("stream-error", "429");
                }
                _ => {
                    let _ = app.emit("stream-error", err_str);
                }
            }
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
        sender.send(true).ok(); // 发送取消信号，忽略无接收者的情况
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
/// 调用 api::fetch_models 获取原始数据，转换为内部 ModelInfo 结构。
/// 注意：context_window 使用默认值 128000，latency_ms 初始为 None。
#[tauri::command]
pub async fn fetch_models(base_url: String, api_key: String) -> Result<Vec<ModelInfo>, String> {
    let raw_models = api::fetch_models(&base_url, &api_key).await?;

    let models: Vec<ModelInfo> = raw_models
        .iter()
        .filter_map(|m| {
            let id = m["id"].as_str()?; // 跳过无 id 字段的模型
            Some(ModelInfo {
                id: id.to_string(),
                provider_id: String::new(), // 由前端填写所属 provider_id
                display_name: id.to_string(),
                context_window: 128000,  // 默认上下文窗口
                latency_ms: None,        // 尚未测速
            })
        })
        .collect();

    Ok(models)
}

/// 测试延迟命令
///
/// 测量指定模型端点的响应延迟，返回毫秒数。
#[tauri::command]
pub async fn test_latency(
    base_url: String,
    api_key: String,
    model_id: String,
) -> Result<u32, String> {
    api::test_latency(&base_url, &api_key, &model_id).await
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
pub async fn save_message(
    app: tauri::AppHandle,
    message: Message,
) -> Result<(), String> {
    storage::append_message(&app, &message).map_err(|e| format!("保存消息失败: {}", e))
}
