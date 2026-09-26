# 开发环境要求（v2.0.0-gpui）

> 本文记录**实测得出**的环境前置条件。全部来自 Phase 00 的 spike，
> 非推测。详见 `docs/tasks/v2.0.0-gpui/research-log.md` §9 §10。

---

## 1. 工具链

| 项 | 值 | 来源 |
|----|-----|------|
| Rust | **1.95.0** | 对齐 zed 的 `rust-toolchain.toml`；已写入仓库根 |
| edition | **2024** | 需 Rust 1.85+ |
| macOS | 实测 **26.4**（25E246），arm64 | — |

`rustup show active-toolchain` 应显示 `1.95.0`（由根 `rust-toolchain.toml` 覆盖）。

> ⚠️ `edition = "2024"` 有一个迁移点：**禁止在隐式借用模式里显式解引用**。
> 从 v1（edition 2021）移植代码时若遇到
> `cannot explicitly dereference within an implicitly-borrowing pattern`，
> 修法是 `|(_, &v)|` → `|&(_, &v)|`。（S00-08 实测，v1 全量移植只此 1 处。）

---

## 2. ⚠️ 网络：github 访问**必须走代理**

**这是硬前置，不是可选优化。**

实测对比：

| 情形 | 结果 |
|------|------|
| 不走代理 | `cargo fetch` 在 **2400 s 超时**，仅拉到 272 MB 裸库，**未完成** |
| 走代理 | **865 s 完成**，`Cargo.lock` 正常生成 |

`raw.githubusercontent.com` 会超时，但 **git 协议（经代理）可用**。

### 环境变量

```bash
export http_proxy=http://127.0.0.1:7890
export https_proxy=http://127.0.0.1:7890
export CARGO_HTTP_PROXY=http://127.0.0.1:7890
export CARGO_NET_GIT_FETCH_WITH_CLI=true
```

（本机代理由 Clash Verge 提供，端口 7890。）

### 已验证的 cargo 配置

`~/.cargo/config.toml` 已有：

```toml
[source.crates-io]
replace-with = 'tuna'
[source.tuna]
registry = 'https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git'
[net]
git-fetch-with-cli = true
```

crates.io 走清华镜像；github 的 git 依赖走代理。

---

## 3. ⚠️ GPUI 接入的三个硬前置（缺任一即失败）

> 全部由 S00-01 实测得出。**其中两个是静默失败**。

### ① `gpui_platform` 必含 `runtime_shaders`

**缺失后果**：构建失败。

```
error: gpui_apple@0.1.0: metal shader compilation failed:
cannot execute tool 'metal' due to missing Metal Toolchain
```

**背景**：`gpui_apple/build.rs` 默认调 `xcrun -sdk macosx metal` 编译
`src/shaders.metal`。该编译器自 **Xcode 16** 起被 Apple 拆为独立可下载组件
（`MetalToolchain`），默认不安装 —— 所以即使装了 Xcode，`metal` 也只是驱动存根。

开启 `runtime_shaders` 后改走 `emit_stitched_shaders()`：只拼接头文件与 `.metal` 源码，
着色器交由 Metal 驱动**运行时**编译，**构建期不再需要 Xcode 工具链**。

> 实测：**未安装 MetalToolchain、也未执行 `sudo xcodebuild -runFirstLaunch`，
> 构建与运行均正常。**

转发链：`gpui_platform` → `gpui_macos` → `gpui_apple`。

### ② `gpui_platform` 必含 `font-kit`

**缺失后果**：**完全没有文字**，且**静默退化** —— 背景与组件照常绘制，唯独没有字形。
只打一条 WARN：

```
WARN gpui_macos::platform] gpui_macos was compiled without the `font-kit` feature,
                          so no text will be rendered.
```

**这是最难排查的一类故障**：窗口开得出来、按钮看得见、进程不报错，只有文字消失。
容易被误判成「主题文字色与背景同色」或「字体解析失败」。

**排查手段：`RUST_LOG=debug`** 看 gpui 自身的日志。**任何 GPUI 显示异常都应先开日志确认。**

转发链：`gpui_platform` → `gpui_macos/font-kit` → `dep:zed-font-kit`（zed 的 fork）。

### ③ 必须在 `theme::init` 之后安装 `ThemeSettingsProvider`

**缺失后果**：开窗后首次渲染 `crates/ui` 的组件时 panic：

```
no state of type theme::theme_settings_provider::GlobalThemeSettingsProvider exists
  at gpui/src/app.rs:2030
```

**做法**：`crates/ui` 的 `init_theme(cx)` 已封装正确顺序：

```rust
theme::init(LoadThemes::JustBase, cx);
theme::set_theme_settings_provider(Box::new(BuddyThemeSettings::default()), cx);
```

**为什么自己实现而不是用 zed 的 `theme_settings`**：后者会拖入整个 `settings` 框架
（`settings` / `settings_content` / `settings_json` / `settings_migrator` / `watch`
/ `release_channel`）。而该 trait 只有 5 个方法。

---

## 4. ⚠️ 必须复制 zed 的 `[patch.crates-io]`

**缺失后果**：编译失败。

```
error[E0599]: no function or associated item named `adopt_raw_pid` found
              for struct `smol::async_process::Child`
              --> crates/util/src/command/darwin.rs:497:43
```

**原因**：zed 用自家 fork 的 `async-process` 补了 `adopt_raw_pid`：

```toml
# zed/Cargo.toml
[patch.crates-io]
async-process = { git = "https://github.com/zed-industries/async-process.git", rev = "0b6d6713…" }
```

**`[patch]` 不会传递给下游消费者。** 任何在 zed workspace 之外使用其内部 crate
（如 `util`）的项目都必须自己复制。

已在仓库根 `Cargo.toml` 配置（5 项，macOS markdown 场景所需），
完整清单与「未复制项及其理由」见 `docs/evidence/s00-06/README.md` §1。

> ⚠️ 若将来引入 `fs` / 终端 / 协作等功能，需按需补齐。

---

## 5. 构建与运行

```bash
# 完整检查
cargo check --workspace

# 运行（S01-01 阶段是骨架窗口）
cargo run -p buddy-app
```

### ⚠️ 不要破坏 v1

根 `Cargo.toml` 有：

```toml
exclude = ["src-tauri"]
```

**这是必需的。** 否则 `src-tauri/Cargo.toml` 会向上找到本 workspace，
因不是成员而报错，导致 **v1 无法构建** —— 而 v1 在 Phase 05 之前必须保持可用
（退路分支，见 `S01-05`）。

**改动 workspace 后应验证**：

```bash
cd src-tauri && cargo check    # 必须仍然通过
```

---

## 6. 已知的环境坑（S00 实测）

| 坑 | 现象 | 处理 |
|----|------|------|
| 屏幕录制权限 | `screencapture` 报 `could not create image from display` | 需在系统设置授权，否则无法程序化截图。**目前无 spec 需要**（2026-09-26 用户决定视觉验收改为目检，见 `S01-06` 决策记录） |
| Xcode 首次启动组件缺失 | `xcodebuild` 报 `CoreSimulator.framework` 缺失 | **与 GPUI 构建无关**（`runtime_shaders` 避开了）。仅 `S08-08` 签名/公证时需处理 |
| `spikes/` 产物丢失 | `spikes/` 被 gitignore | 每个 spike 的结论必须固化到 `docs/evidence/` |
| zed 源码探测目录被清理 | `/tmp/zed-probe` 不持久 | 复现配方见 `research-log` §9.3 |
