// 内置问答工具与通用工具注册。图片查看按当前对话附件单独注册。

use super::{Tool, ToolContext, ToolError, ToolOutput, ToolSafety};
use async_trait::async_trait;
use serde_json::{Value, json};

// ─────────────────────────────────────────────────────────────────────────────
// ask_user — 让模型可以向用户提出选择题
// ─────────────────────────────────────────────────────────────────────────────
//
// 这个 tool 不执行外部操作 — 它的作用是阻塞当前 turn 的
// 工具循环,直到前端用户在内联问答卡里做出选择。编排层 (`chat::ChatEngine::send_message`) 中
// 识别到 ask_user 调用时,会:
//   1. 发射 ToolQuestionRequired 事件(让前端显示内联问答卡)
//   2. 等待 answer_tool_question 命令 invoke,把答案作为 tool result 写回
//
// 因此 execute() 永远不会被调用 — 如果走到这里,说明调用链出错,
// 返回 error 让 model 看到失败。
//
// ToolSafety 用 ReadOnly — 走完正常 tool 循环不会被 ApprovalModal 拦截,
// 由 `chat::ChatEngine::send_message` 里的专门分支处理。

/// ask_user 的参数 schema(JSON,直接发给 LLM)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AskUserArgs {
    /// 要问用户的问题
    pub question: String,
    /// 2-4 个互斥选项
    pub options: Vec<AskUserOption>,
    /// 是否允许多选(默认 false)
    #[serde(default)]
    pub multi_select: bool,
    /// 短标签(显示在 chip 上,最长 12 字符)
    pub header: String,
}

/// ask_user 的单个选项
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AskUserOption {
    /// 1-5 词的简短标签
    pub label: String,
    /// 可选说明
    #[serde(default)]
    pub description: String,
    /// 此选项是否需要用户补充输入(如"其他主题"时让用户补充说明)
    #[serde(default)]
    pub requires_input: bool,
    /// 输入框的占位符(仅在 requires_input=true 时生效)
    #[serde(default)]
    pub input_placeholder: String,
}

/// 用户对 ask_user 的回答(answer_tool_question 命令接收)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AskUserAnswer {
    /// 用户选择的选项索引(单选时取 [0],多选时可能多个)
    pub selected: Vec<usize>,
    /// 对应 selected 中每个选项的补充输入(空字符串表示未填)
    /// Vec 长度与 selected 相同;只有 requires_input=true 的选项需要非空值
    #[serde(default)]
    pub inputs: Vec<String>,
    /// 用户输入的自定义回答(如果 model 允许且用户没用预设选项)
    #[serde(default)]
    pub custom: Option<String>,
}

pub struct AskUserTool;

#[async_trait]
impl Tool for AskUserTool {
    fn name(&self) -> &str {
        "ask_user"
    }

    fn description(&self) -> &str {
        "Ask the user a clarifying question with 2-4 mutually exclusive multiple-choice options. \
         Use this whenever you encounter a choice point that affects the user's outcome \
         (e.g. \"choose a target audience for a draft\"). \
         Each option label must be 1-5 words. Set multi_select=true only if the choices are \
         independent. Do NOT use this for simple yes/no questions — just ask in plain text. \
         Do NOT use this when you can make a reasonable default choice on your own.\n\n\
         For options that need extra info (e.g. \"choose a different topic\"), set \
         requires_input=true and provide input_placeholder. The UI will show a text input below \
         that option and require it to be filled before the user can submit. Typical use: \
         an \"Other / specify...\" option with requires_input=true."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to present to the user, written in the same language as the conversation"
                },
                "header": {
                    "type": "string",
                    "description": "Very short label (max 12 chars), shown as a chip/tag on the question"
                },
                "options": {
                    "type": "array",
                    "minItems": 2,
                    "maxItems": 4,
                    "description": "2-4 options for the user to choose from",
                    "items": {
                        "type": "object",
                        "properties": {
                            "label": {
                                "type": "string",
                                "description": "Short label, 1-5 words"
                            },
                            "description": {
                                "type": "string",
                                "description": "Optional longer explanation of what this option does"
                            },
                            "requires_input": {
                                "type": "boolean",
                                "default": false,
                                "description": "Set true if the user must provide additional input (e.g. a topic, URL, or name) when choosing this option. The UI will show a text input below the option and require it to be filled before submit."
                            },
                            "input_placeholder": {
                                "type": "string",
                                "default": "",
                                "description": "Placeholder text for the input field (e.g. 'Topic' or 'https://...'). Only used when requires_input=true."
                            }
                        },
                        "required": ["label"]
                    }
                },
                "multi_select": {
                    "type": "boolean",
                    "default": false,
                    "description": "Set true if user can select multiple independent options"
                }
            },
            "required": ["question", "header", "options"]
        })
    }

    fn safety(&self) -> ToolSafety {
        ToolSafety::ReadOnly
    }

    async fn execute(&self, _args: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        // ask_user 不走 execute — 由 chat::ChatEngine::send_message 的专门分支处理
        // 如果走到这里,说明调用链出了 bug
        Err(ToolError::Other(
            "ask_user.execute should not be called; handled by chat::ChatEngine::send_message"
                .to_string(),
        ))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 工厂 + 单元测试
// ─────────────────────────────────────────────────────────────────────────────

/// 用当前配置构造内置工具；搜索凭据不依赖本轮聊天模型。
pub fn builtin_tools(config: &crate::models::AppConfig) -> Vec<std::sync::Arc<dyn Tool>> {
    use std::sync::Arc;
    let deepseek_api_key = (config.websearch_backend == crate::models::WebSearchBackend::DeepSeek)
        .then(|| config.deepseek_api_key().map(str::to_owned))
        .flatten();
    vec![
        Arc::new(AskUserTool),
        Arc::new(super::websearch::WebSearchTool::new(
            config.websearch_backend,
            deepseek_api_key,
        )),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // ── builtin_tools 工厂 + 重复名检查 ──

    #[tokio::test]
    async fn configured_deepseek_search_reports_missing_key_through_tool_result() {
        let config = crate::models::AppConfig {
            websearch_backend: crate::models::WebSearchBackend::DeepSeek,
            ..crate::models::AppConfig::default()
        };
        let tools = builtin_tools(&config);
        let search = tools
            .iter()
            .find(|tool| tool.name() == "websearch")
            .unwrap();
        let output = search
            .execute(json!({"query": "测试"}), ToolContext::default())
            .await
            .unwrap();
        assert!(output.is_error);
        let payload: Value = serde_json::from_str(&output.content).unwrap();
        assert_eq!(payload["provider"], "deepseek");
        assert_eq!(payload["status"], "unavailable");
        assert!(
            payload["note"]
                .as_str()
                .unwrap()
                .contains("DeepSeek API Key")
        );
        assert!(payload["note"].as_str().unwrap().contains("设置"));
        assert!(payload["results"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_builtin_tools_have_unique_names() {
        let tools = builtin_tools(&crate::models::AppConfig::default());
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        let unique: HashSet<&str> = names.iter().copied().collect();
        assert_eq!(unique.len(), names.len(), "tool 名重复: {:?}", names);
    }

    #[test]
    fn test_builtin_safety_classification() {
        let tools = builtin_tools(&crate::models::AppConfig::default());
        for t in &tools {
            match t.name() {
                "ask_user" | "websearch" => {
                    assert_eq!(t.safety(), ToolSafety::ReadOnly)
                }
                _ => panic!("unexpected tool: {}", t.name()),
            }
        }
    }
}
