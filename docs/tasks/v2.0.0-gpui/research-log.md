# v2.0.0-gpui 调研记录（证据基础）

> 所有结论均来自对本机 zed 源码（`main`，sparse probe）与 `~/Project/comet` 的实测，非推测。
> 每条附文件/行号，便于后续复核与复现。

---

## 1. GPUI 的依赖来源

| 事实 | 证据 |
|------|------|
| `gpui` **已发布到 crates.io**，版本 `0.2.2`（7 个版本，约 26.7 万下载） | crates.io API `/api/v1/crates/gpui` |
| `theme` / `ui` / `markdown` / `language` / `editor` **未发布**（`publish = false`）。crates.io 上同名 crate 全是无关第三方 | zed `crates/*/Cargo.toml` + crates.io API |
| 平台后端 crate 均为 **Apache-2.0**：`gpui_platform` / `gpui_macos` / `gpui_windows` / `gpui_wgpu` | 各 `Cargo.toml` 的 `license` 字段 |
| zed 的 `theme` / `ui` / `markdown` 为 **GPL-3.0-or-later** | 各 `Cargo.toml` 的 `license` 字段 |

**结论**：`gpui` 可用 crates.io 版本，也可用 git rev；GPL 三件套只能走 git 依赖。

工具链要求（zed `rust-toolchain.toml`）：`channel = "1.95.0"`，workspace `edition = "2024"`。

---

## 2. 依赖闭包实测（决定 GPL 是否划算）

实测 zed `main` 上三者的 zed crate 传递闭包：

| crate | 闭包大小 | 闭包内容 |
|-------|---------|---------|
| **`theme`** | **6** | `collections` `gpui` `gpui_util` `refineable` `syntax_theme` `theme` |
| **`ui`** | **12** | 上述 + `component` `icons` `menu` `gpui_macros` `ui_macros` |
| **`markdown`** | **40+**（下界，24 个 crate 的 Cargo.toml 未取到） | 上述 + `settings` `settings_content` `settings_json` `settings_macros` `migrator` `watch` `release_channel` `lsp` `rpc` `fs` `http_client` `language` `language_core` … |

**毒性来源**：
- `language` → `language_core` → `lsp` → `rpc` → `fs` → `http_client`（zed 的 LSP/项目机制）
- `theme_settings` → `settings` → `settings_content` / `settings_json` / `migrator` / `watch`

**但耦合是浅的**（`crates/markdown/src/markdown.rs`，共 7428 行）：

```
ThemeSettings 调用点: 9     ← 全部是字体/字号读取
settings::    调用点: 13
language::    调用点: 36    ← 其中大半在测试（4894 行以后）
```

`ThemeSettings` 的 9 处用法样本（`markdown.rs`）：`theme_settings.buffer_font.weight`(194)、`agent_buffer_font_size`(197)、`buffer_font_size`(201)、`markdown_preview_font_family`(211)、`ui_font.fallbacks`(226)、`buffer_line_height`(283) …

**结论**：vendor `markdown.rs` + 把 9 处 `ThemeSettings` 换成自有字体配置 + 把 `language::` 换成纯 tree-sitter → 闭包从 40+ 塌到约 18，且全在 gpui/theme/ui 邻域。这是 patch，不是重写。

---

## 3. Comet 的可借鉴性（`~/Project/comet`，199k 行，**MIT**）

### 3.1 分层示范（关键）

| crate | GPUI 依赖 |
|-------|----------|
| `zeron-engine` | **0 个** ✅ |
| `zeron-ui` | 唯一依赖 gpui 的 crate |

Comet `ARCHITECTURE.md:174` 明确写道：不使用 zed 的 GPL crate（`markdown` / `ui` / `theme` / `editor`），三者自研——**因为它自己是 MIT**。

### 3.2 已验证的 GPUI 能力（可作规格说明书）

| 能力 | Comet 实现 | 行数 |
|------|-----------|------|
| Markdown 增量解析 | `crates/ui/src/markdown/parser.rs`（BlockTree，块粒度增量重解析） | 1220 |
| Markdown 渲染 | `crates/ui/src/markdown/render.rs` | 1957 |
| 流式半截标记修补 | `crates/ui/src/markdown/mend.rs`（streamdown `remend` 移植，补全 `**bold` / `[link](partial`） | 413 |
| 流式渐显 | `crates/ui/src/markdown/veil.rs`（**纯绘制层** alpha，不影响 layout，故永不 reflow） | 508 |
| Markdown 文本选择 | `crates/ui/src/markdown/selection.rs` | 436 |
| 语法高亮 | `crates/syntax/src/lib.rs`（tree-sitter，**paint-only** 契约） | 1354 |
| 主题体系 | `crates/ui/src/theme.rs` + `crates/theme/` | 2634 + 4670 |
| 窗内毛玻璃 | `crates/ui/src/frost.rs`（backdrop blur 44px，自绘，跨平台一致） | 180 |
| 滚动边缘渐隐 | `crates/ui/src/edge_fade.rs` | 211 |
| 输入框（含 IME） | `crates/ui/src/composer.rs` | 8768 |
| 聊天记录虚拟列表 | `crates/ui/src/transcript.rs` | 10131 |
| 半透明窗口修正 | `theme.rs:975` `window_background_appearance()` → `WindowBackgroundAppearance::Blurred` | — |
| 自更新（app bundle swap） | `crates/update/src/lib.rs` | 806 |

**语法高亮契约原文**（`~/Project/comet/docs/syntax-highlighting.md`）：
> Highlighting changes foreground color only—never font, weight, style, wrapping, height, or scroll geometry.

### 3.3 Comet 的重要缺口

| 缺口 | 证据 |
|------|------|
| **完全不支持 Windows** | CI 仅 `macos-latest` + `ubuntu-latest`；脚本仅 `package-macos.sh` / `package-linux.sh`；全仓 Windows 引用仅 1 处无关 `cfg` |
| **无 tray icon** | 全仓无 `tray-icon` / `tray` 相关依赖 |
| **无全局热键** | 全仓无 `global-shortcut` / `global-hotkey` |
| **无 autostart** | 全仓无相关依赖 |
| 是常规整窗应用，非浮动面板 | `shell.rs` 有 `titlebar_cluster_start` / 「Traffic-light-aware titlebar layout」 |
| 也内嵌了 webview | `crates/ui/src/browser/macos.rs:26` 使用 `wry` 渲染 HTML 预览 |

> **警示**：Comet 不能证明「GPUI 能做出 Buddy 那种无边框浮动面板」，也不能证明 Windows 行为。

### 3.4 Comet 维护着一个 GPUI fork

`Cargo.toml:52` 硬 pin `zeronsh/zui`（rev `07fd941`），非 zed 官方。其 patch 清单（同文件注释）包括：

- 有界 blur pass + 共享高斯权重
- **透明窗 destination alpha 修正**（Porter-Duff OVER，非 additive）
- **macOS 26 上 `CABackdropLayer` 不再下发导致窗口模糊失效** → 切 `UnderWindowBackground`
- wgpu renderer 里光栅化 BackdropBlur（使 Linux 也有毛玻璃）
- `ImageSource::evict`（修 sprite-atlas 泄漏）
- 渲染器 GPU 内存上界 + 空闲释放
- 水平 per-pixel `EdgeFade`

**含义**：毛玻璃 + 透明窗必然需要 fork 级别 patch；这是持续 rebase 成本。

---

## 4. Windows 兼容性实测（回答「效果是否一致」）

### 4.1 三个平台是三套独立渲染器

| | macOS | Windows | Linux |
|---|---|---|---|
| 图形 API | **Metal**（`objc2-metal`） | **Direct3D 11**（`directx_renderer.rs` / `directx_devices.rs`） | **wgpu**（Vulkan） |
| 文本栈 | **CoreText**（`gpui_macos/src/text_system.rs`） | **DirectWrite**（`gpui_windows/src/direct_write.rs`） | cosmic-text + swash |
| Shader | MSL | **HLSL**（`shaders.hlsl` / `color_text_raster.hlsl` / `alpha_correction.hlsl`） | WGSL |
| 窗口底座 | `NSPanel` 子类 `GPUIPanel` | 原生 HWND + Win32 | X11 / Wayland |

**结论**：不是「一个抽象层 + 三个后端」，是三份手写实现。同代码 ≠ 同画面。

### 4.2 毛玻璃：同一枚举，不同系统效果

> **⚠️ 2026-09-10 更新：本节已不再适用。**
> 产品已决定**不使用毛玻璃 / 半透明**（见 `docs/specs/phase-00/S00-04-backdrop.md`）。
> 最终外观为「不透明实色面板 + 16px 圆角」，Windows 与 macOS 都只做圆角与阴影对齐，
> **无 backdrop 差异需处理**。因此风险 **R5 消除**，`S09-02` 已删除。
>
> 本节内容**保留作为调研证据**（若将来重新考虑毛玻璃，这些差异仍成立）。

`WindowBackgroundAppearance`（`crates/gpui/src/platform.rs:2252`）文档原文对 `Blurred` 标注：
> Transparency, but the contents behind the window are blurred. **Not always supported.**

| 值 | macOS | Windows |
|----|-------|---------|
| `Blurred` | `NSVisualEffectView`（模糊**窗口后方真实内容**） | **Acrylic**：`set_window_composition_attribute(hwnd, …, 4)`（未公开 accent-policy，`is_acrylic = state == 4`，`gpui_windows/src/window.rs:1599`） |
| `MicaBackdrop` | 不存在 | `DWMSBT_MAINWINDOW`（`window.rs:893`） |
| `MicaAltBackdrop` | 不存在 | `DWMSBT_TABBEDWINDOW`（`window.rs:897`） |

