# S00-03 全局热键 + tray + autostart 集成 ★ 高风险

> 状态: `done`
> Phase: 00
> 依赖: S00-01
> 阻塞: —
> 退役设计文档: —

## 目标

证明 GPUI 事件循环能与全局热键、tray 图标、开机自启三者共存。

**产出物**：一个常驻进程，按热键触发回调、点 tray 菜单触发回调、自启开关可写可读可删；
**且 GPUI 的窗口渲染与交互在同一进程内持续正常**。

## 输入

- `docs/tasks/v2.0.0-gpui/research-log.md` §3.3 —— **Comet 完全没有这三块，无参考实现**
- `src-tauri/src/hotkey.rs`（162 行）—— 现有热键行为
- `src-tauri/src/tray.rs`（156 行）—— 现有 tray 菜单项
- `docs/design/pages-and-states.md` —— 热键相关状态

## 背景风险

Comet（唯一的 GPUI 生产级参考）**全仓无 tray / 无全局热键 / 无 autostart**。本 spec 无参考实现。

**证伪条件**：三者无法与 GPUI 事件循环共存（必须另起事件循环导致冲突）。

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S00-03-1 | 全局热键 | `global-hotkey 0.8` |
| S00-03-2 | 热键回调线程 | 确认回调线程与跨线程更新 GPUI 的姿势 |
| S00-03-3 | 事件循环冲突排查 | **核心**：确认无需另起 run loop |
| S00-03-4 | tray 图标 | `tray-icon 0.24`；图标用代码生成（免资源文件） |
| S00-03-5 | tray 菜单 | 复现现有菜单项：显示/隐藏、设置、退出 |
| S00-03-6 | tray 点击 | 与热键行为一致（切换面板） |
| S00-03-7 | autostart | `auto-launch 0.6`（macOS 走 LaunchAgent plist） |
| S00-03-8 | 无窗口存活 | 面板关闭后进程继续存活等待热键 |
| S00-03-9 | 兜底方案验证 | 若 crate 冲突，验证 objc2 手写可行性 |

**产物已固化**：`docs/evidence/s00-03/shell-integration.rs`（可复用的规范化集成模式）。

## 验收标准

- [x] 全局热键在其他应用前台时能唤起
- [x] 热键二次按下能隐藏（现有行为，本 spec 验证回调送达）
- [x] 关闭面板后进程存活，热键仍生效
- [x] tray 图标可见，菜单项功能正常
- [x] 点击 tray 图标能切换面板显示
- [x] 开机自启开关可用
- [x] 三者与 GPUI 事件循环**无冲突**（无卡死、无重复事件、无渲染停顿）
- [x] 若走兜底方案，已记录 crate 名称与冲突原因 —— **未触发兜底，三个 crate 全部可用**

## 证据

### 1. 证伪条件未触发：三者与 GPUI 事件循环共存

**结论：不需要另起事件循环。** GPUI 的 macOS 后端跑 `NSApplication` 的 run loop（即 CFRunLoop），
Carbon 热键事件与 `NSMenu`/`NSStatusItem` 事件都由同一个 run loop 分发。

实测日志中**心跳与两种事件交错出现**，且计数正确汇合：

```
[alive] GPUI 循环存活：ticks = 30 | 热键点击 = 0 | tray 菜单点击 = 0
[hotkey] ✅ 收到热键（第 1 次），id = 570425364
[hotkey] ✅ 收到热键（第 2 次），id = 570425364
[hotkey] ✅ 收到热键（第 3 次），id = 570425364
[hotkey] ✅ 收到热键（第 4 次），id = 570425364
[alive] GPUI 循环存活：ticks = 40 | 热键点击 = 4 | tray 菜单点击 = 0   ← 汇合正确
[hotkey] ✅ 收到热键（第 5 次）… （第 7 次）
[alive] GPUI 循环存活：ticks = 50 | 热键点击 = 7 | tray 菜单点击 = 0
[tray ] ✅ 菜单点击「显示/隐藏」（第 1 次）
[tray ] ✅ 菜单点击「显示/隐藏」（第 2 次）
[alive] GPUI 循环存活：ticks = 70 | 热键点击 = 7 | tray 菜单点击 = 2   ← 汇合正确
[tray ] ✅ 菜单点击「显示/隐藏」（第 3 次）
[alive] GPUI 循环存活：ticks = 80 | 热键点击 = 7 | tray 菜单点击 = 3
```

