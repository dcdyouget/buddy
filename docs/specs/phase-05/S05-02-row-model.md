# S05-02 块粒度行模型与稳定 id

> 状态: `done`
> Phase: 05
> 依赖: S05-01
> 阻塞: —
> 退役设计文档: —

## 目标

一行 = 一条用户消息 / 一段助手内容块 / 一个工具组 / 一张卡片；行 id 稳定，流式结束落盘后 id 不变、不闪烁。

## 输入

- v1 `MessageBubble.tsx` 的块渲染顺序（blocks + 按 `insertAfterBlockIndex` 插入工具调用）
- Comet 行 id 形式 `{msgId}#{partId}.{blockIx}` / `{msgId}#g{groupIx}`

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S05-02-1 | 行模型 | 消息 → 行的纯函数，可单测 |
| S05-02-2 | 最小 splice | 行集合变化按 (id, 版本) 做最小 splice，而非重置列表 |

## 验收标准

- [x] 流式 → 落盘前后行 id 序列一致（测试）
- [x] 历史消息的行顺序与 v1 渲染顺序一致（测试）

## 证据

| 项 | 证据 |
|----|------|
| 实现 | `crates/ui/src/chat/rows.rs`（`build_rows` / `diff` / `Splice`）；commit `cb5eaa5`（随 S05-01 先行实现，登记见决策记录） |
| 稳定 id | 测试 `live_and_persisted_rows_share_ids`：逐事件流式构建的行 id 与结束后、以及按 engine 落盘形态重载的行 id 完全一致；流式中出现过的 id 都在最终集合中（占位行除外） |
| v1 顺序 | 测试 `history_rows_follow_v1_order`、`insert_after_buckets_match_v1`（-1 / 块后 / 有定位时无定位者放末尾） |
| 最小 splice | 测试 `diff_is_minimal_splice`、`streaming_changes_only_last_row`；运行时 T12 |
| 测试发现并修正 | 流式轮次的空分隔块曾成行，结束时被去掉 → 行消失（闪烁）；改为空正文块不成行、无内容时用流式专用占位行 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行 id | 以本轮用户消息 id 为锚：`{u}` / `{u}#{m}.{i}` / `{u}#{m}.t.{调用 id}` | engine 落盘时才生成 assistant / tool 的 id 且不随事件下发；用户消息 id 由界面生成、发送与落盘一致；engine 每工具轮一条 assistant，(m, i) 两边一致 |
| 占位行 | 流式中回答尚无可见内容时出现 `{u}#{m}.pending` | v1 此时只显示呼吸星标；它本就只在流式中存在 |
| 先行实现 | 本 spec 的代码在 S05-01 进行中提交（`cb5eaa5`），状态按纪律补走 `doing` → `done` | S05-01 的列表骨架需要行模型才能验证；如实登记，不回改提交历史 |

## 完成记录

- 日期：2026-09-28
- commit：`cb5eaa5`
- 设计文档处置：无