# Phase 09: 平台对齐（Windows）

## 目标

让 Windows 版本可用，并把两平台的视觉与行为差异收敛到可接受范围。

**启动前提：macOS 全链路验收通过。** 在此之前不启动，避免双平台并行导致无法回退（风险 R8）。

## 相关文档

- `docs/tasks/v2.0.0-gpui/research-log.md` §4 — Windows 实测（**必读**）
- `docs/tasks/v2.0.0-gpui/03-theme.md` D11-D15 — 字体与文字渲染标定入口
- `docs/tasks/v2.0.0-gpui/07-shell.md` — 窗口外壳
- `docs/tasks/v2.0.0-gpui/04-markdown.md` — IME 相关（S05）

## 核心事实（来自实测，决定本 Phase 的范围）

| 事实 | 影响 |
|------|------|
| Windows 是**独立的 Direct3D 11 + DirectWrite 渲染器**，非 Metal 的移植 | 抗锯齿、阴影、圆角裁切会有差异 |
| `WindowBackgroundAppearance::Blurred` → **Acrylic**（非 macOS 的 NSVisualEffectView） | 观感不同 |
| **Mica 采样桌面壁纸，不是窗口后方内容** | 与 macOS vibrancy 语义不同 |
| `DWMWA_SYSTEMBACKDROP_TYPE` **仅 Win11 build 22621+** | Win10 无 Mica |
| **`WindowKind::Floating` 在 Windows 后端 0 次出现，静默落为 `Normal`** | 风险 R2，必须验证 |
| Windows IME 用 **IMM32**（非 TSF），实现完整 | 第三方中文输入法长尾风险 |
| Comet **完全不支持 Windows**，无参考 | 无现成答案 |

## 验收标准

- [ ] Buddy 形态窗口在 Windows 上达成：无边框 + 透明 + 置顶 + 跳过任务栏
- [ ] `WindowKind::Floating` 的静默失效风险已排查并有明确处理
- [ ] 毛玻璃有明确方案与 Win10 / Win11 回退路径
- [ ] 中文输入法实测通过（含至少一个第三方输入法）
- [ ] 中文排版与 macOS 对比差异清单已记录，可接受项已确认
- [ ] 硬约束 1 / 6 / 7 在 Windows 上逐条验证
- [ ] 更新与安装流程在 Windows 上可用

## 开发工作

### 9.1 渲染与视觉对齐（风险 R4 / R5）

| ID | Task | Details |
|----|------|---------|
| D01 | 渲染器差异盘点 | 同一界面在双平台截图对比，逐项列出差异（阴影、抗锯齿、圆角、渐变） |
| D02 | 圆角处理 | 无边框窗在 Win11 由 DWM 默认加圆角，Win10 无；需统一策略 |
| D03 | 阴影 | 现无 `shadow` 选项；确认手写去掉系统阴影后是否有残留（如 DWM drop shadow） |
| D04 | 毛玻璃路径决策 | 三选一：<br>A. `Blurred` → Acrylic（模糊后方内容，最接近 macOS）<br>B. `MicaBackdrop`（Win11 独有，采样壁纸，语义不同）<br>C. **窗内自绘 frost（与 macOS 完全一致，推荐）** |
| D05 | Win10 回退 | Mica 不可用时的降级：不透明背景 / Acrylic / 自绘 |
| D06 | 系统版本探测 | 运行时检测 build 号，选择对应路径 |

### 9.2 字体与文字渲染（风险 R4）

| ID | Task | Details |
|----|------|---------|
| D07 | 字体栈切换 | `PingFang SC` → `Microsoft YaHei`，含 fallback 链 |
| D08 | 等宽字体 | 代码块字体在 Windows 的对应选择 |
| D09 | `TextRenderingMode` | 实测 `Grayscale` 与 `PlatformDefault` 的差异，选定并记录（目标：两平台尽量接近） |
| D10 | 行高标定 | 中文行高在两平台的实测值，修正 `Typography` 的平台差异 |
| D11 | 换行点标定 | 同一段文字两平台的换行位置，评估是否影响卡片高度与布局 |
| D12 | 字重视觉对齐 | `FontWeight` 在两平台的实际观感，必要时分平台指定 |
| D13 | 字号校正 | 若同一字号观感差异明显，考虑平台微调（需谨慎，避免设计令牌分裂） |

### 9.3 窗口行为（风险 R2）

