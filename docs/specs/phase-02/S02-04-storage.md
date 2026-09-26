# S02-04 storage 移植与数据目录

> 状态: `done`
> Phase: 02
> 依赖: S02-03
> 阻塞: —
> 退役设计文档: `docs/design/storage-design.md`

## 目标

把 v1 的 `storage.rs` + `storage/`（722 行）迁入 `buddy-engine`，去掉 `tauri::AppHandle`，改为显式传入数据根目录；默认根目录与 v1 相同。

## 输入

- 移植源：tag `v1-final` 的 `src-tauri/src/storage.rs`、`storage/{atomic_file,attachment_files,config_files}.rs`
- v1 `src-tauri/tauri.conf.json:5` `identifier = com.buddy.chat`
- `docs/evidence/s00-08/engine-integration.md` §5

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S02-04-1 | 去 Tauri | `storage.rs` 12 处 `AppHandle` 参数 → 数据根目录（`&Path` 或 `Storage` 结构体持有） |
| S02-04-2 | 默认目录 | macOS `~/Library/Application Support/com.buddy.chat`，Windows `%APPDATA%\com.buddy.chat`（与 Tauri `app_data_dir` 一致） |
| S02-04-3 | 原子写 / 附件 / 分文件配置 | 原样移植（D19-D21） |
| S02-04-4 | 不做历史迁移 | 用户决策（2026-09-26）：开发阶段，不做旧数据回归（原 D22 取消） |

## 验收标准

- [x] storage 随迁单测全部通过，数量与 v1 一致
- [x] 往返测试：临时目录内 写配置/消息 → 读回 → 相等（T04）
- [x] 默认目录解析结果在本机打印为 `~/Library/Application Support/com.buddy.chat`
- [x] 分层检查 14 项通过；v1 `cargo check` 通过
- [x] `storage-design.md` 已处置并登记；数据目录的兼容性理由已迁到代码注释

## 证据

| 项 | 证据 |
|----|------|
| 源码差异 | `storage/` 3 个文件与 `v1-final` 逐字节一致；`storage.rs` diff 63 行 = 删 `use tauri::Manager` 与两个 Tauri 私有函数、9 处 `app: &tauri::AppHandle` → `data_dir: &Path` 及对应调用、新增 `APP_IDENTIFIER` / `default_data_dir()` / 2 段 why 注释。`grep -n '\bapp\b\|tauri' storage.rs` → 0 |
| 单测 | `--list`：engine `storage::` 11 个，v1 `storage::` 11 个 → 一致 |
| 往返 | `tests/storage_roundtrip.rs` **4 passed**：`config_roundtrip`（缺失→默认；改 hotkey/auto_start 后写读一致）；`messages_roundtrip_across_chunks`（1 单条 + 249 批量 = 250 条 → `chunk_001..003` + manifest，无 `chunk_004`；`load_messages(150,30)` = `m0150..m0179`；全量 250 条与写入逐条 JSON 相等）；`attachments_stay_inside_data_dir`（附件落在 `attachments/`、`data_url` 为空、删除生效、目录外路径删除返回 Err 且文件保留）；`default_data_dir_matches_v1` |
| 默认目录 | `default_data_dir = /Users/gongshaojie/Library/Application Support/com.buddy.chat`（`--nocapture` 输出）。算法与 `tauri-2.11.3/src/path/desktop.rs:247` `app_data_dir()` = `dirs::data_dir().join(identifier)` 相同 |
| 依赖 | 新增 `dirs 6.0.0`（MIT/Apache）→ `dirs-sys 0.5.0`（MIT/Apache）、`option-ext 0.2.0`（**MPL-2.0**，已登记于 `THIRD_PARTY_NOTICES.md` §4，未修改）。engine 依赖 147 → 150，禁用项 0 |
| 回归 | 全量 `cargo test -p buddy-engine`：lib 139 passed / 8 ignored + mock_sse 2 + storage_roundtrip 4；纪律检查 14/14、拦截验证 10/10；`cd src-tauri && cargo check` rc=0 |
| 设计文档处置 | `git rm docs/design/storage-design.md`；台账「退役记录」逐条列出 why 去向；AGENTS.md 索引行已删 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 历史数据回归 | 不做 | 用户：「当前还在开发阶段，不用考虑历史数据迁移」 |
| 默认目录 | 仍与 v1 相同 | 零成本；且两版并存期间可共用配置 |
| 目录 API | 显式 `data_dir: &Path` 参数，而非 `Storage` 结构体 | 与 v1 函数形状一一对应，diff 最小；是否封装留给 S02-07 编排层按调用方式决定 |
| 目录算法 | `dirs` crate（与 Tauri 同源） | 手写各平台路径可能与 Tauri 在边角（如 `XDG_DATA_HOME`）不一致；`dirs 6` 已在锁文件中 |

## 完成记录

- 日期：2026-09-26
- commit：`66a6314`
- 设计文档处置：`docs/design/storage-design.md` **整份删除**，已登记台账

## 备注

⚠️ **v1 与 v2 同时运行时的写冲突**：两版共用数据目录，但 `APPEND_LOCK` 只在进程内有效、无跨进程文件锁。两边同时追加消息会互相覆盖分块而丢消息。开发期间避免同时用两个版本聊天；若将来需要并存，再加文件锁（记录于 `storage.rs` `APP_IDENTIFIER` 注释）
