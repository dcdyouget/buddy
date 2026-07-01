// 数据模型模块：定义应用中所有核心数据结构，包括应用配置、Provider、模型信息、消息及持久化存储相关的结构体

use serde::{Deserialize, Serialize};
use crate::streaming::ContentBlock;

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
/// 一个 Provider 代表一个 AI 模型服务端点（OpenAI、Anthropic 等）。
/// 包含连接信息、提供商类型、兼容性配置和该 Provider 下启用的模型 ID 列表。
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
    /// 提供商类型（openai_compatible / anthropic）
    /// 用于选择正确的 API 适配器
    #[serde(default = "default_provider_type")]
    pub provider_type: String,
    /// 兼容性配置（可选）：用于适配不同厂商的 API 差异
    /// None 时使用 provider_type 对应的默认配置
    #[serde(default)]
    pub compat: Option<CompatConfig>,
}

/// 默认 Provider 类型（向后兼容旧配置）
fn default_provider_type() -> String {
    "openai_compatible".to_string()
}

// ── 兼容性配置（Compat）──────────────────────────────────

/// 兼容性配置：适配不同模型厂商的 API 差异
///
/// 灵感来源于 pi-agent 的 OpenAICompletionsCompat / AnthropicMessagesCompat。
/// 每个字段都是可选的，None 表示使用该 provider_type 的默认行为。
///
/// 不同厂商即使使用同一个 API 格式（如 OpenAI chat/completions），
/// 在以下方面可能存在差异：
/// - thinking/reasoning 参数格式
/// - max_tokens 字段名
/// - stream_options 支持情况
/// - temperature 支持
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompatConfig {
    // ── 通用字段 ──

    /// 思考/推理参数的发送格式
    ///
    /// - "openai" (默认): reasoning_effort 字段
    /// - "deepseek": thinking: { type: "enabled"/"disabled" } + 可选 reasoning_effort
    /// - "openrouter": reasoning: { effort: "..." }
    /// - "qwen": enable_thinking: true/false
    /// - "together": reasoning: { enabled: true/false } + 可选 reasoning_effort
    /// - "zai": thinking: { type: "enabled"/"disabled" }
    #[serde(default)]
    pub thinking_format: Option<String>,

    // ── OpenAI 兼容 API 字段 ──

    /// 最大 token 数字段名
    ///
    /// - "max_tokens" (默认): 标准 OpenAI 字段
    /// - "max_completion_tokens": 部分新 OpenAI 模型使用
    #[serde(default)]
    pub max_tokens_field: Option<String>,

    /// 是否支持 stream_options: { include_usage: true }
    /// 用于在流式响应中获取 token 用量统计
    #[serde(default)]
    pub supports_stream_options_usage: Option<bool>,

    /// 是否支持 reasoning_effort 字段
    /// 部分兼容厂商不支持此字段，即使使用 thinking_format 发送 thinking 参数
    #[serde(default)]
    pub supports_reasoning_effort: Option<bool>,

    /// 是否支持 store 字段（OpenAI 的持久化存储功能）
    #[serde(default)]
    pub supports_store: Option<bool>,

    /// 是否支持 developer 角色（替代 system 角色）
    #[serde(default)]
    pub supports_developer_role: Option<bool>,

    // ── Anthropic API 字段 ──

    /// 是否支持 temperature 参数
    /// Claude Opus 4.7+ 拒绝非默认的 temperature 值
    #[serde(default)]
    pub supports_temperature: Option<bool>,

    /// 是否支持长缓存保留（1h TTL）
    #[serde(default)]
    pub supports_long_cache_retention: Option<bool>,
}

impl CompatConfig {
    /// 获取 thinking_format，带默认值回退
    pub fn thinking_format(&self) -> &str {
        self.thinking_format.as_deref().unwrap_or("openai")
    }

    /// 获取 max_tokens_field，带默认值回退
    pub fn max_tokens_field(&self) -> &str {
        self.max_tokens_field.as_deref().unwrap_or("max_tokens")
    }

    /// 获取 supports_stream_options_usage，带默认值回退
    pub fn supports_stream_options_usage(&self) -> bool {
        self.supports_stream_options_usage.unwrap_or(true)
    }

    /// 获取 supports_reasoning_effort，带默认值回退
    pub fn supports_reasoning_effort(&self) -> bool {
        self.supports_reasoning_effort.unwrap_or(true)
    }

    /// 获取 supports_temperature，带默认值回退
    pub fn supports_temperature(&self) -> bool {
        self.supports_temperature.unwrap_or(true)
    }

    /// 获取 supports_long_cache_retention，带默认值回退
    pub fn supports_long_cache_retention(&self) -> bool {
        self.supports_long_cache_retention.unwrap_or(true)
    }
}

impl Default for CompatConfig {
    fn default() -> Self {
        Self {
            thinking_format: None,
            max_tokens_field: None,
            supports_stream_options_usage: None,
            supports_reasoning_effort: None,
            supports_store: None,
            supports_developer_role: None,
            supports_temperature: None,
            supports_long_cache_retention: None,
        }
    }
}

/// 内置 Provider Compat 预设
///
/// 为已知的模型厂商提供默认的兼容性配置。
/// 用户添加 Provider 时自动应用对应预设。
impl CompatConfig {
    /// 根据 provider_id 获取默认 compat 预设
    pub fn preset_for(provider_id: &str) -> Option<Self> {
        match provider_id.to_lowercase().as_str() {
            // DeepSeek: thinkingFormat=deepseek, 不支持 store/developer_role
            "deepseek" => Some(Self {
                thinking_format: Some("deepseek".into()),
                supports_store: Some(false),
                supports_developer_role: Some(false),
                ..Default::default()
            }),

            // MiniMax: 标准 OpenAI 格式
            "minimax" => Some(Self {
                supports_stream_options_usage: Some(false),
                ..Default::default()
            }),

            // GLM (智谱): thinkingFormat=qwen 风格
            "glm" => Some(Self {
                thinking_format: Some("qwen".into()),
                supports_stream_options_usage: Some(false),
                ..Default::default()
            }),

            // Kimi (月之暗面): 标准 OpenAI 格式，不使用 store
            "kimi" => Some(Self {
                supports_store: Some(false),
                ..Default::default()
            }),

            // MiMo (小米): thinkingFormat=deepseek
            "mimo" => Some(Self {
                thinking_format: Some("deepseek".into()),
                supports_store: Some(false),
                ..Default::default()
            }),

            // Anthropic: 使用 supports_temperature (部分模型不支持)
            "anthropic" => Some(Self {
                supports_temperature: Some(true),
                supports_long_cache_retention: Some(true),
                ..Default::default()
            }),

            // OpenAI: 全默认
            "openai" => Some(Self::default()),

            _ => None,
        }
    }
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
    /// 消息文本内容（含 <think> 标签的原始文本）
    pub content: String,
    /// 结构化内容块（从 content 中解析 <think> 标签得到）
    /// None 表示尚未解析（历史数据兼容）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<ContentBlock>>,
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
