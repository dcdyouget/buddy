# Buddy — Project Instructions

> **本文件是项目的唯一权威入口**，适用于所有 AI agent（Codex / Claude Code / Cursor / Copilot / Gemini / Windsurf / Cline / Zed 等）。
> 其他工具的同名入口文件（`CLAUDE.md`、`GEMINI.md`、`.cursor/rules/` 等）均为**指向本文件的薄指针**，不含独立内容。

## Project Overview

Buddy is a cross-platform (macOS / Windows) AI chat tool. Press a global hotkey → a lightweight frameless window pops up → chat with AI → click away to dismiss. Think Bob (the macOS translate app) but for AI chat.

- **Tech Stack**: Tauri 2 + React 18 + TypeScript + Tailwind CSS v4 + Framer Motion + Zustand
- **Package Target**: `< 10 MB`
- **Design Philosophy**: Zero-chrome frameless window（不透明实色面板 + 16px 圆角）, calm indigo-violet brand

> **不再要求毛玻璃 / 半透明**（2026-09-10 产品决策）。v1 本就是实色界面（`src-tauri/src/platform/macos.rs:154`：「不启用原生 vibrancy」），保持一致可降低迁移风险。
> 详见 `docs/specs/phase-00/S00-04-backdrop.md`。

## 如何开始工作（Agent 工作流）

### 第一步：必读（所有工作）

| 顺序 | 文件 | 内容 |
|------|------|------|
| 1 | **本文件（`AGENTS.md`）** | 项目概览、Document Index、**Hard Constraints 1-10** |
| 2 | `docs/CONVENTIONS.md` | 编码规则（命名 / 文件组织 / 禁止事项） |

### 第二步：判断你在做哪类工作

#### A. 维护 v1（当前线上版本：Tauri + React）

- 按 `docs/CONVENTIONS.md` 与 `docs/design/*` 工作
- 设计文档**尚未退役**，以它们为实现依据

#### B. 参与 v2.0.0-gpui 重构（Tauri → GPUI 纯 Rust）

按顺序读：

| 顺序 | 文件 | 内容 |
|------|------|------|
| 1 | `docs/specs/RULES.md` | **Spec 工作规则**（含设计文档退役规则、证据要求、文档卫生） |
| 2 | `docs/specs/README.md` | **Spec 注册表与进度（唯一权威状态源）** |
| 3 | `docs/tasks/v2.0.0-gpui/research-log.md` | 调研证据（实测数据 + 风险登记 R1-R9 + **§9 zed 源码获取配方**） |
| 4 | 当前要做的 spec 文件 | `docs/specs/phase-NN/S<NN>-<NN>-*.md` |

### 第三步：开始一个 spec

1. 从 `docs/specs/README.md` 选一个 `todo` 且依赖已满足的 spec
2. 把状态改为 `doing`（**同时改 spec 文件头和注册表**，保持一致）
3. 按 spec 的「实现要点」执行，满足全部「验收标准」
4. 填写「证据」段 —— **必须是可复核材料，不接受「OK」「已完成」**
5. 填写「决策记录」段 —— 记录 why（设计文档退役后这是唯一理由来源）
6. 处理该 spec 声明负责退役的设计文档（见 `RULES.md` §7）
7. 状态改为 `done`，填写「完成记录」，同步注册表

### 硬性纪律

- **状态只在 `docs/specs/README.md` 维护**，不另立状态源
- **同一时刻最多 2 个 spec 处于 `doing`**
- **不允许跳过 `doing` 直接 `done`**
- 引用任何文件路径前**先确认它存在**（`RULES.md` §11.1）
- **设计文档实现完成后必须删除**并登记台账（`RULES.md` §7）
- **文档必须入库**：不得在 `.gitignore` 排除 `docs/specs/`、`docs/evidence/`、`docs/design/`、agent 入口文件（`RULES.md` §13）

### 设计即代码（项目原则）

> **代码实现就是设计。** 不存在独立的「设计稿」产物。

