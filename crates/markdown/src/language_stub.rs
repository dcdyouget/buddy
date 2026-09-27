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
    /// 真实实现会检查 grammar 版本是否变化。
    /// stub 恒为 `true` —— 因为 `runs` 恒为空，不会走到查表路径。
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

/// 语言句柄（真实实现持有 tree-sitter 语法与查询）
#[derive(Clone, Debug)]
pub struct Language {
    scope: LanguageScope,
}

impl Language {
    pub fn new(scope: &'static str) -> Self {
        Self {
            scope: LanguageScope(scope),
        }
    }

    /// zed 的 `Language::default_scope()` —— 用于 `CharClassifier`
    ///
    /// 注意**不返回 `Option`**：调用点 `map(|l| l.default_scope())` 期望
    /// `Option<LanguageScope>`（若这里返回 Option 会得到 Option<Option<..>>）。
    pub fn default_scope(&self) -> LanguageScope {
        self.scope
    }

    /// **stub：返回空高亮**（不做语法高亮）
    ///
    /// 注意签名与消费点匹配：**不返回 `Result`**
    /// （`markdown.rs:1671` 直接使用返回值，未做 `?`）。
    pub fn highlight_text_resolved(&self, _rope: &Rope, _range: Range<usize>) -> ResolvedHighlights {
        ResolvedHighlights::default()
    }
}

/// 语言注册表（真实实现按名字/扩展名查语言）
///
/// ⚠️ 本 spike 里始终传 `None`（`Markdown::new(src, None, None, cx)`），
/// 因此下面 3 个方法**永不被调用** —— 只需通过编译。
#[derive(Default, Clone, Debug)]
pub struct LanguageRegistry;

impl LanguageRegistry {
    pub async fn language_for_name_or_extension(
        &self,
        _name: &str,
    ) -> anyhow::Result<Arc<Language>> {
        anyhow::bail!("S00-06 stub：未接入语言注册表")
    }

    pub async fn load_language_for_file_path(
        &self,
        _path: &Path,
    ) -> anyhow::Result<Arc<Language>> {
        anyhow::bail!("S00-06 stub：未接入语言注册表")
    }

    pub async fn language_for_name(&self, _name: &str) -> anyhow::Result<Arc<Language>> {
        anyhow::bail!("S00-06 stub：未接入语言注册表")
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
