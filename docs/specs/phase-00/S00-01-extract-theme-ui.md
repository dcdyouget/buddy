# S00-01 抽取 zed theme + ui 并编译

> 状态: `done`
> Phase: 00
> 依赖: —
> 阻塞: —
> 退役设计文档: —

## 目标

在独立 spike workspace 中引入 zed 的 `theme` + `ui`，跑通 `cargo check`，并记录真实依赖闭包。

**产出物**：一个能编译的最小 GPUI 应用，用到了 `theme` 取色与 `ui` 的至少一个组件，附 `cargo tree` 输出。

## 输入

- `docs/tasks/v2.0.0-gpui/research-log.md` §1 §2 —— 依赖来源与实测闭包（theme 6 个 / ui 12 个）
- `docs/tasks/v2.0.0-gpui/research-log.md` **§9 —— 如何获取 zed 源码（sparse checkout 复现配方，含锁定 rev）**
- `~/Project/comet/docs/research/gpui.md` §1 §6 §8 —— 引入方式与版本 pin 参考
- 若本地无 zed 源码：按 research-log §9.3 自行拉取（临时目录会被清理，**不要假设已存在**）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S00-01-1 | 独立 spike 目录 | `spikes/s00-01-extract/`，不污染主工程；已在 `.gitignore` 排除 |
| S00-01-2 | 工具链 | `rust-toolchain.toml` → `channel = "1.95.0"`；`edition = "2024"` |
| S00-01-3 | git 依赖 | `gpui` / `gpui_platform` / `theme` / `ui` **同一 rev** = `290cbcb9cb6a5dcbe0060431a126ad19e743f2f4` |
| S00-01-4 | 版本兼容验证 | crates.io 上只有 `gpui`（0.2.2），**`gpui_platform` / `gpui_macos` / `theme` / `ui` 均未发布** → 必须走 git |
| S00-01-5 | 最小渲染 | 一个窗口 + `cx.theme()` 取色 + 一个 `ui::Button` |
| S00-01-6 | 闭包记录 | `Cargo.lock` 分析 + `cargo tree` |
| S00-01-7 | 许可证确认 | 记录引入的每个 crate 的 license |

## 验收标准

- [x] `cargo check` 通过
- [x] `cargo run` 能开出窗口并显示 `theme` 取到的颜色与一个 `ui` 组件
- [x] `cargo tree` 输出已存档
- [x] 实际闭包与 research-log §2 的差异已记录
- [x] 许可证清单已记录（确认无计划外 GPL 之外的传染性许可）

## 证据

| 项 | 证据 |
|----|------|
| 编译 | `cargo check` rc=0；`cargo build` rc=0。最终运行二进制 `target/debug/s00-01-extract`（56 MB，debug） |
| 闭包 | `Cargo.lock` 解析：**zed crate 30 个 / 总包 693 个**。清单：`collections component derive_refineable gpui gpui_apple gpui_linux gpui_macos gpui_macros gpui_platform gpui_shared_string gpui_util gpui_web gpui_wgpu gpui_windows http_client icons media menu perf refineable scheduler sum_tree syntax_theme theme ui ui_macros util_macros zlog ztracing ztracing_macro` |
| 闭包差异 | research-log §2 记录 `theme`=6 / `ui`=12，**实测 `ui` 闭包含 30 个 zed crate**。原因是当时有 24 个 crate 的 `Cargo.toml` 未取到，12 是**下界**。实际值未超「>30 即证伪」阈值，但已贴边 |
| 磁盘占用 | `~/.cargo/git/db/zed-*` 652 MB；`~/.cargo/git/db/font-kit-*` 1.6 MB；`target/` 2.5 GB（debug 全量） |
| 运行 | 先出现「窗口灰色、只有按钮形状、**无任何文字**」，加入 `font-kit` 后 **7 行文字全部可见、按钮标签可见**。用户实机确认「没问题」 |
| 主题取色 | 运行时 stdout：`theme.name = One Dark`、`appearance = Dark`、`colors.text = Hsla { h: 0.6139, s: 0.11, l: 0.86, a: 1.0 }`、`colors.element_background = Hsla { h: 0.6194, s: 0.13, l: 0.21, a: 1.0 }` |
| 环境诊断 | `RUST_LOG=debug` 捕获到关键 WARN：`gpui_macos was compiled without the font-kit feature, so no text will be rendered.`（加 feature 后出现次数由 1 → 0） |
| 网络 | **直连 github 40 分钟未完成**（`cargo fetch` 在 2400 s 超时，仅拉到 272 MB 裸库）；走 `http://127.0.0.1:7890` 后 **865 秒**完成，`Cargo.lock` 生成 |
| 许可证 | 平台层 `gpui` / `gpui_platform` / `gpui_macos` / `gpui_apple` = Apache-2.0；`theme` / `ui` = **GPL-3.0-or-later**；`zed-font-kit` fork 随 `font-kit` feature 引入。spike crate 已标 `license = "GPL-3.0-or-later"` |
| 未使用 Xcode | 构建全程**未安装** MetalToolchain，**未**执行 `sudo xcodebuild -runFirstLaunch` |

