# S01-02 GPUI 依赖来源与 rev 锁定

> 状态: `done`
> Phase: 01
> 依赖: S00-01, S00-04
> 阻塞: —
> 退役设计文档: —

## 目标

确定并锁定 GPUI 的依赖来源，确保 `gpui` / `gpui_platform` / `theme` / `ui` 四者版本一致且可复现构建。

**产出物**：`Cargo.toml` 中已锁定的依赖声明 + 来源决策记录 + **是否 fork 的结论**。

> ✅ **前置结论已定（由 S00-02 / S00-03 / S00-04 给出）：不 fork GPUI。**
> 本 spec 无需再判定，只需按「不 fork」落实依赖声明。

## 输入

- `docs/tasks/v2.0.0-gpui/research-log.md` §1 §3.4 —— crates.io 可用的 `gpui 0.2.2`、平台层为 Apache-2.0、Comet 的 fork patch 清单
- `docs/tasks/v2.0.0-gpui/research-log.md` **§9 —— zed 源码获取配方（含锁定 rev `290cbcb`，用于验证 `theme`/`ui` 与 `gpui` 的版本一致性）**
- S00-01 的闭包实测结果
- **S00-02 / S00-03 / S00-04 的「不需要 fork」实测依据**（research-log §11.2 / §12.1 / §13.5）
- `docs/evidence/s00-02/window-patch.rs` —— 窗口外壳补丁（运行时 objc2，非 fork）
- `docs/evidence/s00-04/window-appearance.rs` —— 外观补丁（同上）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S01-02-1 | 来源二选一 | **A**：`gpui` 用 crates.io `0.2.2` + `theme`/`ui` 用 zed git rev（需验兼容）<br>**B**：四者全部同一 zed git rev（最稳） |
| S01-02-2 | rev 锁定 | 若走 git，**四者必须同一 rev**，并在 Cargo.toml 注释记录 rev + 日期 + 来源（可参照 research-log §9.1 记录的探测基准 rev） |
| S01-02-3 | fork 决策 | **已定：不 fork**。窗口外壳（S00-02/S00-03）与外观（S00-04）全部用运行时 objc2 达成，未触及 gpui 源码 |
| S01-02-4 | fork 记录 | **不适用**（不 fork）。但需记录「当初为何不需要」——见「决策记录」 |
| S01-02-5 | 平台 features | `gpui_platform` features 按目标平台声明；macOS 起步，Linux 的 `wayland`/`x11` 暂不开 |
| S01-02-6 | gpui_tokio | 引入 `gpui_tokio` 并确认与 `gpui` 同一来源 |
| S01-02-7 | 可复现性 | 提交 `Cargo.lock`；确认干净环境能构建 |

## 验收标准

- [x] 依赖来源已明确并记录理由（**B：全 git，同一 rev**）
- [x] `gpui` / `gpui_platform` / `theme` / `ui` 版本或 **rev 完全一致**
- [x] `Cargo.lock` 已提交（191900 字节，确认未被 gitignore）
- [x] 干净目录下 `cargo check` 可复现通过
- [x] 是否需要 fork 已有明确结论 —— **不 fork**（依据 S00-02/03/04）
- [x] 若需 fork：patch 清单完整且每项有理由说明 —— **不适用**
- [x] Apache-2.0 的平台层 crate 已确认（无版权风险）

## 证据

### 1. 依赖来源：**B（全部走 zed git，同一 rev）**

| crate | crates.io 是否可获取 | 结论 |
|-------|---------------------|------|
| `gpui` | ✅ `0.2.2`（7 个版本，26.7 万下载） | — |
| `gpui_platform` | ❌ **未发布**（`publish = false`） | 只能 git |
| `gpui_macos` / `gpui_apple` | ❌ 未发布 | 只能 git |
| `theme` | ❌ 未发布（crates.io 上的 `theme` 是无关的 dioxus 主题库） | 只能 git |
| `ui` | ❌ 未发布（crates.io 上的 `ui` 是 2018 年的无关库） | 只能 git |

