# Phase 03: 主题与设计令牌

## 目标

把 `src/styles/global.css`（3478 行）的 CSS 变量体系映射为 Rust `Theme` 结构。

**设计工作不重做，只换编码方式。** `docs/design/design-tokens.md` 是唯一真值来源，逐值映射并校验。

## 相关文档

- `docs/design/design-tokens.md` — 色彩、字体、间距、阴影（**唯一真值来源**）
- `docs/specs/phase-01/S01-06-v1-baseline.md` — v1 实机视觉基线（视觉回归对照，替代已确认不存在的 `prototypes/`）
- `docs/design/pages-and-states.md` — 页面与状态（主题需覆盖的状态色）
- `docs/CONVENTIONS.md` — 「ALWAYS use CSS variables」规则的 Rust 等价物
- `AGENTS.md` — 硬约束 2（单一品牌色）、3（圆角刻度 4/8/12/16/9999）

## 验收标准

- [ ] `docs/design/design-tokens.md` 中每一个令牌都有对应的 Rust 字段，且值逐一核对通过
- [ ] 品牌色 `#5B5FE9` 为唯一品牌色；状态色仅 success / warning / error / info
- [ ] 圆角仅出现 `4 / 8 / 12 / 16 / 9999`
- [ ] 浅色 / 深色两套主题可用
- [ ] 平台字体栈显式声明（macOS `PingFang SC` / Windows `Microsoft YaHei`）
- [ ] `TextRenderingMode` 显式设定并记录理由
- [ ] 无任何硬编码颜色/圆角/间距残留（可用 lint 或 grep 检查）

## 令牌映射清单

需从 CSS 逐项迁移的类别（对照 `global.css`）：

| 类别 | CSS 变量前缀（示例） | Rust 归属 |
|------|---------------------|----------|
| 品牌色 | 单一 `#5B5FE9` 及其衍生 | `Theme::brand` / `accent` |
| 状态色 | success / warning / error / info | `Theme::status_*` |
| 中性色阶 | 文字、边框、分隔线 | `Theme::text_*` / `border` / `hairline` |
| 玻璃 | 面板、卡片、输入框背景 | `Theme::glass` / `card_glass_bg` / `input_glass_bg` |
| 圆角 | 窗口 16px；控件 4/8/12/16/9999 | 常量 + CALayer |
| 阴影 | 卡片、浮层（窗口阴影已关） | `Theme::shadow_*` |
| 阴影 | 卡片、浮层 | `Theme::shadow_*` |
| 圆角 | `4/8/12/16/9999` | 常量 |
| 间距 | 设计刻度 | 常量 |
| 字体 | 字号、行高、字重 | `Typography` |
| 代码字体 | 等宽栈 | `Typography::mono` |
| 语法高亮 | 各 token 颜色 | `SyntaxPalette` |

## 开发工作

### 3.1 基础结构

| ID | Task | Details |
|----|------|---------|
| D01 | 建 `Theme` 结构 | `crates/ui/src/theme.rs`，字段按上表分类 |
| D02 | `Appearance` 枚举 | `Light` / `Dark`（含 `from_window(WindowAppearance)` 参考 Comet `theme.rs:230`） |
| D03 | 全局安装 | `Theme::install(appearance, cx)` → `cx.set_global(...)`；`ActiveTheme` trait 供 `cx.theme()` |
| D04 | 常量集中 | 圆角 / 间距 / 动效时长放常量模块，禁止散落魔数 |

### 3.2 逐值迁移

| ID | Task | Details |
|----|------|---------|
| D05 | 生成映射表 | 把 `global.css` 中每个变量列出，标注 → Rust 字段，形成对照表放本目录 `theme-mapping.md` |
| D06 | 迁移颜色 | 逐值转换，注意 CSS `rgba()` → GPUI `Hsla` 的 alpha 语义 |
| D07 | 迁移外观值 | 面板/卡片/输入框的**不透明填充色**、**圆角**与**阴影**。
> ⚠️ **不含毛玻璃**：产品已决定不用半透明与模糊（见 `S00-04`），因此无 blur 参数需标定。
> ⚠️ `--glass-outline` 白边框已实机否决（观感为「四边白光」），**不得迁移**。 |
| D08 | 迁移阴影 | GPUI 的 shadow 参数与 CSS `box-shadow` 语义不同，需逐个调参对照原型 |
| D09 | 迁移字体 | 字号、行高、字重全部显式；不要依赖「平台默认」 |
| D10 | 语法色板 | 若无现成色板，采用 zed `syntax_theme` 的既有主题并做品牌化调整 |

### 3.3 平台字体（风险 R4）

| ID | Task | Details |
|----|------|---------|
| D11 | 声明平台字体栈 | macOS: `PingFang SC`；Windows: `Microsoft YaHei`；附 fallback 链 |
| D12 | 等宽字体栈 | 代码块与工具输出的等宽字体，两平台分别声明 |
| D13 | `TextRenderingMode` 决策 | 依据 `research-log.md` §4.3：macOS 无次像素 AA，Windows 有。**显式设 `Grayscale` 可让两边更接近**，需实测后定 |
| D14 | 中文 metrics 标定 | 行高、字距、换行点在两平台分别核对，产出标定记录 |
| D15 | 字重语义对齐 | 同一个 `FontWeight` 在两平台视觉重量不同（`PingFang SC` vs `Microsoft YaHei`），逐个核对标题/正文/强调 |

### 3.4 主题切换

| ID | Task | Details |
|----|------|---------|
| D16 | 运行时切换 | 复现现有 `ThemeSetting.tsx` 的切换交互 |
| D17 | 模糊不丢 | **坑点**：GPUI macOS 后端在后台外观变化时会移除 `NSVisualEffectView`，须重新应用（参考 Comet `appearance.rs:260`） |
| D18 | 跟随系统 | 监听系统外观变化并同步 |
| D19 | 切换无闪烁 | 全窗口重绘不得闪白 |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T01 | 令牌完备性测试 | 断言 CSS 中每个令牌都有 Rust 对应项（脚本比对 `theme-mapping.md`） |
| T02 | 硬约束测试 | 断言圆角值集合 ⊆ {4, 8, 12, 16, 9999}；断言品牌色唯一 |
| T03 | 视觉回归 | 截图对比 v1 基线（`docs/evidence/v1-baseline/`，由 S01-06 采集） |
| T04 | 双平台对比截图 | 同一页面 macOS / Windows 各截图，记录差异清单 |
| T05 | 主题切换测试 | 浅/深切换后无残留旧值、无模糊丢失 |

## 备注

D13 / D14 / D15 是 Windows 差异的主要战场，但可在 macOS 阶段先建立机制、留出标定入口，实际标定放 Phase 09。

> **令牌真值来源注记**：`docs/design/design-tokens.md` 头部原引用 `buddy-design/colors_and_type.css`，经审计确认**该路径从未存在**。
> 仓库中曾有一个同名文件 `.design/animation-preview/colors_and_type.css`（Trae 工具生成的动效预览实验，前缀 `--ap-`、主色 `#5B8DEF`），**已删除**——其动效令牌与 `src/styles/global.css` 完全重复且后者更完整，无需迁移。
> 真实令牌来源：`docs/design/design-tokens.md` + `src/styles/global.css`。详见 `docs/specs/design-deletions.md`。