- 不做设计稿 / 视觉稿 / 动效稿另立产物
- 视觉真值：**v1 取 `src/styles/global.css`，v2 取 `Theme`（`crates/ui`）**
- 设计令牌真值：v1 `src/styles/global.css`；v2 由其生成的 `crates/ui/src/theme_system/tokens.rs`（`scripts/theme/gen_tokens.py`）
- 需要看效果就看运行的代码；需要改设计就改代码

### 当前进度（会变化，以注册表为准）

**Phase 00 已完成（9/9）→ 决策：Go。**

| Phase | 状态 |
|-------|------|
| Phase 00 可行性 Spike | ✅ **9/9 完成** |
| Phase 01 工程骨架与分层 | ✅ 6/6（退路 tag `v1-final` 仅在本地，重构完成后推送） |
| Phase 02 引擎层移植 | ✅ 9/9 |
| Phase 03 主题与设计令牌 | ✅ 7/7 |
| Phase 04 Markdown 栈 | ✅ 9/9 |
| Phase 05 | 进行中（以注册表为准） |
| Phase 06–10 | 未开始 |

> **两个高风险门槛（S00-02 / S00-03）已通过；9 项风险中 4 项消除（R1/R3/R5/R7）、1 项降级（R2）。**
> **完全不需要 fork GPUI。** 全栈闭包 770 包（+77），毒性依赖零残留。
>
> 完整结论：`docs/specs/phase-00/S00-09-decision.md`
>
> 已实测的 GPUI 接入硬前置（`runtime_shaders` / `font-kit` / `ThemeSettingsProvider`）与代理要求见
> `docs/tasks/v2.0.0-gpui/research-log.md` §10 与 §9.4；
> macOS 窗口能力见 §11，外壳三件套集成见 §12，窗口外观与「不 fork」见 §13，
> 中文 IME 与多行见 §14，markdown vendor 配方见 §15，`ListState` 陷阱见 §16，
> 引擎去 Tauri 化见 §17，Phase 00 结论见 §18。
>
> 产物代码：`docs/evidence/s00-02/window-patch.rs`、`s00-03/shell-integration.rs`、
> `s00-04/window-appearance.rs`、`s00-05/ime-input.rs`、
> `s00-06/{theme_settings_shim,language_stub,mermaid}.rs` + `README.md`、
> `s00-07/list-integration.rs`、`s00-08/engine-integration.md`。

## Document Index

| Document | Purpose | When to Read |
|----------|---------|-------------|
| `docs/tasks/v1.0.0/overview.md` | Task overview & dependency graph | **Always** — start here |
| `docs/tasks/v1.0.0/*.md` | Individual task specs (16 tasks) | When assigned a task |
| `docs/tasks/v2.0.0-gpui/overview.md` | GPUI 重构：背景、Phase 划分、依赖图（不含状态） | 参与重构时 |
| `docs/tasks/v2.0.0-gpui/research-log.md` | GPUI 重构调研证据（实测数据 + 风险登记 R1-R9） | 参与重构时 |
| `docs/specs/RULES.md` | **Spec 工作规则**（含设计文档退役规则） | **Always** — 参与重构必读 |
| `docs/specs/README.md` | **Spec 注册表与进度（唯一权威状态源）** | **Always** — 参与重构必读 |
| `docs/specs/handoff.md` | **交接说明**（在飞状态、未提交改动归属、阻塞点、Phase 00 全部坑） | **接手项目时先读** |
| `docs/specs/design-deletions.md` | 设计文档退役台账 | 删除设计文档时 |
| `docs/evidence/v1-baseline/` | **v1 基线**：页面×状态目检清单、令牌实测值、性能、长会话样本（S01-06 产出；不含截图，视觉验收由用户对照运行中的 v1 目检） | 做验收时 |
| `docs/design/overview.md` | Architecture overview & key decisions | When needing context |
| `docs/design/pages-and-states.md` | 7 page specs + state machine | When building pages |
| `docs/design/component-mapping.md` | Design → React component map | When building components |
| `docs/design/rust-architecture.md` | Rust module layout & responsibilities | When writing Rust |
| `docs/release-workflow.md` | macOS ARM64 / Windows release, environment checks, OSS updater flow | When implementing or executing releases |
| `docs/CONVENTIONS.md` | Coding rules for ALL agents | **Always** — read once, follow always |

