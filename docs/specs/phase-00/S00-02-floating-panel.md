# S00-02 Buddy 形态浮动面板（macOS）★ 最高风险

> 状态: `done`
> Phase: 00
> 依赖: S00-01
> 阻塞: —
> 退役设计文档: —

## 目标

在 macOS 上复现 Buddy 的窗口形态：**无边框 + 透明 + 非激活 + 全工作区 + 点击外部关闭 + 尺寸保持**。

**产出物**：一个能弹出、点击外部消失、无任何窗口装饰的浮动面板，附逐项窗口属性探测结果。

## 输入

- `docs/tasks/v2.0.0-gpui/research-log.md` §4.4 —— macOS 后端能力实测（含行号）
- `docs/tasks/v2.0.0-gpui/07-shell.md` —— **Tauri 配置映射对照表**（逐项复现清单）
- `src-tauri/tauri.conf.json` —— 现有窗口配置（复现基准）
- `AGENTS.md` 硬约束 1、6、7

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S00-02-1 | 基线窗口 | `WindowKind::Normal`，560×120 透明无装饰 |
| S00-02-2 | 尝试 `PopUp` | 验证是否得到非激活 + popup level + 全工作区；确认可否输入 |
| S00-02-3 | 手写 objc2 补充 | `raw-window-handle` 拿 `NSWindow`，手动设 `setStyleMask:` / `setHasShadow:` / `setLevel:` / `setCollectionBehavior:` |
| S00-02-4 | 点击外部关闭 | `observe_window_activation` + `is_window_active()` → `orderOut:` |
| S00-02-5 | 尺寸保持 | 切页时窗口尺寸不变（硬约束 6） |
| S00-02-6 | 属性自检 | **逐项探测每个属性是否真的生效**（防静默 no-op），输出属性 dump |

**产物已固化**：`docs/evidence/s00-02/window-patch.rs`（可复用的规范化补丁代码，含全部常量与实测对照表）。

## 验收标准

- [x] 窗口无任何装饰 / 红绿灯（硬约束 1）
- [x] 背景完全透明，无残留白底
- [x] 无系统阴影
- [x] 点击外部关闭，且**不打断流式**（硬约束 7）—— 机制已验证，**自然点击触发待 S07-04 确认**（见「遗留与交接」）
- [x] 切页窗口尺寸不变（硬约束 6）
- [x] 置顶与全工作区行为符合预期
- [x] 每一项窗口属性都有**探测证据**（非目视推断）

## 证据

### 1. 四种 `WindowKind` 的原生属性对照（未打补丁，实测 dump）

| kind | 实际类名 | styleMask | level | collectionBehavior | hidesOnDeactivate |
|------|---------|-----------|-------|--------------------|-------------------|
| `Normal` | `GPUIWindow` | `0xf` Titled \| Closable \| Miniaturizable \| Resizable | `0` NSNormalWindowLevel | `0x0` | false |
| **`PopUp`** | **`GPUIPanel`** | `0x8f` 同上 **+ NonactivatingPanel** | **`101` NSPopUpWindowLevel** | **`0x101` CanJoinAllSpaces \| FullScreenAuxiliary** | false |
| `Floating` | `GPUIPanel` | `0xf` | **`3` NSFloatingWindowLevel** | **`0x0`（无全工作区）** | true |
| `Dialog` | `GPUIPanel` | `0xf` | `0` NSNormalWindowLevel | `0x0` | true |

**结论**：`PopUp` 是**唯一**同时提供「非激活面板 + 全工作区可见 + 高层级」的取值。
`Floating` **只给窗口层级，不给全工作区** —— 推翻了原计划「用 `Floating` 同时解决置顶与全工作区」。

### 2. 补丁前 → 补丁后（`PopUp`）

```
[before] styleMask : 0x8b = Titled | Closable | Resizable | NonactivatingPanel
[before] level     : 101 = NSPopUpWindowLevel
[before] hasShadow : true
[before] isOpaque  : false          ← 已透明，说明 window_background:Transparent 已处理

[after ] styleMask : 0x88 = Resizable | NonactivatingPanel     ✅ 零装饰
[after ] level     : 3 = NSFloatingWindowLevel                 ✅
[after ] hasShadow : false                                     ✅
[after ] isOpaque  : false                                     （未变化）
```

**所有补丁项均报告「✅ 生效」，`macOS` 侧未发现任何静默 no-op。**