**两个硬约束**：
1. `DWMWA_SYSTEMBACKDROP_TYPE` **仅 Windows 11 build 22621+ 可用**（`window.rs:1555` 注释明写）→ Win10 无 Mica
2. **Mica 采样桌面壁纸，不是窗口后方内容** → 语义上与 macOS vibrancy 完全不同。若设计意图是「透过面板看到后面的 app」，Win11 + Mica 会得到壁纸

### 4.3 文字渲染必然不同

- `TextRenderingMode` 枚举（`platform.rs`）：`PlatformDefault` / `Subpixel`（ClearType 风格）/ `Grayscale` —— 平台不对称写进 API 了
- Windows 有 `color_text_raster.hlsl` + `alpha_correction.hlsl` → 次像素/彩色栅格化
- macOS 自 Mojave 起系统层面关闭次像素 AA → 灰度
- **字体不同**：`PingFang SC` vs `Microsoft YaHei`。字形宽度、行高、字重插值均不同
- Buddy 是 100% 中文 UI → **这是最显眼的差异**

### 4.4 窗口外壳：macOS 富，Windows 薄（最危险的静默失效点）

`WindowOptions`（`platform.rs:1977`）**没有**这些字段：
`always_on_top`、`visible_on_all_workspaces`、`skip_taskbar`、`accepts_first_mouse`、`shadow`、非激活面板标志。
`window_decorations` 文档明确：仅 X11 / Wayland 有效。

**macOS 后端**（`gpui_macos/src/window.rs`）：
```
build_window_class("GPUIPanel", class!(NSPanel))      // :138   真 NSPanel 子类
NSWindowStyleMaskNonactivatingPanel                    // :90/:1018 非激活面板
NSFloatingWindowLevel = 3                              // :96/:1201 置顶层级
NSWindowCollectionBehaviorCanJoinAllSpaces             // :1236-1238 全工作区
  | FullScreenAuxiliary
canBecomeKeyWindow 覆写                                 // :448
NSVisualEffectView                                     // :16
```

**Windows 后端**（`gpui_windows/src/window.rs`）：
```
WindowKind::AnchoredPopup → :424 处理
WindowKind::Dialog       → :443 / :483  (WS_EX_DLGMODALFRAME | WS_EX_APPWINDOW)
WindowKind::PopUp        → :471-484  (WS_EX_TOOLWINDOW | WS_EX_TOPMOST) + WS_POPUP | WS_CAPTION
WindowKind::Floating     → 全文件 0 次出现  ❌❌
```

> **`WindowKind::Floating` 在 Windows 后端没有分支，会静默落到 `Normal`。**
> macOS 上生效、Windows 上什么都不发生、且不报错。这是最难排查的一类 bug。

`WindowKind::PopUp` 是两边最接近的：macOS 给非激活面板 + popup level + 全工作区；Windows 给 `TOOLWINDOW`（跳过任务栏）+ `TOPMOST`（置顶）。可用，但语义不同。

`focus: false` 两边都支持（Windows 走 `SW_SHOWNOACTIVATE`，`window.rs:559-560`）✓

**仍需手写平台代码或 patch fork 的**：`skipTaskbar`、`shadow: false`、`acceptsFirstMouse`、`visibleOnAllWorkspaces`、非激活面板。

### 4.5 Windows 中文输入法：完整的 IMM32 实现

**修正**：初次 grep 只覆盖了已下载的部分文件，得出「无 IME」的错误结论。取全 `gpui_windows/src/events.rs` 后确认实现完整：

```
WM_IME_STARTCOMPOSITION  → handle_ime_position              // events.rs:156
WM_IME_COMPOSITION       → handle_ime_composition           // events.rs:157
  GCS_COMPSTR   组字串                                       // :747-748
  GCS_RESULTSTR / GCS_COMPSTR                                // :767
  GCS_CURSORPOS 光标位                                       // :1682
  GCS_COMPATTR  组字属性（preedit 下划线渲染用）              // :1686
ImmSetCompositionWindow / ImmSetCandidateWindow(CANDIDATEFORM) // :671/:682-684
ImmAssociateContextEx                                        // :705/:718
ImmGetCompositionStringW                                     // :1436/:1660-1693
WM_CHAR → handle_char_msg                                    // :155
```

macOS 侧为完整 `NSTextInputClient` 协议实现（`gpui_macos/src/window.rs:244-271`：`selectedRange` / `firstRectForCharacterRange:actualRange:` / `insertText:replacementRange:` / `setMarkedText:selectedRange:replacementRange:`）。

**残留风险**：Windows 用的是 **IMM32，不是 TSF**。中/日/韩可用且 Zed 在出货验证过，但新版及第三方中文输入法（搜狗、微信输入法等）在 TSF 模式下行为更好，IMM32 路径上候选窗错位 / 组字丢字属已知行业性问题。

---

## 5. 现有 Buddy 代码的复用实测

对 `src-tauri/src/` 全量 grep `tauri`：

| 目录 | 行数 | `tauri` 引用 | 处理 |
|------|------|-------------|------|
| `providers/` | 2826 | **0** | 原样移植 |
| `tools/` | ~5000 | **0** | 原样移植 |
| `models/` | ~1000 | **0** | 原样移植 |
| `streaming.rs` | 808 | 3 | 仅换事件发射 |
| `storage.rs` | 427 | 12 | 仅换路径 API |
| `commands.rs` | 1868 | 33 | **删除**（IPC 边界消失） |
| `window/positioning.rs` | 510 | 17 | 逻辑保留，改调用 |
| `tray.rs` | 156 | 5 | 重写 |
| `hotkey.rs` | 162 | 5 | 重写 |
| `lib.rs` | 149 | 16 | 重写 |

前端：`src/` 共 17506 行 TS/TSX + `global.css` 3478 行 —— **全部重写**。

### 现有前端已暴露的问题（迁移中应解决）

`src/pages/ChatPage.tsx:402`：
```tsx
{isHistoryVisible && visible.map((msg, i, arr) => {
```
配合 `hasMoreHistory` / `loadOlderMessages` 做**手动分页**，全仓无任何虚拟化（grep `virtual|react-window|virtua|overscan` 无结果）。

同文件注释已承认该问题：
> 之前 `useChatStore()` 无选择器，流式更新会让整页和所有历史消息重复渲染。

`src/utils/bottomFollow.ts` + `src/hooks/useSmoothWheelScroll.ts` 是手写的底部跟随与滚动，GPUI 的 `ListState(Bottom)` 原生覆盖。

---

## 6. 许可证分层（硬约束）

| 层 | crate | 许可 | 约束 |
|----|-------|------|------|
| 平台层 | `gpui` / `gpui_platform` / `gpui_macos` / `gpui_windows` / `gpui_wgpu` | Apache-2.0 | 无约束 |
| 界面层 | `buddy-ui`（含 zed `theme` / `ui` / vendored `markdown`） | **GPL-3.0-or-later** | 分发二进制须提供完整源码；patch 部分须单独以 GPL 释出 |
| 引擎层 | `buddy-engine`（providers / tools / models / storage / streaming） | **MIT / Apache-2.0** | **永久零 GPUI 依赖**，可闭源 |

### 接受 GPL 锁死的项

1. 整包 GPL-3.0-or-later，分发须给源码（已开源，无影响）
2. **永久无法上 Mac App Store**（GPL-3 与 Apple DRM/ToS 冲突）
3. 同一二进制内不可闭源增值 —— 故必须严格分层
4. 修改过的 GPL 代码（vendored `markdown.rs` 的 patch）须单独以 GPL 释出，无需开源整个 Buddy

---

## 7. 风险登记

| # | 风险 | 等级 | 缓解 / 状态 |
|---|------|------|------|
| R1 | **窗口外壳**：「无边框非激活浮动面板 + 全局热键 + tray + autostart」在 GPUI 无现成支持 | ~~高~~ → **已消除** | **S00-02 / S00-03 均通过，且不需要 fork**（见 §11 §12） |
| R2 | **Windows 的 `WindowKind::Floating` 静默 no-op** | **中**（原高） | 已确认 macOS 侧 `PopUp` 是正确取值（§11.1）；Windows 侧仍需 `S09-04` 真机验证 |
| R3 | **GPUI fork 维护成本** | ~~中高~~ → **已消除** | 窗口外壳（S00-02/S00-03）与外观（S00-04）**都不需要 fork**（§13.8）。若将来要做窗内卡片 blur 才需要 |
| R4 | **Windows 字体与文字渲染差异**（中文 UI 最显眼） | 中高 | 显式平台字体栈；`TextRenderingMode` 显式设 `Grayscale`；两套 metrics 断言（`S03-05` / `S09-03`） |
| R5 | ~~Win10 无 Mica；Mica 采样壁纸而非后方内容~~ | ~~中~~ → **已消除** | **不使用系统 backdrop**，差异不存在（§13.1）。`S09-02` 已删除 |
| R6 | **Windows IMM32 输入法长尾问题**（第三方中文输入法） | 中 | **macOS 侧已通过**（`S00-05`）；Windows 侧待真机实测搜狗 / 微信输入法（`S09-07`） |
| R7 | **GPL 传染失控**（engine 层被 UI 依赖污染） | 中 | CI 加断言：`buddy-engine` 依赖树不得出现 GPUI / GPL（`S01-04`） |
| R8 | **无法回退**（双平台并行导致进退两难） | 中 | 严格 macOS 先行；Windows 前保留 Tauri 分支（`S01-05`） |
| R9 | **既有设计资产丢失** | 低 | `docs/design/` 作为移植对照基准；令牌值逐一映射校验。**注**：`S00-04` 发现「设计意图被当成既成事实」（毛玻璃、`prototypes/`）——见 §13.1 |

