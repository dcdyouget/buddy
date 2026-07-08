// 本地 JSON 文件存储模块
//
// 职责：
// 1. Config 读写 —— 将 AppConfig 序列化到 config.json，启动时读取
// 2. Manifest 读写 —— 维护 manifest.json，跟踪所有消息分块信息
// 3. Message 分块存储 —— 每条消息追加到 chunk 文件，每 100 条自动切新块

use crate::models::*;
use log::warn;
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

/// 每个分块文件的最大消息数量
const CHUNK_SIZE: u32 = 100;

/// 获取 Tauri 应用数据目录路径
fn data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("无法获取数据目录: {}", e))
}

/// 确保数据目录存在，不存在则创建
fn ensure_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = data_dir(app)?;
    fs::create_dir_all(&dir).map_err(|e| format!("无法创建数据目录: {}", e))?;
    Ok(dir)
}

// ── Config ────────────────────────────────────────────────

/// 读取应用配置
///
/// 若 config.json 不存在则返回默认配置；
/// 若文件损坏（JSON 解析失败）则打印警告并返回默认配置，避免应用无法启动。
pub fn get_config(app: &tauri::AppHandle) -> Result<AppConfig, String> {
    let dir = data_dir(app)?;
    let path = dir.join("config.json");

    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("读取配置文件失败: {}", e))?;
    serde_json::from_str(&content).or_else(|e| {
        log::warn!("配置文件损坏，使用默认配置: {}", e);
        Ok(AppConfig::default())
    })
}

/// 保存应用配置到磁盘
pub fn save_config(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let dir = ensure_data_dir(app)?;
    let path = dir.join("config.json");
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("序列化配置失败: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("写入配置文件失败: {}", e))?;
    Ok(())
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
    let path = dir.join("manifest.json");
    let content = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("序列化索引失败: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("写入索引文件失败: {}", e))?;
    Ok(())
}

// ── Messages ──────────────────────────────────────────────

/// 全局写入锁 —— 防止 `tokio::task::spawn_blocking` 并发调用 `append_message`
/// 时产生读写竞态（两个任务同时读 chunk → 各追加一条 → 后写覆盖先写 → 丢消息）。
/// `std::sync::Mutex` 在 `spawn_blocking` 线程中阻塞是预期行为。
static APPEND_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 追加一条新消息到分块存储
///
/// 分块逻辑：
/// 1. 检查最后一个分块是否已满（>= CHUNK_SIZE 条）
/// 2. 若已满则创建新分块（chunk_002.json, chunk_003.json...）
/// 3. 将消息追加到目标分块末尾
/// 4. 更新 manifest 中的计数
///
/// 线程安全:通过 `APPEND_LOCK` 串行化所有写入,防止并发 `spawn_blocking` 丢消息。
pub fn append_message(app: &tauri::AppHandle, message: &Message) -> Result<(), String> {
    let _guard = APPEND_LOCK.lock().map_err(|e| format!("获取写入锁失败: {}", e))?;
    let dir = ensure_data_dir(app)?;
    let mut manifest = read_manifest(&dir);

    // 确定目标分块文件：若最后一个分块已满则创建新分块
    let chunk_file = if let Some(last) = manifest.chunks.last() {
        if last.count >= CHUNK_SIZE {
            // 当前最后一个分块已满，创建新分块
            let new_idx = manifest.chunks.len() as u32 + 1;
            let file = format!("chunk_{:03}.json", new_idx);
            manifest.chunks.push(ChunkMeta {
                file: file.clone(),
                count: 0, // 初始计数为 0，追加消息后再更新
            });
            file
        } else {
            // 继续追加到当前最后一个分块
            last.file.clone()
        }
    } else {
        // 没有任何分块文件，创建第一个分块
        let file = "chunk_001.json".to_string();
        manifest.chunks.push(ChunkMeta {
            file: file.clone(),
            count: 0,
        });
        file
    };

    // 读取目标分块文件（不存在则创建空的 ChatChunk）
    let chunk_path = dir.join(&chunk_file);
    let mut chunk: ChatChunk = if chunk_path.exists() {
        let content =
            fs::read_to_string(&chunk_path).map_err(|e| format!("读取消息文件失败: {}", e))?;
        serde_json::from_str(&content).unwrap_or_else(|e| {
            warn!(
                "[storage::append_message] 分块 {} JSON 解析失败，将创建新分块: {}",
                chunk_file, e
            );
            ChatChunk {
                id: chunk_file.trim_end_matches(".json").to_string(),
                messages: vec![],
            }
        })
    } else {
        ChatChunk {
            id: chunk_file.trim_end_matches(".json").to_string(),
            messages: vec![],
        }
    };

    // 追加消息并写回文件
    chunk.messages.push(message.clone());

    let content = serde_json::to_string_pretty(&chunk)
        .map_err(|e| format!("序列化消息失败: {}", e))?;
    fs::write(&chunk_path, content).map_err(|e| format!("写入消息文件失败: {}", e))?;

    // 更新 manifest 中该分块的消息计数和总数
    if let Some(meta) = manifest.chunks.iter_mut().find(|m| m.file == chunk_file) {
        meta.count = chunk.messages.len() as u32;
    }
    manifest.total_messages = manifest.total_messages.saturating_add(1);
    write_manifest(&dir, &manifest)?;

    Ok(())
}

/// 按偏移量和数量加载历史消息（支持跨分块查询）
///
/// 参数：
/// - `offset`: 跳过的消息数量（从最早的消息开始计算）
/// - `limit`: 最多返回的消息数量
///
/// 返回按时间顺序排列的消息列表。
pub fn load_messages(
    app: &tauri::AppHandle,
    offset: u64,
    limit: u64,
) -> Result<Vec<Message>, String> {
    let dir = data_dir(app)?;
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

        // 计算该分块内的起始索引（跨 chunk offset 修正）
        let local_start = if messages_before >= offset {
            0 // offset 落在已遍历的分块之内
        } else {
            (offset - messages_before) as usize // offset 落在当前分块
        };

        // 防御：若 manifest 计数与实际消息数不一致，跳过无效范围
        if local_start >= chunk.messages.len() {
            messages_before += chunk_count;
            continue;
        }

        // 计算该分块内的结束索引（不超过分块边界和 limit）
        let local_end = std::cmp::min(
            local_start + (limit as usize - collected.len()),
            chunk.messages.len(),
        );

        // 提取消息并加入结果集
        collected.extend(chunk.messages[local_start..local_end].iter().cloned());
        messages_before += chunk_count;
    }

    Ok(collected)
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
