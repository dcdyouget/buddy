# 设计文档退役台账

> 规则见 `docs/specs/RULES.md` §7。
> **本文件是待退役与已退役清单的唯一维护处**（RULES.md 不重复维护，见 `RULES.md` §11.2）。
> **没有在本台账登记的删除视为无效。**

---

## ⚠️ 审计发现：失效引用（已修正）

审计时发现 `docs/design/prototypes/` 与 `docs/design/colors_and_type.css` **从未存在**。这两个路径由 `AGENTS.md` 定义，被 `CLAUDE.md`、`docs/tasks/v1.0.0/02-project-scaffold.md`、`docs/design/design-tokens.md` 沿用，并在本次重构文档中一度被继续引用。

真实情况：

| 曾被引用的路径 | 真实情况 |
|--------------|---------|
| `docs/design/prototypes/` | **不存在**。仓库中没有任何「7 个页面原型」HTML |
| `docs/design/colors_and_type.css` | **不存在** |
| `buddy-design/colors_and_type.css` | **不存在**（`design-tokens.md` 与 `CONVENTIONS.md` 引用此路径） |
| `.design/animation-preview/colors_and_type.css` | 曾存在，**已于 2026-09-10 删除**。Trae 工具生成的动效预览实验，非主项目令牌表，前缀 `--ap-`、主色 `#5B8DEF`（≠ 硬约束 2 的 `#5B5FE9`）。其动效令牌与 `src/styles/global.css` 完全重复且后者更完整，故零迁移成本 |

**结论**：设计令牌的真值来源只有 `docs/design/design-tokens.md`；视觉验收基准改为 **用户对照运行中的 v1 目检**（2026-09-26 用户决策，不采集截图；v1 由 S01-05 退路 tag `v1-final` 保证可运行）。

**另需确立的原则（用户明确要求）**：

> **现在的代码实现就是前端设计。** 不存在独立的「设计稿」产物。

这与 `RULES.md` §7（设计文档退役）同源：设计文档只是脚手架，代码才是资产的最终形态。因此：

- 不为动效 / 视觉 / 交互另立设计产物
- 需要的值从 `src/styles/global.css`（v1）或重构后的 `Theme`（v2）读取
- 动效基准以 `global.css` 的 `--duration-*` / `--ease-*` 令牌为准（全仓 76 处引用）

---

## 待退役清单

| 设计文档 | 退役于 spec | why 迁移要求 |
|---------|------------|-------------|
| `docs/design/rust-architecture.md` | S02-*, S07-01 | 模块职责边界的理由需迁移到 crate 级文档注释 |
| `docs/design/design-tokens.md` | S03-02, S03-03, S03-04, S03-05 | 品牌色 `#5B5FE9` 与圆角刻度 `4/8/12/16/9999` 的约束 —— **已确认由 `AGENTS.md` 硬约束 2、3 覆盖**，无需额外迁移 |
| `docs/design/pages-and-states.md` | S05-18, S10-03 | 状态机的转换条件理由需迁移 |
| `docs/design/component-mapping.md` | S05-*, S06-* | 组件拆分的理由需迁移 |
| `docs/design/overview.md` | 最后 | 架构约束需先迁移到 `AGENTS.md` 后再删 |

> 上表中不存在的路径已被移除（原列有 `docs/design/colors_and_type.css` 与 `docs/design/prototypes/`）。

## 不退役

| 文档 | 理由 |
|------|------|
| `docs/tasks/v1.0.0/` | 历史记录，**不得修改**（`RULES.md` §11.3） |
| `docs/tasks/v2.0.0-gpui/research-log.md` | 证据基础，含不可再生的实测数据与风险登记 |
| `docs/evidence/` | 验收证据（v1 基线等），与 research-log 同级 |
| `docs/specs/` | 执行记录本身 |
| `docs/CONVENTIONS.md` | 持续生效的编码规则 |
| `docs/release-workflow.md` | 持续生效的操作手册 |
| `AGENTS.md` | 项目权威入口 |
| `docs/design/design-tokens.md` | 设计令牌唯一真值（辅以 `src/styles/global.css`） |

## 退役记录

> 每次删除后追加。**必须同时更新 `AGENTS.md` 的 Document Index 表。**

| 日期 | 文件/段落 | 替代它的 spec | why 迁移去向 |
|------|----------|-------------|-------------|
| 2026-09-10 | `.design/`（整目录，7 文件） | 无（冗余产物） | 无需迁移：动效令牌与 `src/styles/global.css` 完全重复；用户明确「代码实现就是前端设计」 |

