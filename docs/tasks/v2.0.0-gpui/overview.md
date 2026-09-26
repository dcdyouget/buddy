# v2.0.0-gpui Task Overview

> 目标：将 Buddy 从 Tauri 2 + React 迁移到 GPUI + 纯 Rust
> **本文只记录工作内容与推进顺序，不含时间估算。**
> 证据基础见同目录 `research-log.md`（所有关键结论均来自对 zed / comet 源码的实测）

## 分工：本文 vs `docs/specs/`

| 位置 | 角色 |
|------|------|
| `docs/tasks/v2.0.0-gpui/`（本文） | **背景与拆解依据**：调研证据、Phase 划分、工作项来源。**不承载状态** |
| `docs/specs/` | **执行与状态**：具体 spec、状态、证据、完成记录。**唯一权威状态源** |

→ 执行规则见 `docs/specs/RULES.md`，进度见 `docs/specs/README.md`。
→ 本文的 Phase 划分对应 `docs/specs/` 中的 spec 分组；Phase 文件中的工作项（D/T 编号）是 spec 的素材。

## 决策前提

| 项 | 结论 |
|----|------|
| 许可证 | **接受 GPL-3.0-or-later**。全仓开源无障碍，因此可直接使用 zed 的 `theme` / `ui` / `markdown` |
| 发行方式 | 仅 DMG 直发。接受「永久放弃 Mac App Store」 |
| 平台策略 | **macOS 先行，Windows 第二阶段**。理由见 `research-log.md` 风险登记 |
| 核心动机 | 使用新基础 + 纯原生 Rust，去掉 IPC 层 |

## ASCII / 命名约定

- `buddy-engine` — 引擎层，**零 GPUI 依赖，零 GPL 链接**，可保持 MIT/Apache
- `buddy-ui` — 界面层，唯一依赖 GPUI 的 crate，GPL-3.0-or-later
- `buddy-app` — 可执行入口，组装 engine + ui

> **分层是硬约束。** 唯一正确姿势：GPL 只污染 `buddy-ui`，`buddy-engine` 永久保持可闭源。参见 `research-log.md` §6。

## Phase 列表

| Phase | 名称 | 依赖 | 性质 |
|-------|------|------|------|
| 00 | 可行性 Spike | — | **证伪优先，最高风险先做** |
| 01 | 工程骨架与分层 | 00 | 决定 gpui 依赖来源、许可证、workspace 结构 |
| 02 | 引擎层移植 | 01 | 复用现有 Rust，去掉 Tauri 类型 |
| 03 | 主题与设计令牌 | 01 | global.css → theme.rs |
| 04 | Markdown 栈 | 01 | vendor + patch |
| 05 | 聊天界面 | 03, 04 | 主体工作量 |
| 06 | 设置界面 | 03 | |
| 07 | 应用外壳与窗口行为 | 01 | **第二高风险区（平台相关）** |
| 08 | 更新与发布 | 07 | |
| 09 | 平台对齐（Windows） | 05, 06, 07 | macOS 验收通过后才启动 |
| 10 | 测试与双平台验收 | 02-09 | 与开发并行 |

## Dependency Graph

```
00 Spike（证伪优先）
    │  通过 → 继续；不通过 → 回退 Tauri
    ▼
01 工程骨架与分层
    │
    ├── 02 引擎层移植 ──────────────────┐
    ├── 03 主题与设计令牌 ──┐           │
    ├── 04 Markdown 栈 ─────┤           │
    │                       ▼           │
    │                 05 聊天界面 ──────┤
    │                 06 设置界面 ──────┤
    │                                   ▼
    └── 07 应用外壳与窗口行为 ──► 08 更新与发布
                        │
                        ▼
                  09 平台对齐（Windows）
                        
        02-09 ──► 10 测试与验收（并行）
```

## 推进规则

0. **执行以 `docs/specs/` 为准**：每个 Phase 启动时，从 `docs/specs/README.md` 展开该 Phase 的详细 spec；完成一个 spec 立即更新注册表
1. **Phase 00 不通过就停止**，不做任何后续投入，回退 Tauri 继续演进
2. **Phase 09 在 macOS 验收通过前不启动**，避免双平台并行导致无法回退
3. Phase 00-01 期间保留现有 Tauri 代码于独立目录/分支作为退路，Phase 05 开始后再考虑清理
4. 每个 Phase 完成后更新本文件的状态列

