// 数据模型模块：定义应用中所有核心数据结构，包括应用配置、Provider、模型信息、消息及持久化存储相关的结构体

use serde::{Deserialize, Serialize};

/// 应用全局配置
///
/// 存储用户的所有设置：主题、热键、Provider 列表、模型列表、当前选中模型、开机自启等。
/// 持久化到 `config.json` 文件中。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    /// 主题模式（浅色/深色）
    pub theme: Theme,
    /// 全局热键字符串，如 "CmdOrCtrl+J"
    pub hotkey: String,
    /// 已配置的 API Provider 列表
    pub providers: Vec<ProviderConfig>,
    /// 已获取的可用模型列表（扁平化存储，含 provider_id 关联）
    pub models: Vec<ModelInfo>,
    /// 当前选中的模型 ID
    pub selected_model_id: String,
    /// 是否开机自启
    pub auto_start: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: Theme::Light,
            hotkey: "CmdOrCtrl+J".into(),
            providers: vec![],
            models: vec![],
            selected_model_id: String::new(),
            auto_start: false,
        }
    }
}

/// 主题枚举（支持序列化为小写）
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
}

/// API Provider 配置
///
/// 一个 Provider 代表一个 OpenAI 兼容的 API 端点（如 OpenAI、DeepSeek 等）。
/// 包含连接信息和该 Provider 下启用的模型 ID 列表。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProviderConfig {
    /// Provider 唯一标识
    pub id: String,
    /// Provider 显示名称
    pub name: String,
    /// API 基础 URL（如 https://api.openai.com/v1）
    pub base_url: String,
    /// API 密钥（明文存储）
    pub api_key: String,
    /// 该 Provider 下启用的模型 ID 列表
    pub enabled_model_ids: Vec<String>,
}

/// 模型信息
///
/// 扁平化存储所有可用的 AI 模型，通过 provider_id 关联到对应的 Provider。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelInfo {
    /// 模型唯一标识（如 gpt-4o）
    pub id: String,
    /// 所属 Provider 的 ID
    pub provider_id: String,
    /// 模型显示名称
    pub display_name: String,
    /// 上下文窗口大小（token 数）
    pub context_window: u32,
    /// 测速延迟（毫秒），None 表示尚未测试
    pub latency_ms: Option<u32>,
}

/// 单条聊天消息
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    /// 消息唯一 ID
    pub id: String,
    /// 消息角色（用户或助手）
    pub role: MessageRole,
    /// 消息文本内容
    pub content: String,
    /// 生成该消息的模型 ID（用户消息可为 None）
    pub model_id: Option<String>,
    /// 消息创建时间戳（Unix 秒）
    pub created_at: u64,
}

/// 消息角色枚举（序列化为小写）
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
}

// ── 存储结构体 ──────────────────────────────────────────────

/// 存储清单：记录所有消息分块文件的元信息
///
/// 持久化到 `manifest.json`，用于快速定位消息所在的 chunk 文件。
#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    /// 分块文件列表（按顺序排列）
    pub chunks: Vec<ChunkMeta>,
    /// 消息总数
    pub total_messages: u64,
}

/// 单个分块文件的元信息
#[derive(Debug, Serialize, Deserialize)]
pub struct ChunkMeta {
    /// 分块文件名（如 chunk_001.json）
    pub file: String,
    /// 该分块中的消息数量
    pub count: u32,
}

/// 聊天消息分块
///
/// 每个分块文件最多存储 100 条消息。分块设计避免单文件过大导致的读写性能问题。
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatChunk {
    /// 分块唯一 ID（与文件名对应）
    pub id: String,
    /// 该分块中的所有消息
    pub messages: Vec<Message>,
}
