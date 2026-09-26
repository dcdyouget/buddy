# S02-06 tokio / GPUI 执行器桥接

> 状态: `todo`
> Phase: 02
> 依赖: S02-05
> 阻塞: —
> 退役设计文档: `docs/design/sse-and-api.md`（与 S02-05、S02-07 共同负责，最后完成者删除）

## 目标

在 `buddy-app` 中提供一个桥接层：UI 发起一次对话 → tokio 运行 provider → 事件经 `rx.recv().await` 送回 GPUI 前台，无轮询延迟、不捕获 `Rc`/`Cell`。

## 输入

- `docs/evidence/s00-08/engine-integration.md` §3.3 / §3.4 / §4（`Send` 与 `'static` 约束、推荐骨架）
- `docs/tasks/v2.0.0-gpui/02-engine.md` D07 / D11

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S02-06-1 | 位置 | 桥接属于 app 层（依赖 `gpui_tokio`，GPL）；engine 只暴露 `Send` 的 future 与 channel |
| S02-06-2 | 消费 | `rx.recv().await` 替代 S00-08 的 `try_recv` + 20ms 轮询 |
| S02-06-3 | 合并 | 同一帧内的多个增量合并后一次 `cx.notify()`（D11 反压） |
| S02-06-4 | 取消 | UI 侧取消句柄 → S02-05 验证过的 `cancel_tx` |

## 验收标准

- [ ] `buddy-app` 下一个无界面（或最小界面）驱动程序：对 mock SSE 完成一次对话，记录首个事件到达前台的延迟
- [ ] 取消经桥接层生效（测试或日志证据）
- [ ] engine 依赖树仍 0 处 GPUI；v1 `cargo check` 通过

## 证据

| 项 | 证据 |
|----|------|
| 延迟 | |
| 取消 | |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| | | |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
