# Phase 02: 引擎层移植

## 目标

把现有 Rust 后端（约 10k 行，其中约 8800 行零 Tauri 引用）移入 `buddy-engine`，彻底切断与 Tauri 的耦合。

**这是整个迁移中复用率最高、风险最低的部分。** 目标是把 `commands.rs` 那 1868 行 IPC 胶水整体删除，让 UI 直接调用 engine。

## 相关文档

- `docs/design/rust-architecture.md` — 现有模块职责
- `docs/design/rust-data-models.md` — 结构体定义
- `docs/design/sse-and-api.md` — 流式与 API 约定
- `docs/design/storage-design.md` — JSON 存储布局与分块机制
- `docs/tasks/v2.0.0-gpui/research-log.md` §5 — 各模块 Tauri 引用实测数据

## 验收标准

- [ ] `cargo tree -p buddy-engine` 不含 `tauri*` 与 `gpui*`
- [ ] 现有 Rust 单测全部通过（如 `tools/file_tools/tests.rs`）
- [ ] SSE 流式在无 Tauri 环境下可独立跑通（可用二进制或测试驱动）
- [ ] `src-tauri/src/commands.rs` 已删除，无任何功能缺失
- [ ] 存储层能读写与旧版本兼容的 JSON 文件（不破坏用户已有数据）

## 模块处理清单

| 模块 | 现有行数 | Tauri 引用 | 处理方式 |
|------|---------|-----------|---------|
| `providers/openai_compatible.rs` | 1434 | 0 | 原样移植 |
| `providers/anthropic.rs` | 1248 | 0 | 原样移植 |
| `providers/mod.rs` | 582 | 0 | 原样移植（含 provider 注册/能力表） |
| `tools/image_generation.rs` | 1080 | 0 | 原样移植 |
| `tools/builtin.rs` | 912 | 0 | 原样移植 |
| `tools/websearch/*` | ~2400 | 0 | 原样移植（bing / duckduckgo / so360 / web_fetch / aggregate / relevance） |
| `tools/file_tools/*` | ~740 | 0 | 原样移植（含 tests.rs 171 行） |
| `models/*` | ~1000 | 0 | 原样移植 |
| `streaming.rs` | 808 | 3 | 改事件发射机制 |
| `storage.rs` | 427 | 12 | 去 Tauri 路径 API |
| `storage/*` | ~290 | — | 原子写、附件文件、配置分文件 |
| `window/positioning.rs` | 510 | 17 | 归 Phase 07，逻辑保留 |
| `commands.rs` | 1868 | 33 | **删除** |
| `hotkey.rs` / `tray.rs` / `lib.rs` | 467 | 26 | 归 Phase 07 重写 |

## 开发工作

### 2.1 模块搬迁

| ID | Task | Details |
|----|------|---------|
| D01 | 建 crate 骨架 | `crates/engine/src/{lib,providers,tools,models,storage,streaming}.rs` |
| D02 | 搬 providers | 三个文件整体复制；确认无类型改动 |
| D03 | 搬 tools | 六个子模块整体复制；保留 `#\[cfg(test)\]` 测试 |
| D04 | 搬 models | 整体复制，核对 `docs/design/rust-data-models.md` |
| D05 | 搬 storage | 去掉 Tauri 的 `app_data_dir()` 等 API，改为显式传入数据根目录 |
| D06 | 搬 streaming | 见 2.2 |

### 2.2 流式机制改造（核心）

现状：`streaming.rs` 通过 Tauri event 把 token 推给前端。

| ID | Task | Details |
|----|------|---------|
| D07 | 定义回调/通道契约 | 改为 `Engine` 暴露事件流：`tokio::sync::mpsc` 或 `async_channel`；UI 侧 `cx.spawn` 消费 |
| D08 | 去掉 3 处 Tauri 引用 | 逐处替换为通道发送 |
| D09 | 保留取消语义 | 现有 stop 生成能力（`stopGeneration`）用 `CancellationToken` 或 drop `Task` 实现 |
| D10 | 保留 usage / token 计数 | 现有 token 计数逻辑不得丢失 |
| D11 | 反压处理 | 确认 GPUI foreground executor 下的消费不会阻塞渲染（对齐 Comet 的 coalescing 思路） |
| D12 | 多 provider 流式协议 | 确认 openai_compatible 与 anthropic 的 SSE 差异在 engine 内消化，UI 不感知 |

### 2.3 工具调用与审批

| ID | Task | Details |
|----|------|---------|
| D13 | tool calling 主循环 | 现有 `tools/mod.rs`（309 行）的调度逻辑保留 |
| D14 | 审批流 | 现有 `ApprovalModal.tsx` 对应的后端审批请求 → 改为 engine 发起、UI 响应的请求/响应配对 |
| D15 | 提问流 | `AskUserCard.tsx` / `QuestionPrompt.tsx` 对应的 user-input 请求同上 |
| D16 | MCP 配置模型 | `models/mcp.rs`（162 行）+ `mcp/mod.rs`（39 行）保留 |
| D17 | 文件工具权限 | 现有 `file_tools` 的路径校验逻辑保留，不得放宽 |

### 2.4 存储兼容

| ID | Task | Details |
|----|------|---------|
| D18 | 数据目录定位 | 新实现需指向与旧版本相同的路径，保证用户升级后数据可读 |
| D19 | 分块机制 | `docs/design/storage-design.md` 的消息分块机制保留 |
| D20 | 原子写 | `storage/atomic_file.rs`（83 行）整体移植 |
| D21 | 附件文件 | `storage/attachment_files.rs`（115 行）移植；注意现有 Tauri asset protocol scope `$APPDATA/attachments/**` 需要新的图片加载路径 |
| D22 | 旧数据回归 | 用真实旧数据文件验证可读 |

### 2.5 删除 IPC 层

| ID | Task | Details |
|----|------|---------|
| D23 | 删除 `commands.rs` | 逐函数确认功能已由 engine 直接暴露后删除 |
| D24 | 作废 `ipc-contract.md` | 在文档中标注作废，保留历史 |
| D25 | 清理 serde 边界类型 | 原先仅为 IPC 存在的 DTO 若与 model 重复则合并 |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T01 | 单测迁移 | `tools/file_tools/tests.rs` 等现有测试移到新 crate 并跑通 |
| T02 | 流式集成测试 | 无 Tauri 环境下驱动一次完整 SSE 流，断言 token 序列与结束事件 |
| T03 | 取消测试 | 流式中途取消，断言无残留任务与无资源泄漏 |
| T04 | 存储往返测试 | 写入 → 读取 → 与旧格式逐字节比对 |
| T05 | provider 契约测试 | 两个 provider 各跑一次真实请求（需 API Key，标记为手动/忽略） |
| T06 | 工具调用测试 | 每种 builtin 工具的成功与失败路径 |
| T07 | 分层断言 | `cargo tree` 无 tauri / 无 gpui |

## 备注

本 Phase 不需要 GPUI 知识，可以完全独立推进。建议在 Phase 07（窗口外壳）攻关时并行做，避免阻塞。
