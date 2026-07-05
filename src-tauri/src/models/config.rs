// 配置相关数据模型：AppConfig、Theme、ProviderConfig、CompatConfig

use serde::{Deserialize, Serialize};

/// 应用全局配置
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub theme: Theme,
    pub hotkey: String,
    pub providers: Vec<ProviderConfig>,
    pub models: Vec<super::message::ModelInfo>,
    pub selected_model_id: String,
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

/// 主题枚举
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
}

/// API Provider 配置
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub enabled_model_ids: Vec<String>,
    #[serde(default = "default_provider_type")]
    pub provider_type: String,
    #[serde(default)]
    pub compat: Option<CompatConfig>,
}

fn default_provider_type() -> String {
    "openai_compatible".to_string()
}

/// 兼容性配置
///
/// 用于在 OpenAI 兼容 API 上适配各 Provider 的私有协议差异（DeepSeek / GLM / Kimi 等）。
/// `Option<bool>` 形式存储，三态语义：
/// - `Some(true)` —— 强制开启
/// - `Some(false)` —— 强制关闭
/// - `None` —— 使用 getter 的默认值（一般 `true`，仅 stream_options_usage 例外）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompatConfig {
    #[serde(default)]
    pub thinking_format: Option<String>,
    #[serde(default)]
    pub max_tokens_field: Option<String>,
    #[serde(default)]
    pub supports_stream_options_usage: Option<bool>,
    #[serde(default)]
    pub supports_reasoning_effort: Option<bool>,
    #[serde(default)]
    pub supports_store: Option<bool>,
    #[serde(default)]
    pub supports_developer_role: Option<bool>,
    #[serde(default)]
    pub supports_temperature: Option<bool>,
}

impl CompatConfig {
    /// 当前请求使用何种思考字段格式(`openai` / `deepseek` / `qwen` 等)
    pub fn thinking_format(&self) -> &str {
        self.thinking_format.as_deref().unwrap_or("openai")
    }

    /// Provider 自定义的 token 上限字段名(`max_tokens` / `max_completion_tokens` 等)
    pub fn max_tokens_field(&self) -> &str {
        self.max_tokens_field.as_deref().unwrap_or("max_tokens")
    }

    /// 是否在请求体中追加 `stream_options.include_usage`(用于接收 usage chunk)
    pub fn supports_stream_options_usage(&self) -> bool {
        self.supports_stream_options_usage.unwrap_or(true)
    }

    /// 是否支持 `reasoning_effort` 参数(推理模型)
    pub fn supports_reasoning_effort(&self) -> bool {
        self.supports_reasoning_effort.unwrap_or(true)
    }

    /// 是否发送 `temperature` 参数(某些推理模型会拒绝)
    pub fn supports_temperature(&self) -> bool {
        self.supports_temperature.unwrap_or(true)
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
        }
    }
}