> **Phase 00 结束后重评**（`S00-09`）：R1/R3/R5 已消除；R2 降级为中。

---

## 8. 待确认项（Phase 00 / 01 解决）

> 标记说明：[x] 已由 **S00-01** 实测确认；[ ] 仍待后续 spec。

- [x] **`gpui` 依赖来源定案**：crates.io 只有 `gpui`（0.2.2），`gpui_platform` / `gpui_macos` / `theme` / `ui` **均未发布**（`publish = false`）→ 全部走同一 zed git rev，不混用
- [x] **`theme` + `ui` 能否在干净 workspace 编译**：能。`cargo check` / `cargo build` 均 rc=0
- [x] **实际闭包**：**30 个 zed crate / 693 个包**（原 12 为下界，见 §10.4）
- [x] **工具链**：`1.95.0` 可安装并编译通过（`edition = "2024"`）
- [ ] 毛玻璃是否需要 patch GPUI fork → **S00-04**
- [ ] vendor `markdown.rs` 后 9 处 `ThemeSettings` 的具体替换方案 → **S04-01**（已被 §10.3 的 `ThemeSettingsProvider` 接缝大幅简化）
- [ ] Buddy 形态窗口在 macOS 上能否用 `WindowKind::PopUp` + 手写补充达成 → **S00-02**
- [ ] 全局热键 / tray / autostart 与 GPUI 事件循环的集成方式 → **S00-03**
- [ ] 现有 Rust 测试（`tools/file_tools/tests.rs` 等）移植成本 → **S02-09**
- [ ] `target/` 2.5 GB + git 裸库 652 MB 的体积影响 → **S01-02**（CI 缓存与开发者体验）

---

## 10. GPUI 接入的硬前置条件（由 S00-01 实测得出）

> 本节是 **S00-01** 的核心产出。三个条件缺任一即失败，且**不写代码发现不了**。
> 证据见 `docs/specs/phase-00/S00-01-extract-theme-ui.md` 的「证据」段。

### 10.1 `runtime_shaders` —— 否则构建失败，且必须是完整 Xcode

`crates/gpui_apple/build.rs` 有两条路径：

| 路径 | 行为 | 需要完整 Xcode？ |
|------|------|----------------|
| 默认（`not(runtime_shaders)`） | `xcrun -sdk macosx metal` 把 `src/shaders.metal` 编成 `.air`，再 `xcrun -sdk macosx metallib` 链接 | **是** |
| `runtime_shaders` feature | 仅将 cbindgen 生成头文件与 `.metal` 源码拼接，着色器交由 Metal 驱动**运行时**编译 | **否** |

缺失时的报错：

```
error: gpui_apple@0.1.0: metal shader compilation failed:
cannot execute tool 'metal' due to missing Metal Toolchain
```

**背景**：自 Xcode 16 起 Apple 把 Metal 编译器从 Xcode 基座中拆出，成为单独可下载组件 `MetalToolchain`（`xcodebuild -downloadComponent MetalToolchain`）。因此即使装了 Xcode，`metal` 也只是驱动存根。

**本次未安装 MetalToolchain、也未执行 `sudo xcodebuild -runFirstLaunch`，构建与运行均正常**。→ 使用 `runtime_shaders` 可完全避开 Xcode 工具链。

> 注意：这只解决**构建期**。`S08-08` 的签名与公证仍会用到 Xcode；届时需先修好 Xcode 的首次启动组件（`CoreSimulator.framework` 缺失问题）。

转发链：`gpui_platform` → `gpui_macos` → `gpui_apple`。Comet 也使用此 feature。

### 10.2 `font-kit` —— 否则完全没有文字（静默）

缺失时 `gpui_macos` 仅打一条 WARN 就**静默退化**：背景与组件照常绘制，**唯独没有任何字形**。

```
WARN gpui_macos::platform] gpui_macos was compiled without the `font-kit` feature,
                          so no text will be rendered.
```

**这是最难排查的一类故障**：窗口开得出来、按钮看得见、进程不报错，只有文字消失。现象容易被误判为「主题文字色与背景同色」或「字体解析失败」。

排查手段：`RUST_LOG=debug` 看 gpui 自身的日志。**任何 GPUI 显示异常都应先开日志确认。**

转发链：`gpui_platform` → `gpui_macos/font-kit` → `dep:zed-font-kit`（zed 的 `font-kit` fork，见 `crates/gpui/Cargo.toml:126`）。

### 10.3 `ThemeSettingsProvider` —— 必须自行实现并安装（同时是避坑接缝）

只用 `theme::init()` 后开窗会 panic：

```
no state of type theme::theme_settings_provider::GlobalThemeSettingsProvider exists
  at gpui/src/app.rs:2030
```

调用方在 `crates/ui`（`styles/spacing.rs:53` 的 `ui_density`、`styles/typography.rs`、`components/label/*`、`utils.rs:30` 等）。

**但 zed 已经预留了接缝**（`crates/theme/src/theme_settings_provider.rs`）：`ThemeSettingsProvider` 是一个只有 **5 个方法**的 trait，zed 官方的 `theme_settings` crate 只是它的一个实现。

```rust
pub trait ThemeSettingsProvider: Send + Sync + 'static {
    fn ui_font(&self, cx: &App) -> &Font;
    fn buffer_font(&self, cx: &App) -> &Font;
    fn ui_font_size(&self, cx: &App) -> Pixels;
    fn buffer_font_size(&self, cx: &App) -> Pixels;
    fn ui_density(&self, cx: &App) -> UiDensity;
}
```

通过 `theme::set_theme_settings_provider(Box::new(impl_), cx)` 安装即可（约 30 行）。

**架构收益（重要）**：自行实现后**完全不需要 `theme_settings` crate**，从而避开：

```
settings → settings_content → settings_json → settings_macros → settings_migrator
        → watch → release_channel   …（见 §2 的 40+ 闭包）
```

这比 **S04-01** 原计划的「vendor `markdown.rs` 后手工 patch 9 处 `ThemeSettings`」更干净：**不改 zed 源码，只换 provider 实现**。

### 10.4 依赖闭包实测值（取代 §2 的下界估计）

| 项 | §2 估计 | **实测** |
|----|--------|---------|
| zed crate 数（含 `theme` + `ui`） | 12（下界） | **30** |
| 总包数 | 未估 | **693** |
| `~/.cargo/git/db/zed-*` | — | 652 MB |
| `target/`（debug 全量） | — | 2.5 GB |

30 个 zed crate 清单：

```
collections component derive_refineable gpui gpui_apple gpui_linux gpui_macos
gpui_macros gpui_platform gpui_shared_string gpui_util gpui_web gpui_wgpu
gpui_windows http_client icons media menu perf refineable scheduler sum_tree
syntax_theme theme ui ui_macros util_macros zlog ztracing ztracing_macro
```

> `gpui_linux` / `gpui_windows` / `gpui_web` 出现在 `Cargo.lock` 中属正常（lock 与平台无关），macOS 上不会编译。

**`S01-02` 的 fork 决策与 `S04-03` 的闭包收敛目标应以 30 为基线。**

### 10.5 最小可用依赖声明（已实测）

```toml
# 四个 crate 必须同一 rev（否则类型不兼容）
gpui          = { git = "https://github.com/zed-industries/zed", rev = "290cbcb9cb6a5dcbe0060431a126ad19e743f2f4" }
gpui_platform = { git = "https://github.com/zed-industries/zed", rev = "290cbcb9cb6a5dcbe0060431a126ad19e743f2f4", features = ["runtime_shaders", "font-kit"] }
theme         = { git = "https://github.com/zed-industries/zed", rev = "290cbcb9cb6a5dcbe0060431a126ad19e743f2f4" }
ui            = { git = "https://github.com/zed-industries/zed", rev = "290cbcb9cb6a5dcbe0060431a126ad19e743f2f4" }
```

启动序列（缺一不可）：

```rust
application().run(|cx: &mut App| {
    theme::init(LoadThemes::JustBase, cx);                       // 装 Theme 全局
    theme::set_theme_settings_provider(Box::new(MyProvider), cx); // 否则开窗 panic
    cx.open_window(WindowOptions { … }, |_, cx| cx.new(|_| MyView)).unwrap();
    cx.activate(true);
});
```

### 10.6 常用 API 备忘（均已在源码核对）

| 需求 | 正确写法 | 错误写法 / 注意 |
|------|---------|----------------|
| 设置字号 | `.text_size(px(24.0))` | `.font_size()` **不存在**（`Div` 上无此方法） |
| 设置字体族 | `.font_family("Helvetica")` | — |
| 取主题 | `cx.theme()`（需 `use theme::ActiveTheme`） | `ActiveTheme` 仅 impl 给 `App`，但 `Context<T>: Deref<Target = App>`，故 render 内可用 |
| 取颜色 | `theme.colors().text` / `.element_background` / `.background` | — |
| 系统 UI 字体 | `font(".SystemUIFont")` | macOS 会映射为 `.AppleSystemUIFont` |
| 默认文字色 | `TextStyle::default().color` = **黑色** | 深色主题下**必须显式 `.text_color()`**，否则黑字深底 |
| 默认字号 | `rems(1.)`，`window.rem_size()` 默认 `px(16.)` | 可用 `window.set_rem_size()` 调整 |

