# S00-06 产物：vendor zed markdown 栈的完整配方

> 本文件是**操作手册**。三个 shim/stub 代码在同目录：
> `theme_settings_shim.rs`、`language_stub.rs`、`mermaid.rs`
> 完整证据见 `docs/specs/phase-00/S00-06-markdown.md`。

---

## 0. 结论摘要

| 项 | 结果 |
|----|------|
| 目标（控制闭包） | ✅ **达成** —— 总包数 **718**（S00-01 的 GPUI 基线 693，**markdown 栈只 +25**） |
| patch 工作量 | **3 行 import 级改动** + 3 个 shim/stub（491 行） |
| vendor 代码量 | 4762 行（6 个文件）+ html 子模块 2502 行 = **7264 行** |
| 已剔除的重依赖 | `settings` 全家桶 / `lsp` / `rpc` / `fs` / `language` / `language_core` / `tree-sitter` / `wasmtime` / `node_runtime` / `theme_settings` / `mermaid_render` / `editor` |
| 流式渲染 | ✅ 已验证（306 字符逐字喂入，无重排，自动跟尾正常） |
| **代价** | **无语法高亮**（代码块按纯文本）→ `S04-02` 接 Comet 的 `crates/syntax`（MIT） |

---

## 1. 必须复制的 `[patch.crates-io]`（**阻碍级**）

### 症状

```
error[E0599]: no function or associated item named `adopt_raw_pid`
              found for struct `smol::async_process::Child`
              --> crates/util/src/command/darwin.rs:497
```

### 原因

zed 的 workspace 用自家 fork 的 `async-process` 补了 `adopt_raw_pid`：

```toml
# zed/Cargo.toml
[patch.crates-io]
async-process = { git = "https://github.com/zed-industries/async-process.git", rev = "0b6d6713..." }
```

**`[patch]` 不会传递给下游消费者。** 所以任何在 zed workspace 之外使用其内部 crate
（如 `util`，被 `markdown` 用作 `util::maybe`）的项目，**必须自己复制这些 patch**。

### macOS markdown 场景需要的最小集合

```toml
[patch.crates-io]
tree-sitter-language = { git = "https://github.com/tree-sitter/tree-sitter", rev = "43623ec9bf0eaaf7113285c46e8a09018f181b18" }
async-process = { git = "https://github.com/zed-industries/async-process.git", rev = "0b6d6713570af61806e1e5cb40e0f757cb93fd9d" }
async-task = { git = "https://github.com/smol-rs/async-task.git", rev = "b4486cd71e4e94fbda54ce6302444de14f4d190e" }
notify = { git = "https://github.com/zed-industries/notify", rev = "d842f16b2716bd60f09caf3ae3a894237ab38f54" }
notify-types = { git = "https://github.com/zed-industries/notify", rev = "d842f16b2716bd60f09caf3ae3a894237ab38f54" }
```

zed 完整清单里**未复制**的项及原因：

| 未复制 | 原因 |
|--------|------|
| `windows-capture` | 仅 Windows |
| `calloop` | 仅 Linux |
| `livekit` / `libwebrtc` / `webrtc-sys` | 协作/音频功能，Buddy 不需要 |
| `scratch` | zed 的 corgi 构建工具本地 path 依赖，无法复制；不在闭包内 |
| `tree-sitter-language` | **已复制**（若引入 tree-sitter 语法才需要，为后续 `S04-02` 预留） |

> ⚠️ **若将来引入 `fs` / 终端 / 协作等功能，需按需补齐 patch。**
> 这是使用 zed 内部 crate 的**持续维护成本**，应计入 `S01-02` 的依赖策略评估。

---

## 2. 三个 shim/stub（总计 491 行）

### 2.1 `theme_settings_shim.rs`（143 行）—— 替掉 zed settings 框架

**原理**：`markdown.rs` 有 **21 处**读取 `theme_settings::ThemeSettings::get_global(cx)`
（字体族、字号、行高）。与其改 21 处调用点，不如**提供一个同名同 API 的 shim**。

**patch 只有 1 行**：

```diff
- use theme_settings::ThemeSettings;
+ use crate::theme_settings_shim::ThemeSettings;
```

