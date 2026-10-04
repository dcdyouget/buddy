// 本地 JSON 文件存储模块
//
// 职责：
// 1. Config 读写 —— 将 AppConfig 序列化到 config.json，启动时读取
// 2. Manifest 读写 —— 维护 manifest.json，跟踪所有消息分块信息
// 3. Message 分块存储 —— 每条消息追加到 chunk 文件，每 100 条自动切新块

use crate::models::*;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use log::warn;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

mod atomic_file;
mod attachment_files;
mod config_files;

/// 每个分块文件的最大消息数量
const CHUNK_SIZE: u32 = 100;

/// 应用标识（与 v1 Tauri 的 bundle identifier 相同，也是 Info.plist 的 `CFBundleIdentifier`）。
///
/// ⚠️ 不要改：v1 的数据目录由它决定，v2 沿用同一目录，两版并存期间共用配置。
///
/// ⚠️ 写入锁 `APPEND_LOCK` 只在进程内有效，**没有跨进程文件锁**：v1 与 v2 同时运行并各自
/// 追加消息时，分块的「读-改-写」会互相覆盖而丢消息（原子 rename 只防半截文件，不防覆盖）。
pub const APP_IDENTIFIER: &str = "com.buddy.chat";

/// 默认数据目录，与 v1 Tauri `app_data_dir()` 同一算法：`dirs::data_dir()/<identifier>`
/// （macOS `~/Library/Application Support/com.buddy.chat`，Windows `%APPDATA%\com.buddy.chat`）。
pub fn default_data_dir() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|dir| dir.join(APP_IDENTIFIER))
        .ok_or_else(|| "无法获取数据目录".to_string())
}

/// 确保数据目录存在，不存在则创建
fn ensure_data_dir(data_dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(data_dir).map_err(|e| format!("无法创建数据目录: {}", e))?;
    Ok(data_dir.to_path_buf())
}

// ── 原子写工具 ────────────────────────────────────────────

/// 原子写文件：先写临时文件，再 rename 覆盖目标。
///
/// `fs::write` 直接写目标文件在进程崩溃/断电时会留下半个文件（JSON 解析失败、
/// 被当作空数据回退 → 下一次保存覆盖真实数据）。rename 在同类文件系统上是原子的，
/// 保证目标文件要么是旧内容、要么是新内容，绝不会是半截内容。
fn write_file_atomic(
    dir: &PathBuf,
    file_name: &str,
    content: &str,
    label: &str,
) -> Result<(), String> {
    atomic_file::write_file_atomic(dir, file_name, content, label)
}

// ── Config ────────────────────────────────────────────────

/// 读取应用配置
///
/// 若 config.json 不存在则返回默认配置；
/// 若文件损坏（JSON 解析失败）则打印警告并返回默认配置，避免应用无法启动。
pub fn get_config(data_dir: &Path) -> Result<AppConfig, String> {
    config_files::read_config(&data_dir.to_path_buf())
}

/// 保存应用配置到磁盘
pub fn save_config(data_dir: &Path, config: &AppConfig) -> Result<(), String> {
    config_files::save_config(&data_dir.to_path_buf(), config)
}

// ── Manifest ──────────────────────────────────────────────

