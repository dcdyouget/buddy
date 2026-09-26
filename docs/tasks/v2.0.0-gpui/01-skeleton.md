# Phase 01: 工程骨架与分层

## 目标

建立 `buddy-engine` / `buddy-ui` / `buddy-app` 三层 workspace，确定 GPUI 依赖来源，落实许可证分层。

**这是全程的架构地基。** 分层一旦被破坏（engine 依赖了 GPUI），GPL 就传染到全仓，后续不可逆。

## 相关文档

- `docs/tasks/v2.0.0-gpui/research-log.md` §1 §6 — 依赖来源与许可证分层
- `docs/tasks/v2.0.0-gpui/00-spike.md` — S01 / S09 的产出决定本 Phase 的输入
- `docs/design/rust-architecture.md` — 现有 Rust 模块职责（作为分层对照）
- `docs/CONVENTIONS.md` — 通用编码规则

## 验收标准

- [ ] `cargo check --workspace` 通过
- [ ] `cargo tree -p buddy-engine` 中**不含** `gpui` / `gpui_platform` / 任何 GPL crate
- [ ] `cargo tree -p buddy-ui` 中**不含** `tauri`
- [ ] 三层 crate 的 `license` 字段正确标注
- [ ] `THIRD_PARTY_NOTICES.md` 建立（zed GPL 部分 + Comet MIT 部分 + tree-sitter grammars）
- [ ] CI 有自动化断言阻止 engine 层被污染

## 开发工作

### 1.1 workspace 结构

| ID | Task | Details |
|----|------|---------|
| D01 | 建 workspace | `Cargo.toml` workspace，`resolver = "2"`，members: `crates/engine` `crates/ui` `apps/buddy` |
| D02 | 锁工具链 | `rust-toolchain.toml` → `channel = "1.95.0"`（对齐 zed 要求）；全部 crate `edition = "2024"` |
| D03 | workspace 版本 | `[workspace.package] version / license / edition` 统一管理 |
| D04 | 依赖版本集中 | `[workspace.dependencies]` 声明 gpui / tokio / serde 等，各 crate 用 `.workspace = true` |

### 1.2 GPUI 依赖来源

| ID | Task | Details |
|----|------|---------|
| D05 | 确定版本策略 | 依据 S01 / S09 结论二选一：<br>A. `gpui` 用 crates.io `0.2.2`，`theme`/`ui` 用 zed git rev（需验版本兼容）<br>B. 全部走同一 zed git rev（最稳） |
| D06 | pin 单一 rev | 若走 git，`gpui` / `gpui_platform` / `theme` / `ui` **必须同一 rev**，注释记录 rev 与日期 |
| D07 | fork 决策 | 依据 S04 结论判断是否需要 fork（毛玻璃 / 透明窗 patch）。若需要则建 fork 仓库并记录 patch 清单 |
| D08 | 平台 features | `gpui_platform` features 按目标平台声明：macOS 起步；Linux 的 `wayland`/`x11` 暂不开 |
| D09 | tokio 桥接 | 引入 `gpui_tokio`，在 `main` 中 `Tokio::init(cx)` |

### 1.3 许可证与合规

| ID | Task | Details |
|----|------|---------|
| D10 | 声明 GPL | `crates/ui` 与 `apps/buddy` 设 `license = "GPL-3.0-or-later"` |
| D11 | 声明 engine 许可 | `crates/engine` 设 `license = "MIT"`（或 Apache-2.0），并在 README 写明「engine 层不含 GPL 代码，理由与边界」 |
| D12 | 三层 NOTICE | `THIRD_PARTY_NOTICES.md`：<br>- Apache-2.0: gpui / gpui_platform / gpui_macos / gpui_wgpu<br>- GPL-3.0-or-later: zed `theme` / `ui` / vendored `markdown`<br>- MIT: Comet `syntax`（若采用）<br>- 各 tree-sitter grammar 的许可证逐一列出 |
| D13 | vendor patch 释出 | vendored `markdown.rs` 的修改以独立 patch 文件放仓库（满足 GPL 对修改部分的释出要求） |
| D14 | 顶层 LICENSE | 仓库根 `LICENSE` 说明分层许可：`crates/engine` MIT，其余 GPL-3.0-or-later |

### 1.4 防污染机制

| ID | Task | Details |
|----|------|---------|
| D15 | CI 断言 | 脚本检查 `cargo tree -p buddy-engine` 输出中不得出现 `gpui` / `zed` / GPL crate 名 |
| D16 | 依赖方向校验 | 用 `cargo-deny` 或自定义脚本禁止 `engine → ui` 的依赖方向 |
| D17 | PR 模板 | 提示「改动是否越过分层边界」 |

### 1.5 目录与退路

| ID | Task | Details |
|----|------|---------|
| D18 | 迁移期目录 | 旧 Tauri 代码暂留原位；新代码放 `crates/` + `apps/`，两套并存到 Phase 05 |
| D19 | 退路分支 | 建立并保护 Tauri 版本的退路分支/tag，Phase 09 前不允许删除 |
| D20 | 文档索引 | 更新 `AGENTS.md` 的文档索引，加入 `docs/tasks/v2.0.0-gpui/` |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T01 | 分层断言测试 | CI 中运行的依赖树检查脚本，故意注入违规依赖验证能拦住 |
| T02 | 许可证扫描 | `cargo-deny check licenses` 或 `cargo-about` 生成报告，人工核对 |
| T03 | 空窗口冒烟 | `apps/buddy` 能开出空窗口并退出 |

## 备注

Phase 01 完成后，Phase 02（引擎层）与 Phase 03（主题）、Phase 04（Markdown）可并行推进。
Phase 07（窗口外壳）依赖 Phase 01 的 fork 决策结果。
