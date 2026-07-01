// 存储相关数据模型：Manifest、ChunkMeta、ChatChunk

use serde::{Deserialize, Serialize};
use super::message::Message;

/// 存储清单
#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub chunks: Vec<ChunkMeta>,
    pub total_messages: u64,
}

/// 单个分块元信息
#[derive(Debug, Serialize, Deserialize)]
pub struct ChunkMeta {
    pub file: String,
    pub count: u32,
}

/// 聊天消息分块
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatChunk {
    pub id: String,
    pub messages: Vec<Message>,
}
