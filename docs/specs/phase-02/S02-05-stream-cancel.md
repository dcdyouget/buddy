# S02-05 流式取消语义与事件契约

> 状态: `todo`
> Phase: 02
> 依赖: S02-01
> 阻塞: —
> 退役设计文档: `docs/design/sse-and-api.md`（与 S02-06 共同负责，后完成者删除）

## 目标

实测并用测试锁定「流式中途取消」与「usage 计数」在 engine 中的行为：取消后流及时结束、无残留任务、事件以确定的终止事件收尾。

## 输入

- S00-08 未覆盖项：取消生成（`cancel_rx`）从未被触发（`docs/evidence/s00-08/engine-integration.md` §7）
- `docs/tasks/v2.0.0-gpui/02-engine.md` D09 / D10 / D12 / T03
- S02-01 的 mock SSE 测试设施

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S02-05-1 | 去 Tauri 已完成 | emitter 改 channel 已随 S02-03 完成（S02-03-1 的循环依赖），本 spec 不重复 |
| S02-05-2 | 取消测试 | mock SSE 慢速吐字 → 中途 `cancel_tx.send(true)` → 断言 `stream_chat` 在限定时间内返回、终止事件类型、之后无新事件（T03） |
| S02-05-3 | 两个 provider 都测 | openai_compatible 与 anthropic 各一次（D12） |
| S02-05-4 | usage 计数 | 若 mock 响应带 usage，断言其出现在事件或 `StreamOutcome` 中（D10） |
| S02-05-5 | 发现即记录 | 若取消语义存在缺陷，先记录现象与根因，修复不得改变正常（未取消）路径行为 |

## 验收标准

- [ ] 两个 provider 的取消测试通过，记录取消到返回的实测耗时
- [ ] 取消后 channel 无新事件（断言）
- [ ] usage 行为有测试锁定
- [ ] 分层检查 14 项通过；v1 `cargo check` 通过

## 证据

| 项 | 证据 |
|----|------|
| 取消测试 | |
| usage | |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 改名与收窄 | 原「streaming 去 Tauri 化」→「流式取消语义与事件契约」 | models 与 streaming 互相依赖，去 Tauri 只能随 S02-03 完成；剩余真正未验证的是取消（S00-08 交接） |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
