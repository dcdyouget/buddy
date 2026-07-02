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
    #[serde(default)]
    pub supports_long_cache_retention: Option<bool>,
}

impl CompatConfig {
    pub fn thinking_format(&self) -> &str {
        self.thinking_format.as_deref().unwrap_or("openai")
    }

    pub fn max_tokens_field(&self) -> &str {
        self.max_tokens_field.as_deref().unwrap_or("max_tokens")
    }

    pub fn supports_stream_options_usage(&self) -> bool {
        self.supports_stream_options_usage.unwrap_or(true)
    }

    pub fn supports_reasoning_effort(&self) -> bool {
        self.supports_reasoning_effort.unwrap_or(true)
    }

    pub fn supports_temperature(&self) -> bool {
        self.supports_temperature.unwrap_or(true)
    }

    #[allow(dead_code)]
    pub fn supports_long_cache_retention(&self) -> bool {
        self.supports_long_cache_retention.unwrap_or(true)
    }

    /// 根据 provider_id 获取默认 compat 预设
    #[allow(dead_code)]
    pub fn preset_for(provider_id: &str) -> Option<Self> {
        match provider_id.to_lowercase().as_str() {
            "deepseek" => Some(Self {
                thinking_format: Some("deepseek".into()),
                supports_store: Some(false),
                supports_developer_role: Some(false),
                ..Default::default()
            }),
            "minimax" => Some(Self {
                supports_stream_options_usage: Some(false),
                ..Default::default()
            }),
            "glm" => Some(Self {
                thinking_format: Some("qwen".into()),
                supports_stream_options_usage: Some(false),
                ..Default::default()
            }),
            "kimi" => Some(Self {
                supports_store: Some(false),
                ..Default::default()
            }),
            "mimo" => Some(Self {
                thinking_format: Some("deepseek".into()),
                supports_store: Some(false),
                ..Default::default()
            }),
            "anthropic" => Some(Self {
                supports_temperature: Some(true),
                supports_long_cache_retention: Some(true),
                ..Default::default()
            }),
            "openai" => Some(Self::default()),
            _ => None,
        }
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