**硬约束 1 判定：✅ PASS —— 无 Titled / Closable / Miniaturizable。**

### 3. 两个决定性的硬事实

**(a) `WindowOptions` 两条路都拿不到零装饰窗口**（`gpui_macos/src/window.rs:990-1007`）

| 配置 | 得到的 styleMask |
|------|-----------------|
| `titlebar: Some(..)`（`WindowOptions::default()` 的取值） | `Closable \| Titled`（+ Resizable / Miniaturizable） |
| `titlebar: None` | 仍是 `Titled \| FullSizeContentView` |

且 `window_decorations` 字段在 `gpui_macos` 中**完全没有实现**（grep 零命中）。
→ **硬约束 1 必须靠 objc2 覆盖 `styleMask` 才能满足**，无法通过配置达成。

**(b) gpui 没有窗口 hide/show API**

`Window` 只有 `minimize_window()`；`gpui_macos` 只实现了 `minimize`。
而 Buddy 的「点走消失」不能用最小化（会飞进 Dock）→ 必须手写 AppKit `orderOut:` / `makeKeyAndOrderFront:`。

### 4. GPUI 已经提供的（**无需自己实现，也无需 fork**）

| 能力 | 位置 | 值 |
|------|------|-----|
| `acceptsFirstMouse:` | `gpui_macos/src/window.rs:3394` | **硬编码返回 `YES`** → 满足 Buddy 的 `acceptFirstMouse: true` |
| `canBecomeKeyWindow` | 同文件 `:447` | 硬编码 `YES` → 非激活面板**也能输入文字**，IME 不受影响 |
| `canBecomeMainWindow` | 同文件 `:451` | 硬编码 `YES` |
| 完整 `NSTextInputClient` | 同文件 `:292-296` 等方法表 | `setMarkedText:selectedRange:replacementRange:` / `insertText:replacementRange:` / `firstRectForCharacterRange:actualRange:` / `selectedRange` / `markedRange` / `hasMarkedText` / `unmarkText` / `doCommandBySelector:` / `characterIndexForPoint:` / `validAttributesForMarkedText` / `attributedSubstringForProposedRange:actualRange:` |
| 透明窗 | `WindowBackgroundAppearance::Transparent` | 实测 `isOpaque` 自动变 `false`，**无需 `setOpaque:`** |

### 5. 硬约束 6（切页尺寸不变）：✅ PASS

```
尺寸观测次数 = 11，出现的不同尺寸 = {"560x120"}
```

14 次页面切换（另一轮运行），尺寸与原点**完全未变**。

### 6. 硬约束 7（关闭不中断流式）：✅ PASS

```
[activ] is_window_active = false（stream = 65）
[hide ] orderOut: ← 模拟「点走消失」
[stream] count = 70 | 隐藏时=65，已暗中推进 5
...
[show ] 隐藏已持续 8 秒，请求重新显示（stream = 90）
...
隐藏时 stream = 65，最终 stream = 128 → 隐藏期间推进 63 次   ✅ 硬约束 7 PASS
```

隐藏持续 8 秒期间，「流式」计数从 65 推进到 90（≈30 次，8s ÷ 300ms 吻合），
证明 `orderOut:` 之后进程与后台任务**继续运行**，窗口隐藏不影响流式。

### 7. 「点击外部关闭」的检测机制

`Context::observe_window_activation` + `Window::is_window_active()`。

`is_window_active()` 的 macOS 实现是 `[native_window isKeyWindow]`（`gpui_macos/src/window.rs:1808`），
即**窗口级** key 状态 —— 正是「点走消失」需要的语义。

实测：强制 `resignKeyWindow` → 观察者收到 `false` → 执行 `orderOut:` → 窗口隐藏 ✅
链路完整可用。

gpui **未提供**全局鼠标监听（无 `addGlobalMonitorForEvents`），故该机制是首选方案。

### 8. 用户实机视觉确认

窗口形态经用户逐项目视确认「**都正确**」：

| 项 | 结果 |
|----|------|
| 四个角无红/黄/绿交通灯按钮 | ✅ |
| 顶部无标题栏、无分隔线 | ✅ |
| 四边无边框、**无系统阴影** | ✅ |
| 内部深色背景 + 白字正常绘制（透明窗下内容未受影响） | ✅ |
| 边缘可拖拽改大小 | ✅ |

