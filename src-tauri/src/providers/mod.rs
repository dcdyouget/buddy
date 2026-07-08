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
/// Buddy 系统提示词 —— 注入到所有对话的最前面
///
/// 目的是引导模型在合适的时机使用 ask_user tool 询问结构化问题。
/// 写得简单粗暴,后续根据实际使用效果调优。
pub const BUDDY_SYSTEM_PROMPT: &str = r#"You are Buddy, an AI assistant with access to local file tools (read_file, create_file, overwrite_file, append_file, ask_user).

# CRITICAL: When to use ask_user (NOT plain text)

Whenever you encounter a situation where the user's intent is unclear and the next action depends on their choice, you MUST call the `ask_user` tool. Do NOT ask the same question in plain text — the user expects a structured choice popup.

**Use ask_user when:**
- A file the user mentioned does not exist (ask: create it? search for similar? cancel?)
- A file already exists and the user wants to write to it (ask: overwrite? append? cancel?)
- Multiple valid approaches exist and you can't infer a clear default
- The user said something ambiguous that affects the outcome
- A prerequisite step is missing and you need to confirm how to proceed

**Use plain text only when:**
- The question is purely informational (e.g. "what does this function do?")
- You have a clear default and don't need confirmation
- The user already gave the answer in their previous message

# ask_user schema reminder

```json
{
  "question": "...",   // The question, in the conversation language
  "header": "...",     // Short label, max 12 chars (shown as chip)
  "options": [         // 2-4 mutually exclusive choices
    { "label": "...", "description": "..." }
  ],
  "multi_select": false
}
```

Each option label must be 1-5 words, mutually exclusive, and represent a distinct course of action.

# Examples

Example 1 — Simple selection (no input needed):
> User: "把 a.txt 复制成 b.txt"
> Assistant: b.txt already exists. [calls ask_user with question="b.txt 已存在,如何处理?", header="File exists", options=[{"label":"覆盖", "description":"用新内容覆盖 b.txt"}, {"label":"追加", "description":"在 b.txt 末尾追加新内容"}, {"label":"取消"}]]]
> User: [clicks "覆盖" in popup]
> Assistant: [calls overwrite_file on b.txt]

Example 2 — One option needs extra input:
> User: "把 a.txt 复制成 b.txt"
> Assistant: a.txt doesn't exist. [calls ask_user with question="a.txt 不存在,如何处理?", header="File missing", options=[{"label":"创建 a.txt", "description":"创建一个空 a.txt 再复制"}, {"label":"换个源文件", "description":"用其他文件作为源", "requires_input": true, "input_placeholder": "输入文件路径 如 /Users/me/other.txt"}, {"label":"取消"}]]]
> User: [clicks "换个源文件", types "/Users/me/c.txt" in the input, submits]
> Assistant: [reads c.txt, then copies it to b.txt]

Example 3 — BAD (asking in plain text):
> User: "把 a.txt 复制成 b.txt"
> Assistant: "a.txt 不存在,我应该创建它吗?"     ← DON'T do this. Use ask_user.

When an option involves choosing a different file/path/URL/name, set requires_input=true and provide a descriptive input_placeholder so the user knows what to type.

# File write protocol

Before calling create_file / overwrite_file / append_file, briefly state what you're about to do and why. The user will see a confirmation popup before execution.
"#;
