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
| `docs/design/pages-and-states.md`（已部分退役，剩余：系统菜单栏设置入口与 Global Interactions S07-*） | S07-*, S10-03 | 剩余段落随各 spec 实现后删除 |
| `docs/design/component-mapping.md`（已部分退役，剩余：窗口容器 S07-*） | S07-* | 剩余段落随实现删除 |
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

## 已部分删减的文档

文件仍在，只删除了已实现的段落（文件头标注了已实现 / 未实现）。整份删除时再移入下面的退役记录段。

| 日期 | 文件 / 段落 | 由哪些 spec 替代 | why 迁移去向 |
|------|------------|-----------------|-------------|
| 2026-10-02 | `docs/design/component-mapping.md`（**部分**：Settings Composition、ThemeSetting / HotkeySetting / HotkeyRecorder / KbdRow 角色与文件映射）；`docs/design/pages-and-states.md`（**部分**：剩余设置子项、config store 保存说明与 `html.dark` 主题切换；保留系统菜单栏入口与窗口交互） | S06-05（`crates/ui/src/settings/hotkey.rs` / `hotkey_state.rs`）、S06-06（`crates/ui/src/settings/theme_control.rs`、`crates/ui/src/chat/router_preferences.rs`） | 热键平台展示、KeyUp 提交、修饰键快照及 Phase 07 注册边界 → S06-05 决策记录；仅浅 / 深、先写盘后发布与新 Router 从配置恢复 → S06-06 决策记录；共享配置队列在前项结束后读成功基底 → `router_preferences.rs` 注释。原生控件保存禁用 / 就地错误 / 聚焦提示差异 → 对应 spec 决策记录 |
| 2026-09-29 | `docs/design/pages-and-states.md`（**部分**：State vs Component、EmptyPage、NoApiKeyPage、State Flow、Known Implementation Notes、模型下拉、ChatPage 审批 / 提问；保留 SettingsPage、Global Interactions） | S05-13（提问与审批 `ask_card.rs` / `approval_panel.rs`）、S05-15（模型下拉 `model_menu.rs`）、S05-16（空态 / 无 Key 页）、S05-18（状态机 `crates/ui/src/chat/page_state.rs`、路由器 `router.rs`） | 设置叠加层不卸载底层页、添加 Provider 中间态不得打断设置流程、窗口仅在离开紧凑页时展开一次（硬约束 6 的准确含义）→ `page_state.rs` 模块文档与 `expands_window`；模型菜单独立于窗口绘制、紧凑窗口不扩高 → `model_menu.rs` 模块文档；审批的 Esc 拒绝 / 无论结果都关闭浮层 / 「本次都允许」语义 → `approval_panel.rs` 模块文档，提问的配对与回答载荷 → `ask_card.rs` 模块文档；启动总是空态 → 同文件与 spec 决策记录；`add-provider` 遗留类型值不迁移。**文档与代码不符**：EmptyPage 段写的紧凑窗口 460×78 已过时（v1 实为 560×60，`tauri.conf.json` / `geometry.rs`）；「`setPage` 会在 compact → content 时主动 resize，与硬约束冲突」实为硬约束的准确含义，非冲突 |

| 2026-10-01 | `docs/design/component-mapping.md`（**部分**：聊天 / 空态 / 无 Key 页面组合、聊天组件角色与文件树；保留设置与窗口外壳） | S05-01 至 S05-18；S05-08 声明的 Phase 05 部分退役 | 虚拟列表与行拆分 → `transcript.rs` / `rows.rs` 模块文档；共用输入草稿 → S05-07 决策记录；生图 / 搜索专用分派 → S05-11 / S05-12 决策记录；审批 / 提问与状态页组合 → S05-13 / S05-18 决策记录。原文「毛玻璃容器」已过时，剩余窗口容器按 `AGENTS.md` 实色决策。 |

| 2026-10-01 | `docs/design/pages-and-states.md` SettingsPage 的 overlay 骨架句；`docs/design/component-mapping.md` 外层设置组合与 SlideInPanel 角色（**部分**，设置子项 / 添加 Provider / 窗口外壳保留） | S06-01 | 叠加设置不卸载底层草稿 / 流式任务、切页不改变窗口尺寸、退出动画即释放输入 → `settings/view.rs` / `settings/panel.rs` 模块注释及 S06-01 决策记录。 |

| 2026-10-01 | `docs/design/pages-and-states.md` 添加 Provider 流程；`docs/design/component-mapping.md` AddProviderPanel / ProviderCard / FooterActions 的组合、角色与文件树（**部分**，其余设置子项与窗口外壳保留） | S06-02 | 当前 v1 已单次写入完整配置，原设计文档「分步保存」过时；先写盘后发布、保存失败保留表单、同一配置队列串行、原始模型 ID 与 Provider 作用域隔离 → `router_settings.rs` / `provider_merge.rs` 模块文档及 S06-02 决策记录。 |

| 2026-10-02 | `docs/design/component-mapping.md` 的 ModelList / ModelRow / StatusDot 组合、角色和文件树；`docs/design/pages-and-states.md` 剩余模型列表句（**部分**，主题 / 快捷键 / 窗口段落保留） | S06-03 | 完整作用域 ID、默认回退、精确上下文值与协议能力限制 → `model_config.rs` 及 S06-03 决策记录；串行写入成功基底与菜单即时反馈隔离 → `router_config_save.rs` 模块文档与 S06-03 决策记录；失败回退受控下拉与焦点保持 → `model_list/`。当前 v1 无删除入口，未接入的删除 store 方法不迁移。 |

## 退役记录

> 每次删除后追加。**必须同时更新 `AGENTS.md` 的 Document Index 表。**

