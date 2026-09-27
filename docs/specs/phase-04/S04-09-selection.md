# S04-09 文本选择与复制

> 状态: `blocked`
> Phase: 04
> 依赖: S04-03
> 阻塞: 等待用户目检（`handoff.md` §6.5 第 6 项：拖选 / 双击选词 / 选区颜色）
> 退役设计文档: —

## 目标

消息内文本可选择与复制（含跨块），复制内容为源 markdown 还是渲染文本与 v1 一致。

## 输入

- zed `selection.rs`（已 vendor）；v1 `MessageBubble` 的选择行为

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S04-09-1 | 复制语义 | 以 v1 实测为准 |

## 验收标准

- [x] 复制结果与 v1 一致（记录用例）—— 用例与基准在 `docs/evidence/s04-09/`；两处有意不同见决策记录
- [ ] 拖选 / 双击选词可用 —— **需用户目检**（handoff §6.5 第 6 项）

## 证据

| 项 | 证据 |
|----|------|
| 实现 | `crates/ui/src/markdown/copy.rs`；`gfm::decorations` 挂 `copy_text`；`message_style` 设选区色；commit `a1ed468` |
| vendored 补丁 | `MarkdownDecorations::copy_text`、`RenderedText::lines_for_range`；patch 同步检查通过 |
| v1 基准 | v1 组件真实渲染（vitest 临时文件，已删）→ `v1-rendered.html`；`scripts/v1-baseline/selection_text.swift` → `v1-selection.json` |
| T07 | `markdown_preview -- --selftest`：模拟真实鼠标（移入 → 按下 → 拖到末尾 → 松开，每步一帧）+ 真实按键 Cmd+C（经 `markdown::init` 绑定），读剪贴板（事后恢复用户原剪贴板）→ 与 v1 基准去掉两处界面文字后**逐字节一致**，PASS |
| T07 拦截 | ① 去掉 `copy_text` 回调 → 段落间少空行、表格无制表符、含零宽空格 → FAIL；② 去掉 Cmd+C 绑定 → 剪贴板为空 → FAIL；均退出码 1，已恢复 |
| 基准自纠 | 首次测量未带 v1 气泡内联字号（14px），h3 后空行规则测反；补上后 v1 与规则一致（README「注意」） |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 复制语义 | 渲染后的纯文本（非 markdown 源） | v1 为浏览器选区复制，即渲染文本；「复制整条回答的 markdown」是消息操作按钮（`MessageActions.tsx`，复制源文本）→ Phase 05 |
| 块间分隔 | 按 WebKit 规则：段落（引用中最后一段除外）与 h3–h6 后空行；表格同行 `\t` | 实测 v1 如此；表格制表符粘贴到表格软件可成列 |
| 界面文字 | 不复制代码块头部（语言标签、「复制」）与复选框前空格 | v1 复制它们是副作用（头部文字可被选中），不是内容 |
| 加粗守卫 | 复制时去掉 | S04-05 移交；v1 渲染 `<strong>` 时去掉 |
| 选区颜色 | 品牌色 25% 不透明度（暂定） | v1 用系统高亮色（无 `::selection`）；GPUI 无读取接口；上游把选区画在文字之上，必须半透明。**待用户决定**：保持品牌色，或另做读取系统高亮色（macOS `selectedTextBackgroundColor`，需平台代码） |
| Cmd+C 绑定 | `markdown::init` 中绑定到 `Markdown` 上下文 | 上游靠 zed 键位表；缺绑定则选中后 Cmd+C 无反应（T07 拦截 ② 证实） |
| 全选 | 不在本 spec | v1 的 Cmd+A 选中整个页面；消息列表的全选行为属 Phase 05 |

## 完成记录

- 日期：（待目检通过后填写）
- commit：`a1ed468`
- 设计文档处置：无
