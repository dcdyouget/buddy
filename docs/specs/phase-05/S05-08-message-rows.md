# S05-08 消息行渲染（用户 / 助手 / 流式态 / 错误态）

> 状态: `done`
> Phase: 05
> 依赖: S04-04, S05-02
> 阻塞: —
> 退役设计文档: `docs/design/component-mapping.md`（与 S06-* 共同，Phase 05 完成时处理本阶段部分）

## 目标

用户气泡与助手内容行按 v1 外观渲染；助手走 Phase 04 markdown（含流式渐显）；错误提示条与 v1 一致。

## 输入

- v1 `MessageBubble.tsx`、`LiveMessageBubble.tsx`、`ChatPage.tsx` 错误条（`chat-error`）
- Phase 04 移交（handoff §8.1）：S04-05 / S04-06 / S04-07 / S04-08 的接入方式

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S05-08-1 | 用户气泡 | 最大宽 80%、tint 底、圆角 md/md/md/sm、行高 1.5 |
| S05-08-2 | 助手 | 行高 1.6、续段紧凑衔接（工具循环产生的连续助手消息） |
| S05-08-3 | 流式 | Pacer + 渐显 + 星标（S04-06） |

## 验收标准

- [x] 流式 → 完成的行渲染测试 —— `chat_preview` T12（流式中只重测最后一行、行数稳定）、T15（用户消息换行）；`rows` 单测（位置 / 续段）；`message_row` 单测（留白与 v1 一致）
- [x] 观感与 v1 一致 —— 2026-09-28 用户目检通过（handoff §6.5 第 9 项）

## 证据

| 项 | 证据 |
|----|------|
| 实现 | `crates/ui/src/chat/message_row.rs`（用户气泡、助手行留白、错误条）、`chat/transcript.rs` 行渲染（流式渐显 + 星标、正文 / 用户文本 markdown 实体按行 id 复用）、`components.rs`（v1 `IconButton`）；commit `65635b3` |
| 盒模型 | T15：单行用户消息行高 55px = 行 8+8 + 气泡 8+8 + 行高 21 + 边框 2，与 v1 一致；三行比单行高 42px（2 × 21，`pre-wrap` 换行保留） |
| 留白 | 单测 `single_block_message_matches_v1_total_spacing`：普通消息上下各 12px（行 space-2 + 气泡 space-1）、续段上 4 / 下 0、块间 4 |
| 错误态 | `chat_preview` 的「模拟出错」：出错前已显示的正文保留为消息，提示条显示错误（S05-17 v1 用例同语义） |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 留白分摊 | v1 按整条消息的 padding 分摊到该消息的首行顶部 / 末行底部，块间 4px | v2 把助手消息拆成块行（S05-02），总留白与 v1 相同 |
| 用户正文 | 经 links-only markdown 渲染 | v1 用户文字可选择复制；GPUI 纯文本不可选。links-only 模式不解析 markdown（与 v1 纯文本一致），网址按普通文字显示且点击无动作（v1 不识别链接） |
| 待其他 spec | 用户消息图片 → S05-07；消息操作栏 → S05-14；思考块 → S05-09；工具行 → S05-10 | 按 spec 划分 |

## 完成记录

- 日期：2026-09-28（2026-09-28 用户目检通过）
- commit：`65635b3`
- 设计文档处置：无（`component-mapping.md` 待 Phase 05 / 06 全部完成后统一退役）