---

## 9. 如何获取 zed 源码（复现配方）

> **本节是 S00-01 / S00-06 / S04-* 的前置条件。**
> 本文档所有带行号的结论基于下列 rev。临时探测目录会被清理，因此必须能自行重建。

### 9.1 探测基准

| 项 | 值 |
|----|-----|
| 仓库 | `https://github.com/zed-industries/zed` |
| 分支 | `main` |
| **rev** | `290cbcb9cb6a5dcbe0060431a126ad19e743f2f4` |
| 日期 | 2026-09-10 |
| gpui crate 版本 | `0.2.2` |
| 工具链 | `channel = "1.95.0"`，`edition = "2024"` |

**本文档的行号只对上述 rev 成立。** 跟随 `main` 后行号必然漂移，复查时需按内容搜索而非按行号。

### 9.2 为什么必须用 sparse checkout

zed 是超大型 monorepo，全量 clone 代价不可接受。本次探测使用 `blob:none` + 稀疏检出，**不下载文件内容，只按需拉取**。

### 9.3 复现步骤

```bash
# 1. 建空仓库并配置稀疏检出（只拉 crates/ 的目录结构）
rm -rf /tmp/zed-probe && mkdir /tmp/zed-probe && cd /tmp/zed-probe
git init -q .
git remote add origin https://github.com/zed-industries/zed
git config core.sparseCheckout true
git sparse-checkout init --cone
git sparse-checkout set crates

# 2. 只取目录树与提交对象，不取文件内容
git fetch --depth 1 --filter=blob:none origin main

# 3. 按需检出具体文件（每次会单独拉取该 blob，失败时重试即可）
git checkout -q FETCH_HEAD -- crates/markdown/Cargo.toml
git checkout -q FETCH_HEAD -- crates/theme/Cargo.toml
git checkout -q FETCH_HEAD -- crates/ui/Cargo.toml
git checkout -q FETCH_HEAD -- crates/markdown/src/markdown.rs
```

### 9.4 注意事项

> **⚠️ 最重要的一条：必须走代理。** 见下方「网络」行。

| 事项 | 说明 |
|------|------|
| **网络（硬前置）** | **github 直连不可用**。实测：不走代理时 `cargo fetch` 在 2400 s 超时，仅拉到 272 MB 裸库；走 `http://127.0.0.1:7890` 后 **865 秒完成**。本机代理由 Clash Verge 提供（端口 7890） |
| 代理环境变量 | `export http_proxy=http://127.0.0.1:7890 https_proxy=http://127.0.0.1:7890 CARGO_HTTP_PROXY=http://127.0.0.1:7890 CARGO_NET_GIT_FETCH_WITH_CLI=true` |
| cargo 已配 git-with-cli | `~/.cargo/config.toml` 已有 `net.git-fetch-with-cli = true` 与清华 crates.io 镜像，无需重复配置 |
| 正确性提示 | `raw.githubusercontent.com` 当时会超时，但 git 协议（经代理）可用。**不要用 `curl` 拉单个文件**，用 `git checkout` |
| 拉取失败 | `git checkout` 单文件可能因网络抖动失败，**循环重试 2-3 次**即可 |
| 不带 filter 的 checkout | `git checkout` 大范围路径（如整个 `crates/`）会尝试拉全部内容并超时；**必须指定到文件级** |
| 列文件清单 | `git ls-tree -r --name-only FETCH_HEAD` 不需下载 blob，可用来先确认路径存在（**写文档引用前应做这一步**，参见 `docs/specs/RULES.md` §11.1） |
| 实测体积 | zed 裸库 652 MB；`font-kit` fork 1.6 MB |
| 实测耗时 | `cargo fetch` 865 s（带代理）；首次 `cargo build` 含编译约 26 s（增量）+ 全量依赖编译 |

### 9.5 各 spec 需要哪些文件

| Spec | 需要的路径 |
|------|-----------|
| S00-01 | `crates/{gpui,gpui_platform,gpui_macos,gpui_windows,theme,ui}/Cargo.toml` |
| S00-06 / S04-01 | `crates/markdown/src/markdown.rs`、`crates/markdown/Cargo.toml` |
| S04-02 | `crates/{language,syntax_theme}/Cargo.toml` |
| S01-02 / S07-02 | `crates/gpui/src/platform.rs`、`crates/gpui/src/window.rs`、`crates/gpui_macos/src/window.rs` |
| S09-04 | `crates/gpui_windows/src/{window,events,platform}.rs` |

### 9.6 闭包分析脚本

用以计算 zed crate 传递闭包（本文档 §2 数据由此得出）。核心思路：

1. `git ls-tree -r --name-only FETCH_HEAD` 得到全部 `crates/*/Cargo.toml` 路径
2. 逐个 `git checkout` 需要的 Cargo.toml
3. 解析 `[dependencies]`（**排除 `[dev-dependencies]`**，否则闭包会虚高）
4. 以 `crates/*` 目录名为白名单做 BFS

> 脚本本体为一次性分析工具，未入库。若要重复运行，按上述四步重写即可（不超过 50 行 Python）。

### 9.7 Comet 参考仓库

| 项 | 值 |
|----|-----|
| 路径 | `~/Project/comet`（**本地仓库，非公开依赖**） |
| 版本 | `0.2.59` |
| 许可证 | **MIT**（可合法借鉴代码，需保留版权头） |
| 用途 | markdown 栈、syntax crate、composer IME、transcript 虚拟列表、自更新器的参考实现 |

> Comet 依赖自己的 gpui fork（`zeronsh/zui`，rev `07fd941ad72e7edc812fed317aab66adb69fa8cc`），**不是 zed 官方**。引用其代码时注意 API 可能存在差异。

#### 若本地没有 comet

**不会阻塞 Phase 00。** 各 Spike 的可行性不依赖 comet：

| Spec | 无 comet 时怎么办 |
|------|----------------|
| S00-01 | 不需要 comet（只用 zed + crates.io） |
| S00-02 / S00-03 / S00-04 | 不需要 comet；但要意识到**无参考实现**，这两个 Spike 的风险因此更高 |
| S00-05 | zed `crates/gpui/examples/input.rs` 即可覆盖基础 IME 验证 |
| S00-06 | zed 的 `markdown.rs` + `pulldown-cmark` 可以自行搭；comet 只是参考，不是必需 |
| S00-07 | zed 的 `examples/list_example.rs` 即可验证 `ListState` |

**受影响的是 Phase 04 与 05 的代码借鉴**（markdown 栈 4557 行、syntax 1354 行、composer 8768 行）。无 comet 时需自行实现这些，工作量上升，但技术路径已被验证。

> **注意**：本文档中所有「Comet 的做法是 ⋯」的结论**已经记录在下文与 `research-log.md` 各节**，即使拿不到 comet 源码也仍然可读。只有需要**抄代码**时才必须拿到仓库。

---

## 11. macOS 窗口能力实测（由 S00-02 得出）

> 产物代码：`docs/evidence/s00-02/window-patch.rs`（可直接取用）
> 完整证据：`docs/specs/phase-00/S00-02-floating-panel.md`

### 11.1 四种 `WindowKind` 的原生属性（实测 dump）

| kind | 实际类名 | styleMask | level | collectionBehavior | hidesOnDeactivate |
|------|---------|-----------|-------|--------------------|-------------------|
| `Normal` | `GPUIWindow` | `0xf` Titled \| Closable \| Miniaturizable \| Resizable | `0` Normal | `0x0` | false |
| **`PopUp`** | **`GPUIPanel`** | `0x8f` 同上 **+ NonactivatingPanel** | **`101` PopUp** | **`0x101` CanJoinAllSpaces \| FullScreenAuxiliary** | false |
| `Floating` | `GPUIPanel` | `0xf` | **`3` Floating** | **`0x0`** | true |
| `Dialog` | `GPUIPanel` | `0xf` | `0` Normal | `0x0` | true |

**`PopUp` 是唯一同时给出「非激活面板 + 全工作区 + 高层级」的取值。**
`Floating` **不给全工作区** —— 不要用它同时解决置顶与全工作区。

### 11.2 窗口外壳**不需要 fork GPUI**

S00-02 验证：全部需求可在运行时经 objc2 达成，未触及 gpui 源码。

| 需求 | 实现 |
|------|------|
| 零装饰 | objc2 `setStyleMask:` 去掉 Titled/Closable/Miniaturizable |
| 去系统阴影 | objc2 `setHasShadow:(NO)` |
| 窗口显隐 | objc2 `orderOut:` / `makeKeyAndOrderFront:` |
| 层级 / 全工作区 | objc2 `setLevel:` / `setCollectionBehavior:` |

**fork 的必要性只剩毛玻璃一项**（归 S00-04 判定）。若 S00-04 选窗内自绘方案，则**完全不需要 fork**。

### 11.3 三个必须知道的缺口

