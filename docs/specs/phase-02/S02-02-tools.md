# S02-02 tools 移植

> 状态: `doing`
> Phase: 02
> 依赖: S02-03
> 阻塞: —
> 退役设计文档: `docs/design/rust-data-models.md`（共同负责，见 S02-03）

## 目标

把 v1 的 `tools/`（builtin / file_tools / image_generation / websearch，约 5800 行）原样迁入 `buddy-engine`，含全部既有单测。

## 输入

- 移植源：tag `v1-final` 的 `src-tauri/src/tools/`
- `docs/tasks/v2.0.0-gpui/02-engine.md` D03 / D17 / T01

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S02-02-1 | 依赖面 | tools 仅依赖 `models::ImageAttachment`，零 Tauri 引用 |
| S02-02-2 | 文件工具权限不得放宽 | `check_write_allowed` 与路径校验原样保留（D17） |
| S02-02-3 | 测试依赖 | `file_tools/tests.rs` 用 `tempfile` → engine `dev-dependencies` |
| S02-02-4 | 联网测试 | v1 中依赖网络的测试保持其原有 `#[ignore]` 标记，不新增联网测试 |

## 验收标准

- [ ] `cargo test -p buddy-engine tools::` 通过，测试数与 v1 `cargo test tools::` 一致
- [ ] `check_write_allowed` 与 v1 字节一致（`diff` 为空）
- [ ] 分层检查 14 项通过；v1 `cargo check` 通过

## 证据

| 项 | 证据 |
|----|------|
| 测试数对照 | |
| 权限逻辑一致 | |
| 分层 / v1 回归 | |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| | | |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
