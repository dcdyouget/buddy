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

// ============================================================================
// use 语句 = Java 的 import
// ============================================================================
// - HashMap ≈ Java 的 HashMap<K, V>
// - LazyLock 是 Rust 1.80 引入的线程安全惰性初始化包装器
//   （之前常用 once_cell crate；现在标准库自带）
//   等价于 Java 的：
//     private static final Map<String, Integer> MAP = lazyInit(() -> new HashMap<>());
// ============================================================================
use std::collections::HashMap;
use std::sync::LazyLock;

/// 精确匹配表（仅保留前缀表会给出错误结果的例外）
///
/// 同值覆盖的情况已在 `PREFIX_TABLE` 中处理；本表只承担"覆盖错误前缀"
/// 的角色。维护时新条目需通过现有测试覆盖。
///
/// Rust 关键字 / 语法：
/// - `static`          = Java 的 `static final`（编译期常量或全局变量）
/// - `LazyLock<...>`   = 第一次访问时才初始化的全局变量，且线程安全
/// - `&'static str`    = 字符串字面量的引用；'static 是生命周期标注，
///                       意思是"这个引用可以在程序运行的整个期间有效"
///                       （字符串字面量本身就存放在二进制里，永久有效）
/// - `u32`             = 32 位无符号整数
///
/// 对应 Java：
///     private static final Map<String, Integer> EXACT_MAP =
///         lazyInit(() -> Map.of("gpt-4-0125-preview", 128_000, ...));
static EXACT_MAP: LazyLock<HashMap<&'static str, u32>> = LazyLock::new(|| {
    // HashMap::from([...]) 是数组转 HashMap 的语法糖
    // 类似 Java 的 Map.ofEntries(Map.entry(...), Map.entry(...))
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
///
/// Rust 数组语法：
/// - `&[...]`     = 不可变引用指向的数组字面量，存放在只读数据段（与 'static 生命周期等价）
/// - `&'static`   = 同上，'static 是生命周期标注
/// - `(&str, u32)` = 元组（tuple）类型，固定长度/类型的复合值，类似 Java 没有 tuple 的概念
///                  （Java 17 之后有 record，但仅限命名 record）
/// - `u32` 数字字面量可加下划线增加可读性（128_000 ≡ 128000），纯语法糖
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
///
/// Rust 函数签名：
/// - `pub fn`          = 公开函数
/// - `get_context_window(model_id: &str) -> u32`
///                     - `&str` 是"字符串切片引用"，相当于 Java 的 `String`（不可变视图）
///                       区别：&str 不可增长，String 可增长；这里只读，用 &str 即可
/// - `-> u32`          返回 32 位无符号整数
pub fn get_context_window(model_id: &str) -> u32 {
    // `let lower = ...`：let 是变量绑定；默认不可变（immutable）
    //   Java 中所有局部变量引用都可变，Rust 中需要 `let mut lower = ...` 才能再赋值
    // `to_lowercase()` 返回 String，等价于 Java 的 `modelId.toLowerCase()`
    let lower = model_id.to_lowercase();

    // ── 1. 精确匹配 ──
    // `if let Some(...) = ... { ... }` 是 Rust 的模式匹配（pattern matching）
    // 配合 Option<T> 使用：
    //   - Some(x) 表示有值，绑定 x 给代码块用
    //   - None    表示无值，整个 if 块不执行
    //
    // Java 等价写法（假设存在 getOrNull）：
    //     Integer v = EXACT_MAP.get(lower);
    //     if (v != null) return v;
    //
    // `&` 在模式里表示"取引用"；`&` 紧跟值表示"取引用"；
    // 这里 `Some(&ctx)` 把 HashMap 里值的引用解出来。
    //   *EXACT_MAP.get(...) 返回 Option<&u32>（值的引用，避免拷贝）
    //   `&ctx` 进一步"重新借用"成 &u32，从而能匹配 Some(&u32)
    if let Some(&ctx) = EXACT_MAP.get(lower.as_str()) {
        return ctx;  // `return` 从当前函数返回；Rust 也常用尾部表达式隐式 return
    }

    // ── 2. 前缀匹配 ──
    // `for (prefix, ctx) in PREFIX_TABLE`：
    //   PREFIX_TABLE 是 &[(&str, u32)]，迭代时每个元素是 &(&str, u32)
    //   模式 (prefix, ctx) 自动解引用，把内部字段绑定为 prefix: &&str, ctx: &u32
    // Java 等价：
    //     for (Entry<String, Integer> e : PREFIX_TABLE) { ... }
    for (prefix, ctx) in PREFIX_TABLE {
        // `lower.starts_with(prefix)` ≈ `lower.startsWith(prefix)`
        // `return *ctx`：`*` 是解引用运算符；ctx 是 &u32，*ctx 得到 u32 的拷贝
        if lower.starts_with(prefix) {
            return *ctx;
        }
    }

    // ── 3. 默认 ──
    // 注意：Rust 函数末尾的表达式（无分号）就是返回值，不需要写 return
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