# S04-02 引入 Comet syntax 替换 language stub

> 状态: `done`
> Phase: 04
> 依赖: S04-01
> 阻塞: —
> 退役设计文档: —

## 目标

移植 Comet `crates/syntax`（MIT）为 `crates/syntax`，实现 `language_stub` 的 `highlight_text_resolved` 等接口，使代码块获得语法高亮；语言集以 **v1 实际支持的**为准。

## 输入

- v1 高亮语言（`prism-react-renderer` 2.4.1 运行时注册，`node -e` 实测）：c cpp css go graphql html/xml json js/jsx/flow kotlin markdown objc python reason regex rust sql swift ts/tsx yaml（+ actionscript / coffeescript）；**无** bash / java / ruby / php / toml
- Comet `syntax`：tree-sitter 0.26（上游，非 zed fork，无 wasmtime / cmake）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S04-02-1 | 语言集 | v1 集合 ∩ tree-sitter 可得者；v1 有而无 grammar 的（objc / graphql / reason / regex / actionscript / coffeescript）按纯文本，列入差异 |
| S04-02-2 | grammar 许可证 | 逐一记录进 `THIRD_PARTY_NOTICES.md` |
| S04-02-3 | 只改前景色 | 高亮不改字体 / 字重 / 行高（D 验收「高亮只改前景色」） |

## 验收标准

- [x] 代码块高亮与 v1 同语言集（差异列表明确）
- [x] 同一代码带 / 不带高亮时布局高度相同（T03）
- [x] 无 zed `language` / `tree-sitter`（zed fork）/ `wasmtime` 进入闭包

## 证据

| 项 | 证据 |
|----|------|
| 移植 | `crates/syntax`（buddy-syntax，MIT）= Comet `crates/syntax`（commit a4781608）原样；Comet 自带测试 **18 + 8 passed**（1 ignored） |
| 接入 | `language_stub.rs`：`highlight_text_resolved` 调 `buddy_syntax::highlight`，行内偏移换算为绝对偏移；`LanguageRegistry` 三个 async 方法与 `language_for_tag` 同源 |
| 语言集 | 单测 `only_v1_languages_are_highlighted`：rust / ts / tsx / python / go / json / yaml / cpp / css / html / xml / svg / flow / kotlin / swift / sql / markdown 可识别；bash / sh / toml / java / ruby / php / lua / text / plain / objc / graphql 不启用 |
| 类别映射 | 单测 `rust_code_gets_v1_categories`：`fn` / `let` → keyword、`42` → number、`// hi` → comment、`main` → function，且第二行区间为全文绝对偏移 |
| 配色 | 单测 `syntax_theme_follows_category_order_and_v1_styles`：浅 / 深两套，keyword = `--code-syntax-keyword` + 字重 600、comment 斜体，其余 7 类只设前景色 |
| T03 布局不变 | `cargo run -p buddy-app --example markdown_preview -- --selftest`：5 种语言 11 行、12 个带字重 / 斜体的片段，**最大宽度差 6.1e-5 px（浮点累加），ascent / descent 完全一致** → PASS |
| T03 反证 | 高亮片段改用非等宽 Helvetica → 11 行全部 FAIL，RESULT: FAIL |
| 闭包 | `cargo tree -p buddy-markdown` 中 `wasmtime` / `language` / `lsp` 命中 0；`tree-sitter` 为 crates.io 上游 0.26.11；`Cargo.lock` 788 → 818（+30，全部 MIT），已登记 `THIRD_PARTY_NOTICES.md` |
| GPL patch | 重新生成并验证「上游原文 + patch == vendored」 |
| 提交 | `3aa039b` |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 关键字粗体 / 注释斜体 | **保留**（同 v1），不遵循拆解依据「只改前景色」 | v1 `buddyCodeTheme` 即如此；等宽字体下不改变字宽与行高，T03 实测证明布局不变 |
| 语言集 | v1 集合 ∩ tree-sitter | 与 v1 一致；Comet 额外支持的 11 种是否启用 → 用户决策清单 |
| v1 有、无 grammar 的 6 种 | 按纯文本 | objc / graphql / reason / regex / actionscript / coffeescript 无对应语法；属已知差异 |
| 高亮颜色来源 | Buddy 自建 `SyntaxTheme`（不用 zed 主题） | 保证与 v1 `--code-syntax-*` 一致 |
| Comet 未用的语法包 | 暂时照常编译 | 是否裁剪（减小体积）由 S04-03 用数据决定 |

## 完成记录

- 日期：2026-09-27
- commit：`3aa039b`
- 设计文档处置：—
