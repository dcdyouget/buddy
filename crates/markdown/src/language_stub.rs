//! S00-06 产物：`language` 模块 **stub**（去掉语法高亮的极简版）
//!
//! # 为什么这是**必需**而不是优化
//!
//! 实测依赖链揭示 `language` 是真正的死结：
//!
//! ```text
//! language → tree-sitter (zed fork) → wasmtime-c-api-impl → 需要 cmake        ← 构建直接失败
//! language → settings → settings_json → migrator                             ← settings 又回来了
//! ```
//!
//! 两个后果：
//! 1. **`settings` 被重新拖回**，`theme_settings_shim.rs` 的努力白费
//! 2. `wasmtime` 需要 cmake；`tree-sitter` 是 zed 的 fork
//!
//! 所以要在 zed workspace 之外控制闭包，**必须把 `language` 换掉**。
//!
//! # 耦合面（实测，很小）
//!
//! | 文件 | `language` 引用 |
//! |------|----------------|
//! | `markdown.rs` | 13 处（2 个 `use` + 类型位置） |
//! | `parser.rs` | **0** |
//! | `selection.rs` | **0** |
//! | `path_range.rs` | **0** |
//! | `html.rs` | **0** |
//!
//! # 本 stub 的行为
//!
//! **不做语法高亮**：`highlight_text_resolved` 返回空 `runs`。
//! vendored `markdown.rs` 里的消费点会短路：
//!
//! ```ignore
//! let resolved = block.language.highlight_text_resolved(…);
//! if resolved.runs.is_empty() { return; }   // ← 直接返回，后续 HighlightId 查表不执行
//! ```
//!
//! 因此本 stub 只需通过编译，无需真实高亮。**代码块会按纯文本渲染。**
//!
//! # 真实实现应替换为
//!
//! **Comet 的 `crates/syntax`（MIT，1354 行）** —— 纯 tree-sitter、paint-only 契约、
//! **不依赖 LSP/项目机制**。它正好补上被剔除的这块。

#![allow(dead_code)]

use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use gpui::SharedString;
use smallvec::SmallVec;

/// 高亮 id
///
/// vendored `markdown.rs:4053` 把它交给 `SyntaxTheme::get(impl Into<usize>)`，
/// 所以**只需实现 `Into<usize>`**（`theme::SyntaxTheme` 的音调索引）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct HighlightId(pub usize);

impl From<HighlightId> for usize {
    fn from(value: HighlightId) -> usize {
        value.0
    }
}

/// 解析后的高亮结果
///
/// 字段与 zed 同名，保证 vendored 代码的 `resolved.sources.clone()` /
/// `resolved.runs` 等访问无需改动。
#[derive(Clone, Debug)]
pub struct ResolvedHighlights {
    /// 真实实现里是 `SmallVec<[(Arc<Grammar>, HighlightMap); 2]>`；
    /// 我们的 stub 只要求 `Clone`（唯一被用到的能力）。
    pub sources: SmallVec<[(); 2]>,
    /// 与 zed 同类型：`Arc<[(Range<usize>, HighlightId)]>`
    pub runs: Arc<[(Range<usize>, HighlightId)]>,
}

impl Default for ResolvedHighlights {
    fn default() -> Self {
        Self {
            sources: SmallVec::new(),
            runs: Arc::new([]),
        }
    }
}

impl ResolvedHighlights {
    /// 真实实现会检查 grammar 版本是否变化；本实现的语法随二进制静态链接、不会变化 → 恒为 `true`。
    pub fn is_current(&self) -> bool {
        true
    }
}

/// 语言作用域（用于字符分类）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LanguageScope(pub &'static str);

/// 语言的显示名
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LanguageName(pub SharedString);

impl From<SharedString> for LanguageName {
    fn from(value: SharedString) -> Self {
        LanguageName(value)
    }
}

