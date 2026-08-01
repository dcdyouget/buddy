// IPC 命令处理模块：定义所有前端可调用的 Tauri 命令
//
// 每个 #[tauri::command] 函数对应一个前端 invoke 调用点，
// 负责参数校验、调用业务逻辑、返回结果或错误。

use crate::models::*;
use crate::providers::{self, ProviderType};
use crate::storage;
use crate::streaming::{ContentBlock, QuestionOption, StopReason, StreamEventEmitter};
use crate::tools::{AskUserAnswer, AskUserArgs, AskUserOption, ToolRegistry};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use log::{info, warn};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;
use tauri::{Manager, State};
use tokio::sync::{oneshot, watch};

/// 消息 ID 序列号：同一毫秒内会生成多条消息（工具循环的多条 tool 消息/assistant 分段），
/// 只靠毫秒时间戳会撞 ID，导致前端 React key 冲突。
static MESSAGE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// 生成带全局序列号的消息 ID，保证同毫秒内也不重复。
fn unique_message_id(prefix: &str, turn: usize) -> String {
    format!(
        "{}-{}-{}-{}",
        prefix,
        turn,
        chrono::Utc::now().timestamp_millis(),
        MESSAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}
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
//   2. 发射 ToolQuestionRequired 事件(让前端显示内联问答卡)
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

fn estimate_message_tokens(message: &Message) -> u32 {
    // 图片实际 token 数取决于厂商、尺寸和 detail；这里使用保守固定值，
    // 仅用于本地滑动窗口，避免带图对话被当作零成本。
    estimate_tokens(&message.content)
        .saturating_add((message.images.len() as u32).saturating_mul(1_024))
}

fn validate_image_attachments(images: &[crate::models::ImageAttachment]) -> Result<(), String> {
    const MAX_IMAGES: usize = 4;
    const MAX_DATA_URL_BYTES: usize = 7 * 1024 * 1024;
    const SUPPORTED_MEDIA_TYPES: [&str; 4] = ["image/jpeg", "image/png", "image/gif", "image/webp"];

    if images.len() > MAX_IMAGES {
        return Err(format!("每条消息最多添加 {MAX_IMAGES} 张图片"));
    }
    for image in images {
        if !SUPPORTED_MEDIA_TYPES.contains(&image.media_type.as_str()) {
            return Err(format!("不支持的图片格式：{}", image.media_type));
        }
        if image.path.is_empty() && image.data_url.is_empty() {
            return Err(format!("图片缺少本地路径：{}", image.name));
        }
        if !image.data_url.is_empty() {
            let expected_prefix = format!("data:{};base64,", image.media_type);
            if !image.data_url.starts_with(&expected_prefix) {
                return Err(format!("图片数据格式无效：{}", image.name));
            }
            if image.data_url.len() > MAX_DATA_URL_BYTES {
                return Err(format!("图片超过 5 MB 限制：{}", image.name));
            }
        }
    }
    Ok(())
}

const MAX_GENERATED_IMAGE_BYTES: usize = 25 * 1024 * 1024;

fn image_extension(media_type: &str) -> Option<&'static str> {
    match media_type {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

fn unique_download_path(download_dir: &Path, extension: &str) -> PathBuf {
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let base_name = format!("Buddy-生成图片-{timestamp}");
    let mut path = download_dir.join(format!("{base_name}.{extension}"));
    let mut suffix = 2_u16;
    while path.exists() {
        path = download_dir.join(format!("{base_name}-{suffix}.{extension}"));
        suffix += 1;
    }
    path
}

async fn generated_image_bytes(data_url: &str, media_type: &str) -> Result<Vec<u8>, String> {
    let expected_prefix = format!("data:{media_type};base64,");
    if let Some(encoded) = data_url.strip_prefix(&expected_prefix) {
        // 先按 Base64 编码长度粗略预检，避免对超大 payload 做完整解码
        // （base64：3 字节 → 4 字符；+4 容忍填充与舍入）
        if encoded.len() > (MAX_GENERATED_IMAGE_BYTES / 3) * 4 + 4 {
            return Err("生成图片超过 25 MB，无法下载".to_string());
        }
        let bytes = BASE64_STANDARD
            .decode(encoded)
            .map_err(|_| "生成图片的 base64 数据无效".to_string())?;
        if bytes.len() > MAX_GENERATED_IMAGE_BYTES {
            return Err("生成图片超过 25 MB，无法下载".to_string());
        }
        return Ok(bytes);
    }

    let url = reqwest::Url::parse(data_url).map_err(|_| "生成图片地址无效".to_string())?;
    if url.scheme() != "https" {
        return Err("只允许下载 HTTPS 图片地址".to_string());
    }
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|error| format!("创建图片下载客户端失败：{error}"))?
        .get(url)
        .send()
        .await
        .map_err(|error| format!("下载生成图片失败：{error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "下载生成图片失败：HTTP {}",
            response.status().as_u16()
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_GENERATED_IMAGE_BYTES as u64)
    {
        return Err("生成图片超过 25 MB，无法下载".to_string());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("读取生成图片失败：{error}"))?;
    if bytes.len() > MAX_GENERATED_IMAGE_BYTES {
        return Err("生成图片超过 25 MB，无法下载".to_string());
    }
    Ok(bytes.to_vec())
}

fn stored_image_bytes(
    app: &tauri::AppHandle,
    image: &crate::models::ImageAttachment,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    let attachments_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法获取图片目录：{error}"))?
        .join("attachments");
    let canonical_root = attachments_dir
        .canonicalize()
        .map_err(|_| "图片附件目录不存在".to_string())?;
    let path = PathBuf::from(&image.path);
    let canonical_path = path
        .canonicalize()
        .map_err(|_| format!("图片已删除：{}", image.path))?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err("图片路径不在 Buddy 附件目录中".to_string());
    }
    let metadata =
        std::fs::metadata(&canonical_path).map_err(|_| format!("图片已删除：{}", image.path))?;
    if metadata.len() > max_bytes as u64 {
        return Err(format!("图片文件过大：{}", image.name));
    }
    std::fs::read(&canonical_path).map_err(|error| format!("读取图片失败：{error}"))
}

fn hydrate_image_for_provider(
    app: &tauri::AppHandle,
    image: &mut crate::models::ImageAttachment,
) -> Result<(), String> {
    if !image.data_url.is_empty() {
        return Ok(());
    }
    let bytes = stored_image_bytes(app, image, 5 * 1024 * 1024)?;
    image.data_url = format!(
        "data:{};base64,{}",
        image.media_type,
        BASE64_STANDARD.encode(bytes)
    );
    Ok(())
}

/// Provider API 是无状态的，但无需在每次新提问时重复上传所有历史图片。
/// 当前消息的图片会临时读取为 Base64；历史图片只保留其之前产生的文本上下文。
fn prepare_images_for_provider(
    app: &tauri::AppHandle,
    messages: &mut [Message],
) -> Result<(), String> {
    let current_user_index = messages
        .iter()
        .rposition(|message| message.role == MessageRole::User);
    for (index, message) in messages.iter_mut().enumerate() {
        if message.role != MessageRole::User || message.images.is_empty() {
            continue;
        }
        if Some(index) == current_user_index {
            for image in &mut message.images {
                hydrate_image_for_provider(app, image)?;
            }
        } else {
            message.images.clear();
        }
    }
    Ok(())
}

/// 将生图工具产出的图片持久化为本地附件。
///
/// 返回 (已存储的图片, 失败的张数)。失败的图片不再静默丢弃——
/// 调用方据此把工具结果降级为错误，避免「UI 显示完成但图丢失」。
async fn persist_tool_images(
    app: &tauri::AppHandle,
    images: Vec<crate::models::ImageAttachment>,
) -> (Vec<crate::models::ImageAttachment>, usize) {
    let mut stored = Vec::with_capacity(images.len());
    let mut failed = 0usize;
    for image in images {
        if !image.path.is_empty() {
            stored.push(image);
            continue;
        }
        match generated_image_bytes(&image.data_url, &image.media_type).await {
            Ok(bytes) if !bytes.is_empty() => match storage::store_image_bytes(
                app,
                &image.name,
                &image.media_type,
                &bytes,
                "generated",
            ) {
                Ok(attachment) => stored.push(attachment),
                Err(error) => {
                    failed += 1;
                    warn!("保存生成图片失败：{}", error);
                }
            },
            Ok(_) => {
                failed += 1;
                warn!("生成图片内容为空：{}", image.name);
            }
            Err(error) => {
                warn!("缓存生成图片失败：{}", error);
                // HTTPS 地址本身很小，可作为兼容兜底；Base64 绝不再写入聊天记录。
                if image.data_url.starts_with("https://") {
                    stored.push(image);
                } else {
                    failed += 1;
                }
            }
        }
    }
    (stored, failed)
}

/// 将用户选择的图片复制到 Buddy 应用数据目录。
#[tauri::command]
pub async fn save_chat_image(
    app: tauri::AppHandle,
    name: String,
    media_type: String,
    data_url: String,
) -> Result<crate::models::ImageAttachment, String> {
    storage::store_image_data_url(
        &app,
        &name,
        &media_type,
        &data_url,
        5 * 1024 * 1024,
        "upload",
    )
}

/// 删除已保存的聊天图片附件（用户从输入框移除未发送的图片时调用，
/// 清理已写盘的孤儿文件；仅限应用数据目录内的附件文件）。
#[tauri::command]
pub async fn delete_chat_image(app: tauri::AppHandle, path: String) -> Result<bool, String> {
    storage::delete_attachment_file(&app, &path)
}

/// 将生图工具的结果保存到系统下载目录。
#[tauri::command]
pub async fn download_generated_image(
    app: tauri::AppHandle,
    image: crate::models::ImageAttachment,
) -> Result<String, String> {
    let extension = image_extension(&image.media_type)
        .ok_or_else(|| format!("不支持的图片格式：{}", image.media_type))?;
    let bytes = if image.path.is_empty() {
        generated_image_bytes(&image.data_url, &image.media_type).await?
    } else {
        stored_image_bytes(&app, &image, MAX_GENERATED_IMAGE_BYTES)?
    };
    if bytes.is_empty() {
        return Err("生成图片内容为空".to_string());
    }

    let download_dir = app
        .path()
        .download_dir()
        .map_err(|error| format!("无法获取系统下载目录：{error}"))?;
    tokio::fs::create_dir_all(&download_dir)
        .await
        .map_err(|error| format!("无法创建下载目录：{error}"))?;
    let target = unique_download_path(&download_dir, extension);
    tokio::fs::write(&target, bytes)
        .await
        .map_err(|error| format!("保存图片失败：{error}"))?;

    Ok(target.to_string_lossy().into_owned())
}

/// 估算消息列表的总 token 数
fn estimate_total_tokens(messages: &[Message]) -> u32 {
    messages.iter().map(estimate_message_tokens).sum()
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
        let cost = estimate_message_tokens(&messages[i]);
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
fn build_tool_msg(
    turn: usize,
    call: &crate::models::ToolCall,
    content: String,
    images: Vec<crate::models::ImageAttachment>,
    is_error: bool,
) -> Message {
    Message {
        id: unique_message_id("t", turn),
        role: MessageRole::Tool,
        content,
        images,
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
    let question_started = Instant::now();
    let question_log_id = messages
        .iter()
        .rev()
        .find(|message| message.role == MessageRole::User)
        .map(|message| message.id.clone())
        .unwrap_or_else(|| unique_message_id("q", 0));

    // 从配置中查找模型对应的 Provider
    let config = storage::get_config(&app)?;
    let model = config
        .models
        .iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| "未找到指定的模型".to_string())?;

    let current_user_images = messages
        .iter()
        .rev()
        .find(|message| message.role == MessageRole::User)
        .map(|message| message.images.as_slice())
        .unwrap_or_default();
    validate_image_attachments(current_user_images)?;
    if !current_user_images.is_empty() && !model.supports_vision {
        return Err("当前模型未开启图片输入，请在模型设置中启用“支持图片”".to_string());
    }

    let provider = config
        .providers
        .iter()
        .find(|p| p.id == model.provider_id)
        .ok_or_else(|| "未找到对应的 Provider".to_string())?;

    let provider_type = ProviderType::from_str(&provider.provider_type);

    info!(
        "[llm][{}] 问题开始: model={}, provider={}, type={:?}, history_len={}, estimated_input_tokens={}, context_window={}",
        question_log_id,
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

    // 防止并发 send_message：若有生成任务在跑，拒绝新的，避免第二个任务覆盖
    // 第一个任务的取消通道，导致 stop_generation 对第一个任务失效。
    if state.sender.lock().is_some() {
        return Err("已有生成任务正在进行中".to_string());
    }

    // 创建取消通道
    let (cancel_tx, mut cancel_rx) = watch::channel(false);
    *state.sender.lock() = Some(cancel_tx);

    // 创建统一事件发射器
    let emitter = StreamEventEmitter::new(app.clone());

    // 根据 Provider 类型创建对应的适配器
    let llm_provider = providers::create_provider(&provider_type);
    let compat = provider.compat.as_ref();

    // ── P4: 构造 ToolRegistry(只包含内置 tool,MCP tool 由 P7 注入) ──
    let mut builtin_tools = crate::tools::builtin::builtin_tools(config.allowed_paths.clone());
    if provider_type == ProviderType::OpenAICompatible && model.supports_image_generation {
        if let Some(tool) = crate::tools::image_generation::GenerateImageTool::for_provider(
            provider.base_url.clone(),
            provider.api_key.clone(),
            model_id.clone(),
            &provider.id,
            &provider.name,
        ) {
            builtin_tools.push(std::sync::Arc::new(tool));
        }
    }
    let registry = ToolRegistry::new(builtin_tools);
    // P7 会在此追加 mcp tool

    // 把 messages 拷成可变的 Vec,tool 循环会往里 push assistant(tool_calls) + tool(result)
    let mut conv_messages: Vec<Message> = messages.clone();
    if !model.supports_vision {
        // 切换到纯文本模型后不再把历史图片发送给 Provider。
        for message in &mut conv_messages {
            message.images.clear();
        }
    } else {
        // 图片预处理失败：不要用 `?` 直接退出——那样会泄漏取消通道、
        // 丢失用户消息，且前端收不到任何流事件提示。
        if let Err(error) = prepare_images_for_provider(&app, &mut conv_messages) {
            warn!("[send_message] 图片预处理失败: {}", error);
            state.sender.lock().take();
            flush_pending_messages(app.clone(), pending_persistence).await;
            emitter.error(StopReason::Error, &error, "");
            return Ok(());
        }
    }

    // 每次 send_message 开始时重置"本次都允许"标志，防止上次被中断时残留
    approval
        .approve_all_for_turn
        .store(false, Ordering::Relaxed);

    // 工具循环上限(防止 model 死循环):
    // - 硬上限 20 轮(安全网,正常情况下不会触发)
    // - 软上限:连续 3 轮全部 tool 执行失败则中断,避免 model 反复失败同一操作
    const MAX_TOOL_TURNS: usize = 20;
    const MAX_CONSECUTIVE_FAILED_TURNS: usize = 3;
    let mut turn: usize = 0;
    let mut consecutive_failed_turns: usize = 0;
    let mut api_requests = 0usize;
    let mut total_api_ms = 0u128;
    let mut total_answer_bytes = 0usize;
    let mut total_answer_chars = 0usize;
    let mut total_thinking_bytes = 0usize;
    let mut total_thinking_chars = 0usize;
    let mut total_tool_calls = 0usize;

    let outcome: Result<crate::streaming::StreamOutcome, _> = loop {
        turn += 1;
        if turn > MAX_TOOL_TURNS {
            warn!("[send_message] 达到 tool 轮数硬上限 {}", MAX_TOOL_TURNS);
            break Err(crate::providers::ApiError::NetworkError(format!(
                "已达到最大工具调用轮数 {}",
                MAX_TOOL_TURNS
            )));
        }

        // 滑动窗口(每轮都算)
        let budget = ((model.context_window as f64) * 0.7_f64) as u32;
        let start_idx = compute_window_start(&conv_messages, budget);
        let windowed = &conv_messages[start_idx..];

        let tools = registry.all_definitions();
        let request_id = format!("{}#{}", question_log_id, turn);
        api_requests += 1;
        let api_started = Instant::now();
        let outcome = llm_provider
            .stream_chat(
                &provider.base_url,
                &provider.api_key,
                &model_id,
                &request_id,
                windowed,
                &emitter,
                cancel_rx.clone(),
                compat,
                &tools,
            )
            .await;
        total_api_ms += api_started.elapsed().as_millis();

        let out = match outcome {
            Ok(o) => o,
            Err(e) => break Err(e),
        };
        total_answer_bytes += out.full_text.len();
        total_answer_chars += out.full_text.chars().count();
        total_thinking_bytes += out.thinking_text.len();
        total_thinking_chars += out.thinking_text.chars().count();
        total_tool_calls += out.tool_calls.len();

        // 持久化本轮 assistant 消息
        if !out.full_text.is_empty() || !out.tool_calls.is_empty() || !out.thinking_text.is_empty()
        {
            // 合并思考 + 文本为 blocks
            // - thinking_text 非空时:显式构造 thinking + text 块
            // - thinking_text 为空时:尝试从 full_text 解析 <think> 标签(部分模型把思考包在 text 里)
            let blocks = if !out.thinking_text.is_empty() {
                let mut b = vec![ContentBlock::Thinking {
                    content: out.thinking_text.clone(),
                    is_open: false,
                }];
                if !out.full_text.is_empty() {
                    b.push(ContentBlock::Text {
                        content: out.full_text.clone(),
                    });
                }
                b
            } else {
                ContentBlock::parse_from_text(&out.full_text)
            };
            let assistant_msg = Message {
                id: unique_message_id("a", turn),
                role: MessageRole::Assistant,
                content: out.full_text.clone(),
                images: Vec::new(),
                blocks: Some(blocks),
                model_id: Some(model_id.clone()),
                created_at: chrono::Utc::now().timestamp() as u64,
                tool_calls: if out.tool_calls.is_empty() {
                    None
                } else {
                    Some(out.tool_calls.clone())
                },
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
        info!(
            "[send_message] turn {} 收到 {} 个 tool_call,开始执行",
            turn,
            out.tool_calls.len()
        );
        // 本轮追踪:是否有至少一个 tool 执行成功(非 is_error)?
        let mut turn_has_success = false;
        for call in &out.tool_calls {
            // 用户点击"停止"后，不再执行本轮剩余的工具调用
            if *cancel_rx.borrow() {
                info!("[send_message] turn {} 工具循环被用户取消, 跳过剩余 {} 个调用", turn, out.tool_calls.len());
                break;
            }
            // ── ask_user 特殊分支:不进入普通 tool.execute,而是显示内联问答卡等回答 ──
            if call.name == "ask_user" {
                let args_value: serde_json::Value = match serde_json::from_str(&call.arguments) {
                    Ok(v) => v,
                    Err(e) => {
                        let content = format!("ask_user 参数解析失败: {}", e);
                        emitter.tool_result(&call.id, "ask_user", &content, Vec::new(), true);
                        let tool_msg = build_tool_msg(turn, call, content, Vec::new(), true);
                        pending_persistence.push(tool_msg.clone());
                        conv_messages.push(tool_msg.clone());
                        continue;
                    }
                };
                let parsed: AskUserArgs = match serde_json::from_value(args_value) {
                    Ok(a) => a,
                    Err(e) => {
                        let content = format!("ask_user 参数校验失败: {}", e);
                        emitter.tool_result(&call.id, "ask_user", &content, Vec::new(), true);
                        let tool_msg = build_tool_msg(turn, call, content, Vec::new(), true);
                        pending_persistence.push(tool_msg.clone());
                        conv_messages.push(tool_msg.clone());
                        continue;
                    }
                };

                // 先登记等待槽位，再通知前端显示内联问答卡。
                // 避免用户快速作答时 answer_tool_question 尚找不到对应 slot。
                let q_options: Vec<QuestionOption> = parsed
                    .options
                    .iter()
                    .map(|o| QuestionOption {
                        label: o.label.clone(),
                        description: o.description.clone(),
                        requires_input: o.requires_input,
                        input_placeholder: o.input_placeholder.clone(),
                    })
                    .collect();
                let (tx, rx) = oneshot::channel::<AskUserAnswer>();
                let q_state = app.state::<QuestionState>();
                q_state.pending.lock().push(QuestionSlot {
                    id: call.id.clone(),
                    name: "ask_user".to_string(),
                    arguments: call.arguments.clone(),
                    options: parsed.options.clone(),
                    tx: Some(tx),
                });
                emitter.tool_question_required(
                    &call.id,
                    "ask_user",
                    &parsed.question,
                    q_options,
                    parsed.multi_select,
                    &parsed.header,
                );

                // 阻塞等待用户回答
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
                emitter.tool_result(&call.id, "ask_user", &content, Vec::new(), false);
                let tool_msg = build_tool_msg(turn, call, content, Vec::new(), false);
                pending_persistence.push(tool_msg.clone());
                conv_messages.push(tool_msg.clone());
                continue;
            }

            let tool = match registry.get(&call.name) {
                Some(t) => t,
                None => {
                    let content = format!("tool '{}' 未在 ToolRegistry 中注册", call.name);
                    emitter.tool_result(&call.id, &call.name, &content, Vec::new(), true);
                    let tool_msg = build_tool_msg(turn, call, content, Vec::new(), true);
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
                        // User cancelled during approval wait — remove the slot from pending.
                        // position + remove 必须在同一持锁作用域内完成：并发 approve_tool_call
                        // 可能在两次 lock 之间改动 Vec，导致 idx 失效（误删其他 slot 或越界 panic）。
                        let removed = {
                            let mut pending = approval.pending.lock();
                            if let Some(idx) = pending.iter().position(|s| s.id == call.id) {
                                pending.remove(idx);
                                true
                            } else {
                                false
                            }
                        };
                        if removed {
                            warn!("[send_message] 审批被用户取消, 已移除 pending slot id={}", call.id);
                        } else {
                            warn!("[send_message] 审批取消时找不到 slot id={} (可能已处理)", call.id);
                        }
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
                    emitter.tool_result(&call.id, &call.name, &content, Vec::new(), true);
                    let tool_msg = build_tool_msg(turn, call, content, Vec::new(), true);
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
                    emitter.tool_result(&call.id, &call.name, &content, Vec::new(), true);
                    let tool_msg = build_tool_msg(turn, call, content, Vec::new(), true);
                    pending_persistence.push(tool_msg.clone());
                    conv_messages.push(tool_msg.clone());
                    continue;
                }
            };
            let ctx = crate::tools::ToolContext {
                approve_all_for_turn: approval.approve_all_for_turn.load(Ordering::Relaxed),
                cancel_rx: Some(cancel_rx.clone()),
            };
            let result = tool.execute(args_value, ctx).await;
            let (mut content, raw_images, mut is_error) = match &result {
                Ok(o) => (o.content.clone(), o.images.clone(), o.is_error),
                Err(e) if call.name == "generate_image" => (
                    format!(
                        "执行失败: {}。生图工具已经完成内部重试，请不要在本轮再次调用 generate_image，直接向用户说明失败原因。",
                        e
                    ),
                    Vec::new(),
                    true,
                ),
                Err(e) => (format!("执行失败: {}", e), Vec::new(), true),
            };
            let (images, failed_image_count) = persist_tool_images(&app, raw_images).await;
            if failed_image_count > 0 {
                // 有图片落盘失败：不能再声称"图片生成完成"。
                // 降级为错误并明确告知模型，避免它因"不要重复生成"而丢失整组图片。
                is_error = true;
                if !content.is_empty() {
                    content.push('\n');
                }
                content.push_str(&format!(
                    "[图片保存失败] 有 {} 张生成的图片未能保存到本地，请直接向用户说明失败原因并建议重新生成。",
                    failed_image_count
                ));
            }
            info!(
                "[send_message] turn {} tool '{}' 执行结果: is_error={}, content_len={}, id={}",
                turn,
                call.name,
                is_error,
                content.len(),
                call.id
            );
            if !is_error {
                turn_has_success = true;
            }
            emitter.tool_result(&call.id, &call.name, &content, images.clone(), is_error);
            info!(
                "[send_message] turn {} tool '{}' tool_result 事件已发射 (id={})",
                turn, call.name, call.id
            );
            let tool_msg = build_tool_msg(turn, call, content, images, is_error);
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
                break Err(crate::providers::ApiError::NetworkError(format!(
                    "连续 {} 轮工具调用全部失败,已中断对话",
                    consecutive_failed_turns
                )));
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

    let final_status = match &outcome {
        Ok(out) if out.had_stream_error => "partial_or_aborted",
        Ok(_) => "success",
        Err(_) => "error",
    };
    info!(
        "[llm][{}] 问题总结: status={}, total_ms={}, api_ms={}, api_requests={}, answer_bytes={}, answer_chars={}, thinking_bytes={}, thinking_chars={}, tool_calls={}",
        question_log_id,
        final_status,
        question_started.elapsed().as_millis(),
        total_api_ms,
        api_requests,
        total_answer_bytes,
        total_answer_chars,
        total_thinking_bytes,
        total_thinking_chars,
        total_tool_calls,
    );

    // 不论正常完成、取消还是工具循环报错，均保存本轮已经生成的完整记录。
    flush_pending_messages(app.clone(), pending_persistence).await;

    // 清理:取消通道 + 重置 "本次都允许" 标志(P5 一次性,下次 send_message 重新开始)
    state.sender.lock().take();
    approval
        .approve_all_for_turn
        .store(false, Ordering::Relaxed);
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
    // 同时存在已选选项和自定义文本时，两者都要呈现给模型
    // （用户可能既勾选了选项又补充了自定义回答，不能静默丢弃任何一部分）。
    let custom_suffix = answer
        .custom
        .as_deref()
        .map(str::trim)
        .filter(|custom| !custom.is_empty())
        .map(|custom| format!("\nUser also responded: {}", custom))
        .unwrap_or_default();
    let append_custom = |main: String| -> String {
        if custom_suffix.is_empty() {
            main
        } else {
            format!("{main}{custom_suffix}")
        }
    };

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
                let base = if !input.trim().is_empty() {
                    format!("User selected: {}\nUser input: {}", label, input.trim())
                } else {
                    format!("User selected: {}", label)
                };
                return append_custom(base);
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
            return append_custom(format!("User selected:\n{}", lines.join("\n")));
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
/// 前端用户在内联 AskUserCard 中做出选择后调用。
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
            warn!(
                "[answer_tool_question] 找不到 id={} 的 slot(可能已超时)",
                id
            );
            return Err(format!("question slot {} not found", id));
        }
    };
    let mut slot = pending.remove(idx);
    if let Some(tx) = slot.tx.take() {
        let _ = tx.send(AskUserAnswer {
            selected,
            inputs: inputs.unwrap_or_default(),
            custom,
        });
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

    // 先应用热键变更（先注册新组合、成功后注销旧组合），
    // 失败则整体拒绝保存：避免把无法注册的组合持久化到磁盘，
    // 也避免旧快捷键被注销后新快捷键注册失败（静默失能）。
    crate::hotkey::update_hotkey(&app, &config.hotkey)?;

    storage::save_config(&app, &config)?;

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
    llm_provider
        .test_latency(&base_url, &api_key, &model_id)
        .await
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
            images: Vec::new(),
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

    #[tokio::test]
    async fn generated_image_data_url_decodes_and_rejects_invalid_data() {
        let bytes = generated_image_bytes("data:image/png;base64,aGVsbG8=", "image/png")
            .await
            .unwrap();
        assert_eq!(bytes, b"hello");

        assert!(
            generated_image_bytes("data:image/png;base64,***", "image/png")
                .await
                .is_err()
        );
    }

    #[test]
    fn generated_image_download_path_avoids_overwriting_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let first = unique_download_path(directory.path(), "png");
        std::fs::write(&first, b"existing").unwrap();
        let second = unique_download_path(directory.path(), "png");

        assert_ne!(first, second);
        assert_eq!(
            second.extension().and_then(|value| value.to_str()),
            Some("png")
        );
    }
}
