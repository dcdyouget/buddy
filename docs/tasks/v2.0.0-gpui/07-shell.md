# Phase 07: 应用外壳与窗口行为

## 目标

复现 Buddy 的核心形态：**按全局热键 → 无边框浮动面板弹出 → 聊天 → 点走消失**。

这是**第二高风险区**，且 GPUI 无现成支持、Comet 也无参考实现（见 `research-log.md` §3.3 §4.4）。Phase 00 的 S02 / S03 必须先通过。

## 相关文档

- `docs/tasks/v2.0.0-gpui/00-spike.md` — S02 / S03 的结论是本 Phase 的输入
- `docs/tasks/v2.0.0-gpui/research-log.md` §4.4 — 窗口能力实测对照
- `docs/design/pages-and-states.md` — 窗口状态机（显示/隐藏/流式中）
- `AGENTS.md` — 硬约束 1、6、7
- `docs/release-workflow.md` — 签名与公证（本 Phase 只涉及窗口行为，发布相关归 Phase 08）

## 验收标准

- [ ] **无交通灯按钮 / 零装饰 / 完全无边框**（硬约束 1）
- [ ] 冷启动后热键唤起延迟可接受（需实测并记录）
- [ ] 热键二次按下可切换隐藏；Esc 关闭；点击外部关闭
- [ ] **关闭窗口不停止流式**（硬约束 7）
- [ ] 页面/状态切换时窗口尺寸不变（硬约束 6）
- [ ] 置顶行为正确：面板弹出后在其他窗口之上，但不长期霸占焦点
- [ ] tray 图标与菜单可用
- [ ] 开机自启可用
- [ ] 多显示器下在正确屏幕显示

## Tauri 配置映射对照表（核心）

现有 `src-tauri/tauri.conf.json` 的每一项：

| 现有配置 | GPUI 对应 | 实现路径 |
|---------|-----------|---------|
| `decorations: false` | `window_decorations`（**文档注明仅 X11/Wayland**） | 需手写或 patch fork |
| `transparent: true` | `window_background: Transparent` | 原生支持 ✅ |
| `shadow: false` | 无对应字段 | 需手写 `setHasShadow_(NO)` |
| `acceptFirstMouse: true` | 无对应字段 | 需手写 |
| `focus: false` | `WindowOptions::focus` | 原生支持 ✅ |
| `alwaysOnTop: false` → 唤起时置顶 | 仅 `WindowKind::Floating` 映射到 `NSFloatingWindowLevel` | 需手写 `setLevel_` |
| `visibleOnAllWorkspaces: true` | 仅 `WindowKind::PopUp` 映射到 `CanJoinAllSpaces` | 需手写 `setCollectionBehavior_` |
| `skipTaskbar: true` | Windows 侧 `WS_EX_TOOLWINDOW`；macOS 无对应 | 平台分支处理 |
| `resizable: true` | `is_resizable` | 原生支持 ✅ |
| `minWidth/minHeight` | `window_min_size` | 原生支持 ✅ |
| `center: false` | `WindowBounds` | 原生支持 ✅ |
| macOS 私有 API（透明窗） | — | 平台相关 |
| `window-vibrancy` crate | `WindowBackgroundAppearance::Blurred` | 见 Phase 00 S04 |

**实测依据**（`gpui_macos/src/window.rs`）：`GPUIPanel`（NSPanel 子类）:138、`NSWindowStyleMaskNonactivatingPanel`:90/:1018、`NSFloatingWindowLevel = 3`:96/:1201、`CanJoinAllSpaces | FullScreenAuxiliary`:1236-1238、`canBecomeKeyWindow` 覆写:448。

## 开发工作

### 7.1 窗口创建与形态

| ID | Task | Details |
|----|------|---------|
| D01 | 窗口工厂 | 统一创建入口，参数来自配置；避免散落 |
| D02 | 无装饰 | 依据 S02-2 / S02-3 结论：优先尝试 `WindowKind::PopUp`，不足则 objc2 手写 |
| D03 | 透明背景 | `window_background` 按 Phase 00 S04 决策（系统模糊 or 自绘 frost） |
| D04 | 去阴影 | 手写 `setHasShadow_(NO)` |
| D05 | `acceptsFirstMouse` | 手写，使面板未聚焦时首次点击即生效 |
| D06 | 窗口尺寸记忆 | 复现用户设定尺寸的持久化（硬约束 6 的前提） |
| D07 | 最小尺寸约束 | 复现 `minWidth: 360` / `minHeight: 60` |
| D08 | 内容驱动增高 | 从 60 到展开态的过渡需平滑，且不违反硬约束 6 |

### 7.2 显示 / 隐藏与焦点

