# S02-08 IPC 层作废（命令 → engine API 覆盖表）

> 状态: `todo`
> Phase: 02
> 依赖: S02-01, S02-02, S02-03, S02-04, S02-05, S02-06, S02-07
> 阻塞: —
> 退役设计文档: `docs/design/ipc-contract.md`

## 目标

证明 v1 `commands.rs` 的每一个 `#[tauri::command]` 在 engine 中都有等价的直接调用入口（或明确归属到后续 Phase），从而 v2 不需要 IPC 层。

## 输入

- v1 `src-tauri/src/commands.rs`（1868 行）
- `docs/design/ipc-contract.md`

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S02-08-1 | 覆盖表 | 逐条列出 v1 命令 → engine 函数 / 归属 spec（如窗口类归 Phase 07） |
| S02-08-2 | **不删除 v1 的 `commands.rs`** | v1 在 Phase 05 前必须可用（S01-05 决策）；物理删除随 v1 整体清理进行 |
| S02-08-3 | DTO 合并 | 仅为 IPC 存在、与 model 重复的类型不迁入 engine（D25） |

## 验收标准

- [ ] 覆盖表完整：v1 命令数 = 表中行数（`grep -c '#\[tauri::command\]'` 对照）
- [ ] 表中每个 engine 入口可编译（`cargo check`）
- [ ] `ipc-contract.md` 已处置并登记台账

## 证据

| 项 | 证据 |
|----|------|
| 覆盖表 | |
| 设计文档处置 | |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 范围 | 原「删除 commands.rs」→「覆盖表 + 作废契约」 | 删除会破坏 v1，而 v1 是 Phase 05 前的唯一可用版本与视觉对照基准 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