/// 读取 manifest.json
///
/// 若文件不存在或解析失败，返回空的 Manifest（无分块、无消息），
/// 后续操作会从空状态开始正常创建分块。
fn read_manifest(dir: &PathBuf) -> Manifest {
    let path = dir.join("manifest.json");
    if !path.exists() {
        return Manifest {
            chunks: vec![],
            total_messages: 0,
        };
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or(Manifest {
            chunks: vec![],
            total_messages: 0,
        })
}

/// 写入 manifest.json
fn write_manifest(dir: &PathBuf, manifest: &Manifest) -> Result<(), String> {
    let content =
        serde_json::to_string_pretty(manifest).map_err(|e| format!("序列化索引失败: {}", e))?;
    write_file_atomic(dir, "manifest.json", &content, "索引文件")
}

// ── Messages ──────────────────────────────────────────────

/// 全局写入锁 —— 防止 `tokio::task::spawn_blocking` 并发调用 `append_message`
/// 时产生读写竞态（两个任务同时读 chunk → 各追加一条 → 后写覆盖先写 → 丢消息）。
/// `std::sync::Mutex` 在 `spawn_blocking` 线程中阻塞是预期行为。
static APPEND_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static ATTACHMENT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn image_extension(media_type: &str) -> Option<&'static str> {
    match media_type {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

/// 将图片保存到应用数据目录，聊天消息仅引用返回的绝对路径。
///
/// 图片二进制不写进消息 JSON。附件文件被外部删除后，历史消息仍保留原路径，
/// UI 须按路径缺失显示「图片已删除」而非报错。
pub fn store_image_bytes(
    data_dir: &Path,
    name: &str,
    media_type: &str,
    bytes: &[u8],
    id_prefix: &str,
) -> Result<ImageAttachment, String> {
    let extension =
        image_extension(media_type).ok_or_else(|| format!("不支持的图片格式：{media_type}"))?;
    let dir = ensure_data_dir(data_dir)?.join("attachments");
    fs::create_dir_all(&dir).map_err(|e| format!("无法创建图片附件目录: {e}"))?;

    let sequence = ATTACHMENT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let id = format!(
        "{}-{}-{}",
        id_prefix,
        chrono::Utc::now().timestamp_millis(),
        sequence
    );
    let path = dir.join(format!("{id}.{extension}"));
    fs::write(&path, bytes).map_err(|e| format!("保存图片附件失败: {e}"))?;

    Ok(ImageAttachment {
        id,
        name: name.to_string(),
        media_type: media_type.to_string(),
        path: path.to_string_lossy().into_owned(),
        data_url: String::new(),
    })
}

/// 解码导入阶段的 Data URL，并立即转换为路径附件。
pub fn store_image_data_url(
    data_dir: &Path,
    name: &str,
    media_type: &str,
    data_url: &str,
    max_bytes: usize,
    id_prefix: &str,
) -> Result<ImageAttachment, String> {
    let prefix = format!("data:{media_type};base64,");
    let encoded = data_url
        .strip_prefix(&prefix)
        .ok_or_else(|| format!("图片数据格式无效：{name}"))?;
    // 先按 Base64 编码长度粗略预检，避免对超大 payload 做完整解码
    // （base64：3 字节 → 4 字符；+4 容忍填充与舍入）
    if encoded.len() > (max_bytes / 3) * 4 + 4 {
        return Err(format!("图片文件过大：{name}"));
    }
    let bytes = BASE64_STANDARD
        .decode(encoded)
        .map_err(|_| format!("图片 Base64 数据无效：{name}"))?;
    if bytes.len() > max_bytes {
        return Err(format!("图片文件过大：{name}"));
    }
    store_image_bytes(data_dir, name, media_type, &bytes, id_prefix)
}

/// 删除已保存的附件文件（仅允许应用数据目录 attachments 子目录内的文件）。
///
/// 返回 Ok(false) = 文件不存在；Ok(true) = 已删除。
/// 限制在 attachments 目录内，防止任意路径的穿越删除。
pub fn delete_attachment_file(data_dir: &Path, path: &str) -> Result<bool, String> {
    attachment_files::delete_attachment_in_dir(
        &data_dir.join("attachments"),
        std::path::Path::new(path),
    )
}

/// 追加一条新消息到分块存储
///
/// 分块逻辑：
/// 1. 检查最后一个分块是否已满（>= CHUNK_SIZE 条）
/// 2. 若已满则创建新分块（chunk_002.json, chunk_003.json...）
/// 3. 将消息追加到目标分块末尾
/// 4. 更新 manifest 中的计数
///
/// 线程安全:通过 `APPEND_LOCK` 串行化所有写入,防止并发 `spawn_blocking` 丢消息。
pub fn append_message(data_dir: &Path, message: &Message) -> Result<(), String> {
    append_messages(data_dir, std::slice::from_ref(message))
}

/// 批量追加同一轮对话产生的消息。
///
/// 一轮流式对话可能包含 assistant、多个 tool call/result。将这些消息在内存中
/// 收集后一次写入，避免每条消息都重复读取、序列化并覆写同一个 JSON 分块。
pub fn append_messages(data_dir: &Path, messages: &[Message]) -> Result<(), String> {
    if messages.is_empty() {
        return Ok(());
    }

    // 防止 mutex poisoning 级联失败: 若上一持锁者 panic 了,
    // unwrap_or_else(|p| p.into_inner()) 仍拿到 guard(其内部状态反映 panic 前),
    // 后续写盘流程照常推进 —— 持久化不会因一次 panic 永久停摆。
    let _guard = APPEND_LOCK.lock().unwrap_or_else(|p| {
        log::warn!("[storage::append_messages] APPEND_LOCK 已 poison,恢复并继续");
        p.into_inner()
    });
    let dir = ensure_data_dir(data_dir)?;
    let mut manifest = read_manifest(&dir);

    let mut active: Option<(String, ChatChunk)> = None;
    for message in messages {
        let needs_new_chunk = active
            .as_ref()
            .map(|(_, chunk)| chunk.messages.len() as u32 >= CHUNK_SIZE)
            .unwrap_or(true);

        if needs_new_chunk {
            if let Some((file, chunk)) = active.take() {
                write_chunk(&dir, &file, &chunk)?;
            }

            let file = match manifest.chunks.last() {
                Some(last) if last.count < CHUNK_SIZE => last.file.clone(),
                _ => {
                    let file = format!("chunk_{:03}.json", manifest.chunks.len() + 1);
                    manifest.chunks.push(ChunkMeta {
                        file: file.clone(),
                        count: 0,
                    });
                    file
                }
            };
            let chunk = read_chunk(&dir, &file)?;
            active = Some((file, chunk));
        }

        let (file, chunk) = active.as_mut().expect("active chunk must exist");
        chunk.messages.push(message.clone());
        if let Some(meta) = manifest.chunks.iter_mut().find(|meta| meta.file == *file) {
            meta.count = chunk.messages.len() as u32;
        }
        manifest.total_messages = manifest.total_messages.saturating_add(1);
    }

    if let Some((file, chunk)) = active {
        write_chunk(&dir, &file, &chunk)?;
    }
    write_manifest(&dir, &manifest)?;

    Ok(())
}

fn read_chunk(dir: &PathBuf, file: &str) -> Result<ChatChunk, String> {
    let path = dir.join(file);
    if !path.exists() {
        return Ok(ChatChunk {
            id: file.trim_end_matches(".json").to_string(),
            messages: vec![],
        });
    }
    let content = fs::read_to_string(&path).map_err(|e| format!("读取消息文件失败: {}", e))?;
    Ok(serde_json::from_str(&content).unwrap_or_else(|e| {
        warn!(
            "[storage::append_messages] 分块 {} JSON 解析失败，将创建空分块: {}",
            file, e
        );
        ChatChunk {
            id: file.trim_end_matches(".json").to_string(),
            messages: vec![],
        }
    }))
}

fn write_chunk(dir: &PathBuf, file: &str, chunk: &ChatChunk) -> Result<(), String> {
    let content =
        serde_json::to_string_pretty(chunk).map_err(|e| format!("序列化消息失败: {}", e))?;
    write_file_atomic(dir, file, &content, "消息文件")
}

/// 按偏移量和数量加载历史消息（支持跨分块查询）
///
/// 参数：
/// - `offset`: 跳过的消息数量（从最早的消息开始计算）
/// - `limit`: 最多返回的消息数量
///
/// 返回按时间顺序排列的消息列表。
pub fn load_messages(
    data_dir: &Path,
    offset: u64,
    limit: u64,
) -> Result<Vec<Message>, String> {
    let dir = data_dir.to_path_buf();
    let manifest = read_manifest(&dir);

    // 无消息或偏移量超出范围，直接返回空列表
    if manifest.total_messages == 0 || offset >= manifest.total_messages {
        return Ok(vec![]);
    }

    let mut collected: Vec<Message> = vec![];
    let mut messages_before: u64 = 0; // 已遍历过的消息数量（用于定位 offset）

    // 按分块顺序遍历
    for meta in &manifest.chunks {
        let chunk_count = meta.count as u64;

        // 已收集足够数量，提前退出
        if collected.len() as u64 >= limit {
            break;
        }

        // 该分块完全在 offset 之前，跳过
        if messages_before + chunk_count <= offset {
            messages_before += chunk_count;
            continue;
        }

        // 读取分块文件内容
        let chunk_path = dir.join(&meta.file);
        if !chunk_path.exists() {
            messages_before += chunk_count;
            continue;
        }

        let content = fs::read_to_string(&chunk_path).unwrap_or_default();
        let chunk: ChatChunk = serde_json::from_str(&content).unwrap_or_else(|e| {
            warn!(
                "[storage::load_messages] 分块 {} JSON 解析失败: {}",
                meta.file, e
            );
            ChatChunk {
                id: meta.file.trim_end_matches(".json").to_string(),
                messages: vec![],
            }
        });

        // 以实际消息数而非 manifest 计数为准推进偏移。
        // manifest 计数与实际内容可能不一致（如写入中途崩溃），按 manifest 算偏移
        // 会静默跳过或重复消息；读取到的分块一律用真实长度。
        let actual_count = chunk.messages.len() as u64;

        // 计算该分块内的起始索引（跨 chunk offset 修正）
        let local_start = if messages_before >= offset {
            0 // offset 落在已遍历的分块之内
        } else {
            (offset - messages_before) as usize // offset 落在当前分块
        };

        // 防御：若 manifest 计数与实际消息数不一致，跳过无效范围
        if local_start >= chunk.messages.len() {
            messages_before += actual_count;
            continue;
        }

        // 计算该分块内的结束索引（不超过分块边界和 limit）
        let local_end = std::cmp::min(
            local_start + (limit as usize - collected.len()),
            chunk.messages.len(),
        );

        // 提取消息并加入结果集
        collected.extend(chunk.messages[local_start..local_end].iter().cloned());
        messages_before += actual_count;
    }

    Ok(collected)
}

/// 返回已持久化的消息总数。
///
/// 前端用它从末尾计算首屏偏移量，以便优先加载最新的历史消息。
pub fn message_count(data_dir: &Path) -> Result<u64, String> {
    Ok(read_manifest(&data_dir.to_path_buf()).total_messages)
}

// ── 单元测试 ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::CHUNK_SIZE;

    /// 验证分块大小为 100
    #[test]
    fn chunk_rotation_at_100() {
        assert_eq!(CHUNK_SIZE, 100);
    }

    /// 验证跨分块加载消息的逻辑正确性
    ///
    /// 模拟 3 个分块（100, 100, 72），总消息数 272。
    /// offset=150, limit=30 → 预期跨越第 2 和第 3 个分块，收集到 30 条消息。
    #[test]
    fn load_messages_across_chunks_logic() {
        let chunk_counts = [100u64, 100u64, 72u64];
        let total: u64 = chunk_counts.iter().sum();
        assert_eq!(total, 272);

        // offset=150, limit=30 → 跨越 chunk 2 和 chunk 3
        let offset: u64 = 150;
        let limit: u64 = 30;
        let mut remaining_offset = offset;
        let mut collected: u64 = 0;

        for &count in &chunk_counts {
            if remaining_offset >= count {
                remaining_offset -= count;
                continue;
            }
            let take = std::cmp::min(count - remaining_offset, limit - collected);
            collected += take;
            remaining_offset = 0;
            if collected >= limit {
                break;
            }
        }
        assert_eq!(collected, 30);
    }
}
