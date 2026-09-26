# S01-01 三层 workspace 结构与工具链

> 状态: `done`
> Phase: 01
> 依赖: S00-09（Go）
> 阻塞: —
> 退役设计文档: `docs/design/rust-architecture.md` §模块布局

## 目标

建立 `buddy-engine` / `buddy-ui` / `buddy-app` 三层 workspace 骨架，锁定工具链与依赖版本管理方式。

**产出物**：可 `cargo check` 通过的空 workspace，三个 crate 的依赖方向已正确建立。

## 输入

- `docs/tasks/v2.0.0-gpui/01-skeleton.md` —— 工作项 D01-D04
- `docs/tasks/v2.0.0-gpui/research-log.md` §1 §6 —— 工具链要求（`1.95.0` / `edition 2024`）与许可证分层
- `docs/design/rust-architecture.md` —— 现有模块布局（迁移对照）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S01-01-1 | workspace 根 `Cargo.toml` | `resolver = "2"`，members: `crates/engine` `crates/ui` `apps/buddy` |
| S01-01-2 | 工具链 | `rust-toolchain.toml` → `channel = "1.95.0"`（对齐 zed 要求） |
| S01-01-3 | edition | 三个 crate 全部 `edition = "2024"` |
| S01-01-4 | `[workspace.package]` | 统一 `version` / `license` / `edition` / `authors` |
| S01-01-5 | `[workspace.dependencies]` | gpui / tokio / serde / reqwest 等集中声明，各 crate 用 `.workspace = true` |
| S01-01-6 | 依赖方向 | `buddy-app` → `buddy-ui` → 无；`buddy-app` → `buddy-engine`；**`buddy-ui` 不得依赖 `buddy-engine` 的反向**（engine 不依赖任何 UI） |
| S01-01-7 | lib 入口 | 各 crate `crates/*/src/lib.rs` 最小骨架；`apps/buddy/src/main.rs` |
| S01-01-8 | **`[patch.crates-io]`** | **S00-06 阻碍级发现：`[patch]` 不会传递给下游消费者**。zed 用自家 fork 补了 `async_process::Child::adopt_raw_pid`，不复制这些 patch 则编译失败。macOS markdown 场景需 5 项（清单见 `docs/evidence/s00-06/README.md` §1）。**未复制的项与理由也需记录**（windows-capture / calloop / livekit 系列 / scratch） |
| S01-01-9 | **GPUI 接入硬前置** | S00-01 实测的三个条件，缺任一即失败，写入 workspace 文档：<br>① `gpui_platform` features 必含 **`runtime_shaders`**（否则需完整 Xcode 的 MetalToolchain）<br>② 必含 **`font-kit`**（否则**完全没有文字**，仅一条 WARN 静默退化）<br>③ 必须在 `theme::init` 后安装 `ThemeSettingsProvider`（否则开窗 panic） |
| S01-01-10 | **开发者环境要求** | github 访问**必须走代理**：实测直连 40 min 未完成（`cargo fetch` 超时，仅拉到 272 MB），走代理 865 s 完成。需写入仓库的开发环境说明 |

## 验收标准

- [x] `cargo check --workspace` 通过
- [x] `cargo check` 在 `1.95.0` 工具链下通过（`rustup show` 确认）
- [x] 三个 crate 的 `edition` 均为 2024
- [x] 所有第三方依赖通过 `[workspace.dependencies]` 管理，无 crate 内直接写死版本号
- [x] 依赖方向符合 S01-01-6，`buddy-engine` 不依赖 `buddy-ui`
- [x] **`[patch.crates-io]` 已配置且干净环境可构建**（S00-06 发现）
- [x] **三个 GPUI 硬前置已逐项验证**（特别是 `font-kit`：去掉它文字会静默消失）
- [x] 开发环境说明包含**代理要求**（`docs/dev-environment.md`）
- [x] **v1 未被破坏**（额外加的回归检查，见证据 §5）

## 证据

### 1. workspace 结构

```
buddy/
├── Cargo.toml              ← workspace 根（resolver = "2" + exclude src-tauri + [patch.crates-io]）
├── Cargo.lock              ← 191900 字节，已确认入库（未被 gitignore）
├── rust-toolchain.toml     ← channel = "1.95.0"
├── crates/
│   ├── engine/             ← buddy-engine，MIT，零 GPUI/Tauri
│   └── ui/                 ← buddy-ui，GPL-3.0-or-later
└── apps/
    └── buddy/              ← buddy-app，GPL-3.0-or-later
```

### 2. 工具链

```
$ rustup show active-toolchain
1.95.0-aarch64-apple-darwin (overridden by '.../buddy/rust-toolchain.toml')
```

### 3. 依赖方向（硬约束）

```
$ cargo tree -p buddy-engine --depth 1 | grep -cE "buddy-ui|gpui|theme|ui v"
0                                    ← engine 树里 0 处 GPUI / theme / ui ✅
```

`buddy-ui` 的依赖（正确方向）：

```
buddy-ui v2.0.0 (crates/ui)
├── buddy-engine v2.0.0 (crates/engine)      ← UI 依赖 engine ✅
├── gpui v0.2.2 (zed rev 290cbcb)
├── gpui_platform v0.1.0 (zed rev 290cbcb)
├── gpui_tokio v0.1.0 (zed rev 290cbcb)
├── theme v0.1.0 (zed rev 290cbcb)
└── ui v0.1.0 (zed rev 290cbcb)
```