特别确认了 **`setStyleMask` 在可见窗口上运行时修改未造成内容丢失或重绘异常**
（Apple 文档对此有警告，属本次需实测的风险点）。

### 9. `acceptsFirstMouse` 结论（原计划需 fork，实测不需要）

原 `07-shell.md` 对照表把 `acceptFirstMouse: true` 标为「需手写」。
实测发现 **gpui 已实现且硬编码返回 `YES`** → **无需手写、无需 fork**。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 窗口 kind | **`WindowKind::PopUp`** | 唯一同时给出 NSPanel + `NonactivatingPanel` + `CanJoinAllSpaces` + `FullScreenAuxiliary` 的取值。`Floating` 缺全工作区，`Normal`/`Dialog` 无 `NonactivatingPanel` |
| 零装饰实现方式 | **objc2 运行时覆盖 `styleMask`** | `WindowOptions` 无任何配置路径可达；`window_decorations` 在 macOS 后端未实现 |
| 窗口显隐实现方式 | **objc2 `orderOut:` / `makeKeyAndOrderFront:`** | gpui 只有 `minimize_window()`，隐藏语义不符 |
| 窗口层级 | **`NSFloatingWindowLevel`(3)** 降级，而非 `PopUp` 给的 101 | 101 会压住菜单栏。3 足够「浮在普通窗口之上」。如需更强可改回 |
| 透明实现 | 仅靠 `window_background: Transparent`，**不调 `setOpaque:`** | 实测该选项已使 `isOpaque = false`，额外调用是冗余 |
| **是否 patch GPUI fork** | **本 spec 范围内：不需要** | 窗口外壳的全部需求都在运行时经 objc2 达成，未触及 gpui 源码。fork 的必要性只剩毛玻璃一项（**S00-04** 判定） |
| 「点击外部」检测方式 | `observe_window_activation` + `isKeyWindow` | gpui 无全局鼠标监听；该 API 语义正确且实测链路可用 |

## 完成记录

- 日期：2026-09-10
- commit：（spike 产物在 `spikes/`，已被 gitignore；**规范化产物已固化到 `docs/evidence/s00-02/window-patch.rs`**）
- 设计文档处置：—（本 spec 不涉及设计文档退役）

## 遗留与交接

### ⚠️ 一项未验证（需 S07-04 收口）

**「用户真实点击窗口外部」未能触发 `resignKey`。** 两次运行均由 20 秒兜底的
`force_resign_key` 触发隐藏路径（日志中 `触发方式 = Some("forced")`）。

两种可能，需在 `S07-04` 区分：
1. 测试期间用户未实际点击外部
2. 窗口为 `NonactivatingPanel` + `focus: false`，AppKit 未发送 `windowDidResignKey`

**若属第 2 种**，备选方案是在 `S07-04` 自行注册
`NSEvent.addGlobalMonitorForEvents(matching: NSEventMaskLeftMouseDown | ...)`，
在回调中判断点击位置是否落在窗口外。

> 该机制的**下游链路已全部验证**（resignKey → 观察者 → `orderOut` → 流式继续 → `orderFront` 恢复），
> 因此这一项属于「触发条件」的收口，而非「能力缺失」。

### 交给后续 spec 的结论

| 结论 | 影响 |
|------|------|
| **窗口外壳不需要 fork** | `S01-02` 的 fork 决策范围收窄为「仅毛玻璃」；若 `S00-04` 选定窗内自绘方案，则**完全不需要 fork** |
| `acceptsFirstMouse` / `canBecomeKeyWindow` 由 gpui 提供 | `S07-02` 不必实现这两项 |
| `WindowKind::Floating` 不给全工作区 | `S07-05` 必须用 `PopUp` 或手写 `setCollectionBehavior:` |
| 零装饰必须靠 `styleMask` 覆盖 | `S07-02` 的实现主体；代码见 `docs/evidence/s00-02/window-patch.rs` |
| 显隐必须靠 `orderOut:` / `orderFront:` | `S07-03`（热键切换）、`S07-04`（点走消失） |
| 多窗口/多显示器与定位 | 本 spec **未覆盖**，仍归 `S07-06` |
| `skipTaskbar`（macOS 等价物） | 本 spec **未覆盖**。macOS 侧需调整 `NSApplicationActivationPolicy`（应用级，非窗口级）→ 归 `S07-11` |
