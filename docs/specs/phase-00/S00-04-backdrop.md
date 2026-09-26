# S00-04 毛玻璃双路径验证

> 状态: `done`
> Phase: 00
> 依赖: S00-02
> 阻塞: —
> 退役设计文档: —

## 目标

验证两条毛玻璃路径，并决定主路径：**系统效果**（`WindowBackgroundAppearance::Blurred`）vs **窗内自绘**（frost）。

**产出物**：两条路的截屏对比 + 主路径决策 + 是否需要 patch GPUI fork 的结论。

## 输入

- `docs/tasks/v2.0.0-gpui/research-log.md` §3.2 §4.2 —— Comet `frost.rs` 与平台毛玻璃差异
- `~/Project/comet/crates/ui/src/frost.rs`（180 行）—— 自绘 backdrop blur 参考
- `~/Project/comet/crates/ui/src/theme.rs:975` —— `window_background_appearance()` 用法
- `~/Project/comet/crates/ui/src/appearance.rs:260` —— **坑点**：主题切换会移除 `NSVisualEffectView`
- `docs/design/design-tokens.md` —— 玻璃相关令牌基准
- 当前 v1 应用的实机观感（`docs/design/prototypes/` 经审计确认不存在，不可引用）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S00-04-1 | 路径 A：系统效果 | `window_background: Blurred` → macOS `NSVisualEffectView` |
| S00-04-2 | 路径 A 兼容性 | 确认与透明窗 + 无装饰是否冲突 |
| S00-04-3 | 路径 B：窗内自绘 | 移植 Comet `frost.rs` 思路 |
| S00-04-4 | 路径 B 的跨平台价值 | 自绘在 macOS / Windows / Linux 表现一致 |
| S00-04-5 | 截屏对比 | 用同一设计令牌值做基准 |
| S00-04-6 | 主题切换复测 | gpui 在后台外观变化时会移除 `NSVisualEffectView` |
| S00-04-7 | fork 需求判定 | 判断毛玻璃是否需要 patch fork |

## 验收标准

- [x] 路径 A 可用性与观感已记录
- [x] 路径 B 探明（**结论：需 fork，产品不采用**）
- [x] 两条路的差异清单已记录
- [x] 主路径已选定并写明理由
- [x] 主题切换后模糊不丢失 —— **不适用**（未使用系统模糊）
- [x] 「是否需要 patch fork」已有明确结论
- [x] 若需要 fork，patch 清单已列出 —— **结论：不需要 fork**

## 证据

### 0. 最重要的前置发现：**Buddy v1 并没有毛玻璃**

本 spec 的前提（「验证毛玻璃两条路径」）来自 `AGENTS.md` 的 Design Philosophy
与 `docs/design/design-tokens.md` 的 `.surface-glass` 段。但实测 v1 源码：

| 来源 | 原文 / 事实 |
|------|-----------|
| `src-tauri/src/platform/macos.rs:154` | 「Buddy 当前使用**实色白色界面**；**不启用原生 vibrancy**，避免页面切换中透明区域短暂露出 HUDWindow 的灰色底。」 |
| `src-tauri/Cargo.toml:37` | `window-vibrancy = "0.6"` 已声明，但**全仓 Rust 源码零处使用** |
| `src/styles/global.css` | `backdrop-filter` 全仓**仅 2 行**（同一处：图片下载按钮的小药丸，`.generate-image-download`） |
| `src/styles/global.css` | glass 相关变量**仅 2 个**（`--glass-outline`，浅/深各一） |
| v1 实际「玻璃」 | 半透明实色（`color-mix(...transparent)`）+ 边框 + 阴影 + **CALayer 16px 圆角** |

> `design-tokens.md` 的 `.surface-glass { backdrop-filter: blur(...) saturate(180%) }`
> 是**设计意图，不是 v1 出货的样子**。

**这与 `docs/design/prototypes/` 是同一类问题**：文档里的意图被当成了既成事实，
并被下游文档逐层继承。（对比 `RULES.md` §11.1 的审计发现。）

### 1. 环境与系统设置（排除法）

| 项 | 值 |
|----|-----|
| macOS | **26.4**（25E246），arm64 |
| 系统「降低透明度」 | **关闭** ← 排除「辅助功能导致退化」 |
| 系统「增强对比度」 | 关闭 |
| 窗口 `isOpaque` | `false` ✅ |
| 窗口 `alphaValue` | `1` ✅ |
| `BlurredView.blendingMode` | `0`（BehindWindow）✅ |
| `BlurredView.state` | `1`（Active）✅ |
| `BlurredView.isEmphasized` | `false` ✅ |

### 2. 路径 A：系统效果 —— **可用**

`WindowBackgroundAppearance::Blurred` 由 gpui 实现为：创建一个 `NSVisualEffectView`
子类 `BlurredView`（`gpui_macos/src/window.rs:308`），以 `NSWindowBelow` 加入 contentView，
材质为 **`NSVisualEffectMaterial::Selection`(=4)**（同文件 `:3635`）。

