// ============================================================================
// Provider 模块（多模型提供商抽象层）
// ============================================================================
//
// 灵感来源：pi-agent 的 Provider/Model/Api 架构
//
// 核心思路：所有模型提供商（OpenAI / Anthropic / DeepSeek / GLM / Kimi...）
// 都实现同一个 trait（接口），上层业务只跟 trait 打交道，不关心具体协议。
//
// Java 对比：
//   trait LlmProvider ≈ Java 8 的 interface LlmProvider
//   实现 trait ≈ class AnthropicProvider implements LlmProvider
//   Box<dyn LlmProvider> ≈ LlmProvider provider = new AnthropicProvider();
//                           （dyn = dynamic，运行时多态）
//
// 三大 trait 方法：
//   stream_chat() —— 发起流式对话，返回统一格式的 StreamEvent
//   fetch_models() —— 获取可用模型列表
//   test_latency() —— 测试端点延迟
// ============================================================================

pub mod anthropic;          // 声明并公开 anthropic 子模块（位于 ./anthropic.rs）
pub mod openai_compatible;  // 声明并公开 openai_compatible 子模块（位于 ./openai_compatible.rs）

// ============================================================================
// use 语句 —— 导入外部项
// ============================================================================
use crate::models::{CompatConfig, Message, ModelInfo};  // 引用本 crate 的 models 模块
use crate::streaming::{StreamEventEmitter, StreamOutcome};  // 引用本 crate 的 streaming 模块
use crate::tools::ToolDefinition;                      // 引用本 crate 的 tools 模块
use chrono::{DateTime, FixedOffset, Local};            // 当前本地时间与 UTC 偏移
use std::future::Future;                               // 标准库的 Future trait
use tokio::sync::watch;                                // tokio 异步运行时的 watch channel


/// 从 API 错误响应 JSON 中提取人类可读的错误消息
///
/// 尝试解析常见格式：
/// - `{"error": {"message": "..."}}`（OpenAI/MiniMax/DeepSeek 等）
/// - `{"error": {"error": {"message": "..."}}}`（Anthropic）
/// - 解析失败时回退到截断的原始文本
///
/// 函数签名解析：
/// - `pub fn`               公开函数
/// - `extract_error_message(body_text: &str) -> String`
///                          - 入参 `&str` 是字符串切片（不可变借用），≈ Java 的 `String` 视图
///                          - 返回 `String`（堆分配的、拥有所有权）
pub fn extract_error_message(body_text: &str) -> String {
    // `if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_text) { ... }`
    //
    // Rust 语法点：
    // - `serde_json::from_str::<T>` 显式指定泛型类型参数；Java 用 `fromStr<T>(...)`
    // - `from_str` 返回 `Result<T, E>` 类型：
    //     Ok(value) 解析成功；Err(e) 解析失败
    //   这是 Rust 代替异常的方式（无 try/catch、无检查异常）
    // - `if let Ok(...) = ...` 是模式匹配：只关心 Ok 分支，Err 分支跳过
    //   Java 等价：if (parseJson(body) instanceof Result.Ok) { var json = ...; }
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_text) {
        // ── json["error"]["message"] 这种索引语法是 serde_json::Value 的运算符重载 ──
        // - json["error"]     返回 Value；不存在该字段时返回 Value::Null
        // - .as_str()         尝试转成 &str；类型不对时返回 None
        // - 两层 Some(...)    因为索引也返回 Value（可能 Null），必须再 .as_str() 才能拿到 Option<&str>
        if let Some(msg) = json["error"]["message"].as_str() {
            // `msg.trim()` 返回 &str（仍借用 json 内部），`.to_string()` 转为新分配的 String
            let trimmed = msg.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        // Anthropic 错误格式：error.error.message（双层 error）
        if let Some(msg) = json["error"]["error"]["message"].as_str() {
            let trimmed = msg.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    // 回退：截断原始文本
    // `body_text.chars().take(200)` 字符迭代器取前 200 个字符
    //   - 注意 chars() 才是按 Unicode 码点取，bytes() 才是字节
    //   - Java 的 substring 在 Rust 没有等价的（字符串切片按字节边界）
    let preview: String = body_text.chars().take(200).collect();
    if preview.is_empty() {
        "未知错误".to_string()
    } else {
        preview
    }
}


/// API 错误类型
///
/// Rust enum 携带数据（variant）：
/// - `Unauthorized`     = 无关联数据（unit variant），类似 Java 的 `enum ApiError { Unauthorized }`
/// - `ServerError(u16, String)` = 元组 variant，携带 HTTP 状态码 + 消息
/// - `NetworkError(String)`     = 携带错误消息
///
/// Java 等价（用 sealed class 模拟）：
///     public sealed interface ApiError {
///         record Unauthorized() implements ApiError {}
///         record QuotaExceeded() implements ApiError {}
///         record ServerError(int code, String msg) implements ApiError {}
///         record NetworkError(String msg) implements ApiError {}
///     }
#[derive(Debug)]
pub enum ApiError {
    /// 401 未授权
    Unauthorized,
    /// 429 配额超限
    QuotaExceeded,
    /// 服务端错误（HTTP 状态码, API 返回的错误消息）
    ServerError(u16, String),
    /// 网络错误
    NetworkError(String),
}

// Display trait ≈ Java 的 toString()
// 实现 Display 后就可以用 println!("{}", err) 或 format!("{}", err) 输出
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // match 必须穷举所有 variant（编译器强制检查）
        // 等价于 Java 21 之后的 switch pattern matching
        match self {
            ApiError::Unauthorized => write!(f, "401 Unauthorized"),
            ApiError::QuotaExceeded => write!(f, "429 Quota Exceeded"),
            ApiError::ServerError(code, msg) => write!(f, "{} (HTTP {})", msg, code),
            ApiError::NetworkError(msg) => write!(f, "网络错误: {}", msg),
        }
    }
}