### 三个非显然前置条件（本 Spike 的核心产出）

不写代码无法发现的硬前置，缺任一即失败：

| # | 条件 | 缺失后果 | 发现方式 |
|---|------|---------|---------|
| 1 | `gpui_platform` features 必须含 **`runtime_shaders`** | 构建**失败**：`gpui_apple` build script 报 `cannot execute tool 'metal' due to missing Metal Toolchain`（`metal` 自 Xcode 16 起拆为独立可下载组件） | 首次 `cargo check` 失败 + 读 `crates/gpui_apple/build.rs` |
| 2 | `gpui_platform` features 必须含 **`font-kit`** | **完全不渲染任何文字**，且**静默退化**——仅一条 WARN，背景与组件照常绘制 | `RUST_LOG=debug` 捕获 WARN |
| 3 | 必须**自行实现并安装** `ThemeSettingsProvider` | 开窗即 panic：`no state of type theme::theme_settings_provider::GlobalThemeSettingsProvider exists` | 首次运行 panic 栈指向 `gpui/src/app.rs:2030` |

**运行时编译着色器的代价已验证**：`runtime_shaders` 将 `.metal` 源码拼接后交由 Metal 驱动在运行时编译，实际运行 8 秒零错误、文字与色块均正常渲染，说明该路径可用。

**最终可用依赖声明**：
```toml
gpui          = { git = "…/zed", rev = "290cbcb9…" }
gpui_platform = { git = "…/zed", rev = "290cbcb9…", features = ["runtime_shaders", "font-kit"] }
theme         = { git = "…/zed", rev = "290cbcb9…" }
ui            = { git = "…/zed", rev = "290cbcb9…" }
```

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 依赖来源 | **全部走 zed git（同一 rev）** | crates.io 只有 `gpui`；`gpui_platform` / `gpui_macos` / `theme` / `ui` 均 `publish = false`。混用 crates.io `gpui 0.2.2` 与 git `theme`/`ui` 会有版本错配风险，且无收益 |
| 是否 patch/fork GPUI | **本 Spike 阶段不需要** | `runtime_shaders` 解决了 Xcode 依赖；毛玻璃是否需要 fork 仍归 S00-04 判定 |
| 字体与密度的提供方式 | **自行实现 `ThemeSettingsProvider`（30 行）** | 该 trait 只有 5 个方法。自行实现后**完全不需要** `theme_settings` crate，从而避开 `settings` / `settings_content` / `settings_json` / `settings_macros` / `settings_migrator` / `watch` / `release_channel` 整条依赖链。**这比 S04-01 原计划的「vendor 后手工 patch 9 处 `ThemeSettings`」更干净——不改 zed 源码** |
| 工具链 | 1.95.0（对齐 zed） | zed `rust-toolchain.toml` 指定；已验证可安装并编译通过 |
| `ui_font` 取值 | `font(".SystemUIFont")` | gpui 约定名；macOS 侧由 `gpui_macos/src/text_system.rs:282` 映射为 `.AppleSystemUIFont`。运行确认可渲染中文与英文 |

## 完成记录

- 日期：2026-09-10
- commit：（spike 产物在 `spikes/`，已被 gitignore；验证代码见本 spec 的证据段）
- 设计文档处置：—（本 spec 不涉及设计文档退役）

## 遗留与交接

以下发现已回写 `docs/tasks/v2.0.0-gpui/research-log.md` §10，供其他 agent 与后续 spec 使用：

1. **代理是硬前置**：github 直连不可用，必须 `http_proxy/https_proxy=http://127.0.0.1:7890`
2. **三个 feature / 初始化前置**（上表）
3. **`ThemeSettingsProvider` 接缝**：直接影响 `S04-01` 的实现方案（从 patch 改为替换 provider）
4. **闭包实际值 30**：`S01-02` 的 fork 决策与 `S04-03` 的闭包收敛目标需以 30 为基线

仍待后续 spec 解决（未因本 spec 而消除）：

- 毛玻璃是否需要 patch fork → **S00-04**
- `ui` crate 的组件会读取 provider（`ui_density` / `typography`），Buddy 的令牌体系如何与 `ui` 的排版默认值共存 → **S03-04 / S03-05**
- `target/` 2.5 GB、git 裸库 652 MB 的体积是否可接受 → 需在 `S01-02` 一并评估（影响开发者体验与 CI 缓存策略）
