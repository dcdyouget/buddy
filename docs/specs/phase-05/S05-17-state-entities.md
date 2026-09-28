# S05-17 状态实体拆分与细粒度通知

> 状态: `done`
> Phase: 05
> 依赖: S05-01
> 阻塞: —
> 退役设计文档: —

## 目标

把 v1 1447 行的 chatStore 拆为多个 GPUI `Entity`；流式更新只通知受影响的实体 / 行，不整页重绘。

## 输入

- v1 `src/stores/chatStore.ts`（多数编排已由 engine S02-07 承担，UI 只消费事件）
- S04-06 移交：流式事件队列（文本未放完时工具 / 结束事件排队）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S05-17-1 | 实体划分 | 会话 / 活跃回合 / 草稿 / 交互（审批、提问） |
| S05-17-2 | 事件队列 | 与 v1 `_drainStreamEventQueue` 语义一致 |

## 验收标准

- [x] 流式一个 token 只重绘一行（计数断言）—— T12
- [x] 事件顺序测试（文本放完前的工具 / 结束事件）—— `multi_turn_tool_flow_keeps_order_while_text_pending` 等

## 证据

| 项 | 证据 |
|----|------|
| 状态 | `crates/ui/src/chat/state.rs`（纯数据，移植 v1 chatStore 的流式 / 回合 / 工具 / 水化逻辑）；commit `08c1c76` |
| 实体 | `chat/session.rs` `Conversation`：接收事件批次、流式期间约每 16ms 推进节奏器，**只在 `revision` 变化时 notify**；视图 `observe` 它 |
| v1 用例 | `src/stores/chatStore.test.ts` 14 例逐一移植 + 4 例（历史 `<think>` 拆块、隐藏窗口立即放出、真实时钟节奏、队列中增量合并），`cargo test -p buddy-ui chat::state` 全过 |
| 拦截 | 去掉「结构事件等正文放完」→ 3 例失败；已恢复 |
| 出错分支（补，commit `c2299d6`） | v1 `useStreaming.ts` error 分支：`aborted` 按正常结束（不显示错误）；401 / unauthorized → `needs_api_key`；429 / quota、HTTP 5 / server_error、网络错误 / timeout → 追加并待持久化提示消息（`pending_saves`）；单测 `aborted_is_a_normal_finish`、`error_followups_match_v1`。持久化与切页由会话实体接入 engine 时处理（S05-18） |
| 只重绘一行 | `chat_preview -- --selftest` T12：流式期间 66 次行同步，单次最多重测 1 行，行数不变；拦截（行版本不含正文）→ FAIL |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| UI 状态范围 | 只保留界面需要的部分 | 调用模型、工具循环、持久化已由 engine（S02-07）承担；v1 store 中对应代码不移植 |
| 实体划分 | 会话（状态 + 流式推进）一个实体；草稿随 Composer（S05-06）、审批 / 提问状态在会话内 | 审批与提问由流式事件驱动、随流式结束清空（v1 同），与会话同生命周期；草稿独立是为了输入时不触发列表同步 |
| 「只重绘一行」的口径 | 流式更新只使一行失效并重测；其余行的布局缓存保持 | GPUI 为即时模式，每帧重建可见行元素，但未失效的行不重新测量 / 排版 —— 这才是 v1「整页重渲染」问题的对应物 |
| 流式结束后是否从磁盘重载 | 不重载，保留界面按事件构建的消息 | 行 id 以用户消息为锚（S05-02），与 engine 落盘后重载结果一致（测试）；重载只会多一次 IO 与重建 |
| 事件队列 | 与 v1 同：同一先进先出队列，正文放完前结构事件等待 | v1 用例「按顺序保留工具轮与最终答案」 |

## 完成记录

- 日期：2026-09-28
- commit：`08c1c76`、`cb5eaa5`（行模型）
- 设计文档处置：无