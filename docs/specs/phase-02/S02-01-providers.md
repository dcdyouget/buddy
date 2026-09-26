# S02-01 providers 移植（openai_compatible / anthropic）

> 状态: `todo`
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

- [ ] providers 随迁单测全部通过，数量与 v1 一致
- [ ] mock SSE 集成测试：openai_compatible 与 anthropic 各产生 ≥1 个 `TextDelta` 且以 `Done` 结束
- [ ] 分层检查 14 项通过；v1 `cargo check` 通过
- [ ] `rust-data-models.md` 已按 RULES §7 处置并登记台账

## 证据

| 项 | 证据 |
|----|------|
| 单测 | |
| mock SSE | |
| 分层 / v1 回归 | |
| 设计文档处置 | |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 依赖顺序 | S02-03 → S02-02 → S02-01 | providers 编译期依赖 models/streaming/tools 类型；原注册表「仅依赖 S01-01」不成立 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