/// vendored 代码在 `registry.language_for_name(fallback.as_ref())` 处需要它
impl AsRef<str> for LanguageName {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

/// 文本容器（真实实现是 rope 数据结构）。
///
/// vendored `markdown.rs` 只做 `Rope::from(&str)` 并把它交给
/// `highlight_text_resolved`，范围另行传入 —— 所以 stub 只需持有文本。
#[derive(Clone, Debug)]
pub struct Rope(String);

impl Rope {
    pub fn from(s: &str) -> Self {
        Rope(s.to_string())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Buddy 的语法高亮类别（S04-02）。
///
/// 按 v1 `CodeBlock.tsx` 的 `buddyCodeTheme`（prism 词法类别 → `--code-syntax-*`）归并为 9 类。
/// 顺序即 `HighlightId` 的值：调用方（buddy-ui）必须按**同一顺序**构造 `SyntaxTheme`
/// （`markdown.rs` 以 `syntax_theme.get(HighlightId)` 按下标取样式）。
pub const SYNTAX_CATEGORIES: [&str; 9] = [
    "comment",     // prism: comment / prolog / doctype / cdata（v1 另加 italic）
    "punctuation", // prism: punctuation
    "property",    // prism: property / tag / constant / symbol / deleted
    "number",      // prism: boolean / number
    "string",      // prism: selector / attr-name / string / char / builtin / inserted
    "operator",    // prism: operator / entity / url / string-variable
    "keyword",     // prism: atrule / attr-value / keyword（v1 另加字重 600）
    "function",    // prism: function / class-name
    "special",     // prism: regex / important / variable
];

/// Comet 高亮类别 → Buddy 类别下标（`None` = 用正文代码色，与 prism 未着色的词法类别一致）
fn category_of(kind: buddy_syntax::HighlightKind) -> Option<usize> {
    use buddy_syntax::HighlightKind as K;
    Some(match kind {
        K::Comment => 0,
        K::Punctuation => 1,
        K::Property | K::Tag | K::Constant => 2,
        K::Number | K::Boolean => 3,
        K::String | K::Attribute | K::TypeBuiltin | K::FunctionBuiltin => 4,
        K::Operator | K::MarkupLink => 5,
        K::Keyword => 6,
        K::Function | K::Type | K::Constructor | K::Macro => 7,
        K::StringSpecial | K::Escape | K::Variable | K::VariableSpecial => 8,
        K::Parameter
        | K::Label
        | K::MarkupHeading
        | K::MarkupRaw
        | K::MarkupReference
        | K::MarkupEmphasis
        | K::MarkupStrong
        | K::Embedded
        | K::Invalid => return None,
    })
}

/// 代码块围栏标签 → 高亮方式。
///
/// 用户决定（2026-10-04，S08-06 体积分析）：只内置 Python / Shell / SQL 的 tree-sitter 语法；
/// 其余任何带标签的代码块（rust、ts、java、objc、graphql…）都用 `buddy_syntax::generic` 通用高亮。
/// 纯文本类标签（[`PLAIN_TAGS`]）与无标签代码块不着色。
///
/// 此前（S04-02 / 2026-09-27）按 v1 prism 集合 + 额外 10 种语言内置全部语法，约占发布二进制 30 MB。
fn resolve(tag: &str) -> Option<Highlighter> {
    use buddy_syntax::LanguageId as L;
    let tag = tag.trim().split_ascii_whitespace().next()?.to_ascii_lowercase();
    if PLAIN_TAGS.contains(&tag.as_str()) {
        return None;
    }
    let id = match tag.as_str() {
        // prism 把这些都归入 markup
        "markup" | "xml" | "svg" | "mathml" | "ssml" | "rss" | "atom" => Some(L::Html),
        "flow" => Some(L::JavaScript),
        other => buddy_syntax::language_for_alias(other),
    };
    Some(match id {
        Some(id) if buddy_syntax::supports_language(id) => Highlighter::Grammar(id),
        _ => Highlighter::Generic,
    })
}

/// 不着色的围栏标签：输出、日志、差异等非代码内容。
const PLAIN_TAGS: &[&str] = &[
    "text", "plain", "plaintext", "txt", "log", "logs", "output", "diff", "patch", "csv", "tsv",
];

/// 代码块的高亮方式
#[derive(Clone, Copy, Debug)]
enum Highlighter {
    /// 内置 tree-sitter 语法（Python / Shell / SQL）
    Grammar(buddy_syntax::LanguageId),
    /// 通用词法高亮
    Generic,
}

/// 语言句柄
#[derive(Clone, Debug)]
pub struct Language {
    scope: LanguageScope,
    highlighter: Option<Highlighter>,
}

impl Language {
    pub fn new(scope: &'static str) -> Self {
        Self {
            scope: LanguageScope(scope),
            highlighter: None,
        }
    }

    fn highlighted(highlighter: Highlighter) -> Self {
        Self {
            scope: LanguageScope("source"),
            highlighter: Some(highlighter),
        }
    }

    /// zed 的 `Language::default_scope()` —— 用于 `CharClassifier`
    ///
    /// 注意**不返回 `Option`**：调用点 `map(|l| l.default_scope())` 期望
    /// `Option<LanguageScope>`（若这里返回 Option 会得到 Option<Option<..>>）。
    pub fn default_scope(&self) -> LanguageScope {
        self.scope
    }

    /// 语法高亮（tree-sitter 或通用词法），返回按 [`SYNTAX_CATEGORIES`] 编号的区间。
    ///
    /// 签名与 zed 一致：**不返回 `Result`**（`markdown.rs` 直接使用返回值）；
    /// 失败（超限、未知语言等）返回空结果，消费点据此按纯文本渲染。
    pub fn highlight_text_resolved(&self, rope: &Rope, range: Range<usize>) -> ResolvedHighlights {
        let Some(highlighter) = self.highlighter else {
            return ResolvedHighlights::default();
        };
        let Some(source) = rope.as_str().get(range.clone()) else {
            return ResolvedHighlights::default();
        };
        let lines = match highlighter {
            Highlighter::Grammar(id) => buddy_syntax::highlight(buddy_syntax::HighlightRequest {
                source,
                path: None,
                fence_tag: Some(language_tag(id)),
            })
            .map(|doc| doc.lines),
            Highlighter::Generic => buddy_syntax::generic::highlight(source),
        };
        let Ok(lines) = lines else {
            return ResolvedHighlights::default();
        };
        // Comet 输出为「按行、行内字节偏移」；换算为相对 range.start 的绝对偏移
        let mut runs = Vec::new();
        let mut line_start = 0usize;
        for (line, spans) in source.split_inclusive('\n').zip(lines.iter()) {
            for span in spans {
                if let Some(cat) = category_of(span.kind) {
                    runs.push((
                        range.start + line_start + span.range.start
                            ..range.start + line_start + span.range.end,
                        HighlightId(cat),
                    ));
                }
            }
            line_start += line.len();
        }
        ResolvedHighlights {
            sources: SmallVec::new(),
            runs: runs.into(),
        }
    }
}

/// Comet 按围栏标签识别语言；为已解析出的 `LanguageId` 取一个它认得的标签
fn language_tag(id: buddy_syntax::LanguageId) -> &'static str {
    use buddy_syntax::LanguageId as L;
    match id {
        L::Rust => "rust",
        L::JavaScript => "javascript",
        L::Jsx => "jsx",
        L::TypeScript => "typescript",
        L::Tsx => "tsx",
        L::Python => "python",
        L::Go => "go",
        L::Json => "json",
        L::Markdown => "markdown",
        L::Html => "html",
        L::Css => "css",
        L::Yaml => "yaml",
        L::C => "c",
        L::Cpp => "cpp",
        L::Kotlin => "kotlin",
        L::Swift => "swift",
        L::Sql => "sql",
        L::Jsonc => "jsonc",
        L::Bash => "bash",
        L::Toml => "toml",
        L::CSharp => "csharp",
        L::Java => "java",
        L::Ruby => "ruby",
        L::Php => "php",
        L::Lua => "lua",
        L::Nix => "nix",
        L::Make => "make",
        L::Dockerfile => "dockerfile",
    }
}

/// 按围栏标签同步取语言（见 [`resolve`]）；注册表的 async 方法亦基于它
pub fn language_for_tag(tag: &str) -> Option<Arc<Language>> {
    resolve(tag).map(|h| Arc::new(Language::highlighted(h)))
}

/// 语言注册表：按围栏标签 / 文件路径查语言
#[derive(Default, Clone, Debug)]
pub struct LanguageRegistry;

impl LanguageRegistry {
    pub async fn language_for_name_or_extension(&self, name: &str) -> anyhow::Result<Arc<Language>> {
        language_for_tag(name).ok_or_else(|| anyhow::anyhow!("无高亮语言：{name}"))
    }

