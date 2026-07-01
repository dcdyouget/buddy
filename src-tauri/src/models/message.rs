// 消息相关数据模型：Message、MessageRole、ModelInfo

use serde::{Deserialize, Serialize};
use crate::streaming::ContentBlock;

/// 模型信息
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub provider_id: String,
    pub display_name: String,
    pub context_window: u32,
    pub latency_ms: Option<u32>,
}

/// 单条聊天消息
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<ContentBlock>>,
    pub model_id: Option<String>,
    pub created_at: u64,
}

/// 消息角色枚举
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
}
