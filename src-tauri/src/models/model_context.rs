//! 已知模型 → context_window 映射表
//!
//! 提供 `get_context_window(model_id) -> u32` 函数，用于在获取模型
//! 列表时给每个模型分配合适的上下文窗口大小。匹配顺序：
//! 1. 精确匹配（HashMap 查找，大小写不敏感）
//! 2. 前缀匹配（从最具体到最宽泛）
//! 3. 默认值 128,000

use std::collections::HashMap;
use std::sync::LazyLock;

/// 精确匹配表（小写 model_id → context_window）
static EXACT_MAP: LazyLock<HashMap<&'static str, u32>> = LazyLock::new(|| {
    HashMap::from([
        // ── Anthropic ──
        ("claude-opus-4-8", 200_000),
        ("claude-sonnet-4-6", 200_000),
        ("claude-haiku-4-5", 200_000),
        ("claude-fable-5", 200_000),
        ("claude-opus-4-5", 200_000),
        ("claude-sonnet-4-5", 200_000),
        ("claude-3-5-sonnet-20240620", 200_000),
        ("claude-3-5-sonnet-20241022", 200_000),
        ("claude-3-5-haiku-20241022", 200_000),
        ("claude-3-opus-20240229", 200_000),
        ("claude-3-sonnet-20240229", 200_000),
        ("claude-3-haiku-20240307", 200_000),
        // ── OpenAI ──
        ("gpt-4o", 128_000),
        ("gpt-4o-mini", 128_000),
        ("gpt-4-turbo", 128_000),
        ("gpt-4-turbo-2024-04-09", 128_000),
        ("gpt-4-turbo-preview", 128_000),
        ("gpt-4-0125-preview", 128_000),
        ("gpt-4-1106-preview", 128_000),
        ("gpt-4.5-preview", 128_000),
        ("gpt-4", 8_192),
        ("gpt-4-0314", 8_192),
        ("gpt-4-0613", 8_192),
        ("gpt-4-32k", 32_768),
        ("gpt-4-32k-0314", 32_768),
        ("gpt-4-32k-0613", 32_768),
        ("gpt-3.5-turbo", 16_385),
        ("gpt-3.5-turbo-0125", 16_385),
        ("gpt-3.5-turbo-1106", 16_385),
        ("gpt-3.5-turbo-16k", 16_385),
        ("gpt-3.5-turbo-instruct", 4_096),
        ("o1", 200_000),
        ("o1-preview", 128_000),
        ("o1-mini", 128_000),
        ("o3", 200_000),
        ("o3-mini", 200_000),
        ("o4-mini", 200_000),
        // ── DeepSeek ──
        ("deepseek-chat", 128_000),
        ("deepseek-reasoner", 128_000),
        // ── GLM / Zhipu ──
        ("glm-4", 128_000),
        ("glm-4-flash", 128_000),
        ("glm-4-plus", 128_000),
        ("glm-4-air", 128_000),
        ("glm-4-long", 128_000),
        ("glm-4v", 128_000),
        ("glm-4v-plus", 128_000),
        ("glm-3-turbo", 128_000),
        // ── Kimi / Moonshot ──
        ("moonshot-v1-8k", 8_192),
        ("moonshot-v1-32k", 32_768),
        ("moonshot-v1-128k", 128_000),
        // ── MiniMax ──
        ("abab6.5s-chat", 8_192),
        ("abab6.5-chat", 8_192),
        ("abab7-chat", 256_000),
        ("abab7-chat-preview", 256_000),
    ])
});

/// 前缀匹配表（从最具体到最宽泛，大小写不敏感）
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
/// 1. 精确匹配（大小写不敏感）
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

    // ── 精确匹配测试 ──

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
