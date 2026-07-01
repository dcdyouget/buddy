// Provider 模块
//
// 定义多模型提供商抽象层，灵感来源于 pi-agent 的 Provider/Model/Api 架构。
//
// 核心 trait：LlmProvider
// - stream_chat(): 发起流式对话，返回统一格式的 StreamEvent
// - fetch_models(): 获取可用模型列表
// - test_latency(): 测试端点延迟

pub mod anthropic;
pub mod openai_compatible;

use crate::models::{CompatConfig, Message, ModelInfo};
use crate::streaming::StreamEventEmitter;
use std::future::Future;
use tokio::sync::watch;

/// API 错误类型
#[derive(Debug)]
pub enum ApiError {
    /// 401 未授权
    Unauthorized,
    /// 429 配额超限
    QuotaExceeded,
    /// 服务端错误
    ServerError(u16),
    /// 网络错误
    NetworkError(String),
    /// 流式错误
    StreamError(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Unauthorized => write!(f, "401"),
            ApiError::QuotaExceeded => write!(f, "429"),
            ApiError::ServerError(code) => write!(f, "server_error({})", code),
            ApiError::NetworkError(msg) => write!(f, "network: {}", msg),
            ApiError::StreamError(msg) => write!(f, "stream: {}", msg),
        }
    }
}

/// Provider 类型枚举
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderType {
    /// OpenAI 兼容 API（/v1/chat/completions SSE）
    OpenAICompatible,
    /// Anthropic Messages API（/v1/messages SSE）
    Anthropic,
}

impl ProviderType {
    /// 从字符串解析 ProviderType（兼容旧配置，默认 OpenAI 兼容）
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "anthropic" => ProviderType::Anthropic,
            _ => ProviderType::OpenAICompatible, // 默认和向后兼容
        }
    }

    /// 转换为字符串（用于序列化）
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderType::OpenAICompatible => "openai_compatible",
            ProviderType::Anthropic => "anthropic",
        }
    }
}

/// LlmProvider trait：所有模型提供商的统一接口
///
/// 每个 Provider 实现负责：
/// 1. 将自身 API 的流式响应解析为统一的 StreamEvent
/// 2. 从自身 API 获取模型列表
/// 3. 测试端点延迟
pub trait LlmProvider: Send + Sync {
    /// 发起流式对话
    ///
    /// 参数：
    /// - `base_url`: API 基础 URL
    /// - `api_key`: API 密钥
    /// - `model`: 模型 ID
    /// - `messages`: 对话历史
    /// - `emitter`: 统一事件发射器
    /// - `cancel_rx`: 取消信号接收端
    /// - `compat`: 兼容性配置（可选，None 时使用默认行为）
    ///
    /// 返回累积的完整 AI 回复文本。
    fn stream_chat<'a>(
        &'a self,
        base_url: &'a str,
        api_key: &'a str,
        model: &'a str,
        messages: &'a [Message],
        emitter: &'a StreamEventEmitter,
        cancel_rx: watch::Receiver<bool>,
        compat: Option<&'a CompatConfig>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<String, ApiError>> + Send + 'a>>;

    /// 获取可用模型列表
    fn fetch_models<'a>(
        &'a self,
        base_url: &'a str,
        api_key: &'a str,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Vec<ModelInfo>, String>> + Send + 'a>>;

    /// 测试端点延迟（返回毫秒数）
    fn test_latency<'a>(
        &'a self,
        base_url: &'a str,
        api_key: &'a str,
        model_id: &'a str,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<u32, String>> + Send + 'a>>;
}

/// 根据 ProviderType 创建对应的 Provider 实例
pub fn create_provider(provider_type: &ProviderType) -> Box<dyn LlmProvider> {
    match provider_type {
        ProviderType::OpenAICompatible => Box::new(openai_compatible::OpenAICompatibleProvider),
        ProviderType::Anthropic => Box::new(anthropic::AnthropicProvider),
    }
}