**另需删掉 1 行**（`Settings` trait 只为 `get_global` 而导入，而 shim 把 `get_global` 做成了**固有方法**）：

```diff
- use settings::Settings as _;
```

**shim 需提供的 API 表面**（从实测调用点归纳）：

| 类别 | 项 |
|------|-----|
| 字段 | `ui_font: Font`、`buffer_font: Font`、`buffer_line_height: BufferLineHeight` |
| 固有方法 | `get_global(cx) -> &Self`、`install(cx)` |
| 字号方法 | `ui_font_size`、`buffer_font_size`、`agent_ui_font_size`、`agent_buffer_font_size`、`markdown_preview_font_size`（均 `(&self, &App) -> Pixels`） |
| 字体族方法 | `markdown_preview_font_family`、`agent_ui_font_family`、`agent_buffer_font_family`、`markdown_preview_code_font_family`（均 `(&self) -> &SharedString`） |

**安装时机**：必须在 `theme::init()` 之后。已暴露为公开函数：

```rust
zed_markdown::install_theme_settings_shim(cx);
```

> 若忘记安装，`MarkdownStyle::themed()` 会因 `ThemeSettings::get_global(cx)` 找不到全局而 panic。

### 2.2 `language_stub.rs`（248 行）—— **必需，不是可选优化**

**为什么必需**（实测依赖链）：

```text
language → tree-sitter (zed fork) → wasmtime-c-api-impl → 需要 cmake        ← 构建直接失败
language → settings → settings_json → migrator                             ← settings 被拖回来！
```

第二条尤其致命：**即使 shim 掉 `ThemeSettings`，`language` 也会把 `settings` 重新拖进闭包。**

**耦合面很小（实测）**：

| 文件 | `language` 引用 |
|------|----------------|
| `markdown.rs` | 13 处 |
| `parser.rs` | **0** |
| `selection.rs` | **0** |
| `path_range.rs` | **0** |
| `html.rs` | **0** |

**stub 需提供的类型**：

| 类型 | 说明 | 陷阱 |
|------|------|------|
| `HighlightId` | `pub struct HighlightId(pub usize)` | **需 `From<HighlightId> for usize`** —— 因为消费点是 `SyntaxTheme::get(impl Into<usize>)` |
| `ResolvedHighlights` | `{ sources: SmallVec<[(); 2]>, runs: Arc<[(Range<usize>, HighlightId)]> }` | 需 `is_current()`、`Default`、`Clone`。`sources` 只要求 `Clone`（唯一被用到的能力），不必是真类型 |
| `Rope` | `Rope::from(&str)` + `len()` | 只需持有文本；范围另行传入 |
| `LanguageName` | newtype over `SharedString` | **需 `AsRef<str>`** |
| `Language` | `default_scope()`、`highlight_text_resolved()` | ⚠️ `default_scope()` **不返回 `Option`**（调用点是 `map(\|l\| l.default_scope())`，返回 Option 会得到 `Option<Option<..>>`）；⚠️ `highlight_text_resolved` **不返回 `Result`** |
| `LanguageRegistry` | 3 个 async 方法 | 本 spike 始终传 `None`，方法体可为 `bail!` |
| `CharClassifier` + `CharKind` | `new(Option<LanguageScope>)`、`kind(c)` | **`CharKind` 必须 `Ord`**（调用点用 `std::cmp::max`） |

**行为**：`highlight_text_resolved` 返回空 `runs`，消费点因此短路：

```rust
let resolved = block.language.highlight_text_resolved(…);
if resolved.runs.is_empty() { return; }   // ← 后续 HighlightId 查表不执行
```

**代价**：`CharClassifier` 是简化实现（字母数字→Word / 空白→Whitespace / 其余→标点），
**双击选词精度略低于 zed**。真实实现应换为 tree-sitter 的字符分类查询。

**真实替换目标**：**Comet 的 `crates/syntax`（MIT，1354 行）** —— 纯 tree-sitter、
paint-only 契约、无 LSP/项目依赖。

### 2.3 `mermaid.rs`（100 行）—— 替掉 1836 行 + node/wasm 链路

