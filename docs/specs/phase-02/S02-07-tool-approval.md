# S02-07 对话编排迁入（工具循环 / 审批 / 提问 / 持久化 / 终态事件）

> 状态: `done`
> Phase: 02
> 依赖: S02-05
> 阻塞: —
> 退役设计文档: `docs/design/sse-and-api.md`（与 S02-05、S02-06 共同负责；编排相关段落由本 spec 实现）

## 目标

把 v1 `commands.rs` 中的工具调用主循环与「审批 / 提问」往返（原经 Tauri event + invoke 配对）改为 engine 内的请求-响应 API：engine 发出带 id 的请求，UI 用同一 id 回应。

## 输入

- v1 `src-tauri/src/commands.rs`（tag `v1-final`）中的工具循环、审批与提问实现
- `docs/tasks/v2.0.0-gpui/02-engine.md` D13-D17 / T06

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S02-07-1 | 先读后写 | 先把 v1 的循环、审批、提问逻辑逐段定位并记录行号，再迁移 |
| S02-07-2 | 配对机制 | `oneshot` 按请求 id 配对；UI 未响应时的超时/取消语义与 v1 一致 |
| S02-07-3 | 安全等级 | `ToolSafety` 决定是否需要审批，规则原样保留 |
| S02-07-4 | 测试 | 用 mock provider 返回 tool_call → 审批通过 / 拒绝 两条路径各一测试（可复用 `crates/engine/tests/common/`） |
| S02-07-6 | 持久化归属 | v1 `send_message` 由后端保存新 user 消息、每轮 assistant 消息与 tool result（用 `storage::append_messages` 批量写一轮）；`save_message` 仅供 UI 显式追加单条。迁入 engine 时保持同一归属（原 `storage-design.md`，S02-04 移交） |
| S02-07-5 | **终态事件由编排层发射** | S02-01 实测：provider 事件流以 `TurnEnd` 结束，`Done` / `Error` 由 v1 `commands.rs:642-660` 的 `TerminalStreamEvent` 依 `StreamOutcome` 发射。**必须随工具循环一并迁入 engine**，否则 v2 UI 永远收不到 `Done` |

## 验收标准

- [x] 审批通过与拒绝两条路径测试通过
- [x] 提问（user-input）往返测试通过
- [x] v1 对应逻辑的行号映射表写入证据
- [x] 分层检查 14 项通过；v1 `cargo check` 通过
- [x] （范围扩大后追加）终态 `Done`/`Error` 由 engine 发射且恰好一个、位于最后；取消 / 并发 / 未知模型路径有测试

## 证据

| 项 | 证据 |
|----|------|
| v1 → engine 映射 | 见下表（按函数名在两文件定位的起始行） |
| 改动范围 | `diff -w` v1 与 chat.rs：删除侧除注释外全部为 Tauri 行（`use tauri`、`app: AppHandle`、`State<…>`、`#[tauri::command]`、`app.path()…`、`app.state::<…>()`）及 rustfmt 因缩进加深的重折行；逻辑分支、常量（20 轮上限 / 连续 3 轮失败 / 70% 预算 / 图片上限）未改 |
| 单测 | v1 `commands::` 19 个 → engine `chat::` **18 passed**；差 1 = `frontend_diagnostic_accepts_only_known_stages`（窗口诊断，归 Phase 07） |
| 端到端 | `tests/chat_flow.rs` **8 passed**：纯回复（Done + 持久化 User/Assistant）；写工具审批通过（文件写入、第 2 轮请求带 `"role":"tool"` 与 `call_1`、持久化 User/Assistant/Tool/Assistant）；审批拒绝（未执行、`ToolResult{is_error:true,"用户拒绝执行"}` 回传模型）；本次都允许（2 个写调用只弹 1 次审批）；ask_user（选第 2 项 → `User selected: 方案B` 回传）；审批中停止（只 1 次请求、`Error{Aborted}`、迟到审批返回 Err、占用已释放可立即再发）；并发发送（第二次返回 `已有生成任务正在进行中` 且 0 事件）；未知模型（Err、0 事件、0 落盘）。所有用例断言终态事件恰好 1 个且在最后 |
| 测试有效性 | 变异注入：① 拒绝后仍执行 → 4 个用例失败；② 不发终态事件 → 7 个用例失败；恢复后 8/8 通过 |
| 稳定性 | 首版 10 次中 3 次失败：本机有进程探测本地端口（抓到 `HEAD / HTTP/1.1`），占用 mock 应答位。mock 改为只接受 POST 后，**全量测试连跑 30 次 0 失败**（断言未放宽） |
| 回归 | 全量 `cargo test -p buddy-engine` 175 passed / 0 failed；纪律检查 14/14、拦截验证 10/10；`cd src-tauri && cargo check` rc=0 |

| v1 `commands.rs`（`v1-final`）| 行 | engine `chat.rs` 行 |
|---|---|---|
| `run_blocking` / `unique_message_id` / 日志脱敏 4 函数 | 27–101 | 29–103 |
| `CancelState` / `reserve_generation` / `release_generation` | 107–125 | 110–128 |
| `ApprovalSlot` / `ApprovalState` / `QuestionSlot` / `QuestionState` | 145–195 | 148–200 |
| （新增）`ChatEngine` 结构体 + `new` / `data_dir` | — | 202–235 |
| token 估算 / 图片校验 / 生成图片下载与落盘 | 206–449 | 238–476 |
| `save_chat_image` / `delete_chat_image` / `download_generated_image` | 453–516 | 478–540 |
| 滑动窗口 / `build_tool_msg` / `flush_pending_messages` | 519–606 | 544–628 |
| `TerminalStreamEvent` / `terminal_event_for_outcome` / `cancelled_stream_outcome` / `requires_tool_execution` | 608–701 | 630–723 |
| **`send_message`**（工具循环、审批、ask_user、持久化、终态） | 714–1314 | 741–1346 |
| `stop_generation` / `approve_tool_call` | 1317–1364 | 1349–1396 |
| `format_ask_user_answer` / `answer_tool_question` | 1373–1477 | 1405–1512 |
| `get_config` / `save_config` / `fetch_models` / `test_latency` / `load_messages` / `get_message_count` / `save_message` | 1483–1578 | 1517–1613 |
| `resize_window_to_page` / 前端诊断 2 函数 | 1582–1622 | **未迁 → Phase 07** |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 范围 | 迁入整个 `send_message` 编排（改名） | S02-01 实测终态事件在编排层；只迁「审批配对」会让 v2 UI 收不到 `Done` |
| API 形态 | `ChatEngine`（`Arc`）+ 方法；`send_message` 接收调用方的 `StreamEventEmitter` | 与 v1 命令一一对应、diff 最小；emitter 由调用方持有接收端，返回时 drop → 接收端知道事件已全部送达 |
| 三个 State | 合并为 `ChatEngine` 字段 | v1 由 Tauri 注入全局单例；v2 由持有 `ChatEngine` 的一方共享，语义相同 |
| `save_config` 的热键注册 | 不在 engine | 热键属于应用外壳（Phase 07）；方法注释写明 UI 须先注册热键再保存 |
| 下载目录 | `dirs::download_dir()` | 与 Tauri `download_dir()` 同源 |
| 文件组织 | 单文件 `chat.rs`、保留 v1 函数顺序 | 便于逐段对照 v1；拆分留到 UI 接入后按实际调用方式再定 |

## 完成记录

- 日期：2026-09-27
- commit：`039ed86`
- 设计文档处置：`sse-and-api.md` 编排段落已实现 → 文件头标注更新；剩 UI 消费（S02-06），届时整份删除