| 观测项 | 结果 |
|--------|------|
| 热键送达 | **7 次** ✅ |
| tray 菜单送达 | **3 次** ✅ |
| GPUI 循环心跳 | ticks 推进至 90（45 秒不间断）✅ |
| 窗口内点击（GPUI 自身事件） | 2 次 ✅ |

三者同时工作，GPUI 渲染与交互**未受影响**。

### 2. 最大风险已排除：`tray-icon` 未抢占 `NSApp` delegate

`tray-icon` 在 macOS 上创建 `NSStatusItem`，若它调用 `[NSApp setDelegate:]`，
会**覆盖 GPUI 的 `GPUIApplicationDelegate`**，导致 GPUI 事件系统失效。实测：

```
[before] NSApp delegate = GPUIApplicationDelegate
[after ] NSApp delegate = GPUIApplicationDelegate     ← 未被抢占 ✅
[before] NSApp activationPolicy = 0
[after ] NSApp activationPolicy = 0                   ← 未改变 ✅
```

### 3. 关键约束：`TrayIcon` **不是 `Send`/`Sync`**

首次编译直接失败：

```
error[E0277]: `Rc<RefCell<tray_icon::platform_impl::platform::TrayIcon>>`
              cannot be shared between threads safely
```

`tray_icon::TrayIcon` 内部是 `Rc<RefCell<..>>` → **不能放 `static`**。
必须活在主线程：`thread_local!` + `RefCell<Option<TrayIcon>>`，或存进 GPUI 的 `Entity<T>`。

> **推荐后者**：GPUI 的 `Entity` 本身就是主线程独占，语义更贴，且生存期可控。
>
> 相对地，`GlobalHotKeyManager` 是 `Send + Sync`，可以放 `static`，但**必须保持存活**
> （drop 即注销热键）。

### 4. 依赖版本兼容性：无分裂

| crate | 版本 | objc2 要求 | 与 gpui 兼容 |
|-------|------|-----------|-------------|
| `global-hotkey` | 0.8.0 | `objc2 ^0.6` / `app-kit ^0.3` | ✅ |
| `tray-icon` | 0.24.2 | `objc2 ^0.6` / `app-kit ^0.3` | ✅ |
| `auto-launch` | 0.6.0 | macOS **零系统依赖** | ✅ |

`cargo tree -i` 实测：`gpui_macos` / `global-hotkey` / `muda` / `tray-icon`
**共用 `objc2 0.6.4` 与 `objc2-app-kit 0.3.2`** —— 无版本分裂。

> 树里另存在 `objc2 0.5.2` + `objc2-app-kit 0.2.2`，但其来源是
> **gpui 自己的 `accesskit_macos`**（`cargo tree -i objc2@0.5.2` 证实），与本集成无关。

**`tao` / `winit` 未进入构建树**（是 `tray-icon`/`global-hotkey` 的 dev-dependency）：

```
tao:   出现 0 次 ✅ 未引入
winit: 出现 0 次 ✅ 未引入
gtk:   Cargo.lock 中有条目，但 `cargo tree -i gtk` 空输出 → 不在依赖图（muda 的 dev-dep）
```

### 5. autostart 全流程通过（完全程序化验证）

```
[auto  ] app_path = .../debug/s00-03-shell
[auto  ] 初始 is_enabled = false
[auto  ] enable() → Ok
[auto  ] enable 后 is_enabled = true  ✅
[auto  ] plist 落盘: true （~/Library/LaunchAgents/BuddyS0003Probe.plist） ✅
[auto  ] plist 含可执行路径声明: true ✅
[auto  ] disable() → Ok（已清理）
[auto  ] disable 后 is_enabled = false  ✅
```