> **已移除的失效索引**（经审计确认路径从未存在）：`docs/design/prototypes/`、`docs/design/colors_and_type.css`。
> 仓库中曾有一个 `colors_and_type.css` 在 `.design/animation-preview/`（Trae 工具生成的动效预览实验），**已删除** —— 其令牌与 `src/styles/global.css` 重复。详见 `docs/specs/design-deletions.md`。

### 设计文档退役规则（重要）

`docs/design/*` 是**脚手架，不是永久资产**。代码实现完某份设计文档后**必须删除它** —— 代码即文档，不留冗余信息。

- 删除前须迁移「why」（代码表达不了的决策理由）到代码注释 / spec 的决策记录段
- 删除须在 `docs/specs/design-deletions.md` 登记，未登记视为无效
- 删除后**必须同步更新上方 Document Index 表**，索引与文件系统不一致视为缺陷

完整规则见 `docs/specs/RULES.md` §7，待退役清单见 `docs/specs/design-deletions.md`。

## Hard Constraints

1. **No traffic-light buttons** — zero chrome, no window controls, completely frameless
2. **Single brand color** — `#5B5FE9` (indigo-violet). State colors: `success/warning/error/info` only
3. **Radius scale** — only `4/8/12/16/9999` px
4. **No emoji icons** — use SVG icons exclusively（v1 使用 `lucide-react`，重构后为 svg 资源 + `Theme` 色）
5. **Design tokens only** — never hardcode colors/shadows/spacing（v1 经 CSS 变量，重构后经 `Theme`）
6. **Window never resizes on page switch** — keep user-set dimensions
7. **Esc/click-outside closes window, does NOT stop streaming**
8. **Single conversation stream** — no multi-session UI (but storage layer should accept optional session IDs)
9. **API Key stored in plaintext JSON** — not system keychain (v0.1)
10. **All text in Chinese** (UI labels, hints, settings) — code comments may be English

## Quick Start (after scaffold)

```bash
npm install
npm run tauri dev    # Dev mode with hot-reload
npm run tauri build  # Production build
```

## File Organization

```
buddy/
├── src/                    # React frontend
│   ├── components/         # UI components (flat: one folder per component)
│   ├── stores/             # Zustand stores
│   ├── hooks/              # Custom hooks
│   ├── types/              # TypeScript types (mirror Rust models)
│   ├── App.tsx
│   └── main.tsx
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── commands.rs
│   │   ├── api.rs
│   │   ├── models.rs
│   │   ├── storage.rs
│   │   └── hotkey.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── docs/                   # Design docs + task specs + specs + evidence
│   ├── design/             # Architecture, tokens, pages, IPC（实现完成后逐份退役）
│   ├── tasks/v1.0.0/       # 历史任务记录（不得修改）
│   ├── tasks/v2.0.0-gpui/  # 重构背景与 Phase 划分（不承载状态）
│   ├── specs/              # **Spec 注册表与执行记录（唯一权威状态源）**
│   ├── evidence/           # 验收证据（v1 基线等，不可再生）
│   ├── release-workflow.md # 发布流程
│   └── CONVENTIONS.md      # Coding rules for all agents
├── AGENTS.md               # ← **唯一权威入口**（所有 agent 必读）
├── CLAUDE.md               # 薄指针 → AGENTS.md（Claude Code）
├── GEMINI.md               # 薄指针 → AGENTS.md（Gemini CLI）
├── .cursor/rules/buddy.mdc # 薄指针 → AGENTS.md（Cursor）
├── .github/copilot-instructions.md  # 薄指针 → AGENTS.md（Copilot）
├── .windsurfrules          # 薄指针 → AGENTS.md（Windsurf）
├── .clinerules             # 薄指针 → AGENTS.md（Cline / Roo）
└── .rules                  # 薄指针 → AGENTS.md（Zed）
```

> **各 agent 入口文件一律是薄指针，不得写入独立内容。** 新增入口时见 `docs/specs/RULES.md` §11.4。