| 日期 | 文件/段落 | 替代它的 spec | why 迁移去向 |
|------|----------|-------------|-------------|
| 2026-09-10 | `.design/`（整目录，7 文件） | 无（冗余产物） | 无需迁移：动效令牌与 `src/styles/global.css` 完全重复；用户明确「代码实现就是前端设计」 |

| 2026-09-26 | `docs/design/rust-data-models.md`（整份） | S02-03, S02-02, S02-01（代码在 `crates/engine/src/models/`、`streaming.rs`） | 字段语义已在代码注释：模型 ID 规则与「不按 `::` 猜测」→ `models/model_identity.rs:4,12-13,20`；`ImageAttachment.path` 持久化 / `data_url` 临时 → `models/message.rs` 结构体注释；`parent_message_id` 仅影响嵌套显示 → `message.rs` 字段注释；时间戳为 Unix 秒 → `created_at` 注释；`QuestionOption` camelCase → `streaming.rs:245`；**ID 并非全为 UUID → 本次新增于 `Message.id`**；未知模型上下文回退 128000 → 测试 `test_unknown_model_defaults_to_128k` 锁定。「保存时 `selected_model_id` 必须能在 `models` 中找到」的校验在 v1 `commands.rs`，移交 S02-08 覆盖表。前端 `supports_long_cache_retention` 为 v1 TS 专属，不迁 |

| 2026-09-26 | `docs/design/storage-design.md`（整份） | S02-04（`crates/engine/src/storage.rs` + `storage/`） | 数据目录兼容性 → `storage.rs` `APP_IDENTIFIER` 注释（「不要改」）+ `default_data_dir()` 与 Tauri `app_data_dir()` 同算法；**无跨进程锁、v1/v2 同时写会丢消息** → `APP_IDENTIFIER` 注释；附件被外部删除后保留原路径、UI 显示「图片已删除」→ `store_image_bytes` 注释；持久化归属（`send_message` 由后端写一轮消息）→ S02-07-6；分块 100 条 → 测试 `chunk_rotation_at_100`；损坏分块/manifest 回退为空 → 代码 `warn!` 与注释。明文 `api_key` → AGENTS.md 硬约束 9。「非原子写」「前端只加载首批 100 条」为过时 / v1 前端事实，不迁 |

| 2026-09-27 | `docs/design/ipc-contract.md`（整份） | S02-08（v2 无 IPC；v1 命令 → `chat::ChatEngine` 方法，覆盖表见 `phase-02/S02-08-ipc-retire.md`） | 能力整体移除，逐条核对：camelCase↔snake_case 映射为 Tauri 专属，作废；`save_config` 重注册热键 → `ChatEngine::save_config` 注释（热键归 Phase 07）；模型 ID 规则 → `models/model_identity.rs`；`send_message` 持久化归属 → S02-07-6；`approveAll` 只在当次有效 → `chat.rs` 在开始/结束时重置 `approve_all_for_turn`；隐藏窗口不取消 → AGENTS.md 硬约束 7；StreamEvent 联合类型 → `streaming.rs` `StreamEvent`（代码即契约）。**文档与代码不符之处**：称「Rust 当前没有发射 `thinking_end`」，实际 `streaming.rs` `thinking_end()` 会发；命令表漏列 `get_message_count` 与 3 个图片命令 |

| 2026-09-27 | `docs/design/sse-and-api.md`（整份） | S02-01（Provider / Compatibility）、S02-05（Provider 层取消）、S02-07（编排：`crates/engine/src/chat.rs`）、S02-06（UI 消费：`crates/ui/src/chat_bridge.rs`） | 工具循环上限 20 / 连续 3 轮失败、上下文 70% 预算、图片只随当前提问与工具轮携带 → `chat.rs` 常量与注释（v1 原样）；终态事件晚于持久化与释放占用 → `chat.rs` 注释 + `streaming.rs` `StreamFailure` 注释；Esc/失焦/托盘隐藏不取消 → AGENTS.md 硬约束 7 + `chat_bridge.rs` 模块文档（禁止 `Tokio::spawn` 的理由）；生图工具只对 openai_compatible 注册 → `chat.rs` 注释；未知模型 128000 → 测试锁定。v1 前端「文本增量 rAF 平滑消费」属渲染细节 → 交 S04-06（流式渐显）。**文档与代码不符**：称「当前无 system prompt」，实际 `providers/mod.rs` `BUDDY_SYSTEM_PROMPT` 每次注入 |

| 2026-09-27 | `docs/design/design-tokens.md`（整份） | S03-02（颜色）、S03-03（圆角 / 阴影）、S03-04（排版 / 间距 / 动效）→ `crates/ui/src/theme_system/`（`tokens.rs` 由 `global.css` 生成，WebKit 全量验证） | 品牌色唯一、圆角刻度 → AGENTS.md 硬约束 2、3（生成器另行断言）；「禁止硬编码」→ 硬约束 5（S03-07 加脚本守卫）；阴影用途表 → `theme_system/mod.rs` 模块文档；排版角色 → `theme_system::typography`（以 v1 `.t-*` 实际定义为准）。**文档与代码不符之处**：表面色 8 处（半透明 rgba vs 实色）；列出的 `t-display` / `t-overline` 在代码中从未存在，`body` 字号写 14 而 `.t-body-sm` 实为 13；「红色警告底色 `rgba(220,38,38,0.12)` 例外」在 v1 代码中 0 处使用。S01-06 的 `extract_tokens.py` 改为读 git 历史版本（`e91bbc3:`），基线仍可复现 |

> **注**：`.design/` 未被 git 跟踪，删除不产生 git 记录，仅在此登记以保留删除事实。
