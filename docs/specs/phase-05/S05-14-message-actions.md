# S05-14 消息操作（复制 / 重试 / 编辑）

> 状态: `done`
> Phase: 05
> 依赖: S05-08
> 阻塞: —
> 退役设计文档: —

## 目标

消息操作栏：复制回答（源文本）、回到提问、时间，与 v1 `MessageActions.tsx` 一致。

## 输入

- v1 `MessageActions.tsx`（复制 1.6s 反馈、`getAnswerText` 拼接规则）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S05-14-1 | 复制内容 | 与 v1 `getAnswerText` 一致 |
| S05-14-2 | 显示规则 | 非流式、下一条可见消息不是助手消息、且有正文时，在回答后插入独立行 `{u}#{m}.actions`（`rows.rs`） |
| S05-14-3 | 回到问题 | 把本轮用户消息行滚到视口顶部（`Transcript::scroll_to_row`） |

## 验收标准

- [x] 复制内容测试（`message_actions` 单测；T21 真实点击）
- [x] 观感 —— 用户目检通过（2026-09-29，handoff §6.5 第 14 项）

## 证据

| 项 | 证据 |
|----|------|
| 单测 | `cargo test -p buddy-ui message_actions`、`rows::tests::actions_row_rules_match_v1` |
| T21 | `cargo run -p buddy-app --example chat_preview -- --selftest`：真实鼠标点击复制 → 剪贴板 == 回答正文、「已复制」1.6 秒后恢复、回到问题后首行为该轮提问；拦截：把反馈时长改为 5 秒 → T21 FAIL |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 重试 / 编辑 | 不做 | v1 `MessageActions` 只有复制、回到问题、时间；标题中的「重试 / 编辑」为旧估计 |
| 行边界测量 | 绝对定位 canvas 须显式 `top_0().left_0()` | 否则落在内容流之后，记录的边界下移一整行（T21 定位时发现） |

## 完成记录

- 日期：2026-09-29
- commit：`1ce9d4d`
- 设计文档处置：无（本 spec 未声明退役设计文档）