## 文档索引

| 文件 | 内容 |
|------|------|
| `overview.md` | 本文件：决策前提、Phase 列表、推进规则、资产清单 |
| `research-log.md` | **证据基础**：所有实测结论（含文件/行号）、依赖闭包、许可证分层、风险登记 R1-R9 |
| `00-spike.md` | 可行性 Spike，含证伪条件与结论记录表 |
| `01-skeleton.md` | 工程骨架与分层（防 GPL 污染） |
| `02-engine.md` | 引擎层移植（复用现有 Rust） |
| `03-theme.md` | 主题与设计令牌（含平台字体） |
| `04-markdown.md` | Markdown 栈（vendor + patch） |
| `05-chat-ui.md` | 聊天界面（列表虚拟化 + Composer） |
| `06-settings-ui.md` | 设置界面 |
| `07-shell.md` | 应用外壳与窗口行为（含 Tauri 配置映射对照表） |
| `08-release.md` | 更新与发布 |
| `09-windows.md` | 平台对齐（Windows） |
| `10-testing.md` | 测试与双平台验收（含硬约束验证清单） |

## 状态

> **权威状态在 `docs/specs/README.md`**（按 spec 粒度）。下表仅作 Phase 级概览。

| Phase | 状态 | 文件 |
|-------|------|------|
| 00 | 未开始 | `00-spike.md` |
| 01 | 未开始 | `01-skeleton.md` |
| 02 | 未开始 | `02-engine.md` |
| 03 | 未开始 | `03-theme.md` |
| 04 | 未开始 | `04-markdown.md` |
| 05 | 未开始 | `05-chat-ui.md` |
| 06 | 未开始 | `06-settings-ui.md` |
| 07 | 未开始 | `07-shell.md` |
| 08 | 未开始 | `08-release.md` |
| 09 | 未开始 | `09-windows.md` |
| 10 | 未开始 | `10-testing.md` |

> 每个 Phase 启动时把对应文件细化为可执行的任务表；完成时更新此表状态并填写证据。

## 不可丢失的既有资产

迁移中必须保留、不允许推倒重来的部分（详见 `research-log.md` §5 复用清单）：

| 资产 | 位置 | 处理方式 |
|------|------|---------|
| Provider 适配 | `src-tauri/src/providers/` | **原样移植**，0 处 Tauri 引用 |
| 工具调用 | `src-tauri/src/tools/` | **原样移植**，0 处 Tauri 引用 |
| 数据模型 | `src-tauri/src/models/` | **原样移植**，0 处 Tauri 引用 |
| 流式解析 | `src-tauri/src/streaming.rs` | 仅替换事件发射机制 |
| 存储 | `src-tauri/src/storage.rs` | 仅替换路径 API |
| 窗口定位 | `src-tauri/src/window/positioning.rs` | 逻辑保留，改调用 GPUI |
| 设计令牌 | `src/styles/global.css` | **值保留**，仅换编码方式 |
| 原型与设计文档 | `docs/design/` | 作为移植对照基准 |
| 发布流程 | `docs/release-workflow.md` | 复用 OSS 上传链路 |

## 被替换 / 删除的部分

| 现有 | 去向 |
|------|------|
| `src-tauri/src/commands.rs`（1868 行） | **整体删除**。IPC 边界消失，UI 直接调用 engine |
| `docs/design/ipc-contract.md` | 作废 |
| `src/types/index.ts` | 与 Rust models 合并，不再需要 TS 镜像 |
| `src/stores/*` (Zustand) | 改为 GPUI `Entity` + `Context` |
| `framer-motion` | 改为 GPUI `with_animation` / `Animation` |
| `react-markdown` + `remark-gfm` + `prism-react-renderer` | 改为 Phase 04 的 Rust markdown 栈 |
| `lucide-react` | 改为 svg 资源 + `zed ui` 的 `Icon` |
| Tailwind / CSS 变量 | 改为 Phase 03 的 `Theme` |
| 手动分页 + `bottomFollow.ts` | 改为 `ListState(Bottom)` 原生虚拟化 |
