# S04-02 引入 Comet syntax 替换 language stub

> 状态: `doing`
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

- [ ] 代码块高亮与 v1 同语言集（差异列表明确）
- [ ] 同一代码带 / 不带高亮时布局高度相同（T03）
- [ ] 无 zed `language` / `tree-sitter`（zed fork）/ `wasmtime` 进入闭包

## 证据

| 项 | 证据 |
|----|------|
| | |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| | | |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