| ID | Task | Details |
|----|------|---------|
| D14 | `WindowKind` 语义复核 | **逐项验证** `Normal` / `PopUp` / `Floating` / `Dialog` 在 Windows 的实际效果 |
| D15 | `Floating` 失效处理 | 确认是否 no-op；若是，改用 `PopUp` 或补 fork 分支（`WS_EX_TOOLWINDOW \| WS_EX_TOPMOST`） |
| D16 | 跳过任务栏 | 确认 `WS_EX_TOOLWINDOW` 生效，面板不在任务栏出现 |
| D17 | 置顶层级 | `WS_EX_TOPMOST` 行为；确认与「不长期霸占焦点」的平衡 |
| D18 | 点击外部关闭 | Windows 的失焦事件语义与 macOS 不同，重新验证（含点 tray 的误判） |
| D19 | Esc 关闭 | 键盘事件路径差异验证 |
| D20 | 关闭不中断流式 | 硬约束 7 在 Windows 重新验证 |
| D21 | 尺寸不变 | 硬约束 6 在 Windows 重新验证（DPI 变化场景） |
| D22 | 拖动窗口 | `start_window_move()` 在 Windows 的行为；裸 Win32 实现兜底 |
| D23 | DPI 与缩放 | 125% / 150% / 200% 缩放下的布局与定位 |
| D24 | 多显示器 | 双屏、不同 DPI、负坐标显示器 |
| D25 | 定位逻辑复用 | 移植 `window/positioning.rs` 在 Windows 的等价实现 |

### 9.4 输入法（风险 R6）

| ID | Task | Details |
|----|------|---------|
| D26 | 微软拼音 | 组字、候选窗、提交、光标定位 |
| D27 | 第三方输入法 | **搜狗 / 微信输入法** 实测（IMM32 路径的长尾风险区） |
| D28 | 候选窗定位 | `ImmSetCandidateWindow` 在窗口移动/滚动后的跟随 |
| D29 | Enter 语义 | 组字期间 Enter 只提交候选，不发送消息 |
| D30 | 组字与流式并存 | 流式进行中输入时的表现 |
| D31 | TSF 评估 | 若 IMM32 问题严重，评估引入 TSF 的代价（**可能需改 GPUI 源码**） |

### 9.5 外壳功能

| ID | Task | Details |
|----|------|---------|
| D32 | 全局热键 | `global-hotkey` 在 Windows 的行为；与 macOS 实现的差异 |
| D33 | tray 图标 | Windows system tray + 右键菜单 |
| D34 | 开机自启 | 注册表 Run 项 / 计划任务；与 macOS LaunchAgent 的差异 |
| D35 | 单实例 | Windows 的互斥机制 |
| D36 | 退出行为 | tray 退出、任务管理器结束的清理 |
| D37 | 暂停/休眠恢复 | 睡眠唤醒后热键与窗口状态 |

### 9.6 发布链路

| ID | Task | Details |
|----|------|---------|
| D38 | Windows 打包脚本 | Comet **缺失**此项，需自建（Phase 08 D17） |
| D39 | 安装器 | 对应 macOS DMG 的 Windows 安装体验 |
| D40 | 签名 | 现状确认 |
| D41 | 更新安装 | Phase 08 D09 的 Windows 路径验证（静默安装 + 重启） |
| D42 | 数据目录 | Windows 下与旧 Tauri 版本的数据目录一致性 |

### 9.7 差异登记

| ID | Task | Details |
|----|------|---------|
| D43 | 建立差异清单 | `platform-differences.md`：记录所有确认存在的双平台差异、是否接受、为何 |
| D44 | 接受标准 | 明确「哪些差异不可接受必须修」「哪些接受并记录」 |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T01 | 硬约束断言 | 硬约束 1-10 在 Windows 逐条核对 |
| T02 | 穷举窗口行为 | D14 的 `WindowKind` 四种取值实测记录 |
| T03 | 输入法矩阵 | 微软拼音 + 搜狗 + 微信输入法 × 组字/候选/提交 |
| T04 | DPI 矩阵 | 100% / 125% / 150% / 200% |
| T05 | 多显示器矩阵 | 单屏 / 双屏 / 混合 DPI / 负坐标 |
| T06 | 静默失效排查 | 专门检查「看似生效实则 no-op」的 API（如 `Floating`） |
| T07 | 双平台视觉对比 | 全部 7 个页面截图对比，产出差异报告 |
| T08 | 长时间运行 | 数小时常驻后的内存/句柄/热键可靠性 |
| T09 | 更新升级 | 从旧 Tauri 版升级到 GPUI 版 |

## 备注

本 Phase 的核心工作是**发现差异**而非实现功能。`research-log.md` §4 已预判了主要差异点，但必须在真机上逐项确认。

风险 R2（静默 no-op）值得特别标注：这类问题不会报错，只会「什么都不发生」，是最容易漏到生产环境的类型。建议写一个「窗口行为自检」调试模式，逐项打印每个窗口属性是否真的生效。