    pub async fn load_language_for_file_path(&self, path: &Path) -> anyhow::Result<Arc<Language>> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
        self.language_for_name_or_extension(ext).await
    }

    pub async fn language_for_name(&self, name: &str) -> anyhow::Result<Arc<Language>> {
        self.language_for_name_or_extension(name).await
    }
}

/// 字符类别（用于双击选词、光标移动的词边界）
///
/// **必须实现 `Ord`** —— vendored 代码里用了 `std::cmp::max(kind_a, kind_b)`
/// （`markdown.rs:4791`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CharKind {
    Whitespace = 0,
    Punctuation = 1,
    Word = 2,
}

/// 字符分类器
#[derive(Clone, Debug)]
pub struct CharClassifier {
    scope: Option<LanguageScope>,
}

impl CharClassifier {
    pub fn new(scope: Option<LanguageScope>) -> Self {
        Self { scope }
    }

    /// 简化实现：字母数字下划线视为 Word，空白视为 Whitespace，其余为标点。
    ///
    /// 真实实现会按语言 scope 查 tree-sitter 的字符分类查询。
    /// 对本 spike（验证渲染）足够；**双击选词行为会略粗于 zed**，
    /// 归 `S05-06` 用 Comet 的 syntax 替换时修正。
    pub fn kind(&self, c: char) -> CharKind {
        if c.is_alphanumeric() || c == '_' {
            CharKind::Word
        } else if c.is_whitespace() {
            CharKind::Whitespace
        } else {
            CharKind::Punctuation
        }
    }

    pub fn kind_with(&self, c: char, ignore_punctuation: bool) -> CharKind {
        let k = self.kind(c);
        if ignore_punctuation && k == CharKind::Punctuation {
            return CharKind::Word;
        }
        k
    }

    pub fn scope(&self) -> Option<LanguageScope> {
        self.scope
    }
}
