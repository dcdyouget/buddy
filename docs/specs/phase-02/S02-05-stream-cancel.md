# S02-05 流式取消语义与事件契约

> 状态: `done`
> Phase: 02
> 依赖: S02-01
> 阻塞: —
> 退役设计文档: `docs/design/sse-and-api.md`（与 S02-06、S02-07 共同负责，最后完成者删除）

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

- [x] 两个 provider 的取消测试通过，记录取消到返回的实测耗时
- [x] 取消后 channel 无新事件（断言）
- [x] ~~usage 行为有测试锁定~~ → **不适用**：v1 的 usage 仅写日志，无 API 表面可锁定（见决策记录）
- [x] 分层检查 14 项通过；v1 `cargo check` 通过

## 证据

| 项 | 证据 |
|----|------|
| 取消测试 | `tests/stream_cancel.rs` **4 passed**：`{openai_compatible,anthropic}_cancel_mid_stream`（收到首个 `TextDelta` 后取消）与 `{…}_cancel_while_waiting_headers`（服务端 10s 后才发响应头，200ms 时取消）。实测取消→返回：读流中 **14.8–50.8 µs**，等响应头 **70.5–84.5 µs**（两轮运行的范围，上限断言 500ms） |
| 取消契约 | 四个用例均断言：`had_stream_error = true`、`terminal_error = {Aborted, "用户取消"}`、`full_text` 保留已累积文本（读流中 = "你好"、等响应头 = ""）；取消后事件流无新增 `TextDelta`，且**无** `Done` / `Error` / `TurnEnd`（provider 不发终态，与 S02-01 契约一致） |
| 无残留连接 | 断言 mock 服务端在 2s 内退出（脚本本需约 6s / 10s），即客户端已断开 HTTP 连接。首版 mock 在等待响应头时不读 socket，察觉不到断开 → 2 个用例失败；改为等待期间 `select!` 监听对端关闭后通过（**修的是测试工具，断言未放宽**） |
| 稳定性 | 全量 `cargo test -p buddy-engine` 连续 5 次：每次 **149 passed / 0 failed** |
| usage | v1 中 usage 只在 `openai_compatible.rs` `parse_prompt_cache_usage` 与 `anthropic.rs:609-628,771-824` 解析后写 `info!` 日志；不进 `StreamEvent`、不进 `StreamOutcome`；v1 前端 `src/` 中唯一相关引用是兼容开关 `supports_stream_options_usage`（`src/types/index.ts:21`） |
| 回归 | 纪律检查 14/14、拦截验证 10/10；`cd src-tauri && cargo check` rc=0 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 改名与收窄 | 原「streaming 去 Tauri 化」→「流式取消语义与事件契约」 | models 与 streaming 互相依赖，去 Tauri 只能随 S02-03 完成；剩余真正未验证的是取消（S00-08 交接） |
| usage 验收 | 不适用 | 02-engine.md D10「usage/token 计数不得丢失」的 v1 真实形态只是日志，随 provider 原样迁入即保留；若 v2 要在 UI 展示 usage 属新功能，不在移植范围 |
| 编排层取消 | 不在本 spec | 审批 / 提问 / 工具循环边界的取消检查在 v1 `commands.rs` 的 `send_message`，随编排迁移归 S02-07；`sse-and-api.md` 相应段落改由 S02-07 退役 |

## 完成记录

- 日期：2026-09-26
- commit：`596b441`
- 设计文档处置：`sse-and-api.md` 部分实现 → 文件头已标注已实现 / 未实现（RULES §7.2）；整份删除归 S02-06 / S02-07 中最后完成者