| ID | Task | Details |
|----|------|---------|
| D09 | 全局热键唤起 | 依赖 S03-1 结论 |
| D10 | 热键切换隐藏 | 二次按下隐藏 |
| D11 | Esc 关闭 | 复现现有行为；**注意 Esc 不能中断流式**（硬约束 7） |
| D12 | 点击外部关闭 | macOS `windowDidResignKey` 等价；需区分「点外部」与「点 tray / 菜单」 |
| D13 | 关闭不中断流式 | 流式在 engine 侧继续，重开后内容/状态完整 |
| D14 | 置顶层级 | 唤起时置顶，失焦后是否需要降级需实测决策 |
| D15 | 全工作区可见 | `setCollectionBehavior_` 手写 |
| D16 | 不抢焦点场景 | 现有 `focus: false` 语义的保留（视产品需要） |
| D17 | 显示/隐藏动画 | 复现 `WindowEntrance.tsx`（178 行）的入场效果，用 `with_animation` |
| D18 | 减弱动效 | 系统开启「减弱动态效果」时关闭入场动画 |

### 7.3 多显示器与定位

| ID | Task | Details |
|----|------|---------|
| D19 | 复用定位逻辑 | 移植 `src-tauri/src/window/positioning.rs`（510 行）与 `geometry.rs`（168 行） |
| D20 | 坐标语义核对 | GPUI 的 `Pixels` / `Bounds` 与 Tauri 的坐标差异需逐处核对 |
| D21 | 活动屏幕判定 | 跟随鼠标所在屏幕或上次使用屏幕（现有行为需确认） |
| D22 | 屏幕变化响应 | 分辨率/缩放变化、显示器插拔时的重定位 |
| D23 | 显示器边缘吸附 | 现有几何逻辑中的边缘处理保留 |

### 7.4 窗口拖动

| ID | Task | Details |
|----|------|---------|
| D24 | 拖动区域 | 复现 `src/hooks/useDragHandle.ts`（171 行）的交互：仅特定区域可拖 |
| D25 | 拖动实现 | GPUI `start_window_move()`；注意 `app_owns_titlebar_drag` 选项 |
| D26 | 拖动与文本选择隔离 | 拖动区域不得干扰消息选择 |

### 7.5 Tray 与自启

| ID | Task | Details |
|----|------|---------|
| D27 | tray 图标 | 依赖 S03-3；含深浅色主题下的图标适配 |
| D28 | tray 菜单 | 移植 `src-tauri/src/tray.rs`（156 行）菜单项：显示/隐藏、设置、退出 |
| D29 | 点击 tray 切换面板 | 与 D10 行为一致 |
| D30 | 退出清理 | 退出时正确停止流式、flush 存储 |
| D31 | 开机自启 | 依赖 S03-5；复现现有 `tauri-plugin-autostart` 行为与设置项联动 |
| D32 | 事件循环整合 | **关键**：确认 tray / 热键 / GPUI 共用同一 run loop，无冲突 |

### 7.6 事件循环与生命周期

| ID | Task | Details |
|----|------|---------|
| D33 | 单实例 | 防止重复启动多个 Buddy 实例 |
| D34 | 无窗口时存活 | 面板关闭后进程继续存活（等待热键），tray 常驻 |
| D35 | 激活策略 | macOS `ActivationPolicy` 设定（不占用 Dock 或占用，需决策） |
| D36 | 休眠/唤醒 | 唤醒后热键仍生效、窗口仍在正确位置 |
| D37 | 崩溃隔离 | 流式/网络异常不得导致外壳崩溃 |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T01 | 硬约束断言 | 逐条核对硬约束 1 / 6 / 7，含录屏证据 |
| T02 | 焦点行为测试 | 唤起/隐藏/失焦/重新唤起，焦点归属正确 |
| T03 | 多显示器测试 | 单屏 / 双屏 / 不同缩放 / 外接切换 |
| T04 | 关闭不断流测试 | 流式中关闭 → 等待 → 重开，内容完整 |
| T05 | 尺寸不变测试 | 所有页面与状态切换（硬约束 6） |
| T06 | 热键测试 | 唤起 / 隐藏 / 唤醒后 / 与其他应用热键共存 |
| T07 | tray 测试 | 菜单项全部可用，与面板状态同步 |
| T08 | 自启测试 | 开启后重启系统，验证自动启动且不弹窗 |
| T09 | 冷启动耗时 | 测量从进程启动到可响应热键的时间 |
| T10 | 常驻资源 | 空闲时的内存与 CPU 占用 |

## 备注

本 Phase 的风险集中在 D02 / D03 / D12 / D15 / D32。若 Phase 00 的 S02 / S03 通过，这些即为「已知可行但需手写」；若未通过，本 Phase 不应启动。

预估需要 patch GPUI fork 的项：无装饰 + 去阴影 + `acceptsFirstMouse`（若 `WindowKind` 不足）。patch 需集中管理并记录 rebase 说明（风险 R3）。
