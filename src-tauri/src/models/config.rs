// ============================================================================
// 配置相关数据模型：AppConfig、Theme、ProviderConfig、CompatConfig
// ============================================================================
//
// 这些 struct 的全部字段都会被 serde 序列化到磁盘 JSON 中；
// 也是 Tauri IPC 与前端共享的数据结构。
//
// Rust vs Java 速览：
// - `impl Default for X { fn default() -> Self { ... } }`
//     是 Rust 给 struct 提供"无参构造默认值"的惯用写法
//     ≈ Java 的 `public X() { this.theme = Theme.Light; ... }` 或 Lombok 的 `@Builder @Default`
// - `#[serde(default = "fn_name")]` 告诉 serde：反序列化时若字段缺失，调用该函数得到默认值
// ============================================================================

use serde::{Deserialize, Serialize};

use super::mcp::McpServerConfig;

/// 应用全局配置（持久化到磁盘的根 JSON 对象）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    // 所有字段都带 serde 默认值：老版本/半写入的 config.json 缺字段时用默认值补齐，
    // 而不是整个解析失败回退成默认配置、下一次保存把用户的真实配置覆盖掉。
    #[serde(default = "default_theme")]
    pub theme: Theme, // 当前主题
    #[serde(default = "default_hotkey")]
    pub hotkey: String, // 全局快捷键字符串，如 "CmdOrCtrl+J"
    #[serde(default)]
    pub providers: Vec<ProviderConfig>, // 用户配置的所有 API 提供商
    // Vec<T> ≈ Java 的 ArrayList<T>，但栈上是指针
    #[serde(default)]
    pub models: Vec<super::message::ModelInfo>, // 已知模型列表（含 context_window 等）
    #[serde(default)]
    pub selected_model_id: String, // UI 当前选中的模型 ID
    #[serde(default)]
    pub auto_start: bool, // 开机自启动

    // ── Tool / MCP 相关字段 ──
    // 缺省时使用 vec![] —— 老 config.json 无这些字段也能正常加载
    /// write 类 tool 允许写入的路径前缀列表（白名单）
    /// 空数组 = 不限制（向后兼容）
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    /// MCP server 配置列表
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

fn default_theme() -> Theme {
    Theme::Light
}

fn default_hotkey() -> String {
    "CmdOrCtrl+J".to_string()
}

// 给 AppConfig 提供默认值；新建配置文件时使用
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: Theme::Light,
            hotkey: "CmdOrCtrl+J".into(),
            providers: vec![],
            models: vec![],
            selected_model_id: String::new(),
            auto_start: false,
            allowed_paths: vec![],
            mcp_servers: vec![],
        }
    }
}

/// 主题枚举
///
/// `#[serde(rename_all = "lowercase")]` 让 Light/Dark 序列化为 "light"/"dark"
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
}

/// API Provider 配置（一个用户填写的 API key 就是一个 ProviderConfig）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProviderConfig {
    pub id: String,                     // UUID 或人工填写的 ID
    pub name: String,                   // 用户起的显示名
    pub base_url: String,               // API 基础 URL（如 https://api.openai.com）
    pub api_key: String,                // API 密钥（v0.1 明文存储，见 CLAUDE.md 约束 #9）
    pub enabled_model_ids: Vec<String>, // 该 Provider 下用户勾选启用的模型

    // `#[serde(default = "default_provider_type")]`：
    //   反序列化时若 JSON 缺这个字段，调用下面的 default_provider_type() 函数取值
    //   作用：老配置没有 provider_type 字段也能正常加载（向后兼容）
    #[serde(default = "default_provider_type")]
    pub provider_type: String, // "openai_compatible" 或 "anthropic"

    // `Option<CompatConfig>`：可空；serde(default) 在缺字段时给 None
    //   Java 8 之前没有 Optional；Java 8+ 的 Optional 与此用法类似但语义不同
    #[serde(default)]
    pub compat: Option<CompatConfig>,
}

// `#[serde(default = "...")]` 引用的辅助函数
// 必须返回 `String`（与字段类型一致）
// 没有 `pub`，仅模块内使用
fn default_provider_type() -> String {
    "openai_compatible".to_string()
}

/// 兼容性配置
///
/// 用于在 OpenAI 兼容 API 上适配各 Provider 的私有协议差异（DeepSeek / GLM / Kimi 等）。
/// `Option<bool>` 形式存储，三态语义：
/// - `Some(true)`  —— 强制开启
/// - `Some(false)` —— 强制关闭
/// - `None`        —— 使用 getter 的默认值（一般 `true`，仅 stream_options_usage 例外）
///
/// Rust `Option<bool>` 三态 vs Java 的 `Boolean`：
//  - `Boolean` 可空，性能差，不强制处理空值
//  - `Option<bool>` 不可空且强制处理 None 情况，更安全
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompatConfig {
    // 多个字段都标了 `#[serde(default)]`，JSON 缺字段时使用 Option 的默认值 None
    #[serde(default)]
    pub thinking_format: Option<String>, // 思考参数格式："openai"/"deepseek"/"qwen"/...
    #[serde(default)]
    pub max_tokens_field: Option<String>, // token 上限字段名："max_tokens" / "max_completion_tokens"
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
    pub supports_tools: Option<bool>,
    /// 前端类型里存在但此前 Rust 侧缺失的字段；补上避免保存配置时被 serde 静默丢弃
    #[serde(default)]
    pub supports_long_cache_retention: Option<bool>,
}

// impl 块：给 CompatConfig 添加方法（类似 Java 的 getter）
// Rust 没有强制 getter；这些是"带默认值的读取方法"
impl CompatConfig {
    /// 当前请求使用何种思考字段格式("openai" / "deepseek" / "qwen" 等)
    pub fn thinking_format(&self) -> &str {
        // `self.thinking_format.as_deref()`：Option<String> → Option<&str>
        //   Some(s) → Some(s.as_str())
        //   None    → None
        // `.unwrap_or("openai")`：None 时返回默认值
        self.thinking_format.as_deref().unwrap_or("openai")
    }

    /// Provider 自定义的 token 上限字段名("max_tokens" / "max_completion_tokens" 等)
    pub fn max_tokens_field(&self) -> &str {
        self.max_tokens_field.as_deref().unwrap_or("max_tokens")
    }

    /// 是否在请求体中追加 `stream_options.include_usage`(用于接收 usage chunk)
    pub fn supports_stream_options_usage(&self) -> bool {
        self.supports_stream_options_usage.unwrap_or(true)
    }

    /// 是否支持 `reasoning_effort` 参数(推理模型)
    #[allow(dead_code)] // 将来推理模型配置中使用
    pub fn supports_reasoning_effort(&self) -> bool {
        self.supports_reasoning_effort.unwrap_or(true)
    }

    /// 是否发送 `temperature` 参数(某些推理模型会拒绝)
    pub fn supports_temperature(&self) -> bool {
        self.supports_temperature.unwrap_or(true)
    }

    pub fn supports_tools(&self) -> bool {
        self.supports_tools.unwrap_or(true)
    }

    #[allow(dead_code)] // 预留：前端类型中有此字段，保持与前端一致
    pub fn supports_long_cache_retention(&self) -> bool {
        self.supports_long_cache_retention.unwrap_or(true)
    }
}

// 给 CompatConfig 提供默认值（用于 `Option<CompatConfig>` 的兜底场景）
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
            supports_tools: None,
            supports_long_cache_retention: None,
        }
    }
}