**因为四者中有三个只能走 git，所以四个全部走 git。**
混用（crates.io `gpui 0.2.2` + git `theme`）会引入「版本是否对应」的未验证风险，且无任何收益。

> ⚠️ **crats.io 的 `gpui 0.2.2` 与本 rev 是否同源，未验证。**
> 走全 git 即回避了这个问题。

### 2. rev 一致性 ✅

`Cargo.lock` 里所有 zed 来源的 crate：

```
rev 290cbcb9cb6a5dcbe0060431a126ad19e743f2f4: 31 个 crate
    gpui             v0.2.2 ← 直接依赖
    gpui_platform    v0.1.0 ← 直接依赖
    gpui_tokio       v0.1.0 ← 直接依赖
    theme            v0.1.0 ← 直接依赖
    ui               v0.1.0 ← 直接依赖

不同 rev 数: 1   ✅
```

> **版本号不同不是问题**：`gpui` 声明 `0.2.2`，其余声明 `0.1.0` —— 这是 zed 自己的版本策略。
> **关键是 rev 相同**（同一 commit），这才能保证类型兼容。

### 3. 直接依赖清单（5 项）

| crate | 用途 |
|-------|------|
| `gpui` | 框架本体 |
| `gpui_platform` | 平台后端选择（`application()` 入口）+ 两个必需 feature |
| `gpui_tokio` | tokio ↔ GPUI 桥接（S00-08 实测可用） |
| `theme` | 设计令牌（GPL） |
| `ui` | zed 组件库（GPL） |

### 4. 平台层许可证确认 ✅

| crate | license |
|-------|---------|
| `gpui` | **Apache-2.0** |
| `gpui_platform` | **Apache-2.0** |
| `gpui_macos` | **Apache-2.0** |
| `gpui_apple` | **Apache-2.0** |
| `gpui_tokio` | **Apache-2.0** |
| `gpui_wgpu` | **Apache-2.0** |
| `gpui_shared_string` | **Apache-2.0** |
| `gpui_util` | **Apache-2.0** |
| —— | —— |
| `theme` | **GPL-3.0-or-later** |
| `ui` | **GPL-3.0-or-later** |

**平台层无 GPL 污染**；GPL 只在 `theme` / `ui`（以及后续的 vendored `markdown`），
而它们**只被 `buddy-ui` 使用**。

对照（我们**不**使用，归 `S04-02` 用 Comet 替代）：`markdown` / `language` / `editor` /
`workspace` / `project` 均为 **GPL-3.0-or-later**。

### 5. 干净目录可复现构建 ✅

```bash
$ CLEAN=$(mktemp -d)
$ CARGO_TARGET_DIR=$CLEAN cargo check --workspace --locked
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.49s
```

| 项 | 值 |
|----|-----|
| **实际编译单元** | **442 个 crate** |
| 关键 crate 是否都编译了 | `gpui` / `gpui_macos` / `gpui_apple` / `theme` / `buddy-engine` / `buddy-ui` / `buddy-app` —— **全部 ✓** |
| 耗时 | 28.49 s（`cargo check`，无 codegen） |
| 产物占用 | 612 MB |
| `rmeta` 文件 | 446 个 |

**`--locked` 通过**意味着 `Cargo.lock` 完整且与实际依赖一致 ——
新克隆仓库的人用同一份 lock 能构建出相同结果。

> 这条验证特意**数了编译单元数（442）**，而不是只看 `Finished`。
> 理由：S00-07 的教训是「仪表盘可以在窗口空白时显示 PASS」。

### 6. 是否需要 fork：**否** ✅

依据（全部实测）：

| 需求 | 原担心需 fork | 实测结论 |
|------|-------------|---------|
| 零装饰窗口 | `WindowOptions` 无路径可达 | 运行时 objc2 `setStyleMask:`（`S00-02`） |
| 窗口显隐 | gpui 只有 `minimize_window()` | 运行时 objc2 `orderOut:` / `makeKeyAndOrderFront:`（`S00-02`） |
| 16px 圆角 | — | 运行时 objc2 CALayer（`S00-04`） |
| 窗口模糊 | — | gpui 内置 `Blurred` 实测正常；且**产品不需要**（`S00-04`） |
| 窗内卡片 blur | 基础 gpui 无此原语 | **产品不需要**，不构成 fork 理由（`S00-04`） |
| tray / 热键 / 自启 | gpui 无此三者 | 现成 crate + 运行时集成（`S00-03`） |

