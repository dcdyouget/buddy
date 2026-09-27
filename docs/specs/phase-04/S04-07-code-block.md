# S04-07 代码块渲染与复制

> 状态: `done`
> Phase: 04
> 依赖: S04-02
> 阻塞: —
> 退役设计文档: —

## 目标

代码块：语言标签、等宽不换行、横向滚动、复制按钮，外观取自 v1 `CodeBlock.tsx` 与 `--code-*` 令牌。

## 输入

- v1 `src/components/chat/CodeBlock.tsx`（229 行）、`PlainCodeBlock.tsx`

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S04-07-1 | 复制 | 点击复制源码，状态反馈与 v1 一致 |

## 验收标准

- [x] 复制内容与源码逐字节一致（测试）—— 「源码」按 v1 实际语义：CommonMark 代码文本去掉一个末尾换行
- [x] 外观与 v1 一致 —— 2026-09-28 用户目检通过（handoff §6.5 第 4 项）

## 证据

| 项 | 证据 |
|----|------|
| 实现 | `crates/ui/src/markdown/code_block.rs`（渲染器、复制、`v1_language`、复制动画）；`crates/ui/src/icons.rs`（lucide `copy` / `check`）；`markdown::message_style`；commit `200d6ad`、`0a83f01` |
| vendored 补丁 | `crates/markdown/src/markdown.rs` 接通 `CodeBlockRenderer::Custom`；`patches/zed-markdown-290cbcb.patch` 已重生成，纪律检查「S04-03 GPL patch 同步」通过；`VENDOR.md` 修改清单已登记 |
| T04 自检 | `cargo run -p buddy-app --example markdown_preview -- --selftest` → `T04: 代码块 6 个，渲染器调用 6 次；直接切源码会出错的块 2 个` / `PASS S04-07 T04`。6 个块覆盖：rust、ts 超长行、无语言、bash、列表内 python、缩进式 |
| T04 反证 | 「直接切源码」在列表内与缩进式两块上与 v1 不同（自检内断言 ≥ 2） |
| T04 拦截 | 临时删去 `code_text` 的去末尾换行 → `FAIL T04 块 0/1/2…`，selftest 退出码 1；已恢复 |
| 单测 | `cargo test -p buddy-ui` 中 `code_block`（关键帧 0.72/1.12/1、v1 语言标识规则）、`easing`（端点、单调、回弹；对照值独立二分求得 0.8778336）、`icons`（资源可加载） |
| 运行日志 | `RUST_LOG=debug` 运行目检窗口 6 s，无 svg / asset / error 日志 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 渲染途径 | 补丁接通 vendored `CodeBlockRenderer::Custom`，不改 Default | 上游本 rev 的 Custom 为空实现（`render`/`transform` 从不调用、全仓无调用方），Default 只有悬浮图标按钮，做不出 v1 的头部栏；接通 Custom 改动集中（约 50 行）且不影响 Default |
| 复制内容 | 取解析事件的代码文本，去一个末尾换行 | 与 v1 相同（react-markdown 给出 CommonMark 文本，再 `replace(/\n$/, '')`）；直接切源码会带上列表缩进（T04 反证） |
| 语言标识 | 信息串首词的 `[\w-]+` 前缀，缺省 `text`，显示转小写 | 逐字对应 v1 `/language-([\w-]+)/` 与 `textTransform: lowercase`；`plain/plaintext/text/txt` 为纯文本块：无标签、行高 1.75 |
| 已复制状态 | 全局 `(markdown 实体, 块起点)` 集合 + 2 s 定时移除 | v1 `setTimeout(2000)`；不改 vendored 的私有 `copied_code_blocks` |
| 图标 | 引入 lucide `copy` / `check` 两个 SVG（ISC，已登记 NOTICES），`icons::Assets` 嵌入 | 硬约束 4 只用 SVG；图形与 v1 同源 |
| 复制成功动画 | 实现（0.72→1.12→1，200 ms，逐段 ease-spring） | v1 有；新增 `theme_system::easing` 供后续动效复用 |
| 已知差距 | ① hover 变色无 120 ms 过渡（GPUI hover 为即时）；② 横向滚动条不绘制（macOS 浮动滚动条仅滚动时出现，观感接近）；~~③ 减弱动效下仍播放复制动画~~ → S04-06 已接入（系统开启时不播放） | 目检时请关注 ①② 是否可接受 |
| 流式未闭合围栏 | 不在本 spec | v1 流式中未闭合围栏显示为无复制按钮的纯文本块 → S04-06（v1 用例 `:245`） |

## 完成记录

- 日期：2026-09-28（2026-09-28 用户目检通过）
- commit：`200d6ad`、`0a83f01`
- 设计文档处置：无