| # | 缺口 | 后果 |
|---|------|------|
| 1 | **`WindowOptions` 无法得到零装饰窗口** | `titlebar: Some(..)`（`default()` 的取值）→ `Closable\|Titled`；`titlebar: None` → 仍 `Titled\|FullSizeContentView`。且 `window_decorations` 在 `gpui_macos` **完全未实现**（grep 零命中）→ 硬约束 1 只能靠 objc2 |
| 2 | **gpui 没有窗口 hide/show** | 只有 `minimize_window()`，隐藏语义不符（会飞进 Dock）→ 必须 `orderOut:` |
| 3 | **gpui 无全局鼠标监听** | 无 `addGlobalMonitorForEvents` → 「点击外部」用 `observe_window_activation` + `isKeyWindow` |

### 11.4 gpui 已经提供、无需自己实现的（重要，避免重复劳动）

| 能力 | 位置 | 值 |
|------|------|-----|
| `acceptsFirstMouse:` | `gpui_macos/src/window.rs:3394` | **硬编码 `YES`** |
| `canBecomeKeyWindow` | 同 `:447` | 硬编码 `YES` → 非激活面板也能输入 |
| `canBecomeMainWindow` | 同 `:451` | 硬编码 `YES` |
| 完整 `NSTextInputClient` | 同 `:292-296` | `setMarkedText:` / `insertText:` / `firstRectForCharacterRange:` / `selectedRange` / `markedRange` / `hasMarkedText` / `doCommandBySelector:` … |
| 透明窗 | `WindowBackgroundAppearance::Transparent` | `isOpaque` 自动变 `false`，**不需要 `setOpaque:`** |

> 原 `07-shell.md` 对照表把 `acceptFirstMouse: true` 标为「需手写」——**已推翻，无需手写**。

### 11.5 关键 API 备忘

| 需求 | 写法 |
|------|------|
| 取原生窗口 | `window.window_handle()` → `RawWindowHandle::AppKit(ns_view)` → `[ns_view window]` |
| 判断窗口是否 key | `Window::is_window_active()`，macOS 实现为 `[native_window isKeyWindow]`（`:1808`） |
| 监听激活变化 | `cx.observe_window_activation(window, \|this, window, cx\| { window.is_window_active() })` |
| 强制 resignKey（测试用） | objc2 `[ns_window resignKeyWindow]`；gpui 自身也用它（`:3022-3038`） |

### 11.6 ⚠️ 未验证项

**「用户真实点击窗口外部」未能触发 `resignKey`**（S00-02 两次运行均为兜底强制触发）。
需在 `S07-04` 区分是「用户未点击」还是「`NonactivatingPanel` + `focus:false` 下 AppKit 不发该消息」。
若是后者，备选方案为自行注册 `NSEvent.addGlobalMonitorForEvents`。

**下游链路已全部验证**（resignKey → 观察者 → `orderOut` → 流式继续 → `orderFront` 恢复），
故这项属于「触发条件」收口，非能力缺失。

### 11.7 已实测通过的硬约束

| 硬约束 | 结果 | 证据 |
|--------|------|------|
| 1（零装饰） | ✅ PASS | styleMask `0x8b` → `0x88`，无 Titled/Closable/Miniaturizable |
| 6（切页尺寸不变） | ✅ PASS | 11 次观测 / 14 次切页，尺寸恒为 `560x120` |
| 7（关闭不中断流式） | ✅ PASS | 隐藏 8 秒期间计数从 65 → 90 |

**用户实机视觉确认**：无红绿灯 / 无标题栏 / 无边框 / 无阴影 / 内容正常 / 可拖拽改大小 —— 全部正确。
特别验证了「在可见窗口上运行时改 `styleMask` 未造成内容丢失」（Apple 文档对此有警告）。

---

## 12. 外壳三件套与 GPUI 的集成（由 S00-03 得出）

> 产物代码：`docs/evidence/s00-03/shell-integration.rs`（可直接取用）
> 完整证据：`docs/specs/phase-00/S00-03-hotkey-tray-autostart.md`

### 12.1 结论：三者与 GPUI **共用同一个事件循环**，无需另起

GPUI 的 macOS 后端跑 `NSApplication` 的 run loop（即 CFRunLoop）。Carbon 热键事件与
`NSMenu`/`NSStatusItem` 事件都由同一个 run loop 分发，**在 `application().run(..)` 闭包内
直接初始化即可**。

实测：热键送达 7 次、tray 菜单送达 3 次、GPUI 心跳同时推进至 90 tick，三者计数正确汇合。
**证伪条件（必须另起事件循环）未触发。**

### 12.2 `tray-icon` **不会**抢占 `NSApp` delegate（最大风险已排除）

`tray-icon` 创建 `NSStatusItem`，理论上可能调 `[NSApp setDelegate:]` 从而覆盖
`GPUIApplicationDelegate`、使 GPUI 事件系统失效。实测创建前后均为 `GPUIApplicationDelegate`。

### 12.3 `TrayIcon` **不是 `Send`/`Sync`** —— 必须活在主线程

```
error[E0277]: `Rc<RefCell<...TrayIcon>>` cannot be shared between threads safely
```

- **不能放 `static`**
- 推荐放进 **GPUI `Entity<T>`**（主线程独占，生存期可控）
- 次选 `thread_local!` + `RefCell<Option<TrayIcon>>`
- 相反，`GlobalHotKeyManager` 是 `Send + Sync`，可放 `static`，但**必须保持存活**（drop 即注销热键）

### 12.4 依赖版本（无分裂）

| crate | 版本 | objc2 要求 | 与 gpui |
|-------|------|-----------|---------|
| `global-hotkey` | 0.8 | `objc2 ^0.6` / `app-kit ^0.3` | ✅ 共用 `objc2 0.6.4` / `app-kit 0.3.2` |
| `tray-icon` | 0.24 | 同上 | ✅ |
| `auto-launch` | 0.6 | macOS **零系统依赖** | ✅ |

- 树里的 `objc2 0.5.2` / `app-kit 0.2.2` 来自 gpui 自己的 `accesskit_macos`，与本集成无关
- **`tao` / `winit` 未进入构建树**（dev-dependency）；`gtk` 在 `Cargo.lock` 有条目但 `cargo tree -i gtk` 为空 → 不在依赖图
- 闭包增量：**693 → 743（+50 包）**

### 12.5 autostart（macOS）

`auto-launch` 写/删 `~/Library/LaunchAgents/<app_name>.plist`，**无系统级依赖**。
必须 `set_use_launch_agent(true)`，否则走旧的 AppleScript Login Items 路径。

实测全流程通过：`enable()` → `is_enabled()==true` → plist 落盘且含 `ProgramArguments` →
`disable()` → `is_enabled()==false` → plist 删除。

### 12.6 两个实现注意事项

| 事项 | 说明 |
|------|------|
| 热键/tray 回调**在主线程但拿不到 `Context`** | 需 channel（`try_send` + GPUI 侧 `recv().await`）桥接。spike 用原子量 + 轮询，有最长一个轮询周期的延迟，**S07 应改用 channel** |
| `skipTaskbar` 的 macOS 等价物是**应用级** | 需把 `NSApplicationActivationPolicy` 从 `Regular`(0) 改为 `Accessory`(1)。实测 GPUI 设为 `Regular` 且 tray 创建后未变。归 `S07-11`，**不是窗口级设定** |

### 12.7 风险登记更新

| # | 风险 | 变化 |
|---|------|------|
| R1 | 窗口外壳（含 tray/热键冲突） | **降级**：S00-02 证明不需要 fork；S00-03 证明 tray 不抢 delegate、三者共用事件循环 |

---

## 13. 窗口外观实测与产品决策（由 S00-04 得出）

> 产物代码：`docs/evidence/s00-04/window-appearance.rs`
> 完整证据：`docs/specs/phase-00/S00-04-backdrop.md`

### 13.1 产品决策：不使用毛玻璃 / 半透明

> **最终外观：不透明实色面板 + 16px 圆角 + 无边框 + 无模糊。**

这不是降级 —— **v1 本来就是实色界面**：

| 来源 | 事实 |
|------|------|
| `src-tauri/src/platform/macos.rs:154` | 「Buddy 当前使用**实色白色界面**；**不启用原生 vibrancy**」 |
| `src-tauri/Cargo.toml:37` | `window-vibrancy` 已声明但**全仓零处使用** |
| `src/styles/global.css` | `backdrop-filter` 仅 2 行（同一处：图片下载按钮的小药丸） |
| v1 实际「玻璃」 | 半透明实色 + 边框 + 阴影 + **CALayer 16px 圆角** |

> `design-tokens.md` 的 `.surface-glass { backdrop-filter }` 是**设计意图，不是 v1 出货的样子** ——
> 与 `docs/design/prototypes/` 属同一类问题：**文档里的意图被当成了既成事实**。

### 13.2 最终外观配置（逐项实测）

| 项 | 值 | 实现 |
|----|-----|------|
| 窗口 kind | `WindowKind::PopUp` | gpui |
| 窗口背景 | `Transparent` | gpui —— **仅为圆角，非毛玻璃** |
| 零装饰 | 去 Titled/Closable/Miniaturizable | objc2 `setStyleMask:` |
| 阴影 | 关闭 | objc2 `setHasShadow:` |
| 圆角 | 16px | objc2 CALayer `setCornerRadius:` + `masksToBounds` |
| 面板填充 | **不透明** Theme 色值 | gpui `Styled::bg` |
| 面板边框 | **无** | — |
| 模糊 / vibrancy | **不用** | — |

