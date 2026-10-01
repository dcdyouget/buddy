//! 添加 Provider 的纯状态机。
//!
//! 该模块不触碰 engine 或 GPUI；异步请求由上层启动，再把带 revision 的结果交回。

use buddy_engine::models::{ModelInfo, ProviderConfig};
use std::collections::BTreeSet;

use super::provider_presets::{preset, CUSTOM_PRESET_ID};

#[path = "provider_form_model.rs"]
mod model;
#[cfg(test)]
#[path = "provider_form_tests.rs"]
mod tests;

const DEFAULT_PROTOCOL: &str = "openai_compatible";
const DEFAULT_THINKING_FORMAT: &str = "openai";
const DEFAULT_MAX_TOKENS_FIELD: &str = "max_tokens";
const EMPTY_MODELS_ERROR: &str = "未获取到模型列表（该厂商可能不支持 /models 端点）";

/// 当前异步操作。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Busy {
    #[default]
    /// 没有正在进行的异步操作。
    Idle,
    /// 正在获取模型列表。
    Fetch,
    /// 正在测试首个模型延迟。
    Latency,
    /// 正在保存 Provider 配置。
    Save,
}

/// 交给 engine 的一次请求快照。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderRequest {
    /// 发起请求时的表单 revision。
    pub revision: u64,
    /// 请求使用的 Base URL。
    pub url: String,
    /// 请求使用的 API Key。
    pub key: String,
    /// 请求使用的 Provider 协议。
    pub protocol: String,
}

/// 提交给配置合并层的 Provider 与已选模型。
#[derive(Clone, Debug)]
pub struct ProviderSubmission {
    /// 要写入配置的 Provider。
    pub provider: ProviderConfig,
    /// 已选中的模型，顺序决定默认模型。
    pub models: Vec<ModelInfo>,
}

/// 添加 Provider 的表单状态。
pub struct ProviderForm {
    // v1 的 customProviderType 独立于预设，切离自定义后仍保留。
    custom_protocol: String,
    /// 当前预设 ID；自定义服务使用 `custom`。
    pub preset: Option<String>,
    /// 当前协议 ID。
    pub protocol: String,
    /// Provider Base URL。
    pub url: String,
    /// Provider API Key。
    pub key: String,
    /// 是否显示明文 API Key。
    pub show_key: bool,
    /// Compat thinking 格式。
    pub thinking_format: String,
    /// Compat token 字段名。
    pub max_tokens_field: String,
    /// 用户是否改过 Compat 字段。
    pub compat_edited: bool,
    /// 最近一次成功拉取的模型列表。
    pub models: Vec<ModelInfo>,
    /// 当前选中的模型原始 ID。
    pub selected: BTreeSet<String>,
    /// 最近一次可展示的错误。
    pub error: Option<String>,
    /// 当前异步操作。
    pub busy: Busy,
    /// 连接参数版本，用于丢弃旧响应。
    pub revision: u64,
}

impl Default for ProviderForm {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderForm {
    /// 创建空白 Provider 表单。
    pub fn new() -> Self {
        Self {
            custom_protocol: DEFAULT_PROTOCOL.to_string(),
            preset: None,
            protocol: DEFAULT_PROTOCOL.to_string(),
            url: String::new(),
            key: String::new(),
            show_key: false,
            thinking_format: DEFAULT_THINKING_FORMAT.to_string(),
            max_tokens_field: DEFAULT_MAX_TOKENS_FIELD.to_string(),
            compat_edited: false,
            models: Vec::new(),
            selected: BTreeSet::new(),
            error: None,
            busy: Busy::Idle,
            revision: 0,
        }
    }

    /// 选择预设；切换连接目标会使此前请求的结果失效。
    pub fn select_preset(&mut self, id: &str) {
        self.preset = Some(id.to_string());
        if let Some(item) = preset(id) {
            self.protocol = item.protocol.to_string();
            self.url = item.url.to_string();
            self.thinking_format = if item.thinking_format.is_empty() {
                DEFAULT_THINKING_FORMAT.to_string()
            } else {
                item.thinking_format.to_string()
            };
            self.max_tokens_field = if item.max_tokens_field.is_empty() {
                DEFAULT_MAX_TOKENS_FIELD.to_string()
            } else {
                item.max_tokens_field.to_string()
            };
        } else if id == CUSTOM_PRESET_ID {
            self.protocol = self.custom_protocol.clone();
            self.url.clear();
            self.thinking_format = DEFAULT_THINKING_FORMAT.to_string();
            self.max_tokens_field = DEFAULT_MAX_TOKENS_FIELD.to_string();
        }
        self.compat_edited = false;
        self.invalidate_connection();
    }

    /// 设置 Base URL，并使此前请求的结果失效。
    pub fn set_url(&mut self, value: String) {
        if self.url != value {
            self.url = value;
            self.invalidate_connection();
        }
    }

    /// 设置 API Key，并使此前请求的结果失效。
    pub fn set_key(&mut self, value: String) {
        if self.key != value {
            self.key = value;
            self.invalidate_connection();
        }
    }

    /// 设置协议，并使此前请求的结果失效。
    pub fn set_protocol(&mut self, value: String) {
        if self.preset.as_deref() == Some(CUSTOM_PRESET_ID) {
            self.custom_protocol = value.clone();
        }
        if self.protocol != value {
            self.protocol = value;
            self.invalidate_connection();
        }
    }

    /// 设置 thinking Compat 字段并标记 Compat 已编辑。
    pub fn set_thinking_format(&mut self, value: String) {
        if self.thinking_format != value {
            self.thinking_format = value;
            self.compat_edited = true;
        }
    }

    /// 设置 token Compat 字段并标记 Compat 已编辑。
    pub fn set_max_tokens_field(&mut self, value: String) {
        if self.max_tokens_field != value {
            self.max_tokens_field = value;
            self.compat_edited = true;
        }
    }

    fn request(&self) -> ProviderRequest {
        ProviderRequest {
            revision: self.revision,
            url: self.url.clone(),
            key: self.key.clone(),
            protocol: self.protocol.clone(),
        }
    }

    fn invalidate_connection(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.models.clear();
        self.selected.clear();
        self.error = None;
        self.busy = Busy::Idle;
    }
}