/// Provider 类型枚举（业务层用哪个变体去 dispatch 到对应实现）
///
/// 没有 #[serde] 属性，因此不会序列化；纯运行时分类用
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderType {
    /// OpenAI 兼容 API（/v1/chat/completions SSE）
    OpenAICompatible,
    /// Anthropic Messages API（/v1/messages SSE）
    Anthropic,
}

// impl 块可以挂在任何类型上 —— 给 ProviderType 加方法
impl ProviderType {
    /// 从字符串解析 ProviderType（兼容旧配置，默认 OpenAI 兼容）
    ///
    /// `ProviderType` 在外部以字符串形式存储于 `AppConfig::providers[].provider_type`；
    /// 解析时大小写不识别小写以外的大小写变体，未匹配的一律回落到 OpenAI 兼容模式以
    /// 向后兼容历史配置。
    pub fn from_str(s: &str) -> Self {
        // `match` 关键字是 Rust 的 switch：
        //   match 值 { 模式 => 表达式, ... }
        // `s.to_lowercase().as_str()` 把 String 转成 &str 才能在 match 中模式匹配
        // `_ => ...` 是通配分支，类似 Java 的 `default:`
        match s.to_lowercase().as_str() {
            "anthropic" => ProviderType::Anthropic,
            _ => ProviderType::OpenAICompatible, // 默认和向后兼容
        }
    }
}


