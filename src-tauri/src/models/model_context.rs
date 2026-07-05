//! 已知模型 → context_window 映射表
//!
//! 提供 `get_context_window(model_id) -> u32` 函数，用于在获取模型
//! 列表时给每个模型分配合适的上下文窗口大小。匹配顺序：
//! 1. `EXACT_MAP` —— **例外表**，只放 PREFIX_TABLE 给出错误结果的具体 model_id
//!    (例如 `gpt-4-1106-preview` 在 `gpt-4` 前缀下会被错误归为 8K，需强制 128K)
//! 2. `PREFIX_TABLE` —— **主要路由表**，按从最具体到最宽泛的顺序匹配前缀
//! 3. 默认值 128,000
//!
//! 维护原则：新增模型时优先尝试在 `PREFIX_TABLE` 加前缀；只有在
//! 前缀会给出错误结果时，才把该 model_id 单独加进 `EXACT_MAP`。

use std::collections::HashMap;
use std::sync::LazyLock;

/// 精确匹配表（仅保留前缀表会给出错误结果的例外）
///
/// 同值覆盖的情况已在 `PREFIX_TABLE` 中处理；本表只承担"覆盖错误前缀"
/// 的角色。维护时新条目需通过现有测试覆盖。
static EXACT_MAP: LazyLock<HashMap<&'static str, u32>> = LazyLock::new(|| {
    HashMap::from([
        // ── OpenAI 例外：gpt-4-*-preview 系列被 gpt-4 前缀误判为 8K ──
        ("gpt-4-0125-preview", 128_000),
        ("gpt-4-1106-preview", 128_000),
        ("gpt-4.5-preview", 128_000),
        // ── OpenAI 例外：instruct 模型覆盖 gpt-3.5-turbo 的 16K ──
        ("gpt-3.5-turbo-instruct", 4_096),
    ])
});

/// 前缀匹配表（从最具体到最宽泛，大小写不敏感）
///
/// **顺序很重要！** 如 "gpt-4o-mini" 必须在 "gpt-4o" 之前，
/// "gpt-4o" 必须在 "gpt-4" 之前。
static PREFIX_TABLE: &[(&str, u32)] = &[
    // ── Anthropic（全部 200K）──
    ("claude-", 200_000),
    // ── OpenAI（最具体在前）──
    ("gpt-4.5", 128_000),
    ("gpt-4o-mini", 128_000),
    ("gpt-4o", 128_000),
    ("gpt-4-turbo", 128_000),
    ("gpt-4-32k", 32_768),
    ("gpt-4", 8_192),
    ("gpt-3.5-turbo-16k", 16_385),
    ("gpt-3.5-turbo-instruct", 4_096),
    ("gpt-3.5-turbo", 16_385),
    ("o4-mini", 200_000),
    ("o3-mini", 200_000),
    ("o3", 200_000),
    ("o1-mini", 128_000),
    ("o1-preview", 128_000),
    ("o1", 200_000),
    // ── DeepSeek ──
    ("deepseek-chat", 128_000),
    ("deepseek-reasoner", 128_000),
    ("deepseek", 128_000),
    // ── GLM / Zhipu ──
    ("glm-4v-plus", 128_000),
    ("glm-4v", 128_000),
    ("glm-4-flash", 128_000),
    ("glm-4-plus", 128_000),
    ("glm-4-air", 128_000),
    ("glm-4-long", 128_000),
    ("glm-4", 128_000),
    ("glm-3-turbo", 128_000),
    ("glm-", 128_000),
    // ── Kimi / Moonshot ──
    ("moonshot-v1-8k", 8_192),
    ("moonshot-v1-32k", 32_768),
    ("moonshot-v1-128k", 128_000),
    ("moonshot-v1", 128_000),
    ("moonshot", 128_000),
    // ── MiniMax ──
    ("abab6.5s-chat", 8_192),
    ("abab6.5", 8_192),
    ("abab7-chat", 256_000),
    ("abab7", 256_000),
];

