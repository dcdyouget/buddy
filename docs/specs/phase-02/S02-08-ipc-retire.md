# S02-08 IPC 层作废（命令 → engine API 覆盖表）

> 状态: `done`
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
| S02-08-4 | 配置保存校验 | 「保存时 `selected_model_id` 非空则必须能在 `models` 中找到」在 v1 `commands.rs`（`model_identity.rs:82` 注释称「交给命令层校验」），覆盖表须标出其 engine 去向（原 `rust-data-models.md` 的约定，S02-01 移交） |

## 验收标准

- [x] 覆盖表完整：v1 命令数 = 表中行数（`grep -c '#\[tauri::command\]'` 对照）
- [x] 表中每个 engine 入口可编译（`cargo check`）
- [x] `ipc-contract.md` 已处置并登记台账

## 证据

| 项 | 证据 |
|----|------|
| 覆盖表 | 见下。v1 `git grep -c '#\[tauri::command\]'` = 17，其中 `commands.rs:3` 为注释，实际 16 个，与 `lib.rs:126-143` 注册数一致；表中 16 行（14 → engine、2 → Phase 07） |
| 编译检查 | `tests/api_surface.rs` 的 `_every_v1_command_has_an_engine_entry` 引用全部 14 个 engine 入口并标注类型，改名/改签名即编译失败；`cargo test -p buddy-engine --test api_surface` 1 passed |
| 默认模型校验（S02-08-4） | `save_config_rejects_unknown_selected_model`：非空且不存在的 `selected_model_id` → `Err("选择的模型不在可用模型列表中")` 且不写盘；空值 → 写盘成功 |
| 设计文档处置 | `git rm docs/design/ipc-contract.md`；台账逐条列出 why 去向；AGENTS.md 索引行已删。**发现文档与代码不符**：文档称「Rust 当前没有发射 `thinking_end`」，而 `streaming.rs` 有 `thinking_end()` 发射；命令表漏 4 个命令 |
| 提交 | `ff52929` |

### 命令（v1 `lib.rs:126-143` `generate_handler!` 注册 16 个）

| # | v1 命令 | v2 去向 |
|---|---------|---------|
| 1 | `send_message` | `ChatEngine::send_message(emitter, messages, model_id)` |
| 2 | `stop_generation` | `ChatEngine::stop_generation()` |
| 3 | `approve_tool_call` | `ChatEngine::approve_tool_call(id, approved, approve_all)` |
| 4 | `answer_tool_question` | `ChatEngine::answer_tool_question(id, selected, inputs, custom)` |
| 5 | `get_config` | `ChatEngine::get_config()` |
| 6 | `save_config` | `ChatEngine::save_config(config)`（含默认模型校验）；**热键重注册 → S07-03**，UI 须先注册再保存 |
| 7 | `fetch_models` | `ChatEngine::fetch_models(base_url, api_key, provider_type)` |
| 8 | `test_latency` | `ChatEngine::test_latency(base_url, api_key, model_id, provider_type)` |
| 9 | `load_messages` | `ChatEngine::load_messages(offset, limit)` |
| 10 | `get_message_count` | `ChatEngine::get_message_count()` |
| 11 | `save_message` | `ChatEngine::save_message(message)` |
| 12 | `save_chat_image` | `ChatEngine::save_chat_image(name, media_type, data_url)` |
| 13 | `delete_chat_image` | `ChatEngine::delete_chat_image(path)` |
| 14 | `download_generated_image` | `ChatEngine::download_generated_image(image)` |
| 15 | `resize_window_to_page` | **S07-01 / S07-06**（窗口尺寸与定位；硬约束 6「切页不改尺寸」） |
| 16 | `log_window_frontend_diagnostic` | **S07-08**（入场动画诊断；GPUI 同进程可直接打日志） |

### 后端 → 前端事件（v1 `emit`）

| v1 事件 | 发射处 | v2 去向 |
|---------|--------|---------|
| `stream-event`（16 种） | `streaming.rs` `StreamEventEmitter` | `StreamEventEmitter::channel()` 的接收端（S02-03），UI 消费归 S02-06 |
| `buddy:window-will-show` / `buddy:window-will-hide` | `window/mod.rs:66`、`hotkey.rs:50` | S07-03 / S07-08（同进程直接调用，无需事件） |
| `selected-text` | `platform/macos.rs:138` | S07-03（唤起时带入选中文本） |
| `auto-start-changed` | `tray.rs:45` | S07-10 / S07-13 |
| `open-settings` | `tray.rs:104` | S07-09（tray 菜单）→ S06-01 |

### 前端经 Tauri 插件调用的能力

| 插件 | 用途 | v2 去向 |
|------|------|---------|
| `@tauri-apps/plugin-shell` | 打开外部链接 | S04-08（markdown 链接）/ S06-* |
| `@tauri-apps/plugin-updater`、`plugin-process` | 更新与重启 | S08-01 ~ S08-05 |
| Rust 侧 `tauri-plugin-global-shortcut` / `autostart` / `log` | 热键 / 自启 / 日志 | S07-03 / S07-10 / 应用初始化 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 范围 | 原「删除 commands.rs」→「覆盖表 + 作废契约」 | 删除会破坏 v1，而 v1 是 Phase 05 前的唯一可用版本与视觉对照基准 |

## 完成记录

- 日期：2026-09-27
- commit：`ff52929`
- 设计文档处置：`docs/design/ipc-contract.md` **整份删除**，已登记台账
