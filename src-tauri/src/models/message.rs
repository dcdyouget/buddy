// ============================================================================
// 消息相关数据模型：Message、MessageRole、ModelInfo
// ============================================================================
//
// 这些 struct 会通过 serde 在以下边界之间来回转换：
//   1. 磁盘 JSON（持久化）             ←→ Rust struct   (serde::Deserialize/Serialize)
//   2. 前端 JS（IPC 事件 payload）     ←→ Rust struct   (serde, 经 Tauri 序列化)
//   3. 内存中不同模块之间传递          ←→ Rust struct   (直接 move)
//
// Rust vs Java 速览：
// - `struct` 在 Rust 中是"纯数据 + 行为通过 impl 块挂载"，不像 Java class 默认带继承
// - `derive` 是过程宏：编译器自动生成代码，等价于 Java 的 Lombok @Data / @AllArgsConstructor
// ============================================================================

// `use` 语句 = Java 的 import
use crate::streaming::ContentBlock;
use serde::{Deserialize, Serialize}; // serde 是 Rust 最主流的序列化框架 // 引入兄弟模块（crate:: = 当前 crate 根）

// ============================================================================
// ModelInfo —— 模型元信息
// ============================================================================
//
// Rust 语法：
// - `pub struct ModelInfo { ... }` 声明一个公开的结构体（Java 的 public class）
// - 字段默认 private，加 `pub` 才对外开放
// - 所有字段都直接写在 struct 体里，没有 Java 那样的 getter/setter
//
// `#[derive(...)]` 属性（attribute）：
// - 属性是 Rust 给编译器/宏的"注解"，写在它修饰的项之前，用 #[...] 包围
// - `Debug`     自动实现 std::fmt::Debug trait，可用于 println!("{:?}", x)
// - `Serialize` 自动实现 serde::Serialize，可序列化为 JSON
// - `Deserialize` 自动实现 serde::Deserialize，可从 JSON 反序列化
// - `Clone`     自动实现 clone() 方法（深拷贝）；Java 的 Cloneable 接口
//
// 对应到 Java：
// @Data @AllArgsConstructor @NoArgsConstructor @ToString @JsonSerialize @JsonDeserialize
// public class ModelInfo { ... }
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelInfo {
    pub id: String,              // 模型 ID，例如 "claude-sonnet-4-6"
    pub provider_id: String,     // 所属 provider 的 ID
    pub display_name: String,    // 在 UI 上展示给用户看的人类可读名
    pub context_window: u32,     // 上下文窗口大小（token 数）；u32 = 32 位无符号整数
    pub latency_ms: Option<u32>, // 测得的延迟（毫秒）；Option<T> 表示"可能没有值"
    //   - Some(120) = 有值 120
    //   - None      = 缺测
    //   这是 Rust 代替 null 的方式，类型系统强制你处理空值
    /// 用户确认该模型支持图片输入。老配置缺少此字段时按 false 处理。
    #[serde(default)]
    pub supports_vision: bool,
    /// 用户确认该模型可调用图片生成工具。老配置缺少此字段时按 false 处理。
    #[serde(default)]
    pub supports_image_generation: bool,
}

/// 一次工具调用的标识
///
/// 来自 assistant 消息的 `tool_calls` 字段。
/// `id` 由模型提供（OpenAI 是 `call_xxx`，Anthropic 是 `toolu_xxx`），
/// `name` 是被调用的工具名，`arguments` 是原始 JSON 字符串
/// （流式拼接得到，结构完整性由执行方校验）。
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// 原始 JSON 参数（流式拼装时可能不完整；执行时再 `serde_json::from_str` 校验）
    pub arguments: String,
}

/// 消息携带的图片附件：用户消息用于模型输入，tool 消息用于展示生成结果。
///
/// `data_url` 统一保存为 `data:<media_type>;base64,<data>`，Provider 适配层
/// 再转换成 OpenAI 或 Anthropic 各自的图片内容块。
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImageAttachment {
    pub id: String,
    pub name: String,
    pub media_type: String,
    pub data_url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    /// 用户消息用于模型输入，tool 消息用于展示生成结果。老消息缺少时保持为空。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageAttachment>,
    /// 用 serde 字段级属性控制序列化行为：
    /// - `default`                ：反序列化时若字段缺失，使用默认值（这里是 None）
    /// - `skip_serializing_if = "Option::is_none"` ：当值为 None 时，整个字段不出现在 JSON 中
    ///
    /// Java 中需要手写 Jackson 的 @JsonInclude(Include.NON_NULL) 来达到同样效果
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<ContentBlock>>,
    pub model_id: Option<String>,
    pub created_at: u64, // u64 = 64 位无符号；这里存放 Unix 时间戳（秒）

    // ── Tool 协议相关字段 ──
    // 都是 Option + skip_serializing_if, 老消息没有这些字段也能正常加载
    /// assistant 消息携带的 tool_calls
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// tool 消息携带:对应的 tool_call.id
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// tool 消息携带:工具名（调试/显示用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// tool 消息携带:执行是否出错（false = 成功，true = 错误）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    /// user 消息: 若设置, 表示这是对指定 assistant 消息的"回应",
    /// 在 UI 上嵌套渲染在父消息内部。 数据上仍作为独立 user 消息发给模型
    /// (让模型拿到完整上下文),仅用于前端按父 ID 嵌套。 持久化后 reload 仍生效。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_message_id: Option<String>,
}

// ============================================================================
// MessageRole —— 消息角色枚举
// ============================================================================
//
// Rust enum 与 Java enum 的最大区别：
// - Rust 的 enum 可以"携带数据"，每个 variant 可以拥有不同类型/数量的关联值
// - Java 的 enum 本质上是单例类的集合，只能挂常量字段，不能为每个 enum 值持有不同数据
//
// #[serde(rename_all = "lowercase")] 属性：
//   告诉 serde 把 Rust 的 PascalCase 变体名序列化成全小写字符串：
//     User       ↔  "user"
//     Assistant  ↔  "assistant"
//     Tool       ↔  "tool"
//   这样 JSON 里就是 `"role": "user"` 而不是 `"role": "User"`
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,      // 用户消息
    Assistant, // AI 回复消息
    Tool,      // 工具执行结果（携带 tool_call_id 指回对应的 tool_call）
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_image_capabilities_default_to_false_for_old_configs() {
        let model: ModelInfo = serde_json::from_value(serde_json::json!({
            "id": "legacy-model",
            "provider_id": "legacy-provider",
            "display_name": "Legacy Model",
            "context_window": 128000,
            "latency_ms": null
        }))
        .unwrap();

        assert!(!model.supports_vision);
        assert!(!model.supports_image_generation);
    }
}