`markdown.rs` 有 **90 处**引用 `mermaid` 模块，删不干净，所以提供 **API 兼容的空实现**。

**需提供**：

| 项 | 说明 |
|----|------|
| `ParsedMarkdownMermaidDiagram` | 需 **`Debug`**（被 `#[derive(Debug)]` 的父结构持有） |
| `ParsedMarkdownMermaidDiagramContents` | 同上，另需 `Clone`/`PartialEq`/`Eq`/`Hash` |
| `MermaidState` | 需 `Default`、`Clone`、以及 `clear(cx)` / `update(&ParsedMarkdown, impl Fn(usize)->f32, cx)` / `natural_size(&Contents) -> Option<Size<Pixels>>` / `rerasterize_diagram(&Contents, f32, cx)` |
| `extract_mermaid_diagrams` | **事件类型必须用 `crate::parser::MarkdownEvent`**，不要直接 `use pulldown_cmark::Event`（生命周期不一致会编译失败）；返回空 `BTreeMap` |
| `render_mermaid_diagram` | 11 个参数，`unreachable!()` |

**运行前提**：`MarkdownOptions::render_mermaid_diagrams` 默认 `false` →
`extract` 返回空 → `render_mermaid_diagram` **永不被调用**。

**代价**：含 mermaid 的代码块按普通代码块渲染。

---

## 3. vendor 清单

| 文件 | 行数 | 备注 |
|------|------|------|
| `markdown.rs` | 7428 | 主文件；patch 3 行 |
| `parser.rs` | 1893 | 未改动 |
| `selection.rs` | 598 | 未改动 |
| `path_range.rs` | 245 | 未改动 |
| `html.rs` + `html/` | 14 + 2502 | 未改动（HTML 块渲染） |
| `mermaid.rs` | — | **替换为 100 行 stub** |

**Cargo 配置要点**：`markdown.rs` 是 crate 根，故用

```toml
[lib]
name = "zed_markdown"
path = "src/markdown.rs"
```

**许可证**：`gpui` 为 Apache-2.0，其 `examples/` 与 `crates/markdown` 为 **GPL-3.0-or-later**
（见 `crates/markdown/Cargo.toml` 的 `license` 字段）。Buddy 已全盘接受 GPL，
但 vendored 文件的**原始版权头与来源注释必须保留**（`S01-03`）。

---

## 4. 已验证的行为

| 项 | 结果 |
|----|------|
| 编译 | ✅ lib + bin 均通过 |
| 闭包 | ✅ **718 包**（基线 693，**+25**） |
| 重依赖剔除 | ✅ 上表全部确认不在 `Cargo.lock` |
| 流式渲染 | ✅ 306 字符**逐字**喂入（每 16ms 1 字符），无崩溃 |
| 行尾稳定性 | ✅ 用户确认「行尾正常」 |
| 各结构渲染 | ✅ 用户确认「markdown 显示正常」：标题/粗斜体/行内代码/删除线/列表/嵌套列表/代码块/表格/引用/分割线 |
| 自动跟尾 | ✅ `ScrollHandle::scroll_to_bottom()` 有效（用户确认） |

---

## 5. 未覆盖 / 交接

| 项 | 交接 |
|----|------|
| **语法高亮** | `S04-02` —— 接 Comet `crates/syntax`（MIT） |
| **双击选词精度** | `S04-02` —— `CharClassifier` 换成 tree-sitter 字符分类 |
| **增量重解析**（只重解析未稳定块） | `S04-04` —— 本 spike 用 `append` 全量重解析，未测增量 |
| **mend**（半截标记修补） | `S04-05` |
| **veil**（流式渐显） | `S04-06` |
| **`ListState` 跟尾**（Buddy 真实场景） | `S00-07` / `S05-04` —— 本 spike 用的是 `ScrollHandle`，非虚拟列表 |
| **`extract_mermaid_diagrams` 的事件类型陷阱** | 记入本文件 §2.3；实现时勿直接 `use pulldown_cmark::Event` |
| **`[patch]` 的长期维护** | `S01-02` —— 评估是否可接受 |