**图层树实测（关键：必须延迟复探）**：

```
[t=0]  图层树类名（1 个）: ["NSViewBackingLayer"]
       ❌ 未发现 CABackdropLayer            ← ⚠️ 这是测量错误！
[t≈1s] 图层树类名（16 个）: ["NSViewBackingLayer", "CALayer", "CABackdropLayer",
        "CALayer", "CALayer", "CALayer", "CAChameleonLayer", "NSViewBackingLayer",
        "NSViewBackingLayer", "CALayer", "CABackdropLayer", "CALayer", "CALayer",
        "CALayer", "CAChameleonLayer", "CAMetalLayer"]
       ✅ 发现 backdrop 层: ["CABackdropLayer", "CABackdropLayer"] → 模糊在渲染
[t≈3s] 同上，稳定
```

> **⚠️ 教训（本 spec 最重要的方法论发现）**：
> 首次探测在 `render()` 内执行，早于首次合成，只看到 `NSViewBackingLayer`。
> 若不延迟复探，会**误判为「macOS 26 打坏了模糊，需要 fork」**。
> **任何图层级探测都必须延迟复探并观察稳定性。**

**运行时改材质的对照**（`setMaterial:`，回读确认生效）：

| 材质 | 值 | 图层树 | 模糊 |
|------|----|--------|------|
| `Selection`（gpui 默认） | 4 | `CABackdropLayer` ×2，共 16 层 | ✅ 渲染 |
| `UnderWindowBackground`（Comet 修法） | 21 | `CABackdropLayer` ×2，共 14 层 | ✅ 渲染 |
| `HUDWindow`（v1 注释提到的） | 13 | 同上 | ✅ 渲染 |
| `WindowBackground` | 12 | 同上 | ✅ 渲染 |

**关于「macOS 26 打坏模糊」的传闻**：Comet 的 fork 注释称
「macOS 26 stopped vending `CABackdropLayer` for Selection」。
**在 macOS 26.4 + 本 rev 上未复现**。该传闻可能适用于更早的 26.x，或与其 fork 内其他改动相关。

**gpui 的 `remove_layer_background` 不是原因**：它只移除**饱和度滤镜**（`colorSaturate`）
与 `CAChameleonLayer`（桌面染色），不碰模糊本身（`window.rs:3651-3705`）。

### 3. 路径 B：窗内自绘 —— **需要 fork，产品不采用**

**基础 gpui 没有「窗内 backdrop blur」原语。** 全仓搜索 `backdrop|Backdrop`：

```
gpui/src/platform.rs:2268  /// The Mica backdrop material, supported on Windows 11.   MicaBackdrop,
gpui/src/platform.rs:2270  /// The Mica Alt backdrop material, supported on Windows 11. MicaAltBackdrop,
```

仅有 Windows 专用枚举值，在 macOS 上是 no-op。macOS 侧只实现了 `Opaque` 与 `Blurred` 两值
（`gpui_macos/src/window.rs:1845-1860`）。

Comet 的 `frost.rs`（180 行）依赖其 fork 新增的 `BackdropBlur` 原语
（其 `Cargo.toml` 注释：`a6c1ad5 adds BackdropBlur + horizontal EdgeFade + glass fixes`）。

→ **要做事内卡片级模糊，必须 fork 给渲染器加原语。**

### 4. 产品决策：不透明 + 圆角，**不用毛玻璃**

用户明确：「**可以不要半透明的效果，不强求**」。且这**与 v1 实际行为一致**
（v1 就是实色界面 + 关闭 vibrancy），因此不是降级，而是**保持 v1 观感、降低迁移风险**。

### 5. 最终外观配置（逐项实测）

| 项 | 值 | 实现 | 实测结果 |
|----|-----|------|---------|
| 窗口 kind | `WindowKind::PopUp` | gpui `WindowOptions` | NSPanel + NonactivatingPanel ✅ |
| 窗口背景 | `Transparent` | gpui `WindowOptions` | `isOpaque=false` ✅（**仅为圆角，非毛玻璃**） |
| 零装饰 | 去 Titled/Closable/Miniaturizable | objc2 `setStyleMask:` | `0x8b → 0x88` ✅ |
| 阴影 | 关闭 | objc2 `setHasShadow:` | ✅ |
| 圆角 | **16px** | objc2 CALayer `setCornerRadius:` | `cornerRadius=16, masksToBounds=true` ✅ 与 v1 一致 |
| 面板填充 | **不透明** Theme 色值 | gpui `Styled::bg` | ✅ |
| 面板边框 | **无** | — | 见下 |
| 模糊 / vibrancy | **不使用** | — | — |

**用户实机确认**：「可以没问题」（无红黄绿、无白光、圆角正常、观感可接受）。

### 6. 两个被否决的方案（记录以免重做）

**(a) `--glass-outline` 白边框 —— 用户否决**