**风险 R3（fork 维护成本）因此消除。** 依赖走纯 zed git rev，不维护 fork。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 依赖来源 | **B：全部同一 zed git rev** | 四者中有三个未发布，只能 git；统一走 git 避免版本错配风险 |
| 是否 fork | **否** | 外壳与外观全部运行时 objc2 达成；唯一的 fork 理由（窗内 blur）产品不需要 |
| rev 记录位置 | `Cargo.toml` 的 `[workspace.dependencies]` 注释 | 含上游仓库、分支、使用说明；升级时同步修改 |
| 是否用 crates.io 的 `gpui 0.2.2` | **否** | 与本 rev 是否同源未验证；全 git 可回避该问题 |
| `Cargo.lock` | **入库** | 应用类项目需可复现构建 |

## 证据

| 项 | 证据 |
|----|------|
| 依赖声明片段 | |
| rev 一致性 | |
| 可复现构建 | |
| fork patch 清单 | |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 来源 | **B：四者全部同一 zed git rev** | crates.io 只有 `gpui`（0.2.2），`gpui_platform`/`gpui_macos`/`theme`/`ui` 均 `publish = false`。混用有版本错配风险且无收益 |
| 是否 fork | **否** | 见下方证据链 |
| fork 的理由 | 不适用 | — |

**不 fork 的证据链**（各项均已实测）：

| 需求 | 原担心需 fork 的原因 | 实测结论 |
|------|-------------------|---------|
| 零装饰窗口 | `WindowOptions` 无路径可达，`window_decorations` 在 macOS 未实现 | 运行时 objc2 `setStyleMask:` 即可（`S00-02`） |
| 窗口显隐 | gpui 只有 `minimize_window()` | 运行时 objc2 `orderOut:` / `makeKeyAndOrderFront:`（`S00-02`） |
| 16px 圆角 | — | 运行时 objc2 CALayer `setCornerRadius:`（`S00-04`） |
| 窗口模糊 | — | gpui 内置 `Blurred` 实测正常；且**产品不需要**（`S00-04`） |
| 窗内卡片 blur | 基础 gpui 无此原语 | **产品不需要**，故不构成 fork 理由（`S00-04`） |
| tray / 热键 / 自启 | gpui 无此三者 | 现成 crate + 运行时集成，未改 gpui（`S00-03`） |

## 完成记录

- 日期：2026-09-11
- commit：（rev 已锁定于根 `Cargo.toml` 的 `[workspace.dependencies]`；`Cargo.lock` 入库）
- 设计文档处置：—

## 备注

**结论已于 Phase 00 定下：不 fork。** 原担心需要 fork 的两件事（窗口外壳、毛玻璃）均已被证伪：

- 窗口外壳：全部需求可在运行时经 objc2 达成（`S00-02` / `S00-03`）
- 毛玻璃：产品已决定不使用（`S00-04`）；gpui 内置的 `Blurred` 实测也正常

因此 GPUI 依赖走**纯 zed git rev**，不维护 fork。风险 **R3 消除**。

> Comet 的 patch 清单（research-log §3.4）显示其 fork 主要服务于：透明窗 destination alpha 修正、
macOS 26 `CABackdropLayer` 失效、backdrop blur 光栅化。
> 这些**均不在 Buddy 的需求范围内**——尤其是窗内 backdrop blur，产品不需要。

Comet 的 patch 清单（research-log §3.4）显示其 fork 主要服务于：透明窗 destination alpha 修正、macOS 26 `CABackdropLayer` 失效、backdrop blur 光栅化。若这些不在 Buddy 需要范围内，可避免 fork。
