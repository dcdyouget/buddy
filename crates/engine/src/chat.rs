// 对话编排模块：自 v1 `src-tauri/src/commands.rs`（tag `v1-final`）迁入
//
// v1 中这些逻辑以 `#[tauri::command]` 形式经 IPC 暴露；v2 UI 与 engine 同进程，
// 改为 `ChatEngine` 的方法直接调用。三个 Tauri `State`（取消 / 审批 / 提问）合并为
// `ChatEngine` 的字段；`AppHandle` 仅用于定位数据目录，改为 `ChatEngine::data_dir`。
// 窗口类命令（resize_window_to_page / 前端诊断日志）属于应用外壳，不在 engine。

use crate::models::*;
use crate::providers::{self, ProviderType};
use crate::storage;
use crate::streaming::{ContentBlock, QuestionOption, StopReason, StreamEventEmitter};
use crate::tools::{AskUserAnswer, AskUserArgs, AskUserOption, ToolRegistry};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use futures_util::StreamExt;
use log::{info, warn};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;
use tokio::sync::{oneshot, watch};

/// 消息 ID 序列号：同一毫秒内会生成多条消息（工具循环的多条 tool 消息/assistant 分段），
/// 只靠毫秒时间戳会撞 ID，导致前端 React key 冲突。
static MESSAGE_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const MAX_TOOL_LOG_ARGUMENT_CHARS: usize = 2_000;
const MAX_VISION_IMAGES_PER_REQUEST: usize = 4;

/// 将同步文件 I/O 与 JSON/Base64 处理移到 blocking 线程池，
/// 避免占用 Tokio 工作线程而延迟流式事件和取消命令。
async fn run_blocking<T, F>(operation: &'static str, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|error| format!("{operation}任务失败: {error}"))?
}

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

/// 记录工具请求时保留可诊断参数，同时避免密钥和超长内容写入本地日志。
fn tool_arguments_for_log(raw: &str) -> String {
    let value = match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(mut value) => {
            redact_sensitive_tool_arguments(&mut value);
            serde_json::to_string(&value).unwrap_or_else(|_| "<参数序列化失败>".to_string())
        }
        Err(_) => format!("<无效 JSON> {raw}"),
    };
    truncate_log_value(&value, MAX_TOOL_LOG_ARGUMENT_CHARS)
}

fn redact_sensitive_tool_arguments(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            for (name, value) in fields {
                if is_sensitive_tool_argument(name) {
                    *value = serde_json::Value::String("[已脱敏]".to_string());
                } else {
                    redact_sensitive_tool_arguments(value);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                redact_sensitive_tool_arguments(value);
            }
        }
        _ => {}
    }
}

fn is_sensitive_tool_argument(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().replace(['_', '-'], "").as_str(),
        "apikey"
            | "authorization"
            | "password"
            | "secret"
            | "token"
            | "accesstoken"
            | "refreshtoken"
    )
}

fn truncate_log_value(value: &str, max_chars: usize) -> String {
    let mut characters = value.chars();
    let truncated = characters.by_ref().take(max_chars).collect::<String>();
    if characters.next().is_some() {
        format!("{truncated}…(已截断)")
    } else {
        truncated
    }
}
/// 取消状态：跨命令共享的取消通道
///
/// 使用 watch channel 实现：send_message 创建发送端，
/// stop_generation 通过发送端发出取消信号。
/// Mutex 包裹保证线程安全（Tauri 命令可能在不同线程执行）。
#[derive(Default)]
pub struct CancelState {
    pub sender: Mutex<Option<watch::Sender<bool>>>,
}

/// 原子地占用生成通道。检查与写入必须使用同一把锁，否则两个同时到达的
/// `send_message` 都可能通过检查，并让后一个请求覆盖前一个请求的取消 sender。
fn reserve_generation(state: &CancelState) -> Result<watch::Receiver<bool>, String> {
    let (sender, receiver) = watch::channel(false);
    let mut active_sender = state.sender.lock();
    if active_sender.is_some() {
        return Err("已有生成任务正在进行中".to_string());
    }
    *active_sender = Some(sender);
    Ok(receiver)
}

fn release_generation(state: &CancelState) {
    state.sender.lock().take();
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
    /// oneshot sender:UI 调用 `ChatEngine::approve_tool_call` 通过这里推回结果
    pub tx: Option<oneshot::Sender<bool>>,
}

/// 共享审批状态
#[derive(Default)]
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
    /// oneshot sender:UI 调用 `ChatEngine::answer_tool_question` 通过这里推回结果
    pub tx: Option<oneshot::Sender<AskUserAnswer>>,
}

/// 共享问题等待状态
#[derive(Default)]
pub struct QuestionState {
    /// 当前挂起待回答的 slot 列表
    pub pending: Mutex<Vec<QuestionSlot>>,
}
/// 对话引擎：持有数据目录与三类跨调用共享状态（v1 为三个 Tauri `State`）。
///
/// 所有方法取 `&self`；UI 侧用 `Arc<ChatEngine>` 共享，并把 `send_message`
/// spawn 到 tokio（future 为 `Send + 'static` 需先 clone `Arc` §3.3/§3.4）。
pub struct ChatEngine {
    data_dir: PathBuf,
    cancel: CancelState,
    approval: ApprovalState,
    question: QuestionState,
}

impl ChatEngine {
    /// `data_dir` 通常为 `storage::default_data_dir()`（与 v1 同目录）；测试传临时目录。
    pub fn new(data_dir: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            data_dir,
            cancel: CancelState::default(),
            approval: ApprovalState::default(),
            question: QuestionState::default(),
        })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

// ── 滑动窗口辅助函数 ──

/// 文本 token 启发式估算：ASCII 约 3 字符/token，非 ASCII 按 2 token/字符
/// 保守预留。不同模型分词并不一致，窗口仍保留 30% 余量。
fn estimate_tokens(text: &str) -> u32 {
    let (ascii, unicode) = text
        .chars()
        .fold((0u32, 0u32), |(ascii, unicode), character| {
            if character.is_ascii() {
                (ascii.saturating_add(1), unicode)
            } else {
                (ascii, unicode.saturating_add(1))
            }
        });
    if ascii == 0 && unicode == 0 {
        return 0;
    }
    (ascii / 3).saturating_add(unicode.saturating_mul(2)).max(1)
}