`Transparent` 与「毛玻璃」无关：前者只让 `isOpaque=false` 以便圆角透出桌面；后者会额外添加 `NSVisualEffectView` 子视图。

### 13.3 ⚠️ 方法论：图层级探测**必须延迟复探**

```
[t=0]  图层树（1 个）: ["NSViewBackingLayer"]        ← ⚠️ 合成之前，会误判
[t≈1s] 图层树（16 个）: [..., "CABackdropLayer" ×2, ..., "CAChameleonLayer", "CAMetalLayer"]
       ✅ 发现 backdrop 层 → 模糊在渲染
```

若只在 `render()` 内首探，会得出**「macOS 26 打坏了模糊、需要 fork」的错误结论**。

**推论**：`S07-12`（窗口行为自检模式）、`S09-01`、`S09-04` 做任何图层/属性探测时都必须延迟复探并观察稳定性。

### 13.4 关于「macOS 26 打坏模糊」的传闻 —— 本机未复现

Comet 的 fork 注释称「macOS 26 stopped vending `CABackdropLayer` for Selection」。
在 **macOS 26.4 + zed rev `290cbcb`** 上，材质 `Selection`(4) 正常工作，产生 `CABackdropLayer` ×2。

同时实测 4 个材质均正常：`Selection`(4) / `UnderWindowBackground`(21) / `HUDWindow`(13) / `WindowBackground`(12)，
**无需 Comet 的 `UnderWindowBackground` 修法**。

已排除的其他可能：系统「降低透明度」关闭；窗口属性全对（`isOpaque=false`/`alphaValue=1`/`blendingMode=BehindWindow`/`state=Active`）；
gpui 的 `remove_layer_background` 只移除饱和度滤镜与 `CAChameleonLayer`，不碰模糊。

> 该传闻可能适用于更早的 26.x，或与 Comet fork 内其他改动相关。**将来若需模糊，请自行复测。**

### 13.5 窗内 backdrop blur 需 fork（但产品不需要）

基础 gpui **无「窗内卡片级 blur」原语**（全仓 `backdrop` 只有 Windows 专用的 `MicaBackdrop`/`MicaAltBackdrop` 枚举值）。
Comet 的 `frost.rs` 依赖其 fork 新增的 `BackdropBlur`。

→ 只有做窗内卡片模糊才需要 fork。**产品不需要，故不 fork。**

### 13.6 两个被否决的方案（避免重做）

| 方案 | 否决理由 |
|------|---------|
| `--glass-outline` 白边框（`rgba(255,255,255,0.19)`） | 实机观感为**「四边白光」**，用户明确不要。`design-tokens.md` 的该令牌**不得照抄** |
| 窗内卡片级 backdrop blur | 需 fork；产品无此需求 |

### 13.7 另一个方法教训：分项 Spike 的测试窗口不代表成品

S00-04 首轮**只打圆角补丁、忘了 S00-02 的零装饰补丁**，导致测试窗口顶部仍带红黄绿 —— 用户当即指出。

**教训**：分项 Spike 时若各次测试的窗口外壳不同，「分项看起来对」**不能累积成「成品对」**。
**最终验收必须用「外壳 + 外观」完整组合的窗口**（此要求已传给 `S10-03`）。

### 13.8 风险登记更新

| # | 风险 | 变化 |
|---|------|------|
| R3 | GPUI fork 维护成本 | **消除** —— 窗口外壳（S00-02/S00-03）与外观（S00-04）都不需要 fork |
| R5 | Win10 无 Mica / Mica 采样壁纸 | **消除** —— 不使用系统 backdrop，差异不存在 |
| R4 | Windows 字体与文字渲染差异 | 仍在（与玻璃无关的部分） |

---

## 14. 中文 IME 与多行文本输入实测（由 S00-05 得出）

> 产物代码：`docs/evidence/s00-05/ime-input.rs`（4 个缺陷的修法 + 多行实现）
> 完整证据：`docs/specs/phase-00/S00-05-ime.md`

### 14.1 核心结论：官方 `input.rs` 示例自带 4 个缺陷

**`crates/gpui/examples/input.rs` 只能当骨架，不能当 IME / 多行正确性参考。**

| # | 位置 | 缺陷 | 后果 |
|---|------|------|------|
| 1 | `character_index_for_point` | 断言 `last_layout.text == self.content` 未考虑 placeholder | **一装输入法就崩** |
| 2 | `replace_and_mark_text_in_range` | `+ range.end` 应为 `+ range.start` | 光标越界 → 候选窗偏移 |
| 3 | `TextElement::prepaint` | 整段 content 一次 `shape_line` | **多行输入 panic** |
| 4 | `replace_and_mark_text_in_range` | UTF-16 区间算术产出越界 `selected_range` | **组字中 Cmd+C/X panic** |

它显然只在「无 placeholder + 英文 + 单行」场景下被验证过。
**这解释了 Comet 为何要写 8768 行 `composer.rs`——不是过度设计。**

### 14.2 `shape_line` 拒绝换行 —— 多行输入必须换 API

`TextSystem::shape_line` 文档与 `debug_assert!`（`gpui/src/text_system.rs:420`）：

> Note that this method can only shape a single line of text.
> **It will panic if the text contains newlines.**
> If you need to shape multiple lines of text, use `Self::shape_text` instead.

**两条可用路径**：

| 方案 | 适用 | 说明 |
|------|------|------|
| 按 `\n` 拆**逻辑行** + 逐行 `shape_line` | 逐行渲染、允许水平滚动 | spike 采用，见产物 `MultiLine` |
| `shape_text` → `SmallVec<[WrappedLine; 1]>` | 需要**软换行**（长行自动折行） | `WrappedLine` 自带 `paint()`；高度需用 `WrappedLineLayout.wrap_boundaries` 算 |

> Buddy 的 Composer 若要长行自动折行，**必须用第二条**，且自动增高要按**视觉行数**而非 `\n` 个数算。

### 14.3 IME 候选窗定位的三个必要条件

| # | 条件 | 缺失后果 |
|---|------|---------|
| 1 | 实现 `bounds_for_range`（返回 range 的**屏幕矩形**） | 候选窗无锚点 |
| 2 | 光标/组字变化后调用 `Window::invalidate_character_coordinates()` | 候选窗不跟随。**官方示例漏了这一调用（0 次）** |
| 3 | 多行时 `bounds_for_range` 必须同时算**行索引** | 候选窗永远停在第一行 |

### 14.4 Enter 三态由 gpui 平台层保证（前提是 `marked_text_range()` 正确）

`gpui_macos/src/window.rs:2635-2695`：

```rust
let is_composing = input_handler.marked_text_range().flatten().is_some();
if is_composing || is_ime_printable_key || (…) {
    let handled: BOOL = msg_send![input_context, handleEvent: native_event];
    if let Some(h) = do_command_handled.take() { return h as BOOL; }
    else if handled == YES { return YES; }   // ← IME 消费，不走应用 keyDown
    …
}
```

**推论（重要）**：若 `marked_text_range()` 返回 `None` 而实际正在组字，
`is_composing` 为 false → Enter 被当普通按键 → **组字中误发送**。

**所以 `marked_text_range()` 必须与 `marked_range` 字段严格同步。**

实测：组字期间按 Enter **0 次**到达应用 action。

### 14.5 一个隐蔽陷阱：渲染层 clamp 会掩盖语义层越界

实测 `selected_range.end` 可达 12 而 `content` 仅 10 字节。
渲染层（`MultiLine::locate`）与 `bounds_for_range` 的 clamp 让**视觉完全正常**，
但 `copy()` / `cut()` 的 `content[selected_range]` 会 **panic**。

> **教训**：当渲染层为容错做了 clamp 时，必须在语义层也做校验 ——
> 否则越界会被静默吸收，直到某个「非渲染」路径（剪贴板、序列化、索引）才炸。

⚠️ **上游根因未查清**：官方示例的 UTF-16 ↔ UTF-8 坐标系语义存在不一致
（`range_from_utf16` / `text_for_range` / `new_selected_range_utf16`）。
本 spike 只做防御性 clamp。**`S05-06` 需重新核对坐标系语义。**

### 14.6 实测通过的清单（macOS 26.4 / zed rev `290cbcb`）

| 项 | 结果 |
|----|------|
| 拼音组字 + preedit 下划线 | ✅ 34 次组字更新，`marked=Some(..)` 正确 |
| 中文提交 | ✅ `"你好"` / `"多少啊"` / `"就恢复的师傅几点开始阿富汗"` 等 8 次 |
| 候选窗跟随 | ✅ `characterIndexForPoint:` 实测被 IME 调用 |
| Enter 三态 | ✅ 组字中 **0 次**到达应用；普通 Enter 发送；Shift+Enter 换行 |
| 多行 + 自动增高 | ✅ Shift+Enter 5 次，行数 2→6，零 panic |
| 组字中 `Cmd+C` | ✅ clamp 后不崩 |
| panic 总数 | **0** |

**结论：GPUI 能支撑 Buddy 的中文多行 Composer，但应用层必须自己处理上述边界。**

---

## 15. zed markdown 栈的 vendor + patch 配方（由 S00-06 得出）

> 完整操作手册：`docs/evidence/s00-06/README.md`
> 产物代码：`docs/evidence/s00-06/{theme_settings_shim,language_stub,mermaid}.rs`
> 完整证据：`docs/specs/phase-00/S00-06-markdown.md`

