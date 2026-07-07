// ============================================================================
// Tool 系统 —— 内置 tool + MCP tool 统一抽象
// ============================================================================
//
// 核心抽象：
// - Tool trait:所有可调用工具的统一接口(内置 + MCP 共享)
// - ToolDefinition:发给 LLM 的工具 schema(OpenAI 兼容的 JSON Schema)
// - ToolRegistry:聚合内置 + 所有 MCP 工具,提供按名查找 + 执行
// - ToolSafety:ReadOnly / Write 分类,决定是否需要用户审批
//
// P1 阶段:只搭骨架,内置 tool 在 P2 实现,MCP tool 在 P6-P7 实现。
// LlmProvider trait 在 P1 接受 tools 参数但暂不解析 tool_calls(P3-P5)。
// ============================================================================

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub mod builtin;
#[allow(unused_imports)]
pub use builtin::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolSafety {
    #[default]
    ReadOnly,
    Write,
}

/// Tool 执行上下文
///
/// 携带审批、取消信号等运行时信息,内置 tool 不需要全部用到,
/// 但 MCP 包装的 tool 可能用到(目前 P6 暂不实现,留接口)。
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    /// 当前 turn 是否处于"本次都允许"模式
    /// (P4 实现审批状态机后由 commands::send_message 注入)
    #[allow(dead_code)] // MCP tool 接入后会使用
    pub approve_all_for_turn: bool,
}

/// Tool 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// 文本内容(直接发给 model 作为 tool_result)
    pub content: String,
    /// true = 执行出错,model 会看到 is_error=true 并据此调整
    /// (Q6 决策:不中断整轮,让 model 自适应)
    pub is_error: bool,
}

impl ToolOutput {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
        }
    }

    #[allow(dead_code)] // 公共 API，MCP tool 集成后使用
    pub fn err(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
        }
    }
}

/// Tool 执行错误
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("参数错误: {0}")]
    InvalidArgs(String),
    #[error("权限拒绝: {0}")]
    PermissionDenied(String),
    #[error("I/O 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),
    #[allow(dead_code)] // MCP tool 接入后使用
    #[error("MCP 错误: {0}")]
    Mcp(String),
    #[allow(dead_code)] // 预留通用错误类型
    #[error("其他: {0}")]
    Other(String),
}

/// 工具定义 —— 发给 LLM 的 schema
///
/// 字段对齐 OpenAI tool 格式:
///   `{ "type": "function", "function": { "name", "description", "parameters" } }`
/// Anthropic 在 anthropic.rs 转换层重新映射到 `{ name, description, input_schema }`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// 工具名(全局唯一)
    pub name: String,
    /// 给 LLM 看的工具描述
    pub description: String,
    /// OpenAI 兼容的 JSON Schema(Anthropic 转换层会原封不动传给 input_schema)
    pub parameters: Value,
    /// 安全分类(本地用,不发给 LLM)
    #[serde(skip)]
    #[allow(dead_code)] // 供 Tool trait definition() 方法设置，commands 中通过 trait safety() 读取
    pub safety: ToolSafety,
}

/// Tool trait —— 所有可调用工具的统一接口
///
/// 实现要求:
/// - name() 返回全局唯一的工具名
/// - definition() 返回发给 LLM 的 schema
/// - safety() 返回安全分类(决定是否需要审批)
/// - execute() 真正执行,args 是 LLM 流式拼接后的 JSON 字符串
///
/// 内置 tool 直接实现(见 builtin 子模块)
/// MCP tool 通过 McpClientAdapter 间接实现
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    fn safety(&self) -> ToolSafety;

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolOutput, ToolError>;

    /// 派生的 ToolDefinition(给 LLM 看)
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters_schema(),
            safety: self.safety(),
        }
    }
}

/// Tool 注册表
///
/// 聚合内置 tool + 所有 MCP server tool。
/// 新消息开始时调用一次 `aggregate()` 收集 tool 列表(决策:不热更新)。
pub struct ToolRegistry {
    /// 启动时注册的内置 tool
    builtin: HashMap<String, Arc<dyn Tool>>,
    /// 当前活跃的 MCP tool(每次 send_message 开始时重建)
    mcp: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new(builtin: Vec<Arc<dyn Tool>>) -> Self {
        let mut map = HashMap::new();
        for t in builtin {
            map.insert(t.name().to_string(), t);
        }
        Self {
            builtin: map,
            mcp: HashMap::new(),
        }
    }

    /// 注入 MCP tool(在 send_message 开始时调用)
    #[allow(dead_code)] // P7 MCP 集成时使用
    pub fn set_mcp_tools(&mut self, tools: Vec<Arc<dyn Tool>>) {
        self.mcp.clear();
        for t in tools {
            self.mcp.insert(t.name().to_string(), t);
        }
    }

    /// 按名查找(mcp 优先,因为它可能覆盖内置 tool 名)
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.mcp.get(name).or_else(|| self.builtin.get(name)).cloned()
    }

    /// 收集所有 tool 的 definition(发给 LLM)
    pub fn all_definitions(&self) -> Vec<ToolDefinition> {
        let mut defs: Vec<ToolDefinition> = self
            .builtin
            .values()
            .map(|t| t.definition())
            .chain(self.mcp.values().map(|t| t.definition()))
            .collect();
        // 按 name 排序,保证发给 LLM 的顺序稳定(便于缓存和调试)
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        defs
    }

    /// 总 tool 数
    #[allow(dead_code)] // P7 MCP 集成后前端可能查询
    pub fn len(&self) -> usize {
        self.builtin.len() + self.mcp.len()
    }

    #[allow(dead_code)] // P7 MCP 集成后使用
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct DummyTool;
    #[async_trait]
    impl Tool for DummyTool {
        fn name(&self) -> &str { "dummy" }
        fn description(&self) -> &str { "test" }
        fn parameters_schema(&self) -> Value { json!({"type": "object"}) }
        fn safety(&self) -> ToolSafety { ToolSafety::ReadOnly }
        async fn execute(&self, _args: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput::ok("dummy"))
        }
    }

    #[test]
    fn test_registry_builtin_lookup() {
        let reg = ToolRegistry::new(vec![Arc::new(DummyTool)]);
        assert!(reg.get("dummy").is_some());
        assert!(reg.get("missing").is_none());
    }

    #[test]
    fn test_registry_mcp_overrides_builtin() {
        struct OverrideTool;
        #[async_trait]
        impl Tool for OverrideTool {
            fn name(&self) -> &str { "dummy" }
            fn description(&self) -> &str { "override" }
            fn parameters_schema(&self) -> Value { json!({}) }
            fn safety(&self) -> ToolSafety { ToolSafety::ReadOnly }
            async fn execute(&self, _: Value, _: ToolContext) -> Result<ToolOutput, ToolError> {
                Ok(ToolOutput::ok("mcp"))
            }
        }
        let mut reg = ToolRegistry::new(vec![Arc::new(DummyTool)]);
        reg.set_mcp_tools(vec![Arc::new(OverrideTool)]);
        let t = reg.get("dummy").unwrap();
        assert_eq!(t.description(), "override", "MCP tool should override builtin with same name");
    }

    #[test]
    fn test_definitions_sorted() {
        let reg = ToolRegistry::new(vec![Arc::new(DummyTool)]);
        let defs = reg.all_definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "dummy");
    }

    #[test]
    fn test_tool_output_constructors() {
        let ok = ToolOutput::ok("hi");
        assert!(!ok.is_error);
        let err = ToolOutput::err("bad");
        assert!(err.is_error);
    }
}
