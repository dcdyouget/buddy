# S02-01 providers 移植（openai_compatible / anthropic）

> 状态: `done`
> Phase: 02
> 依赖: S02-02, S02-03
> 阻塞: —
> 退役设计文档: `docs/design/rust-data-models.md`（共同负责，见 S02-03；本 spec 为三者中最后完成者，负责删除）

## 目标

把 v1 的 `providers/`（mod / openai_compatible / anthropic，3264 行）迁入 `buddy-engine`，无 Tauri 环境下可对本地 mock SSE 服务完成一次完整流式对话。

## 输入

- 移植源：tag `v1-final` 的 `src-tauri/src/providers/`
- `docs/evidence/s00-08/engine-integration.md` §1.2（edition 2024 唯一修正点）、§3.1（`Done.full_text` 含 think 标签）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S02-01-1 | 依赖面 | `models` / `streaming::{StreamEventEmitter, StreamOutcome, StopReason}` / `tools::{ToolDefinition, ToolSafety}` —— 由 S02-02、S02-03 提供 |
| S02-01-2 | edition 2024 | `\|(_, &v)\|` → `\|&(_, &v)\|`（S00-08 实测唯一一处） |
| S02-01-3 | 空文件 | `providers/openai_composite_safe.rs` 为 0 字节且未被 `mod` 声明 → 不迁 |
| S02-01-4 | mock SSE | 用本地 TCP 监听写一个最小 SSE 响应，驱动两个 provider 各一次，断言事件序列以 `Done` 结束（T02） |

## 验收标准

- [x] providers 随迁单测全部通过，数量与 v1 一致
- [x] mock SSE 集成测试：openai_compatible 与 anthropic 各产生 ≥1 个 `TextDelta` 且以 ~~`Done`~~ **`TurnEnd`** 结束（验收标准修正，见决策记录）
- [x] 分层检查 14 项通过；v1 `cargo check` 通过
- [x] `rust-data-models.md` 已按 RULES §7 处置并登记台账

## 证据

| 项 | 证据 |
|----|------|
| 单测 | `--list`：engine `providers::` 26 个（ignored 0）；v1 `src-tauri` `providers::` 26 个（ignored 0）→ 一致。全量 `cargo test -p buddy-engine`：lib **128 passed / 8 ignored**，`tests/mock_sse` **2 passed** |
| 源码差异 | `mod.rs` / `anthropic.rs` 与 `v1-final` diff **0 行**；`openai_compatible.rs` diff 2 行 = `:711` `.find(\|(_, &v)\| …)` → `.find(\|&(_, &v)\| …)`（edition 2024，与 S00-08 预测的唯一一处吻合） |
| mock SSE | `openai_compatible_streams_to_turn_end`：请求行 `POST /chat/completions HTTP/1.1`、`authorization: bearer test-key`、体含 `"stream":true`；事件 `Start → TextStart → TextDelta("你好") → TextDelta("，世界") → TextEnd → TurnEnd{0}`；`StreamOutcome.full_text == "你好，世界"`。`anthropic_streams_to_turn_end`：`POST /v1/messages`、`x-api-key: test-key`，同样以 `TurnEnd{0}` 收尾 |
| 代理反证 | 本机 `HTTP_PROXY=http://127.0.0.1:7890`。移走 `.cargo/config.toml` 后两个测试均 `ServerError(502, "未知错误")`；放回（`NO_PROXY=127.0.0.1,localhost`）后 2 passed |
| 分层 / v1 回归 | engine 0 处 tauri 代码引用；`cargo tree -p buddy-engine -e normal \| grep -ciE 'tauri\|gpui'` → 0；纪律检查 14/14、拦截验证 **10/10**；`cd src-tauri && cargo check` rc=0 |
| 设计文档处置 | `git rm docs/design/rust-data-models.md`；台账「退役记录」逐条列出 why 去向（新增 `Message.id` 非 UUID 注释）；AGENTS.md 索引行已删 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 依赖顺序 | S02-03 → S02-02 → S02-01 | providers 编译期依赖 models/streaming/tools 类型；原注册表「仅依赖 S01-01」不成立 |
| 验收标准「以 `Done` 结束」 | 改为「以 `TurnEnd` 结束 + `StreamOutcome` 正确」 | 原标准基于错误假设：v1 中 provider 从不发 `Done`，终态事件在 `commands.rs:642-660` 编排层发射。按 v1 真实契约验收，编排层迁移交 S02-07（S02-07-5） |
| 回环代理 | 工作区 `.cargo/config.toml` `[env] NO_PROXY` | 本机 profile 全局设了 `HTTP(S)_PROXY`；测试内 `set_var` 在并行测试中不安全；`[env]` 只影响 cargo 启动的进程且不覆盖用户已设值 |

## 完成记录

- 日期：2026-09-26
- commit：`89f92b8`
- 设计文档处置：`docs/design/rust-data-models.md` **整份删除**，已登记台账（S02-03 / S02-02 / S02-01 共同替代）