| 2026-09-26 | `docs/design/rust-data-models.md`（整份） | S02-03, S02-02, S02-01（代码在 `crates/engine/src/models/`、`streaming.rs`） | 字段语义已在代码注释：模型 ID 规则与「不按 `::` 猜测」→ `models/model_identity.rs:4,12-13,20`；`ImageAttachment.path` 持久化 / `data_url` 临时 → `models/message.rs` 结构体注释；`parent_message_id` 仅影响嵌套显示 → `message.rs` 字段注释；时间戳为 Unix 秒 → `created_at` 注释；`QuestionOption` camelCase → `streaming.rs:245`；**ID 并非全为 UUID → 本次新增于 `Message.id`**；未知模型上下文回退 128000 → 测试 `test_unknown_model_defaults_to_128k` 锁定。「保存时 `selected_model_id` 必须能在 `models` 中找到」的校验在 v1 `commands.rs`，移交 S02-08 覆盖表。前端 `supports_long_cache_retention` 为 v1 TS 专属，不迁 |

| 2026-09-26 | `docs/design/storage-design.md`（整份） | S02-04（`crates/engine/src/storage.rs` + `storage/`） | 数据目录兼容性 → `storage.rs` `APP_IDENTIFIER` 注释（「不要改」）+ `default_data_dir()` 与 Tauri `app_data_dir()` 同算法；**无跨进程锁、v1/v2 同时写会丢消息** → `APP_IDENTIFIER` 注释；附件被外部删除后保留原路径、UI 显示「图片已删除」→ `store_image_bytes` 注释；持久化归属（`send_message` 由后端写一轮消息）→ S02-07-6；分块 100 条 → 测试 `chunk_rotation_at_100`；损坏分块/manifest 回退为空 → 代码 `warn!` 与注释。明文 `api_key` → AGENTS.md 硬约束 9。「非原子写」「前端只加载首批 100 条」为过时 / v1 前端事实，不迁 |

| 2026-09-27 | `docs/design/ipc-contract.md`（整份） | S02-08（v2 无 IPC；v1 命令 → `chat::ChatEngine` 方法，覆盖表见 `phase-02/S02-08-ipc-retire.md`） | 能力整体移除，逐条核对：camelCase↔snake_case 映射为 Tauri 专属，作废；`save_config` 重注册热键 → `ChatEngine::save_config` 注释（热键归 Phase 07）；模型 ID 规则 → `models/model_identity.rs`；`send_message` 持久化归属 → S02-07-6；`approveAll` 只在当次有效 → `chat.rs` 在开始/结束时重置 `approve_all_for_turn`；隐藏窗口不取消 → AGENTS.md 硬约束 7；StreamEvent 联合类型 → `streaming.rs` `StreamEvent`（代码即契约）。**文档与代码不符之处**：称「Rust 当前没有发射 `thinking_end`」，实际 `streaming.rs` `thinking_end()` 会发；命令表漏列 `get_message_count` 与 3 个图片命令 |

| 2026-09-27 | `docs/design/sse-and-api.md`（整份） | S02-01（Provider / Compatibility）、S02-05（Provider 层取消）、S02-07（编排：`crates/engine/src/chat.rs`）、S02-06（UI 消费：`crates/ui/src/chat_bridge.rs`） | 工具循环上限 20 / 连续 3 轮失败、上下文 70% 预算、图片只随当前提问与工具轮携带 → `chat.rs` 常量与注释（v1 原样）；终态事件晚于持久化与释放占用 → `chat.rs` 注释 + `streaming.rs` `StreamFailure` 注释；Esc/失焦/托盘隐藏不取消 → AGENTS.md 硬约束 7 + `chat_bridge.rs` 模块文档（禁止 `Tokio::spawn` 的理由）；生图工具只对 openai_compatible 注册 → `chat.rs` 注释；未知模型 128000 → 测试锁定。v1 前端「文本增量 rAF 平滑消费」属渲染细节 → 交 S04-06（流式渐显）。**文档与代码不符**：称「当前无 system prompt」，实际 `providers/mod.rs` `BUDDY_SYSTEM_PROMPT` 每次注入 |

> **注**：`.design/` 未被 git 跟踪，删除不产生 git 记录，仅在此登记以保留删除事实。