fn estimate_message_tokens(message: &Message) -> u32 {
    // 图片实际 token 数取决于厂商、尺寸和 detail；这里使用保守固定值，
    // 仅用于本地滑动窗口，避免带图对话被当作零成本。
    let calls = message
        .tool_calls
        .iter()
        .flatten()
        .fold(0u32, |total, call| {
            total
                .saturating_add(estimate_tokens(&call.arguments))
                .saturating_add(estimate_tokens(&call.name))
        });
    estimate_tokens(&message.content)
        .saturating_add(6) // 角色、消息边界和调用元数据也占用上下文。
        .saturating_add(calls)
        .saturating_add((message.images.len() as u32).saturating_mul(1_024))
}

fn validate_image_attachments(
    images: &[crate::models::ImageAttachment],
    max_image_bytes: usize,
) -> Result<(), String> {
    const SUPPORTED_MEDIA_TYPES: [&str; 4] = ["image/jpeg", "image/png", "image/gif", "image/webp"];

    if images.len() > MAX_VISION_IMAGES_PER_REQUEST {
        return Err(format!(
            "每条消息最多添加 {MAX_VISION_IMAGES_PER_REQUEST} 张图片"
        ));
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
            // base64 每 4 个字符对应 3 字节；按解码后大小与服务商上限比较。
            let decoded = (image.data_url.len() - expected_prefix.len()) / 4 * 3;
            if decoded > max_image_bytes {
                return Err(format!(
                    "{}：{}",
                    crate::models::image_limit_message(max_image_bytes),
                    image.name
                ));
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

async fn unique_download_path(download_dir: &Path, extension: &str) -> Result<PathBuf, String> {
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let base_name = format!("Buddy-生成图片-{timestamp}");
    let mut path = download_dir.join(format!("{base_name}.{extension}"));
    let mut suffix = 2_u16;
    while tokio::fs::try_exists(&path)
        .await
        .map_err(|error| format!("检查下载文件失败：{error}"))?
    {
        path = download_dir.join(format!("{base_name}-{suffix}.{extension}"));
        suffix += 1;
    }
    Ok(path)
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
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| format!("读取生成图片失败：{error}"))?;
        if bytes.len().saturating_add(chunk.len()) > MAX_GENERATED_IMAGE_BYTES {
            return Err("生成图片超过 25 MB，无法下载".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn stored_image_bytes(
    data_dir: &Path,
    image: &crate::models::ImageAttachment,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    let attachments_dir = data_dir.join("attachments");
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
    data_dir: &Path,
    image: &mut crate::models::ImageAttachment,
    max_image_bytes: usize,
) -> Result<(), String> {
    if !image.data_url.is_empty() {
        return Ok(());
    }
    let bytes = stored_image_bytes(data_dir, image, max_image_bytes).map_err(|error| {
        if error.starts_with("图片文件过大") {
            format!(
                "{}：{}",
                crate::models::image_limit_message(max_image_bytes),
                image.name
            )
        } else {
            error
        }
    })?;
    image.data_url = format!(
        "data:{};base64,{}",
        image.media_type,
        BASE64_STANDARD.encode(bytes)
    );
    Ok(())
}

/// 附件引用留在模型上下文中，像素只在当前请求需要时临时注入。
/// JSON 编码避免文件名、路径中的换行破坏引用格式。
fn append_image_references(message: &mut Message) {
    if message.images.is_empty() {
        return;
    }
    let references: Vec<_> = message
        .images
        .iter()
        .map(|image| {
            let mut reference = serde_json::json!({
                "id": image.id,
                "name": image.name,
                "path": image.path,
                "media_type": image.media_type,
            });
            // 生图下载失败时仍可保留远程链接，但不能声称存在可回读的本地文件。
            if image.path.is_empty() && image.data_url.starts_with("https://") {
                reference["url"] = serde_json::json!(image.data_url);
            }
            reference
        })
        .collect();
    message.content.push_str("\n\n<buddy_attachments>\n");
    message
        .content
        .push_str(&serde_json::to_string(&references).expect("attachment references serialize"));
    message.content.push_str("\nUse read_file(path) when you need to inspect an image that is not included in this request.\n</buddy_attachments>");
}

fn prepare_images_for_provider(
    data_dir: &Path,
    messages: &mut [Message],
    max_image_bytes: usize,
    supports_vision: bool,
) -> Result<(), String> {
    let current_user_index = messages
        .iter()
        .rposition(|message| message.role == MessageRole::User);
    for (index, message) in messages.iter_mut().enumerate() {
        if message.images.is_empty() {
            continue;
        }
        // 兼容旧记录和直接 API 输入：只有 Base64 的附件先转成可回读的本地文件。
        // 不因不相关的损坏历史附件阻断当前提问；当前输入的错误仍正常报告。
        for image in &mut message.images {
            if image.path.is_empty() && image.data_url.starts_with("data:") {
                let limit = if Some(index) == current_user_index {
                    max_image_bytes
                } else {
                    max_image_bytes.max(MAX_GENERATED_IMAGE_BYTES)
                };
                match storage::store_image_data_url(
                    data_dir,
                    &image.name,
                    &image.media_type,
                    &image.data_url,
                    limit,
                    "imported",
                ) {
                    Ok(stored) => image.path = stored.path,
                    Err(error) if Some(index) == current_user_index => return Err(error),
                    Err(error) => warn!("无法迁移历史图片 {}: {error}", image.id),
                }
            }
        }
        append_image_references(message);
        if supports_vision && Some(index) == current_user_index {
            for image in &mut message.images {
                hydrate_image_for_provider(data_dir, image, max_image_bytes)?;
            }
        } else {
            message.images.clear();
        }
    }
    Ok(())
}

/// 将工具读取或生成的图片持久化为本地附件，已有附件直接复用。
///
/// 返回 (已存储的图片, 失败的张数)。失败的图片不再静默丢弃——
/// 调用方据此把工具结果降级为错误，避免「UI 显示完成但图丢失」。
async fn persist_tool_images(
    data_dir: &Path,
    images: Vec<crate::models::ImageAttachment>,
) -> (Vec<crate::models::ImageAttachment>, usize) {
    if images.is_empty() {
        return (Vec::new(), 0);
    }
    let mut stored = Vec::with_capacity(images.len());
    let mut failed = 0usize;
    let attachment_root = tokio::fs::canonicalize(data_dir.join("attachments"))
        .await
        .ok();
    for mut image in images {
        if !image.path.is_empty() {
            let source = tokio::fs::canonicalize(&image.path).await;
            if let (Some(root), Ok(source)) = (&attachment_root, source) {
                if source.starts_with(root) {
                    // read_file 回读已有附件时复用文件，不反复保存同一张图片。
                    image.path = source.to_string_lossy().into_owned();
                    image.data_url.clear();
                    stored.push(image);
                    continue;
                }
            }
        }
        match generated_image_bytes(&image.data_url, &image.media_type).await {
            Ok(bytes) if !bytes.is_empty() => {
                let storage_dir = data_dir.to_path_buf();
                let name = image.name.clone();
                let media_type = image.media_type.clone();
                match run_blocking("保存生成图片", move || {
                    storage::store_image_bytes(
                        &storage_dir,
                        &name,
                        &media_type,
                        &bytes,
                        "generated",
                    )
                })
                .await
                {
                    Ok(attachment) => stored.push(attachment),
                    Err(error) => {
                        failed += 1;
                        warn!("保存生成图片失败：{}", error);
                    }
                }
            }
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

impl ChatEngine {
    /// 将用户选择的图片复制到 Buddy 应用数据目录。
    pub async fn save_chat_image(
        &self,
        name: String,
        media_type: String,
        data_url: String,
    ) -> Result<crate::models::ImageAttachment, String> {
        let data_dir = self.data_dir.clone();
        run_blocking("保存聊天图片", move || {
            storage::store_image_data_url(
                &data_dir,
                &name,
                &media_type,
                &data_url,
                crate::models::MAX_IMAGE_BYTES_CEILING,
                "upload",
            )
        })
        .await
    }

    /// 删除已保存的聊天图片附件（用户从输入框移除未发送的图片时调用，
    /// 清理已写盘的孤儿文件；仅限应用数据目录内的附件文件）。
    pub async fn delete_chat_image(&self, path: String) -> Result<bool, String> {
        let data_dir = self.data_dir.clone();
        run_blocking("删除聊天图片", move || {
            storage::delete_attachment_file(&data_dir, &path)
        })
        .await
    }

    /// 将生图工具的结果保存到系统下载目录。
    /// 下载目录与 v1 Tauri `download_dir()` 同源（`dirs::download_dir()`）。
    pub async fn download_generated_image(
        &self,
        image: crate::models::ImageAttachment,
    ) -> Result<String, String> {
        let extension = image_extension(&image.media_type)
            .ok_or_else(|| format!("不支持的图片格式：{}", image.media_type))?;
        let bytes = if image.path.is_empty() {
            generated_image_bytes(&image.data_url, &image.media_type).await?
        } else {
            let data_dir = self.data_dir.clone();
            run_blocking("读取生成图片", move || {
                stored_image_bytes(&data_dir, &image, MAX_GENERATED_IMAGE_BYTES)
            })
            .await?
        };
        if bytes.is_empty() {
            return Err("生成图片内容为空".to_string());
        }

        let download_dir =
            dirs::download_dir().ok_or_else(|| "无法获取系统下载目录".to_string())?;
        tokio::fs::create_dir_all(&download_dir)
            .await
            .map_err(|error| format!("无法创建下载目录：{error}"))?;
        let target = unique_download_path(&download_dir, extension).await?;
        tokio::fs::write(&target, bytes)
            .await
            .map_err(|error| format!("保存图片失败：{error}"))?;

        Ok(target.to_string_lossy().into_owned())
    }
}

/// 估算消息列表的总 token 数
fn estimate_total_tokens(messages: &[Message]) -> u32 {
    messages.iter().fold(0u32, |total, message| {
        total.saturating_add(estimate_message_tokens(message))
    })
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

    // 以用户提问为边界保留整轮，避免裁剪出孤立 tool result；read_file 的临时
    // visual 消息属于原有工具轮次，不是新的用户提问。
    let starts: Vec<usize> = std::iter::once(0)
        .chain(messages.iter().enumerate().filter_map(|(index, message)| {
            (index > 0 && message.role == MessageRole::User && !message.id.starts_with("visual-"))
                .then_some(index)
        }))
        .collect();
    let mut start = *starts.last().unwrap();
    let mut cost = estimate_total_tokens(&messages[start..]);
    for &earlier in starts.iter().rev().skip(1) {
        let next_cost = cost.saturating_add(estimate_total_tokens(&messages[earlier..start]));
        if next_cost > token_budget {
            break;
        }
        start = earlier;
        cost = next_cost;
    }
    start
}

/// 恢复中断的工具轮次：每个调用必须有对应结果，孤立结果不能发送给 API。
/// 仅修复请求副本；不执行历史调用、不改写原始历史。
fn repair_tool_history(messages: Vec<Message>) -> Vec<Message> {
    let mut repaired = Vec::with_capacity(messages.len());
    let mut iter = messages.into_iter().peekable();
    while let Some(message) = iter.next() {
        if message.role == MessageRole::Tool {
            continue;
        }
        let calls = message.tool_calls.clone().unwrap_or_default();
        repaired.push(message);
        if calls.is_empty() {
            continue;
        }
        let mut results = std::collections::HashMap::new();
        while iter
            .peek()
            .is_some_and(|message| message.role == MessageRole::Tool)
        {
            let result = iter.next().unwrap();
            if let Some(id) = &result.tool_call_id {
                results.entry(id.clone()).or_insert(result);
            }
        }
        for call in calls {
            repaired.push(results.remove(&call.id).unwrap_or_else(|| {
                build_tool_msg(
                    0,
                    &call,
                    "工具调用已中断，未获得结果；如仍需要，请重新调用。".into(),
                    Vec::new(),
                    true,
                )
            }));
        }
    }
    repaired
}

/// 按预算从磁盘尾部读取上下文，界面分页不参与模型上下文选择。
fn load_context_history(
    data_dir: &Path,
    current: Message,
    budget: u32,
) -> Result<Vec<Message>, String> {
    let mut offset = storage::message_count(data_dir)?;
    let mut pages = Vec::new();
    let mut cost = estimate_message_tokens(&current);
    while offset > 0 && cost < budget {
        let size = offset.min(256);
        offset -= size;
        let page = storage::load_messages(data_dir, offset, size)?;
        cost = cost.saturating_add(estimate_total_tokens(&page));
        pages.push(page);
    }
    let mut messages: Vec<_> = pages.into_iter().rev().flatten().collect();
    messages.push(current);
    let messages = repair_tool_history(messages);
    let start = compute_window_start(&messages, budget);
    Ok(messages.into_iter().skip(start).collect())
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
async fn flush_pending_messages(data_dir: PathBuf, pending: Vec<Message>) -> Result<(), String> {
    if pending.is_empty() {
        return Ok(());
    }

    let count = pending.len();
    match tokio::task::spawn_blocking(move || storage::append_messages(&data_dir, &pending)).await {
        Ok(Ok(())) => {
            info!("[send_message] 本轮 {} 条消息已批量持久化", count);
            Ok(())
        }
        Ok(Err(e)) => {
            let message = format!("批量持久化消息失败: {e}");
            warn!("[send_message] {}", message);
            Err(message)
        }
        Err(e) => {
            let message = format!("批量持久化任务失败: {e}");
            warn!("[send_message] {}", message);
            Err(message)
        }
    }
}

enum TerminalStreamEvent {
    Done {
        full_text: String,
    },
    Error {
        reason: StopReason,
        message: String,
        partial_text: String,
    },
}

impl TerminalStreamEvent {
    fn partial_text(&self) -> &str {
        match self {
            Self::Done { full_text } => full_text,
            Self::Error { partial_text, .. } => partial_text,
        }
    }

    fn persistence_failure(self, failure: String) -> Self {
        let partial_text = self.partial_text().to_string();
        let message = match self {
            Self::Done { .. } => format!("聊天记录保存失败: {failure}"),
            Self::Error { message, .. } => format!("{message}；聊天记录保存失败: {failure}"),
        };
        Self::Error {
            reason: StopReason::Error,
            message,
            partial_text,
        }
    }

    fn emit(self, emitter: &StreamEventEmitter) {
        match self {
            Self::Done { full_text } => emitter.done(StopReason::Stop, &full_text),
            Self::Error {
                reason,
                message,
                partial_text,
            } => emitter.error(reason, &message, &partial_text),
        }
    }
}

fn terminal_event_for_outcome(
    outcome: &Result<crate::streaming::StreamOutcome, crate::providers::ApiError>,
    turn: usize,
) -> TerminalStreamEvent {
    match outcome {
        Ok(outcome) if !outcome.had_stream_error => TerminalStreamEvent::Done {
            full_text: outcome.full_text.clone(),
        },
        Ok(outcome) => {
            let failure = outcome.terminal_error.as_ref();
            TerminalStreamEvent::Error {
                reason: failure
                    .map(|failure| failure.reason.clone())
                    .unwrap_or(StopReason::Error),
                message: failure
                    .map(|failure| failure.message.clone())
                    .unwrap_or_else(|| "流式请求未正常结束".to_string()),
                partial_text: outcome.full_text.clone(),
            }
        }
        Err(error) => {
            let message = if turn > 1 {
                format!(
                    "{} (工具调用后请求失败, 可能是 Provider 不支持 tool 回传格式)",
                    error
                )
            } else {
                error.to_string()
            };
            TerminalStreamEvent::Error {
                reason: StopReason::Error,
                message,
                partial_text: String::new(),
            }
        }
    }
}

fn cancelled_stream_outcome() -> crate::streaming::StreamOutcome {
    crate::streaming::StreamOutcome::failed(
        String::new(),
        String::new(),
        StopReason::Aborted,
        "用户取消",
    )
}

fn requires_tool_execution(outcome: &crate::streaming::StreamOutcome) -> bool {
    !outcome.had_stream_error && !outcome.tool_calls.is_empty()
}

impl ChatEngine {
    /// 发送消息
    ///
    /// UI 调用此方法发起 AI 对话。根据 Provider 类型分发到对应适配器：
    /// 1. 查找模型和 Provider 配置
    /// 2. 持久化用户消息到本地存储
    /// 3. 创建取消通道并存入共享状态
    /// 4. 调用对应 Provider 的 stream_chat 进行流式对话
    /// 5. 流式完成后自动持久化 AI 回复
    /// 6. 错误时发送对应事件给 UI
    /// 7. 清理取消通道
    ///
    /// 事件经调用方传入的 `emitter` 发出（`StreamEventEmitter::channel()`）；本方法返回时
    /// `emitter` 被 drop，接收端据此得知本次对话的事件已全部送达。
    /// 返回 `Err` 仅发生在占用生成通道之前（模型/Provider 缺失、图片校验失败、已有任务在跑），
    /// 此时不发射任何事件；其余结局一律以唯一的 `Done` 或 `Error` 事件告知。
    pub async fn send_message_from_history(
        &self,
        emitter: StreamEventEmitter,
        current: Message,
        model_id: String,
    ) -> Result<(), String> {
        if current.role != MessageRole::User {
            return Err("当前输入必须是用户消息".into());
        }
        let directory = self.data_dir.clone();
        let selected = model_id.clone();
        let messages = run_blocking("读取模型上下文", move || {
            let config = storage::get_config(&directory)?;
            let model = config
                .models
                .iter()
                .find(|model| model.id == selected)
                .ok_or_else(|| "未找到指定的模型".to_string())?;
            load_context_history(
                &directory,
                current,
                (model.context_window as f64 * 0.7) as u32,
            )
        })
        .await?;
        self.send_message(emitter, messages, model_id).await
    }

    pub async fn send_message(
        &self,
        emitter: StreamEventEmitter,
        messages: Vec<Message>,
        model_id: String,
    ) -> Result<(), String> {
        let state = &self.cancel;
        let approval = &self.approval;
        let question_started = Instant::now();
        let question_log_id = messages
            .iter()
            .rev()
            .find(|message| message.role == MessageRole::User)
            .map(|message| message.id.clone())
            .unwrap_or_else(|| unique_message_id("q", 0));

        // 从配置中查找模型对应的 Provider
        let config_dir = self.data_dir.clone();
        let config = run_blocking("读取配置", move || storage::get_config(&config_dir)).await?;
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
        let provider = config
            .providers
            .iter()
            .find(|p| p.id == model.provider_id)
            .ok_or_else(|| "未找到对应的 Provider".to_string())?;
        let max_image_bytes = provider.max_image_bytes();
        validate_image_attachments(current_user_images, max_image_bytes)?;
        if !current_user_images.is_empty() && !model.supports_vision {
            return Err("当前模型未开启图片输入，请在模型设置中启用“支持图片”".to_string());
        }

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

        // 按模型轮次批量保存；用户输入和待执行调用在耗时操作前先落盘。
        let mut pending_persistence: Vec<Message> = messages.last().cloned().into_iter().collect();

        // 防止并发 send_message：检查与占用在同一把锁下完成，不能让第二个请求覆盖
        // 第一个请求的取消通道。
        let mut cancel_rx = reserve_generation(state)?;

        // 根据 Provider 类型创建对应的适配器
        let llm_provider = providers::create_provider(&provider_type);
        let compat = provider.compat.as_ref();
        let provider_model_id = raw_model_id(model);

        // ── P4: 构造 ToolRegistry(只包含内置 tool,MCP tool 由 P7 注入) ──
        let mut builtin_tools = crate::tools::builtin::builtin_tools(config.allowed_paths.clone());
        if provider_type == ProviderType::OpenAICompatible && model.supports_image_generation {
            if let Some(tool) = crate::tools::image_generation::GenerateImageTool::for_provider(
                provider.base_url.clone(),
                provider.api_key.clone(),
                provider_model_id.to_string(),
                &provider.id,
                &provider.name,
            ) {
                builtin_tools.push(std::sync::Arc::new(tool));
            }
        }
        let registry = ToolRegistry::new(builtin_tools);
        let tools = registry.all_definitions();
        let fixed_request_cost = estimate_tokens(providers::BUDDY_SYSTEM_PROMPT).saturating_add(
            tools.iter().fold(0u32, |total, tool| {
                total
                    .saturating_add(estimate_tokens(&tool.description))
                    .saturating_add(estimate_tokens(&tool.parameters.to_string()))
            }),
        );
        // P7 会在此追加 mcp tool

        // 把 messages 拷成可变的 Vec,tool 循环会往里 push assistant(tool_calls) + tool(result)
        let mut conv_messages = repair_tool_history(windowed_messages.to_vec());
        if !model.supports_vision {
            // 切换到纯文本模型后不再把历史图片发送给 Provider。
            for message in &mut conv_messages {
                append_image_references(message);
                message.images.clear();
            }
        } else {
            // 图片预处理失败：不要用 `?` 直接退出——那样会泄漏取消通道、
            // 丢失用户消息，且前端收不到任何流事件提示。
            let storage_dir = self.data_dir.clone();
            conv_messages = match run_blocking("读取聊天图片", move || {
                prepare_images_for_provider(
                    &storage_dir,
                    &mut conv_messages,
                    max_image_bytes,
                    true,
                )?;
                Ok(conv_messages)
            })
            .await
            {
                Ok(messages) => messages,
                Err(error) => {
                    warn!("[send_message] 图片预处理失败: {}", error);
                    let persistence_error =
                        flush_pending_messages(self.data_dir.clone(), pending_persistence)
                            .await
                            .err();
                    release_generation(state);
                    approval
                        .approve_all_for_turn
                        .store(false, Ordering::Relaxed);
                    let terminal = TerminalStreamEvent::Error {
                        reason: StopReason::Error,
                        message: error,
                        partial_text: String::new(),
                    };
                    match persistence_error {
                        Some(persistence_error) => terminal.persistence_failure(persistence_error),
                        None => terminal,
                    }
                    .emit(&emitter);
                    return Ok(());
                }
            };
        }

        // 请求中的 Base64 是临时内容；落盘的本条输入使用已保存的附件路径。
        if let Some(pending) = pending_persistence.last_mut() {
            if let Some(prepared) = conv_messages
                .iter()
                .find(|message| message.id == pending.id)
            {
                pending.images = prepared.images.clone();
                for image in &mut pending.images {
                    image.data_url.clear();
                }
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

        let outcome: Result<crate::streaming::StreamOutcome, _> = 'conversation: loop {
            if providers::cancellation_requested(&cancel_rx) {
                break 'conversation Ok(cancelled_stream_outcome());
            }
            if let Err(error) = flush_pending_messages(
                self.data_dir.clone(),
                std::mem::take(&mut pending_persistence),
            )
            .await
            {
                break Err(crate::providers::ApiError::NetworkError(error));
            }
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
            let start_idx =
                compute_window_start(&conv_messages, budget.saturating_sub(fixed_request_cost));
            let windowed = &conv_messages[start_idx..];
            // 整轮工具消息不可拆开；过大的当前轮次应明确报错，不能继续发出超限请求。
            let request_cost = estimate_total_tokens(windowed).saturating_add(fixed_request_cost);
            if request_cost > budget {
                break Err(crate::providers::ApiError::NetworkError(
                    "当前提问或工具结果超过模型上下文预算，请缩短输入、减少文件内容或切换更大上下文的模型。".into(),
                ));
            }
            let request_id = format!("{}#{}", question_log_id, turn);
            api_requests += 1;
            let api_started = Instant::now();
            let outcome = llm_provider
                .stream_chat(
                    &provider.base_url,
                    &provider.api_key,
                    provider_model_id,
                    &request_id,
                    windowed,
                    &emitter,
                    cancel_rx.clone(),
                    compat,
                    &tools,
                )
                .await;
            // 普通 Chat Completions/Anthropic 请求均无状态。下一请求仅保留附件引用，
            // 需要再次看图时由 read_file 显式载入，避免工具循环自动重传所有像素。
            for message in &mut conv_messages {
                message.images.clear();
            }
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
            if !out.full_text.is_empty()
                || !out.tool_calls.is_empty()
                || !out.thinking_text.is_empty()
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

            // Provider 以错误或取消结束时，已经持久化可恢复的部分回复；绝不能执行它
            // 可能携带的半截 tool_call，也不能进入下一轮模型请求。
            if !requires_tool_execution(&out) {
                break Ok(out);
            }

            // 工具可能长时间等待审批或产生副作用，先保存调用记录，重启后可识别中断。
            if let Err(error) = flush_pending_messages(
                self.data_dir.clone(),
                std::mem::take(&mut pending_persistence),
            )
            .await
            {
                break Err(crate::providers::ApiError::NetworkError(error));
            }

            // 有 tool_calls → 顺序执行(并发=1;P4 决定),再调 stream_chat
            info!(
                "[send_message] turn {} 收到 {} 个 tool_call,开始执行",
                turn,
                out.tool_calls.len()
            );
            // 本轮追踪:是否有至少一个 tool 执行成功(非 is_error)?
            let mut turn_has_success = false;
            let mut next_request_images = Vec::new();
            for call in &out.tool_calls {
                info!(
                    "[tool] 请求: question_id={}, turn={}, id={}, name={}, arguments={}",
                    question_log_id,
                    turn,
                    call.id,
                    call.name,
                    tool_arguments_for_log(&call.arguments)
                );
                // 用户点击"停止"后，不再执行本轮剩余的工具调用
                if *cancel_rx.borrow() {
                    info!(
                        "[send_message] turn {} 工具循环被用户取消, 跳过剩余 {} 个调用",
                        turn,
                        out.tool_calls.len()
                    );
                    break 'conversation Ok(cancelled_stream_outcome());
                }
                // ── ask_user 特殊分支:不进入普通 tool.execute,而是显示内联问答卡等回答 ──
                if call.name == "ask_user" {
                    let args_value: serde_json::Value = match serde_json::from_str(&call.arguments)
                    {
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
                    let q_state = &self.question;
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
                            None
                        }
                        result = rx => {
                            Some(match result {
                                Ok(a) => a,
                                Err(_) => {
                                    warn!("[send_message] ask_user oneshot 失败");
                                    AskUserAnswer { selected: vec![], inputs: vec![], custom: Some("(无响应)".to_string()) }
                                }
                            })
                        }
                    };

                    let Some(answer) = answer else {
                        break 'conversation Ok(cancelled_stream_outcome());
                    };
                    if providers::cancellation_requested(&cancel_rx) {
                        break 'conversation Ok(cancelled_stream_outcome());
                    }

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
                            None
                        }
                        result = rx => {
                            Some(match result {
                                Ok(b) => b,
                                Err(_) => {
                                    warn!("[send_message] 审批 oneshot 失败,按拒绝处理");
                                    false
                                }
                            })
                        }
                    };
                    let Some(approved) = approved else {
                        break 'conversation Ok(cancelled_stream_outcome());
                    };
                    if providers::cancellation_requested(&cancel_rx) {
                        break 'conversation Ok(cancelled_stream_outcome());
                    }
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
                if providers::cancellation_requested(&cancel_rx) {
                    break 'conversation Ok(cancelled_stream_outcome());
                }
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
                    supports_vision: model.supports_vision,
                    max_image_bytes,
                };
                let tool_started = Instant::now();
                let result = tool.execute(args_value, ctx).await;
                let elapsed_ms = tool_started.elapsed().as_millis();
                let (mut content, mut raw_images, mut is_error) = match &result {
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
                if let Ok(output) = &result {
                    if next_request_images.len() + output.model_images.len()
                        > MAX_VISION_IMAGES_PER_REQUEST
                    {
                        content = format!(
                            "同一次请求最多读取 {MAX_VISION_IMAGES_PER_REQUEST} 张图片，请分批读取剩余图片。"
                        );
                        raw_images.clear();
                        is_error = true;
                    }
                }
                let (images, failed_image_count) =
                    persist_tool_images(&self.data_dir, raw_images).await;
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
                    "[tool] 完成: question_id={}, turn={}, id={}, name={}, is_error={}, output_chars={}, images={}, elapsed_ms={}",
                    question_log_id,
                    turn,
                    call.id,
                    call.name,
                    is_error,
                    content.chars().count(),
                    images.len(),
                    elapsed_ms
                );
                if !is_error {
                    turn_has_success = true;
                    if model.supports_vision {
                        if let Ok(output) = &result {
                            for (index, visual) in output.model_images.iter().enumerate() {
                                let mut visual = visual.clone();
                                // read_file 的视觉副本绑定到已保存的附件，后续引用可再次读取。
                                if let Some(stored) = images.get(index) {
                                    visual.id = stored.id.clone();
                                    visual.path = stored.path.clone();
                                }
                                next_request_images.push(visual);
                            }
                        }
                    }
                }
                emitter.tool_result(&call.id, &call.name, &content, images.clone(), is_error);
                info!(
                    "[send_message] turn {} tool '{}' tool_result 事件已发射 (id={})",
                    turn, call.name, call.id
                );
                let tool_msg = build_tool_msg(turn, call, content, images, is_error);
                pending_persistence.push(tool_msg.clone());
                let mut request_tool_msg = tool_msg;
                append_image_references(&mut request_tool_msg);
                request_tool_msg.images.clear();
                conv_messages.push(request_tool_msg);
            }
            if !next_request_images.is_empty() {
                // 必须在整组 tool result 后插入视觉输入，不能打断并列工具协议。
                // 此消息仅供 Provider 请求使用，不进入 UI 或持久化历史。
                let mut visual_input = Message {
                    id: unique_message_id("visual", turn),
                    role: MessageRole::User,
                    content: "Images returned by read_file:".to_string(),
                    images: next_request_images,
                    blocks: None,
                    model_id: None,
                    created_at: chrono::Utc::now().timestamp() as u64,
                    tool_calls: None,
                    tool_call_id: None,
                    tool_name: None,
                    is_error: None,
                    parent_message_id: None,
                };
                append_image_references(&mut visual_input);
                conv_messages.push(visual_input);
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

        let mut terminal_event = terminal_event_for_outcome(&outcome, turn);

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

        // 终态事件必须晚于持久化和占用释放：前端收到 done/error 后可以立刻开始下一轮，
        // 此时后端不能仍持有上一轮的 CancelState。
        if let Err(persistence_error) =
            flush_pending_messages(self.data_dir.clone(), pending_persistence).await
        {
            terminal_event = terminal_event.persistence_failure(persistence_error);
        }
        release_generation(state);
        approval
            .approve_all_for_turn
            .store(false, Ordering::Relaxed);
        terminal_event.emit(&emitter);
        Ok(())
    }

    /// 取消当前生成。没有进行中的生成时为空操作。
    pub fn stop_generation(&self) {
        if let Some(sender) = self.cancel.sender.lock().as_ref() {
            info!("[stop_generation] 用户取消生成");
            sender.send(true).ok();
        }
    }

    /// Tool 审批
    ///
    /// UI 在用户点击"允许"/"拒绝"后调用（id 取自 `ToolApprovalRequired` 事件）。
    /// - approve_all=true → 本轮剩余所有 write tool 都自动批准
    /// - approve_all=false → 只批准这一个(approved=true)或拒绝这一个(approved=false)
    ///
    /// 后端通过 oneshot::Sender 把结果推回 send_message 的 await 点。
    pub fn approve_tool_call(
        &self,
        id: &str,
        approved: bool,
        approve_all: bool,
    ) -> Result<(), String> {
        let approval = &self.approval;
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

impl ChatEngine {
    /// 用户回答 ask_user
    ///
    /// UI 在内联问答卡中做出选择后调用（id 取自 `ToolQuestionRequired` 事件）。
    /// 把 AskUserAnswer 通过 oneshot 推回 send_message 的 await 点。
    pub fn answer_tool_question(
        &self,
        id: &str,
        selected: Vec<usize>,
        inputs: Option<Vec<String>>,
        custom: Option<String>,
    ) -> Result<(), String> {
        let question = &self.question;
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
}

/// 获取应用配置命令
///
/// 从磁盘读取完整的 AppConfig 返回给前端。
impl ChatEngine {
    pub async fn get_config(&self) -> Result<AppConfig, String> {
        let data_dir = self.data_dir.clone();
        run_blocking("读取配置", move || storage::get_config(&data_dir)).await
    }

    /// 保存应用配置命令
    ///
    /// 在写入磁盘之前进行基本校验：
    /// - 如果 selected_model_id 非空，确保该模型在 models 列表中存在
    /// ⚠️ v1 在写盘前先调用 `hotkey::update_hotkey`（先注册新组合、成功后注销旧组合，失败则拒绝保存）。
    /// 热键属于应用外壳，engine 不持有；UI 层必须在调用本方法**之前**完成热键注册。
    pub async fn save_config(&self, config: AppConfig) -> Result<(), String> {
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

        let data_dir = self.data_dir.clone();
        run_blocking("保存配置", move || {
            storage::save_config(&data_dir, &config)
        })
        .await?;

        Ok(())
    }

    /// 获取模型列表命令
    ///
    /// 根据 Provider 类型调用对应适配器的 fetch_models。
    /// context_window 通过已知模型映射表获取，未知模型默认 128000。
    pub async fn fetch_models(
        &self,
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
    pub async fn test_latency(
        &self,
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
    pub async fn load_messages(&self, offset: u64, limit: u64) -> Result<Vec<Message>, String> {
        let data_dir = self.data_dir.clone();
        run_blocking("加载历史消息", move || {
            storage::load_messages(&data_dir, offset, limit)
        })
        .await
    }

    /// 返回本地历史消息总数，供前端计算分页起点。
    pub async fn get_message_count(&self) -> Result<u64, String> {
        let data_dir = self.data_dir.clone();
        run_blocking("读取消息数量", move || {
            storage::message_count(&data_dir)
        })
        .await
    }

    /// 保存消息命令
    ///
    /// 将单条消息追加到本地存储分块文件中。
    /// 若当前分块已满则自动创建新分块，并更新 manifest 计数。
    pub async fn save_message(&self, message: Message) -> Result<(), String> {
        let data_dir = self.data_dir.clone();
        run_blocking("保存消息", move || {
            storage::append_message(&data_dir, &message)
        })
        .await
        .map_err(|error| format!("保存消息失败: {error}"))
    }
}

// ── 测试 ──

#[cfg(test)]
mod tests {
    use super::*;

    fn data_url_image(decoded_bytes: usize) -> crate::models::ImageAttachment {
        crate::models::ImageAttachment {
            id: "img".into(),
            name: "a.png".into(),
            media_type: "image/png".into(),
            path: String::new(),
            data_url: format!(
                "data:image/png;base64,{}",
                BASE64_STANDARD.encode(vec![0u8; decoded_bytes])
            ),
        }
    }

    #[test]
    fn historical_image_references_do_not_read_or_include_pixels() {
        let directory = tempfile::tempdir().unwrap();
        let mut old = make_msg("旧图");
        let mut image = data_url_image(8);
        image.name = "image\"\nname.png".into();
        image.path = directory
            .path()
            .join("deleted.png")
            .to_string_lossy()
            .into_owned();
        old.images.push(image);
        let current = make_msg("继续分析旧图");
        let mut messages = vec![old, current];
        prepare_images_for_provider(directory.path(), &mut messages, 1, true).unwrap();
        assert!(messages[0].images.is_empty());
        assert!(messages[0].content.contains("<buddy_attachments>"));
        assert!(messages[0].content.contains("deleted.png"));
        assert!(messages[0].content.contains("image\\\"\\nname.png"));
        assert!(!messages[0].content.contains("base64"));
    }

    #[test]
    fn tool_images_remain_available_as_textual_references() {
        let directory = tempfile::tempdir().unwrap();
        let mut tool = make_msg("图片已生成");
        tool.role = MessageRole::Tool;
        tool.images.push(data_url_image(8));
        let mut messages = vec![tool, make_msg("请检查生成结果")];
        prepare_images_for_provider(directory.path(), &mut messages, 1, false).unwrap();
        assert!(messages[0].images.is_empty());
        assert!(messages[0].content.contains("read_file(path)"));
        assert!(!messages[0].content.contains("base64"));
    }

    #[test]
    fn base64_only_input_gets_a_readable_attachment_path() {
        let directory = tempfile::tempdir().unwrap();
        let mut user = make_msg("请查看图片");
        user.images.push(data_url_image(8));
        let mut messages = vec![user];
        prepare_images_for_provider(directory.path(), &mut messages, 1024, true).unwrap();
        let image = &messages[0].images[0];
        assert!(Path::new(&image.path).is_file());
        assert!(messages[0].content.contains(&image.path));
        assert!(!image.data_url.is_empty());
    }

    #[tokio::test]
    async fn reading_existing_attachment_reuses_file_and_drops_base64_from_history() {
        let directory = tempfile::tempdir().unwrap();
        let mut image = storage::store_image_bytes(
            directory.path(),
            "existing.png",
            "image/png",
            b"existing image",
            "test",
        )
        .unwrap();
        let original_path = image.path.clone();
        image.data_url = format!(
            "data:image/png;base64,{}",
            BASE64_STANDARD.encode(b"existing image")
        );
        let (stored, failed) = persist_tool_images(directory.path(), vec![image]).await;
        assert_eq!(failed, 0);
        assert_eq!(
            stored[0].path,
            Path::new(&original_path)
                .canonicalize()
                .unwrap()
                .to_string_lossy()
        );
        assert!(stored[0].data_url.is_empty());
        assert_eq!(
            std::fs::read_dir(directory.path().join("attachments"))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn image_limit_follows_provider() {
        let mut provider = crate::models::ProviderConfig {
            id: "minimax".into(),
            name: "MiniMax".into(),
            base_url: "https://api.minimaxi.com/v1".into(),
            api_key: String::new(),
            enabled_model_ids: vec![],
            provider_type: "openai_compatible".into(),
            compat: None,
        };
        assert_eq!(
            provider.max_image_bytes(),
            crate::models::MINIMAX_IMAGE_BYTES
        );
        provider.base_url = "https://api.openai.com/v1".into();
        assert_eq!(
            provider.max_image_bytes(),
            crate::models::DEFAULT_IMAGE_BYTES
        );
    }

    #[test]
    fn send_validation_uses_decoded_size_against_provider_limit() {
        let eight_mb = 8 * 1024 * 1024;
        let image = [data_url_image(eight_mb)];
        assert!(validate_image_attachments(&image, crate::models::MINIMAX_IMAGE_BYTES).is_ok());
        let error =
            validate_image_attachments(&image, crate::models::DEFAULT_IMAGE_BYTES).unwrap_err();
        assert!(error.starts_with("单张图片不能超过 5 MB"), "{error}");
    }

    #[test]
    fn generation_reservation_is_atomic_and_reusable_after_release() {
        let state = CancelState {
            sender: Mutex::new(None),
        };

        let receiver = reserve_generation(&state).unwrap();
        assert!(reserve_generation(&state).is_err());
        state.sender.lock().as_ref().unwrap().send(true).unwrap();
        assert!(providers::cancellation_requested(&receiver));

        release_generation(&state);
        assert!(reserve_generation(&state).is_ok());
    }

    #[test]
    fn stream_failure_with_tool_calls_never_enters_tool_execution() {
        let mut outcome = crate::streaming::StreamOutcome::failed(
            "已收到的部分回复".to_string(),
            String::new(),
            StopReason::Error,
            "连接中断",
        );
        outcome.tool_calls.push(crate::models::ToolCall {
            id: "partial-call".to_string(),
            name: "write_file".to_string(),
            arguments: r#"{"path":"/tmp/example"#.to_string(),
        });

        assert!(!requires_tool_execution(&outcome));
        match terminal_event_for_outcome(&Ok(outcome), 1) {
            TerminalStreamEvent::Error {
                reason,
                message,
                partial_text,
            } => {
                assert!(matches!(reason, StopReason::Error));
                assert_eq!(message, "连接中断");
                assert_eq!(partial_text, "已收到的部分回复");
            }
            TerminalStreamEvent::Done { .. } => panic!("stream failure must emit error"),
        }
    }

    #[test]
    fn cancellation_does_not_produce_a_follow_up_tool_turn() {
        let outcome = cancelled_stream_outcome();

        assert!(outcome.had_stream_error);
        assert!(!requires_tool_execution(&outcome));
        match terminal_event_for_outcome(&Ok(outcome), 1) {
            TerminalStreamEvent::Error { reason, .. } => {
                assert!(matches!(reason, StopReason::Aborted));
            }
            TerminalStreamEvent::Done { .. } => panic!("cancellation must emit error"),
        }
    }

    #[test]
    fn persistence_failure_replaces_done_but_keeps_partial_text() {
        let terminal = TerminalStreamEvent::Done {
            full_text: "已完成但尚未保存的回复".to_string(),
        }
        .persistence_failure("磁盘已满".to_string());

        match terminal {
            TerminalStreamEvent::Error {
                reason,
                message,
                partial_text,
            } => {
                assert!(matches!(reason, StopReason::Error));
                assert!(message.contains("聊天记录保存失败"));
                assert_eq!(partial_text, "已完成但尚未保存的回复");
            }
            TerminalStreamEvent::Done { .. } => panic!("persistence failure must emit error"),
        }
    }

    fn make_msg(content: &str) -> Message {
        Message {
            id: unique_message_id("u", 0), // v1 用 uuid（仅测试依赖）；此处只需唯一
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

    #[test]
    fn window_keeps_tool_round_and_visual_input_together() {
        let old = make_msg("old".repeat(100).as_str());
        let user = make_msg("current");
        let call = ToolCall {
            id: "call".into(),
            name: "read_file".into(),
            arguments: "{}".into(),
        };
        let mut assistant = make_msg("");
        assistant.role = MessageRole::Assistant;
        assistant.tool_calls = Some(vec![call.clone()]);
        let result = build_tool_msg(0, &call, "result".repeat(50), Vec::new(), false);
        let mut visual = make_msg("visual input");
        visual.id = "visual-1".into();
        let messages = vec![old, user, assistant, result, visual];
        assert_eq!(compute_window_start(&messages, 1), 1);
    }

    #[test]
    fn interrupted_history_completes_missing_calls_and_drops_orphans() {
        let first = ToolCall {
            id: "first".into(),
            name: "read_file".into(),
            arguments: "{}".into(),
        };
        let second = ToolCall {
            id: "second".into(),
            ..first.clone()
        };
        let mut assistant = make_msg("");
        assistant.role = MessageRole::Assistant;
        assistant.tool_calls = Some(vec![first.clone(), second.clone()]);
        let orphan = build_tool_msg(0, &first, "orphan".into(), Vec::new(), false);
        let completed = build_tool_msg(0, &first, "done".into(), Vec::new(), false);
        let messages =
            repair_tool_history(vec![orphan, assistant, completed, make_msg("continue")]);
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[1].tool_call_id.as_deref(), Some("first"));
        assert_eq!(messages[1].content, "done");
        assert_eq!(messages[2].tool_call_id.as_deref(), Some("second"));
        assert_eq!(messages[2].is_error, Some(true));
    }

    #[test]
    fn window_accounts_for_tool_arguments() {
        let mut message = make_msg("");
        message.tool_calls = Some(vec![ToolCall {
            id: "call".into(),
            name: "write_file".into(),
            arguments: "x".repeat(3000),
        }]);
        assert!(estimate_message_tokens(&message) >= 1000);
    }

    // ── estimate_tokens ──

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_single_char() {
        assert_eq!(estimate_tokens("a"), 1); // 1/3=0, .max(1)=1
        assert_eq!(estimate_tokens("中"), 2);
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
        // 中文不能套用英文的 chars/3，否则长中文上下文会被明显低估。
        assert_eq!(estimate_tokens("你好世界"), 8);
        assert_eq!(estimate_tokens("这是一段比较长的中文文本内容"), 28);
    }

    #[test]
    fn tool_request_log_preserves_query_and_redacts_secrets() {
        let arguments = tool_arguments_for_log(
            r#"{"query":"2026年夏アニメ 7月新番 一覧","api_key":"secret","nested":{"token":"value"}}"#,
        );

        assert!(arguments.contains("2026年夏アニメ 7月新番 一覧"));
        assert!(!arguments.contains("secret"));
        assert!(!arguments.contains("value"));
        assert_eq!(arguments.matches("[已脱敏]").count(), 2);
    }

    #[test]
    fn tool_request_log_truncates_long_arguments() {
        let raw = format!(
            r#"{{"query":"{}"}}"#,
            "x".repeat(MAX_TOOL_LOG_ARGUMENT_CHARS)
        );
        let arguments = tool_arguments_for_log(&raw);

        assert!(arguments.ends_with("…(已截断)"));
        assert!(arguments.chars().count() > MAX_TOOL_LOG_ARGUMENT_CHARS);
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
        // 每条正文约 1 token，另有 6 token 消息开销；预算保留最近 5 条。
        let start = compute_window_start(&msgs, 35);
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

    #[tokio::test]
    async fn generated_image_download_path_avoids_overwriting_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let first = unique_download_path(directory.path(), "png").await.unwrap();
        std::fs::write(&first, b"existing").unwrap();
        let second = unique_download_path(directory.path(), "png").await.unwrap();

        assert_ne!(first, second);
        assert_eq!(
            second.extension().and_then(|value| value.to_str()),
            Some("png")
        );
    }
}
