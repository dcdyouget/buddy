# S02-03 models 与流式数据类型移植

> 状态: `doing`
> Phase: 02
> 依赖: S01-01
> 阻塞: —
> 退役设计文档: `docs/design/rust-data-models.md`（与 S02-01、S02-02 共同负责，最后完成者删除）

## 目标

把 v1 的 `models/`（含 `model_identity`）、`mcp/` 与 `streaming.rs` 原样迁入 `buddy-engine`，其中 `StreamEventEmitter` 从 Tauri `AppHandle` 改为 channel。

## 输入

- 移植源：tag `v1-final`（`9cc244a`）的 `src-tauri/src/models/`（1114 行）、`src-tauri/src/mcp/mod.rs`（39 行）、`src-tauri/src/streaming.rs`（808 行）
- `docs/evidence/s00-08/engine-integration.md` §1.1（emitter 改法，已实测）
- `docs/dev-environment.md` §1（edition 2024 迁移点）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S02-03-1 | models + streaming 必须同批 | `models/message.rs:16` 用 `streaming::ContentBlock`，`streaming.rs:22` 用 `models::ImageAttachment` —— **互相依赖，无法拆成两个可编译的 spec** |
| S02-03-2 | emitter 去 Tauri | `app: AppHandle` → `tx: UnboundedSender<StreamEvent>`；加 `channel()` 便捷构造（S00-08 §1.1） |
| S02-03-3 | 原样移植 | 除 emitter 与 edition 2024 必需修正外不改逻辑；保留 `#[cfg(test)]` 测试 |
| S02-03-4 | 依赖 | 按编译需要补 workspace 依赖，不引入任何 GPUI/Tauri |

## 验收标准

- [ ] `cargo check -p buddy-engine` 通过，engine 源码 `grep -rn tauri` 仅剩注释或 0 处
- [ ] 随迁的 models / streaming 单测在 `cargo test -p buddy-engine` 中全部通过，数量与 v1 同名测试一致
- [ ] `check-discipline.py` 14 项通过（engine 依赖树 0 处 GPUI/Tauri）
- [ ] v1 `cd src-tauri && cargo check` 仍通过

## 证据

| 项 | 证据 |
|----|------|
| 编译 | |
| 测试 | |
| 分层 | |
| v1 回归 | |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 合并 streaming 类型进本 spec | 是 | 见 S02-03-1；拆开会产生中间死亡态（RULES §2-4） |

## 完成记录

- 日期：
- commit：
- 设计文档处置：

## 备注

`streaming.rs` 的去 Tauri 化随本 spec 完成，S02-05 相应收窄为「取消语义与事件契约」。