### 15.1 闭包收敛结果（**取代 §2 的 40+ 估计**）

| 指标 | 值 |
|------|-----|
| **总包数** | **718** |
| S00-01 的 GPUI 基线 | 693 |
| **markdown 栈净增** | **+25** |
| zed crate 数 | 31（基线 30） |

**已从闭包彻底消失**：`settings` / `settings_content` / `settings_json` / `settings_macros`
/ `migrator` / `watch` / `release_channel` / `lsp` / `rpc` / `fs` / `language` /
`language_core` / `tree-sitter` / `wasmtime` / `node_runtime` / `theme_settings` /
`mermaid_render` / `editor`。

### 15.2 阻碍级发现：必须复制 zed 的 `[patch.crates-io]`

```
error[E0599]: no function `adopt_raw_pid` found for struct `smol::async_process::Child`
```

zed 用自家 fork 补了该方法，而 **`[patch]` 不会传递给下游消费者**。
任何在 zed workspace 之外使用其内部 crate（如 `util`）的项目**必须自己复制**。

macOS markdown 场景需要 5 项：`tree-sitter-language` / `async-process` / `async-task` /
`notify` / `notify-types`（完整清单与未复制项的理由见 evidence §1）。

> **这是使用 zed 内部 crate 的持续维护成本**，归 `S01-02` 评估。

### 15.3 阻碍级发现：`language` **必须移除**（非可选优化）

```text
language → tree-sitter (zed fork) → wasmtime-c-api-impl → 需要 cmake      ← 构建失败
language → settings → settings_json → migrator                           ← settings 被拖回！
```

**第二条是关键**：即使 shim 掉 `ThemeSettings`，`language` 也会把 `settings` 重新拖进闭包。
所以「移除 `language`」是控制闭包的**必要条件**，不是性能优化。

**耦合面很小**：`markdown.rs` 13 处，其余 4 个 vendored 文件**各 0 处**。

### 15.4 关键技巧：用 API 兼容 shim，把 patch 从 21 处降到 1 行

原计划手工改 21 处 `ThemeSettings` 调用点。实际只改 **3 行 import**：

```diff
- use theme_settings::ThemeSettings;   →  use crate::theme_settings_shim::ThemeSettings;
- use settings::Settings as _;         →  （删除：shim 把 get_global 做成固有方法）
- use language::{…};                   →  use crate::language_stub::{…};
```

加 3 个 shim/stub（**491 行**）：

| 文件 | 行数 | 替掉 |
|------|------|------|
| `theme_settings_shim.rs` | 143 | zed settings 框架（21 处调用的 API 表面） |
| `language_stub.rs` | 248 | `language`（13 处调用） |
| `mermaid.rs` | 100 | 1836 行 mermaid + node/wasm 链路（90 处引用） |

**vendor 总量 12680 行**（6 个文件 + html 子模块），`markdown.rs` 只 patch 3 行。

### 15.5 三个 stub 的 API 陷阱（实现时勿踩）

| # | 陷阱 | 说明 |
|---|------|------|
| 1 | `HighlightId` 必须 `From<HighlightId> for usize` | 消费点是 `SyntaxTheme::get(impl Into<usize>)` |
| 2 | `Language::default_scope()` **不返回 `Option`** | 调用点是 `map(\|l\| l.default_scope())`；返回 Option 会得到 `Option<Option<..>>` |
| 3 | `Language::highlight_text_resolved()` **不返回 `Result`** | 调用点直接使用返回值 |
| 4 | `CharKind` 必须 **`Ord`** | 调用点用 `std::cmp::max(kind_a, kind_b)` |
| 5 | `extract_mermaid_diagrams` 的事件类型必须用 **`crate::parser::MarkdownEvent`** | 直接 `use pulldown_cmark::Event` 会因生命周期不一致而编译失败 |
| 6 | mermaid 结构体需 **`Debug`** | 被 `#[derive(Debug)]` 的父结构持有 |

### 15.6 实测通过的清单

| 项 | 结果 |
|----|------|
| 编译（lib + bin） | ✅ |
| 闭包收敛 | ✅ 718 包（+25） |
| 流式渲染 | ✅ 306 字符逐字喂入（每 16ms 1 字符），无崩溃 |
| 各 markdown 结构 | ✅ 标题/粗斜体/行内代码/删除线/列表/嵌套/代码块/表格/引用/分割线 |
| 行尾稳定性 | ✅ 用户确认「行尾正常」 |
| 自动跟尾（`ScrollHandle`） | ✅ 用户确认生效 —— **仅能力验证** |

### 15.7 代价与交接

| 已裁掉 | 交接 |
|--------|------|
| **语法高亮** | `S04-02` → 接 **Comet `crates/syntax`**（MIT，1354 行，纯 tree-sitter） |
| **双击选词精度** | `S04-02` → `CharClassifier` 换 tree-sitter 字符分类查询 |
| **mermaid** | 不实现（Buddy 无需求） |
| **增量重解析 / mend / veil** | `S04-04` / `S04-05` / `S04-06` → 对齐 Comet 架构 |

**未验证**：`ListState` 的跟尾（本 spec 用 `ScrollHandle`）；`S00-07` 必须单独验证
「变高行虚拟化 + 流式增长 + 用户上滑打断」。

---

## 16. `ListState` 虚拟列表实测（由 S00-07 得出）

> 产物代码：`docs/evidence/s00-07/list-integration.rs`（四个陷阱 + 推荐骨架）
> 完整证据：`docs/specs/phase-00/S00-07-list.md`

### 16.1 虚拟化成立：**3.55 行渲染/帧 vs 总行数 1005（0.35%）**

```
区间: 帧 9→339（+330）  行渲染 1031→2203（+1172）
平均每帧行渲染 = 3.55 次     行总数 = 1005     占比 = 0.35%  → ✅ PASS
```

每帧只渲染视口内 + overdraw 的约 4 行。

**这是相对 v1 的核心改进**：`src/pages/ChatPage.tsx:402` 是 `visible.map(...)` + 手动分页，
**全仓无任何虚拟化**，且该文件注释已承认「流式更新会让整页和所有历史消息重复渲染」。

### 16.2 跟尾机制：`FollowMode::Tail` + **显式 `scroll_to_end()`**

**两者都要，缺一不可**：

- `FollowMode::Tail` 只设置 `follow_state` 标志；`is_following_tail()` 只是报告它
- 实际把视口钉到底部的是 `scroll_to_end()`（`list.rs:603`）

参考实现：`agent_ui/src/conversation_view/thread_view.rs:1692`。

**打断与恢复是框架自动的**（`list.rs:940`：滚轮上滑 → `stop_following()`）：

```
[1s] at_end=Some(true)  following=true     ← 贴底
[3s] at_end=Some(false) following=false    ← 用户上滑 → 自动停
[5s] at_end=Some(true)  following=true     ← 回底 → 自动恢复
```

> **推论**：v1 的 `bottomFollow.ts`（贴底 + 70px 吸附带）**不必移植**。
> 但 Comet 的「贴底弹簧 + 前馈追踪」仍需自己做（框架只保证非弹簧式贴底）。

### 16.3 ⚠️ 四个实现陷阱（三个是踩坑得出，且两个互相掩盖）

| # | 陷阱 | 症状 | 正确做法 |
|---|------|------|---------|
| 1 | `list()` 未加 `flex_grow_1()` | **静默渲染 0 行**（不报错不警告） | 给 `List` 自己加 `.flex_grow_1()`；`thread_view.rs:6115` |
| 2 | 在 `set_scroll_handler` 回调内调 `ListState` 访问器 | `panic: RefCell already mutably borrowed`（`list.rs:485`） | 回调内**只用 `ListScrollEvent` 字段** |
| 3 | `ListAlignment::Bottom` + 行高未知 | `is_scrolled_to_end()` 返回 `None`，视口空白 | **必须 `measure_all()`** |
| 4 | 只设 `FollowMode::Tail` | `following=true` 但不滚动 | 追加后**显式 `scroll_to_end()`** |

**陷阱互相掩盖的机制（重要的验证方法论）**：
陷阱 1 让列表渲染 0 行 → 没有滚动事件 → **陷阱 2 的 panic 永不触发**。
**修复陷阱 1 之后才暴露陷阱 2。分步验证时必须预期「修好一个会暴露下一个」。**

### 16.4 `measure_all()` 是 Bottom 对齐的必要条件（含代价）

| 构造方式 | 首帧渲染行数 | `is_scrolled_to_end()` | 可见 |
|---------|------------|----------------------|------|
| `new(1000, Bottom, …)`（无测量） | 6 | `None` | ❌ |
| 增量 `splice`（无测量） | 126 | `None` | ❌ |
| **`measure_all()`** | **1000** | **`Some(true)`** | ✅ |
| `with_uniform_item_height()` | — | — | ✅（仅固定行高） |

**根因**（`list.rs:488`）：`if summary.has_unknown_height { return None; }`
行高来自 `ListItem::size_hint()`，而 `list()` 的闭包返回 `AnyElement`，
**无法提供 size hint**。

**代价**：
```
[measure_all] 1000 行耗时 167 ns        ← 只设标志
[首帧] 渲染 1000 行，耗时 249 ms         ← 真正的 O(n) 成本
```
100 条 ≈ 25ms（可忽略）；1000 条 = 249ms（一次性）。

### 16.5 与 v1 的对比

