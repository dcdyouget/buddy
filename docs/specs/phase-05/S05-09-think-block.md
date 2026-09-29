# S05-09 思考块

> 状态: `done`
> Phase: 05
> 依赖: S05-08
> 阻塞: —
> 退役设计文档: —

## 目标

思考过程折叠 / 展开，与 v1 `ThinkSection.tsx` 一致（默认折叠、流式中状态）。

## 输入

- v1 `ThinkSection.tsx`、`thinkParser.ts`（`<think>` 标签拆分）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S05-09-1 | 折叠状态 | 默认折叠；流式中显示进行态 |

## 验收标准

- [x] 折叠 / 展开与流式态 —— 用户目检通过（2026-09-29，handoff §6.5 第 12 项）

## 证据

| 项 | 证据 |
|----|------|
| 实现 | `crates/ui/src/chat/think_block.rs`；列表中按行 id 记住展开状态、展开内容用 markdown（`thinking_style`）；commit `6aa8ffa` |
| 单测 | `previews_match_v1`（第一行 80 字 / 流式末尾 96 字）、`animations_follow_v1_keyframes`（加载点与光泽关键帧） |
| T20 | `chat_preview`「模拟思考与工具」：流式思考行出现在行模型中（行类型 think） |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 左侧强调边 | 覆盖在 1px 边框上的 2px 竖条 | GPUI 边框四边同色 |
| 光泽 | 两段渐变拼成「透明→高光→透明」 | GPUI 渐变只有两个色标 |
| 入场动画 | 不做（v1 `think-section-enter` 200ms 淡入上移） | 次要；目检若在意再补 |

## 完成记录

- 日期：2026-09-29
- commit：`6aa8ffa`
- 设计文档处置：无（本 spec 未声明退役设计文档）