首轮测试用了 `rgba(0xFFFFFF28)` 的 2px 边框（模仿 v1 的 `--glass-outline`，
暗色模式值为 `rgba(255,255,255,0.19)`）。用户反馈：**「为什么四个边有白色的光？不要」**。

→ 面板**不设边框**。边界区分依靠背景色差（必要时可改回系统阴影）。

**(b) 窗内卡片级 backdrop blur —— 需 fork，产品不需要**

见 §3。

### 7. 一个测试搭建错误（方法教训）

首轮 S00-04 测试**只打了圆角补丁，忘了打 S00-02 的零装饰补丁**，
导致测试窗口顶部**仍带红黄绿交通灯** —— 用户指出：「注意最后的成品不要顶部的红黄绿」。

**教训**：分项 Spike 时，若各次测试的窗口外壳不同，
「分项看起来对」**不能累积成「成品对」**。
**最终验收必须用「外壳 + 外观」完整组合的窗口。**

### 8. 产物

`docs/evidence/s00-04/window-appearance.rs` —— 规范化最终外观配置，
含三路径对比存档与「必须延迟复探」的告警。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| **是否使用毛玻璃** | **不使用** | 用户明确「可以不要半透明的效果，不强求」。且 v1 本就是实色界面（`macos.rs:154`），保持一致可降低迁移风险 |
| 窗口背景取值 | `WindowBackgroundAppearance::Transparent` | **仅为让圆角能透出桌面**（否则是矩形硬边）。与毛玻璃无关 |
| 圆角实现 | objc2 CALayer `setCornerRadius: 16` + `masksToBounds` | 与 v1 完全一致（v1 用同一手法） |
| 面板填充 | 不透明 Theme 色值 | 用户要求不要半透明 |
| 面板边框 | **无** | `--glass-outline` 实机观感为「四边白光」，用户否决 |
| 阴影 | 关闭（沿用 S00-02） | 用户确认的观感。若深色背景下边界不清，可改为开启系统阴影 |
| **是否 patch GPUI fork** | **不需要** | 「不透明 + 圆角」全部可用 gpui 内置能力 + objc2 运行时补丁达成。基础 gpui 无窗内 blur 原语，但产品不需要该能力 |
| 路径 B（窗内自绘 blur） | 不采用 | 需 fork 给渲染器加 `BackdropBlur` 原语；产品无此需求 |
| 路径 A 的材质（若将来要用） | 保持 gpui 默认 `Selection` | 实测 4 个材质均正常渲染，无需 Comet 的 `UnderWindowBackground` 修法 |

## 完成记录

- 日期：2026-09-10
- commit：（spike 产物在 `spikes/`，已被 gitignore；**规范化产物已固化到 `docs/evidence/s00-04/window-appearance.rs`**）
- 设计文档处置：`docs/design/design-tokens.md` 的 `.surface-glass` 段已删除（见「遗留与交接」）

## 遗留与交接

### 连带完成的事项

| 事项 | 处理 |
|------|------|
| `AGENTS.md` Design Philosophy 的 "frosted glass" | **已删除**（产品不再要求毛玻璃） |
| `docs/design/design-tokens.md` 的 `.surface-glass { backdrop-filter }` 段 | **已删除**（按「代码即设计」原则，不留用不上的设计意图） |
| `research-log.md` §4.2（Windows 玻璃差异） | **已标注「不再适用」** |
| 风险登记 **R3**（GPUI fork 维护成本） | **消除** —— 外壳与外观都不需要 fork |
| 风险登记 **R5**（Win10 无 Mica / Mica 采样壁纸） | **消除** —— 不使用系统 backdrop，差异不存在 |
| `S09-02`（毛玻璃三选一与 Win10 回退） | **已删除** —— 前提消失。Phase 09 从 10 spec 减为 9 |
| `S03-03`（玻璃与阴影令牌迁移） | **简化** —— 从「玻璃令牌 + 分平台 blur 标定」改为「不透明色值 + 圆角 + 阴影」 |
| `S01-02` 的 fork 决策 | **结论：不 fork** |

### 交给后续 spec 的结论

| 结论 | 影响 |
|------|------|
| **完全不 fork GPUI** | `S01-02` 直接定案；GPUI 依赖走纯 zed git rev，无需维护 fork |
| `--glass-outline` 已否决 | `S03-03` 不得照抄 `design-tokens.md` 的该令牌 |
| 圆角 16px 由 CALayer 实现 | `S03-03` 只需定值，`S07-02` 实现 |
| 无边框 | `S03-03` / `S05-*` 不得引入描边 |
| 阴影当前关闭 | 若 `S10-03` 视觉验收发现深色背景下边界不清，再评估开启 |
| Windows 侧只需对齐圆角与阴影 | `S09-01` 范围收窄；**无 backdrop 差异需处理** |
| **图层探测必须延迟复探** | `S07-12`（窗口行为自检模式）必须遵守；`S09-01`/`S09-04` 同样适用 |
| **最终验收须用外壳+外观完整窗口** | `S10-03` 页面视觉验收的前置要求 |