| 维度 | v1（React） | GPUI `ListState` |
|------|------------|-----------------|
| 虚拟化 | ❌ 无（手动分页） | ✅ 0.35% 行渲染/帧 |
| 跟尾 / 打断 / 恢复 | 自写 `bottomFollow.ts` | ✅ 框架自动 |
| 平滑滚动 | 自写 `useSmoothWheelScroll.ts`（183 行） | 框架原生 |
| 行高缓存 | 无 | `sum_tree`，偏移↔索引 O(log n) |
| 首帧成本 | 无（但全量渲染） | 249ms / 1000 行 |

**v1 的 `bottomFollow.ts` + `useSmoothWheelScroll.ts` + 手动分页可整体删除。**

### 16.6 未验证（交接）

| 项 | 交接 |
|----|------|
| **视口上方高度变化时的滚动锚点保持** | `S05-03`（本 spec 与 v1 都未完整处理） |
| 5000 行（首帧成本外推 ≈1.2s） | `S05-01` 需评估是否分块加载 |
| 内存占用 | `S10-05` |
| **与 markdown 行的协同** | `S05-01`（本 spike 用纯文本行） |
| 块粒度行 + 稳定 id | `S05-02` |
| `splice_focusable` 焦点管理 | 未验证 |
| 滚动 FPS 精确值 | 只测了渲染次数，未测 FPS |

---

## 17. 引擎层去 Tauri 化实测（由 S00-08 得出）

> 产物文档：`docs/evidence/s00-08/engine-integration.md`
> 完整证据：`docs/specs/phase-00/S00-08-engine-loop.md`

### 17.1 端到端跑通：真实 API → SSE → markdown

```
[config] model = MiniMax-M3  provider = MiniMax  base_url = https://api.minimaxi.com/v1
[done] full_text = 836 字符，thinking = 0 字符，tool_calls = 0，had_stream_error = false
[ui  ] 事件 = 113，文本增量 = 58（358 字符），思考增量 = 49（463 字符）
```

链路：**v1 真实配置 → `create_provider` → 真实 API 请求 → SSE → 113 事件 → vendored markdown 渲染**
用户确认「markdown 渲染正常」。

### 17.2 从 v1 移植引擎层的**全部改动 = 4 处**

| # | 文件 | 改动 |
|---|------|------|
| 1 | `streaming.rs` | `StreamEventEmitter`：`app: AppHandle` + `event_name` → `tx: UnboundedSender<StreamEvent>`（**唯一 Tauri 耦合点，3 处**） |
| 2 | `openai_compatible.rs` | edition 2021 → 2024 模式修正（1 处） |
| 3 | `Cargo.toml` | 补 `parking_lot` |
| 4 | `tools` | 只取 2 个类型（~40 行）；完整模块归 `S02-02` |

**engine 规模 5255 行，改动 4 处。**

### 17.3 IPC 层整体消失

v1：UI → `invoke()` → `commands.rs`（**1868 行**）→ provider
v2：UI → **直接调用** provider

### 17.4 ✅ 风险 R7（GPL 传染）**实证可消除**

```
gpui / tauri / zed_markdown / theme / ui  →  全部不在 engine 依赖树
```

engine 直接依赖：`reqwest` / `tokio` / `serde` / `serde_json` / `chrono` / `dom_query` /
`base64` / `parking_lot` / `futures-util` / `async-trait` / `thiserror` / `log`。

**分层不是设想，已验证可行。** `S01-04` 的 CI 断言有了实施基础。

### 17.5 ⚠️ 发现一：`Done.full_text` **含 ` thinking` 标签**

```
文本增量 358 + 思考增量 463 = 821
Done.full_text              = 836
                        差 = 15     ← ` thinking`(7) + ``(8) = 15
```

**根因**（`openai_compatible.rs:674`）：

```rust
full_response.push_str(delta);            // 累积**原始** delta
emitter.text_delta(content_index, delta); // 这里才经 InlineThinkParser 拆分
```

| 来源 | 内容 | 用途 |
|------|------|------|
| `Done.full_text` | 原始文本（**含 think 标签**） | 持久化 |
| `TextDelta` 累加 | 显示文本（已剥离） | 渲染 |
| `ThinkingDelta` 累加 | 思考文本 | `ThinkSection` |

> **`S05-08` / `S05-09` 不得把 `Done.full_text` 当显示文本用** —— 会出现字面 ` thinking` 标签。

### 17.6 ⚠️ 发现二：内联 think 字符数在 `StreamOutcome` 里拿不到

`StreamOutcome.thinking_text` 只统计**原生** reasoning 字段（DeepSeek `reasoning_content`），
**不含** `InlineThinkParser` 从 ` thinking` 标签提取的部分。

实测：`thinking_text = 0`，但事件流有 **49 次 `ThinkingDelta`、463 字符**。

→ 思考块的字符数/折叠状态必须在 **UI 侧累加事件**。

### 17.7 ⚠️ 发现三 / 四：GPUI ↔ tokio 边界的两条约束

**（a）tokio 任务要求 `Send`，不能用 GPUI 侧类型**

```
error: future cannot be sent between threads safely
  `Rc<std::cell::Cell<bool>>` is not `Send`
```

GPUI 是单线程模型（`Entity` / `Rc`），tokio 任务要求 `Send`。跨边界只能用 channel / `Task`。

**（b）spawn 的 future 需 `'static` 所有权**

```
error[E0597]: `config.providers` does not live long enough
```

需先克隆字段再 move 进 future。

> 这两条是 `S02-06`（tokio / GPUI 桥接）的核心约束。

### 17.8 顺带验证：v1 数据兼容零成本

直接读 `~/Library/Application Support/com.buddy.chat/config.json` 即拿到 v1 的
`MiniMax-M3` 模型与 `MiniMax` provider，**无需任何迁移**。

一致性来源：同一路径 + 复用 `AppConfig` 结构 + `ProviderConfig` / `ModelInfo` 未改动。
→ `S02-04` 可据此确认兼容零成本。

### 17.9 未覆盖（交接）

| 项 | 交接 |
|----|------|
| **取消生成（`cancel_rx`）** | `S02-05`（本 spec 只创建了 watch channel，未触发） |
| 工具调用链路 | `S02-02` / `S02-07`（本 spec 传空 `tools`） |
| providers 既有测试 | `S02-09`（本 spec 未跑） |
| `try_recv` 轮询 → `recv().await` | `S02-06` |
| 多轮对话 / 历史消息 | `S05-*` |
| 存储层 | `S02-04` |

---

## 18. Phase 00 结论汇总（由 S00-09 得出）

> 完整结论见 `docs/specs/phase-00/S00-09-decision.md`

### 18.1 决策：**Go**

两个硬性门槛（S00-02 / S00-03）全部通过 → 无 No-Go 触发条件。

### 18.2 风险登记最终状态

| # | 风险 | 原等级 | 复评 | 依据 |
|---|------|-------|------|------|
| R1 | 窗口外壳 | 高 | **消除** | S00-02/03 证明不需要 fork，tray 不抢 delegate |
| R2 | Windows `Floating` 静默 no-op | 高 | **中** | macOS 侧已定 `PopUp`；Windows 待 `S09-04` |
| R3 | GPUI fork 维护成本 | 中高 | **消除** | S00-04 定不 fork |
| R4 | Windows 字体/文字渲染差异 | 中高 | **中高（不变）** | 归 `S03-05` / `S09-03` |
| R5 | Win10 无 Mica / Mica 采样壁纸 | 中 | **消除** | 不用系统 backdrop；`S09-02` 已删 |
| R6 | Windows IMM32 输入法长尾 | 中 | **中** | macOS 侧已过；Windows 待 `S09-07` |
| R7 | GPL 传染失控 | 中 | **可消除（已实证）** | S00-08 验证 engine 依赖树 0 处 GPUI/Tauri/zed |
| R8 | 无法回退 | 中 | **中（不变）** | `S01-05` 建立退路 |
| R9 | 既有设计资产丢失 | 低 | **低，但暴露新问题类** | 见 §18.4 |

**9 项中 4 项消除、1 项降级。**

### 18.3 最终资源画像

| 项 | 值 |
|----|-----|
| 全栈闭包 | **770 包**（GPUI 基线 693，markdown +25，engine +52） |
| 毒性依赖残留 | **0** |
| 需要 fork | **否** |
| engine 层可复用代码 | **5255 行**（移植改动仅 4 处） |
| v1 前端将被替换 | 17506 行 TS/TSX + 3478 行 CSS |
| 已裁掉的能力 | 语法高亮（`S04-02` 用 Comet MIT 方案补） |

### 18.4 ⚠️ 一条需要延续的纪律

Phase 00 反复暴露**同一类问题**：**「文档里的意图被当成了既成事实」**。

| 实例 | 真相 |
|------|------|
| `docs/design/prototypes/` | AGENTS.md + CLAUDE.md + tasks/v1.0.0 层层引用 → **路径从未存在** |
| 「毛玻璃」 | AGENTS.md Design Philosophy 写着 → v1 `macos.rs:154` **明写不启用 vibrancy** |
| 「`ThemeSettings` 需手工 patch 9 处」 | 实际只需 1 行 import + shim（S00-06） |
| 「`acceptFirstMouse` 需手写」 | gpui 已硬编码返回 `YES`（S00-02） |
| 「`ThemeSettings` 闭包 12 个」 | 实为 30 个（S00-01） |

**规则已立于 `RULES.md` §11。Phase 01+ 首次接触任何文档结论时，应先验证再采用。**
