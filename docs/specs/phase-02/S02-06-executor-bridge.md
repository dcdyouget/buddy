# S02-06 tokio / GPUI 执行器桥接

> 状态: `done`
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

- [x] `buddy-app` 下一个无界面（或最小界面）驱动程序：对 mock SSE 完成一次对话，记录首个事件到达前台的延迟
- [x] 取消经桥接层生效（测试或日志证据）
- [x] engine 依赖树仍 0 处 GPUI；v1 `cargo check` 通过

## 证据

| 项 | 证据 |
|----|------|
| 位置 | 桥接放在 `crates/ui/src/chat_bridge.rs`（而非 app）：app 按分层规则不直接依赖 zed crate，`gpui_tokio` 只在 ui 层出现 |
| 驱动程序 | `cargo run -p buddy-app --example chat_bridge -- --mock`：开真实 GPUI 窗口、经桥接驱动 `ChatEngine`、对本地 mock SSE（20 段 × 20ms）跑 4 个场景，打印 `RESULT: PASS`，exit 0（连续 3 次） |
| 延迟 | 场景 1：首批事件到达前台 **12–16 ms**（发送 → 首次 `on_batch`，4 次运行）；`rx.recv().await` 驱动，无轮询 |
| 批量合并 | 25 事件 → 23–25 次交付（mock 事件每 20ms 一段，本就稀疏；逻辑为「醒来后取光已到达事件、只回调一次」） |
| 关窗不断流 | 场景 2：首批事件到达后 `window.remove_window()` 销毁视图 → 454 ms 内 user + assistant 已落盘 |
| 占用释放 | 场景 3：关窗后立即新开窗口再发，`Ok` 且终态 `Done` |
| 取消 | 场景 4：生成中 `stop_generation()` → 前台收到 `Error(Aborted): 用户取消` |
| 陷阱 ① 反证 | 把桥接改回 `gpui_tokio::Tokio::spawn`（S00-08 骨架写法）：场景 2 **FAIL**（5005 ms 未落盘）、场景 3 **FAIL**（占用卡死）、场景 4 FAIL，`RESULT: FAIL` exit 1；恢复后 PASS。源码依据：`gpui_tokio.rs` `Tokio::spawn` 内 `defer(abort)` |
| 陷阱 ② | 在 GPUI 前台直接 `await engine.get_message_count()` → panic `there is no reactor running, must be called from the context of a Tokio 1.x runtime`（`chat.rs:34` `run_blocking`）；新增 `spawn_engine` 后通过 |
| 分层 / 回归 | `cargo check --workspace` 0 warning；纪律检查 14/14、拦截验证 10/10；`Cargo.lock` 仅新增 ui→tokio 与 app 开发依赖（`buddy-engine` 条目 0 处 gpui）；`cd src-tauri && cargo check` rc=0 |
| 待用户目检 | 手动模式 `cargo run -p buddy-app --example chat_bridge`（真实配置的沙盒副本）→ 列入 Phase 02 确认清单 |
| 提交 | `b1aaab3` |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 不用 `Tokio::spawn` | `Tokio::handle(cx).spawn` 分离式 | 见陷阱 ①；否则违反硬约束 7 且卡死生成占用 |
| engine async 方法入口 | 统一 `spawn_engine` | 见陷阱 ②；同时保证 UI 丢弃 Task 不截断写操作 |
| 合并策略 | 醒来后 `try_recv` 取光再回调一次 | 最简单的背压：不引入定时器；真正的 60fps 节流留给 S04-06 / S05-08 按渲染成本决定 |
| 手动模式数据目录 | 复制 `config.json` 到 `target/buddy-dev-data/` | 不向 v1 真实历史写入演示消息；`target/` 已被 gitignore |

## 完成记录

- 日期：2026-09-27
- commit：`b1aaab3`
- 设计文档处置：`docs/design/sse-and-api.md` **整份删除**（本 spec 为最后负责方），已登记台账