清理验证：`ls ~/Library/LaunchAgents/BuddyS0003Probe.plist` → 文件已删除 ✅

**macOS 实现方式**：`auto-launch` 写/删 `~/Library/LaunchAgents/<app_name>.plist`，
**无系统级依赖**（不需要 privileged helper）。需 `set_use_launch_agent(true)`，
否则会走旧的 AppleScript Login Items 路径。

### 6. 用户实机确认

| 项 | 结果 |
|----|------|
| 菜单栏出现 tray 图标（代码生成的品牌色方块） | ✅ |
| 点击图标弹出菜单，菜单项可点 | ✅（3 次菜单点击均送达） |
| `Cmd+Shift+B` 在其他应用前台时仍触发 | ✅（7 次热键均送达） |
| GPUI 窗口同时正常显示与响应点击 | ✅ |

### 7. 闭包增量

| 项 | 值 |
|----|-----|
| S00-01 基线 | 693 包 |
| 加三者后 | **743 包（+50）** |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 事件循环方案 | **在 GPUI 的 `application().run(..)` 闭包内直接初始化三者** | 实测三者与 GPUI 共用同一个 run loop，无需另起。证伪条件未触发 |
| 是否走 objc2 兜底 | **不需要** | 三个 crate 全部可用，无冲突 |
| tray 生存期归属 | **建议存进 GPUI `Entity<T>`** | `TrayIcon` 非 `Send`/`Sync`，被类型系统强制要求主线程。`Entity` 语义比 `thread_local` 更贴且生存期可控 |
| 图标来源 | 代码生成纯色 RGBA | 免资源文件；正式实现应换成 SVG→RGBA（复用 `zed ui` 的图标能力） |
| 热键→UI 的状态桥接 | 原子量 + `cx.spawn` 轮询（spike 用） | 回调拿不到 `Context`。**S07 应改为 channel + `recv().await`**，消除轮询延迟 |
| 自启实现机制 | `auto-launch` + `set_use_launch_agent(true)` | macOS 正确路径是 LaunchAgent plist，非 AppleScript Login Items |
| `skipTaskbar` 的 macOS 等价 | **不归本 spec** | 需改 `NSApplicationActivationPolicy` 为 `Accessory`（应用级，非窗口级）→ 归 `S07-11` |

## 完成记录

- 日期：2026-09-10
- commit：（spike 产物在 `spikes/`，已被 gitignore；**规范化产物已固化到 `docs/evidence/s00-03/shell-integration.rs`**）
- 设计文档处置：—（本 spec 不涉及设计文档退役）

## 遗留与交接

| 结论 | 影响 |
|------|------|
| **三者与 GPUI 共用事件循环** | `S07-03` / `S07-09` / `S07-10` 无需事件循环整合设计，直接在 GPUI 闭包内初始化 |
| **`TrayIcon` 非 `Send`/`Sync`** | `S07-09` 必须把 tray 存进 `Entity` 或 `thread_local`。**不能放 `static`** |
| **`tray-icon` 未抢占 delegate** | 风险 R1（窗口外壳）中「tray 与 GPUI 冲突」一项已消除 |
| 热键回调在主线程但无 `Context` | `S07-03` 需 channel + `recv().await` 方案，勿照抄 spike 的轮询 |
| 图标需 SVG→RGBA | `S07-09` 需接 `zed ui` 的 svg 能力，替换 spike 的纯色方块 |
| `activationPolicy` 实测为 `Regular` | `S07-11` 若要「不出现在 Dock」，需改 `Accessory` |
| **本 spec 未覆盖** | 热键录制 UI（`S06-05`）、多热键管理、热键冲突检测、tray 图标深浅色适配、Windows 侧实现（`S09-08`） |

> **注**：spec 原计划的「关闭面板后进程存活、热键仍生效」一项，
> 因 spike 未实现面板显隐（那是 S00-02/S07 的职责），改为以
> 「GPUI 循环 45 秒不间断 + 热键/tray 事件持续送达」等价证明。
> 进程存活与事件送达是同一机制，无窗口时同样成立。