`buddy-app` 的依赖（只有 4 项，**不直接依赖 gpui / theme**）：

```
buddy-app v2.0.0 (apps/buddy)
├── buddy-engine v2.0.0
├── buddy-ui v2.0.0
├── env_logger v0.11.11
└── log v0.4.34
```

### 4. 三个硬前置逐项验证

| # | 前置 | 验证方式 | 结果 |
|---|------|---------|------|
| ① | `gpui_macos` 启用 `font-kit` | `cargo tree -p gpui_macos -f "{p} features={f}"` | ✅ `font-kit` 在 features 中 |
| ② | `gpui_apple` 启用 `runtime_shaders` | 同上 | ✅ `runtime_shaders` 在 features 中 |
| ③ | `ThemeSettingsProvider` 已安装 | 运行时检查 panic | ✅ **0 次 panic**（封装在 `buddy_ui::init_theme`） |

**运行时验证**（骨架窗口，`RUST_LOG=debug`）：

```
$ ./target/debug/buddy
（进程存活，无崩溃）

$ grep -c "no text will be rendered" /tmp/skeleton2.log
0                                    ← font-kit 生效 ✅
$ grep -c "panicked" /tmp/skeleton2.log
0                                    ← ThemeSettingsProvider 正确安装 ✅
```

**用户实机确认**：「有文字」—— 窗口正确显示三行文本。

> 这一项特意请用户目视确认，因为 `font-kit` 缺失是**静默失败**
> （窗口正常、组件正常、唯独无文字），而 S00-07 的教训是
> 「我的仪表盘可以在窗口空白的显示 PASS」。

### 5. 额外加的回归检查：**v1 未被破坏** ✅

根 `Cargo.toml` 的 `exclude = ["src-tauri"]` 是必需的 ——
否则 `src-tauri/Cargo.toml` 会向上找到本 workspace，因不是成员而报错，
导致 v1 无法构建（v1 在 Phase 05 前必须保持可用，见 `S01-05`）。

```
$ cd src-tauri && cargo check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 50.40s
    rc=0                              ← v1 仍可独立构建 ✅
```

### 6. `[patch.crates-io]` 已配置

5 项（macOS markdown 场景所需），完整清单与「未复制项及理由」见
`docs/evidence/s00-06/README.md` §1。

### 7. `.gitignore` 已补

```gitignore
# v2.0.0-gpui workspace（根 Cargo.toml）
/target/
# ⚠️ Cargo.lock **必须入库**（见 S01-02-7）
```

验证：`target/` 被忽略 ✅；`Cargo.lock` 可见（将入库）✅

### 8. 产物

`docs/dev-environment.md` —— 开发者环境要求（工具链 / 代理 / 三个硬前置 /
`[patch.crates-io]` / **不要破坏 v1** / 已知环境坑）。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| workspace 根加 `exclude = ["src-tauri"]` | **必须** | 否则 v1 的 Tauri 构建会失败。已实测验证 |
| GPUI 表面从 `buddy-ui` **重导出** | 是 | 让「UI 层是唯一接触 zed 的地方」。`apps/buddy` 只依赖两个分层 crate，日后换 GPUI rev 只改一处 |
| `apps/buddy` 直接依赖 `gpui` / `theme` | **否** | 同上。改走 `buddy_ui::gpui::…` / `buddy_ui::ActiveTheme` |
| `buddy-engine` 加 `#![forbid(unsafe_code)]` | 是 | 引擎层应可完全安全审计；`unsafe` 只在 UI 层的 objc2 补丁中出现 |
| 依赖版本集中管理 | 全部走 `[workspace.dependencies]` | 已验证 0 处 crate 内写死版本 |
| 是否 `cargo check` 时用共享 target | 开发时可用 `CARGO_TARGET_DIR` | 加速；但 **v1 回归检查必须用 v1 自己的 target** |

## 证据

| 项 | 证据 |
|----|------|
| cargo check | |
| 工具链版本 | |
| 依赖方向图 | `cargo tree` 输出 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| crate 命名 | `buddy-engine` / `buddy-ui` / `buddy-app` | |
| edition | 2024 | 对齐 zed |

## 完成记录

- 日期：2026-09-11
- commit：（workspace 骨架已落地：根 `Cargo.toml` / `rust-toolchain.toml` / `crates/engine` / `crates/ui` / `apps/buddy` / `docs/dev-environment.md`）
- 设计文档处置：`rust-architecture.md` 的模块布局段落在 S02-* 完成后退役（本 spec 仅建立骨架，**暂不删**）

## 备注

`edition = "2024"` 需要 Rust 1.85+，与 zed 要求的 `1.95.0` 一致。若 GPUI 后续抬高版本要求，此处需同步。

### Phase 00 回填（S00-09 §6）

本 spec 在 Phase 00 后半段获得三项**阻碍级**输入，均已在实现要点中体现：

| 发现 | 来源 | 影响 |
|------|------|------|
| `[patch.crates-io]` 必须复制 | S00-06 | 不复制则**编译失败**（`adopt_raw_pid` 找不到） |
| `font-kit` 必开 | S00-01 | 不开则**完全没有文字**，且静默 |
| github 必须走代理 | S00-01 | 不走代理 `cargo fetch` 无法完成 |

**`buddy-engine` 的分层可行性已在 S00-08 实证**：engine 依赖树 0 处 GPUI / Tauri / zed crate。
S01-01 只需把该结构正式化，而非探索。