// ============================================================================
// LlmProvider trait —— 所有模型提供商的统一接口
// ============================================================================
//
// Rust trait ≈ Java 8 的 interface（但更强大）
// 关键差异：
//   - 默认 trait 方法可以有默认实现（default fn ...），类似 Java 8 的 default 方法
//   - 关联类型（associated type）类似 Java 的泛型接口
//   - 没有继承，但可以通过 trait + trait bound 做组合
//
// `: Send + Sync` 是 trait bound，要求实现者必须同时实现 Send 和 Sync（线程安全标记）
//   - Send：可以跨线程移动所有权
//   - Sync：可以跨线程共享引用 &T
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
    /// - `tools`: 可用 tool 列表（空切片 = 不传 tool 字段，行为与无 tool 时一致）
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
        tools: &'a [ToolDefinition],
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<StreamOutcome, ApiError>> + Send + 'a>>;

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
///
/// 工厂函数（Factory Method 模式）
///
/// 返回 `Box<dyn LlmProvider>`：
///   - Box：堆装箱；dyn：动态分发；LlmProvider：trait 类型
///   ≈ Java 的 `LlmProvider provider = switch (type) { case A -> new AImpl(); case B -> new BImpl(); }`
///
/// 为什么用 Box<dyn ...> 而不是直接返回具体类型？
///   - 因为 ProviderType::OpenAICompatible 和 ProviderType::Anthropic 是不相关的具体类型
///   - 要返回"任意一种"，必须有共同抽象（trait 对象） + 装箱（Box）才能编译通过
///   - Java 里靠继承 + 多态直接 `return new XxxProvider();` 即可（接口返回）
pub fn create_provider(provider_type: &ProviderType) -> Box<dyn LlmProvider> {
    match provider_type {
        // Box::new(...) 把栈上的具体值搬到堆上
        // 不写 Box::new 的话返回类型不匹配（编译器会提示 size mismatch）
        ProviderType::OpenAICompatible => Box::new(openai_compatible::OpenAICompatibleProvider),
        ProviderType::Anthropic => Box::new(anthropic::AnthropicProvider),
    }
}
/// Buddy 系统提示词基础规则 —— 注入到所有对话的最前面。
/// 当前时间由 `current_system_prompt` 在请求模型时动态补充。
pub const BUDDY_SYSTEM_PROMPT: &str = r#"You are Buddy, an AI assistant with access to local tools. Reply in the user's language, be direct and accurate.

Use only the tools provided in this request. Never invent tool capabilities, file contents, command output, or execution results.

Use `ask_user` only when the user must choose between 2-4 materially different, mutually exclusive paths and you cannot infer a reasonable default. Do not use it for informational questions, simple confirmations, open-ended follow-ups, or choices with a clear default. A missing or existing file alone is not enough; use `ask_user` only if the user's choice changes the outcome.

When calling `ask_user`, write its question, header, options, and input placeholder in the user's language. Keep the header short, options concise and mutually exclusive. Set `requires_input` when an option needs a path, URL, name, or other value.

When you use `websearch`, ground the answer in the returned materials, add source links near the claims they support when practical, and end the final answer with a concise source list (use `数据来源` for Chinese replies). Format every source as a Markdown link such as `- [Source title](https://example.com/page)`. Never expose a bare URL, repeat the URL beside the link, or list a result you did not use.

For writes, use the appropriate file tool; the app handles approval. Do not claim that an operation succeeded until its tool result confirms it."#;

/// 使用指定时间生成完整系统提示词，便于测试并保证相对时间有明确基准。
pub fn build_system_prompt(now: DateTime<FixedOffset>) -> String {
    format!(
        "{BUDDY_SYSTEM_PROMPT}\n\nCurrent local date and time: {}.\nTreat this timestamp as the authoritative reference for relative dates such as \"today\", \"now\", and \"latest\". For time-sensitive web searches, include the explicit date when it improves accuracy.",
        now.format("%Y-%m-%d %H:%M:%S %:z")
    )
}

/// 在每次请求模型时读取本机当前时间，避免应用长时间运行后时间信息过期。
pub fn current_system_prompt() -> String {
    build_system_prompt(Local::now().fixed_offset())
}

#[cfg(test)]
mod system_prompt_tests {
    use super::build_system_prompt;
    use chrono::{FixedOffset, TimeZone};

    #[test]
    fn system_prompt_includes_explicit_local_time() {
        let offset = FixedOffset::east_opt(8 * 60 * 60).unwrap();
        let now = offset
            .with_ymd_and_hms(2026, 7, 30, 14, 5, 9)
            .single()
            .unwrap();

        let prompt = build_system_prompt(now);

        assert!(prompt.contains("2026-07-30 14:05:09 +08:00"));
        assert!(prompt.contains("\"today\", \"now\", and \"latest\""));
        assert!(prompt.contains("time-sensitive web searches"));
        assert!(prompt.contains("use `数据来源` for Chinese replies"));
        assert!(prompt.contains("[Source title](https://example.com/page)"));
        assert!(prompt.contains("Never expose a bare URL"));
    }
}
