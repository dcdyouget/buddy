# S02-02 tools 移植

> 状态: `done`
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

- [x] `cargo test -p buddy-engine tools::` 通过，测试数与 v1 `cargo test tools::` 一致
- [x] `check_write_allowed` 与 v1 字节一致（`diff` 为空）
- [x] 分层检查 14 项通过；v1 `cargo check` 通过

## 证据

| 项 | 证据 |
|----|------|
| 测试数对照 | `cargo test -p buddy-engine tools::` → **73 passed / 0 failed / 8 ignored**。`--list`：engine `tools::` 81 个（ignored 8）；v1 `src-tauri` `tools::` 81 个（ignored 8）→ 一致。全量 `cargo test -p buddy-engine` → 102 passed / 8 ignored |
| 权限逻辑一致 | `tools/` 15 个文件逐个对 `git show v1-final:…` 做 `diff -q` → **0 个有差异**（含 `builtin.rs:108 check_write_allowed` 与 `:38 check_write_allowed_with_symlinks`） |
| 分层 / v1 回归 | `grep -rn tauri crates/engine/src` → 0；`check-discipline.py` S01-04-1「147 个依赖，0 处禁止项」（`tempfile` 为 dev-dependency，不计入），14 项通过；`cd src-tauri && cargo check` rc=0 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| `commands.rs` 相关注释 | 暂保留 | `tools/mod.rs:44,132`、`builtin.rs:432,441,565,568` 描述 v1 的 commands 分派；ask_user / 审批改写归 S02-07，届时一并更新 |

## 完成记录

- 日期：2026-09-26
- commit：`32204d8`
- 设计文档处置：`rust-data-models.md` 无 tools 专属段落；整份删除归 S02-01（见 S02-03 完成记录）