/// 获取指定模型的上下文窗口大小（token 数）
///
/// 匹配顺序：
/// 1. 精确匹配（大小写不敏感）—— 仅命中 `EXACT_MAP` 中的例外条目
/// 2. 前缀匹配（从最具体到最宽泛）
/// 3. 返回默认值 128,000
pub fn get_context_window(model_id: &str) -> u32 {
    let lower = model_id.to_lowercase();

    // 1. 精确匹配
    if let Some(&ctx) = EXACT_MAP.get(lower.as_str()) {
        return ctx;
    }

    // 2. 前缀匹配
    for (prefix, ctx) in PREFIX_TABLE {
        if lower.starts_with(prefix) {
            return *ctx;
        }
    }

    // 3. 默认
    128_000
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 精确匹配（例外）测试 ──

    #[test]
    fn test_openai_exceptions() {
        // 被 gpt-4 前缀误判为 8K 的预览版
        assert_eq!(get_context_window("gpt-4-0125-preview"), 128_000);
        assert_eq!(get_context_window("gpt-4-1106-preview"), 128_000);
        // 没有 4.5 前缀
        assert_eq!(get_context_window("gpt-4.5-preview"), 128_000);
        // instruct 覆盖了 gpt-3.5-turbo 的 16K
        assert_eq!(get_context_window("gpt-3.5-turbo-instruct"), 4_096);
    }

    // ── 前缀匹配测试 ──

    #[test]
    fn test_anthropic_models() {
        assert_eq!(get_context_window("claude-sonnet-4-6"), 200_000);
        assert_eq!(get_context_window("claude-opus-4-8"), 200_000);
        assert_eq!(get_context_window("claude-haiku-4-5"), 200_000);
        assert_eq!(get_context_window("claude-fable-5"), 200_000);
        assert_eq!(get_context_window("Claude-Sonnet-4-6"), 200_000); // 大小写
        assert_eq!(get_context_window("claude-3-5-sonnet-20241022"), 200_000);
        assert_eq!(get_context_window("claude-3-opus-20240229"), 200_000);
    }

    #[test]
    fn test_openai_gpt4o_models() {
        assert_eq!(get_context_window("gpt-4o"), 128_000);
        assert_eq!(get_context_window("gpt-4o-mini"), 128_000);
        // 前缀匹配：新版 gpt-4o 变体
        assert_eq!(get_context_window("gpt-4o-2024-11-20"), 128_000);
    }

    #[test]
    fn test_openai_gpt4_models() {
        assert_eq!(get_context_window("gpt-4"), 8_192);
        assert_eq!(get_context_window("gpt-4-0613"), 8_192);
        assert_eq!(get_context_window("gpt-4-32k"), 32_768);
        assert_eq!(get_context_window("gpt-4-turbo"), 128_000);
        assert_eq!(get_context_window("gpt-4-turbo-preview"), 128_000);
    }

    #[test]
    fn test_openai_gpt35_models() {
        assert_eq!(get_context_window("gpt-3.5-turbo"), 16_385);
        assert_eq!(get_context_window("gpt-3.5-turbo-0125"), 16_385);
        assert_eq!(get_context_window("gpt-3.5-turbo-instruct"), 4_096);
    }

    #[test]
    fn test_openai_reasoning_models() {
        assert_eq!(get_context_window("o1"), 200_000);
        assert_eq!(get_context_window("o1-mini"), 128_000);
        assert_eq!(get_context_window("o1-preview"), 128_000);
        assert_eq!(get_context_window("o3"), 200_000);
        assert_eq!(get_context_window("o3-mini"), 200_000);
        assert_eq!(get_context_window("o4-mini"), 200_000);
    }

    #[test]
    fn test_deepseek_models() {
        assert_eq!(get_context_window("deepseek-chat"), 128_000);
        assert_eq!(get_context_window("deepseek-reasoner"), 128_000);
        // 前缀匹配
        assert_eq!(get_context_window("deepseek-chat-v3"), 128_000);
    }

    #[test]
    fn test_glm_models() {
        assert_eq!(get_context_window("glm-4"), 128_000);
        assert_eq!(get_context_window("glm-4-flash"), 128_000);
        assert_eq!(get_context_window("glm-4-plus"), 128_000);
        assert_eq!(get_context_window("glm-4v"), 128_000);
        assert_eq!(get_context_window("glm-3-turbo"), 128_000);
    }

    #[test]
    fn test_kimi_models() {
        assert_eq!(get_context_window("moonshot-v1-8k"), 8_192);
        assert_eq!(get_context_window("moonshot-v1-32k"), 32_768);
        assert_eq!(get_context_window("moonshot-v1-128k"), 128_000);
        // 前缀匹配
        assert_eq!(get_context_window("moonshot-v1-256k"), 128_000);
    }

    #[test]
    fn test_minimax_models() {
        assert_eq!(get_context_window("abab6.5s-chat"), 8_192);
        assert_eq!(get_context_window("abab6.5-chat"), 8_192);
        assert_eq!(get_context_window("abab7-chat"), 256_000);
        assert_eq!(get_context_window("abab7-chat-preview"), 256_000);
    }

    // ── 前缀匹配边界测试 ──

    #[test]
    fn test_prefix_ordering() {
        // gpt-4o-mini 开头包含 gpt-4o，gpt-4o 开头包含 gpt-4
        // 必须匹配到最具体的
        assert_eq!(get_context_window("gpt-4o-mini"), 128_000); // 不是 8_192
        assert_eq!(get_context_window("gpt-4o"), 128_000); // 不是 8_192
        assert_eq!(get_context_window("gpt-4"), 8_192);

        // o1-mini 必须匹配到 128_000 而不是 o1 的 200_000
        assert_eq!(get_context_window("o1-mini"), 128_000);
        assert_eq!(get_context_window("o1"), 200_000);
    }

    // ── 默认值测试 ──

    #[test]
    fn test_unknown_model_defaults_to_128k() {
        assert_eq!(get_context_window("some-unknown-model"), 128_000);
        assert_eq!(get_context_window("my-custom-llm"), 128_000);
        assert_eq!(get_context_window(""), 128_000);
    }
}
