# S05-16 空态与无 Key 态

> 状态: `done`
> Phase: 05
> 依赖: S05-06
> 阻塞: —
> 退役设计文档: —

## 目标

无消息时的空态页、未配置 Key 时的引导页，与 v1 一致。

## 输入

- v1 `src/pages/EmptyPage.tsx`、`NoApiKeyPage.tsx`

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S05-16-1 | 文案 | 中文，逐字取 v1：「请先设置 API Key」「设置」「展开对话」 |
| S05-16-2 | 空态页 | `chat/empty_page.rs`：透明外壳 + 独立气泡输入框（`Composer::set_standalone`）+ 顶部居中「展开」按钮 + 错误条（复用 `message_row::error_banner`） |
| S05-16-3 | 无 Key 页 | `chat/no_key_page.rs`：整块可点击面板（点击 / 聚焦后 Enter、空格）→ 事件 `OpenSettings` |
| S05-16-4 | 独立气泡 | v1 `.empty-shell .input-dock.is-standalone`：无外边距、圆角 xl、`--window-outline` 描边、内侧 `--window-inner-highlight`、`--shadow-floating-md`；不自动撑高；聚焦不加底色；视口高 ≤180px 时撑满 |
| S05-16-5 | 图标与提示 | 新增图标 `KeyRound`、`ChevronUp`；`components::TextTooltip`（v1 用原生 `title`，GPUI 无原生提示） |

页面只发事件（`EmptyPageEvent` / `NoKeyPageEvent`）；页面切换、发送前的配置校验属 S05-18。

## 验收标准

- [x] 文案与 v1 逐字一致（对照 `EmptyPage.tsx` / `NoApiKeyPage.tsx`）
- [x] 无 Key 页：点击、Enter、空格触发，其他键不触发（T23）
- [x] 空态页：展开、紧凑窗口撑满、较高窗口贴底、不撑高、错误条（T24）
- [x] 观感 —— 用户目检通过（2026-09-29，handoff §6.5 第 16 项；首轮反馈的三处问题已修复并复核）

## 证据

| 项 | 证据 |
|----|------|
| 单测 | `cargo test -p buddy-ui no_key_page`（Enter / 空格 / 其他键） |
| T23 | `cargo run -p buddy-app --example pages_preview -- --selftest`：真实鼠标点击面板与图标处、聚焦后 Enter / 空格各发出一次「前往设置」，`a` / `tab` 不发。拦截：去掉空格 → FAIL；去掉点击回调 → FAIL |
| T24 | 同上：点「展开」发出展开、按钮之外不发；560×60 窗口中输入框垂直中心 30.0（= 窗口中心）、560×260 窗口中 239.0（贴底）；四行草稿时输入框高 24.0（一行，不撑高）；错误条出现在输入区上方，点关闭发出事件。拦截：紧凑窗口不撑满 → FAIL；独立气泡恢复撑高 → FAIL；展开按钮无反应 → FAIL；错误条不渲染 → FAIL |
| 目检反馈修复（2026-09-29） | ① 「展开」按钮被输入区盖住不可见：按钮在子元素中排在输入区前面，v1 为 `z-index: 2` → 改为排在输入区之后绘制（目检确认可见）；② 多行时滚轮无效：输入框每帧把滚动位置拉回光标处 → 仅在光标位置 / 内容长度变化时才保持光标可见（`TextArea::revealed`）。T24 新增「滚轮上翻」：滚动偏移 56 → 16 并保持；拦截：恢复每帧拉回 → 56 → 56，FAIL。此缺陷同样存在于对话页输入区（S05-06），一并修复；③ 展开按钮悬停不变色（目检反馈）：`svg` 取自身文字样式，不跟随父元素的 `hover` 样式 → 改为 `on_hover` 记录状态、图标与底色显式设置；T24 新增悬停进入 / 移出检查，拦截：悬停回调不生效 → 进入为 false，FAIL |
| 回归 | `chat_preview --selftest` T11–T22 全部 PASS（Composer 改动未影响对话页） |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 无 Key 面板描边 | `--border-default` 代替 `--glass-outline` | `--glass-outline`（白色半透明边）已按用户决定不迁移（`tokens.rs` `EXCLUDED`，S00-04）；Composer 早已如此处理 |
| 独立气泡描边 | 保持 v1 的 `--window-outline` | 该令牌已迁移，不是 `--glass-outline` |
| 紧凑窗口撑满 | 以窗口视口高 ≤180px 判定 | 等价 v1 `@media (max-height: 180px)`；页面自己不知道窗口尺寸策略（Phase 07） |
| 展开按钮图标线宽 | 沿用图标集默认线宽 | v1 为 `strokeWidth 1.8`，差 0.2px，GPUI `svg` 不逐次改线宽；目检时若可见差异再议 |
| 悬停提示 | 自绘最小 `TextTooltip` | v1 用 `title` 属性（浏览器原生提示），GPUI 没有等价物；仅用于「展开对话」 |
| 键盘触发 | 面板聚焦后 Enter / 空格 | 对应 v1 `tabIndex=0` + `onKeyDown`；Tab 顺序与焦点环随 S05-18 / Phase 07 的窗口焦点策略再定 |
| 拖拽区 `.empty-drag-region` | 不做 | 窗口拖拽属 Phase 07 |
| 紧凑窗口尺寸 | 不在本 spec 设定 | v1 初始 560×60（`tauri.conf.json`），窗口尺寸与「切页不改尺寸」（硬约束 6）属 Phase 07；`docs/design/pages-and-states.md` 写的 460×78 已过时（该文档由 S05-18 退役） |
| 发送 / 模型选择 / 设置 | 由输入区自身事件承担 | 页面不重复实现；模型下拉归 S05-15，配置校验与跳转归 S05-18 |

## 完成记录

- 日期：2026-09-29
- commit：`080fd1d`、`2f6fc27`、`a3479fe`
- 设计文档处置：无（`docs/design/pages-and-states.md` 由 S05-18 / S10-03 退役；该文档中 EmptyPage 的窗口尺寸描述已与 v1 不符，退役时不必迁移）
