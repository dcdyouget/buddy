//! Provider 预设。
//!
//! 数据与 v1 `src/types/index.ts` 的 `PROVIDER_PRESETS` 保持一一对应。

/// 一个内置 Provider 预设。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderPreset {
    /// 预设 ID。
    pub id: &'static str,
    /// 展示名称。
    pub name: &'static str,
    /// 默认 Base URL。
    pub url: &'static str,
    /// 卡片上的首字母标识。
    pub letter: &'static str,
    /// Provider 协议 ID。
    pub protocol: &'static str,
    /// 空字符串表示使用 engine 的默认 thinking 格式。
    pub thinking_format: &'static str,
    /// 空字符串表示使用 engine 的默认 token 字段。
    pub max_tokens_field: &'static str,
}

/// v1 内置 Provider 预设列表。
pub const PRESETS: &[ProviderPreset] = &[
    ProviderPreset {
        id: "deepseek",
        name: "DeepSeek",
        url: "https://api.deepseek.com",
        letter: "D",
        protocol: "openai_compatible",
        thinking_format: "deepseek",
        max_tokens_field: "",
    },
    ProviderPreset {
        id: "openai",
        name: "OpenAI",
        url: "https://api.openai.com/v1",
        letter: "O",
        protocol: "openai_compatible",
        thinking_format: "",
        max_tokens_field: "max_completion_tokens",
    },
    ProviderPreset {
        id: "anthropic",
        name: "Anthropic",
        url: "https://api.anthropic.com",
        letter: "A",
        protocol: "anthropic",
        thinking_format: "",
        max_tokens_field: "",
    },
    ProviderPreset {
        id: "openrouter",
        name: "OpenRouter",
        url: "https://openrouter.ai/api/v1",
        letter: "O",
        protocol: "openai_compatible",
        thinking_format: "openrouter",
        max_tokens_field: "",
    },
    ProviderPreset {
        id: "glm",
        name: "GLM / 智谱",
        url: "https://open.bigmodel.cn/api/paas/v4",
        letter: "G",
        protocol: "openai_compatible",
        thinking_format: "",
        max_tokens_field: "",
    },
    ProviderPreset {
        id: "minimax",
        name: "MiniMax",
        url: "https://api.minimaxi.com/v1",
        letter: "M",
        protocol: "openai_compatible",
        thinking_format: "",
        max_tokens_field: "",
    },
    ProviderPreset {
        id: "moonshot",
        name: "Moonshot / 月之暗面",
        url: "https://api.moonshot.cn/v1",
        letter: "M",
        protocol: "openai_compatible",
        thinking_format: "",
        max_tokens_field: "",
    },
    ProviderPreset {
        id: "qwen",
        name: "Qwen / 通义千问",
        url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        letter: "Q",
        protocol: "openai_compatible",
        thinking_format: "qwen",
        max_tokens_field: "",
    },
    ProviderPreset {
        id: "zhipu",
        name: "Zhipu / 智谱",
        url: "https://open.bigmodel.cn/api/paas/v4",
        letter: "Z",
        protocol: "openai_compatible",
        thinking_format: "",
        max_tokens_field: "",
    },
    ProviderPreset {
        id: "qianfan",
        name: "百度千帆",
        url: "https://qianfan.baidubce.com/v2",
        letter: "B",
        protocol: "openai_compatible",
        thinking_format: "",
        max_tokens_field: "",
    },
    ProviderPreset {
        id: "doubao",
        name: "豆包 / 火山方舟",
        url: "https://ark.cn-beijing.volces.com/api/v3",
        letter: "D",
        protocol: "openai_compatible",
        thinking_format: "",
        max_tokens_field: "",
    },
];

/// 自定义 Provider 的特殊预设 ID。
pub const CUSTOM_PRESET_ID: &str = "custom";

/// 查找内置预设。
pub fn preset(id: &str) -> Option<&'static ProviderPreset> {
    PRESETS.iter().find(|item| item.id == id)
}

#[cfg(test)]
mod tests {
    use super::PRESETS;

    #[test]
    fn matches_v1_provider_presets() {
        let expected = [
            (
                "deepseek",
                "DeepSeek",
                "https://api.deepseek.com",
                "D",
                "openai_compatible",
                "deepseek",
                "",
            ),
            (
                "openai",
                "OpenAI",
                "https://api.openai.com/v1",
                "O",
                "openai_compatible",
                "",
                "max_completion_tokens",
            ),
            (
                "anthropic",
                "Anthropic",
                "https://api.anthropic.com",
                "A",
                "anthropic",
                "",
                "",
            ),
            (
                "openrouter",
                "OpenRouter",
                "https://openrouter.ai/api/v1",
                "O",
                "openai_compatible",
                "openrouter",
                "",
            ),
            (
                "glm",
                "GLM / 智谱",
                "https://open.bigmodel.cn/api/paas/v4",
                "G",
                "openai_compatible",
                "",
                "",
            ),
            (
                "minimax",
                "MiniMax",
                "https://api.minimaxi.com/v1",
                "M",
                "openai_compatible",
                "",
                "",
            ),
            (
                "moonshot",
                "Moonshot / 月之暗面",
                "https://api.moonshot.cn/v1",
                "M",
                "openai_compatible",
                "",
                "",
            ),
            (
                "qwen",
                "Qwen / 通义千问",
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
                "Q",
                "openai_compatible",
                "qwen",
                "",
            ),
            (
                "zhipu",
                "Zhipu / 智谱",
                "https://open.bigmodel.cn/api/paas/v4",
                "Z",
                "openai_compatible",
                "",
                "",
            ),
            (
                "qianfan",
                "百度千帆",
                "https://qianfan.baidubce.com/v2",
                "B",
                "openai_compatible",
                "",
                "",
            ),
            (
                "doubao",
                "豆包 / 火山方舟",
                "https://ark.cn-beijing.volces.com/api/v3",
                "D",
                "openai_compatible",
                "",
                "",
            ),
        ];
        assert_eq!(PRESETS.len(), expected.len());
        for (actual, expected) in PRESETS.iter().zip(expected) {
            assert_eq!(
                (
                    actual.id,
                    actual.name,
                    actual.url,
                    actual.letter,
                    actual.protocol,
                    actual.thinking_format,
                    actual.max_tokens_field,
                ),
                expected
            );
        }
    }
}
