# S05-18 页面状态机

> 状态: `blocked`
> Phase: 05
> 依赖: S05-16
> 阻塞: 等待用户目检（`handoff.md` §6.5 第 17 项）
> 退役设计文档: `docs/design/pages-and-states.md`（与 S10-03 共同）

## 目标

chat / empty / no-api-key / settings 的切换，与 v1 一致；窗口尺寸在切换时不变（硬约束 6）。

## 输入

- v1 `src/stores/uiStore.ts`、`src/App.tsx`（`PageRenderer` 与配置 / 菜单副作用）、`src/hooks/useStreaming.ts`（流式结束后的落点）、`src/pages/{EmptyPage,ChatPage,NoApiKeyPage}.tsx`、`src/utils/windowResize.ts`
- `docs/design/pages-and-states.md`（转换条件；见「完成记录」中的处置）、`docs/evidence/v1-baseline/README.md` §3（窗口行为事实）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S05-18-1 | 转换条件 | `chat/page_state.rs`：`PageState`（当前页 / 上一页 / 设置叠加层的底层页）、`config_changed`、`stream_finished`、`close_settings`、`classify_empty_send`、`expands_window`；纯逻辑，9 个单测 |
| S05-18-2 | 对话页 | `chat/chat_page.rs`：v1 `ChatPage` 外壳（`buddy-shell` 面板 + 消息列表 + 错误条 + 输入区）；输入区与空态页**共用同一个实体**（v1 草稿在 `chatStore`，两页共享） |
| S05-18-3 | 路由器与 engine 接入 | `chat/router.rs`：`preload`（配置 + 最新一页历史，经 tokio）→ `PageRouter`；发送走 `chat_bridge::start_chat`（发给 engine 的是「已载入历史 + 本条用户消息」，同 v1）；`pending_saves` → `save_message`；`HistoryLoader` → `load_messages`；401 → 无 Key 页；`Stop` → `stop_generation` |
| S05-18-4 | 设置叠加层 | 设置页本体归 S06-01；此处只有占位（标题 + 「返回」），但叠加、返回、底层页不卸载的逻辑是真的 |
| S05-18-5 | 窗口尺寸 | 路由器不接触窗口，只发 `RouterEvent::PageChanged`；「离开紧凑页时展开一次」由 Phase 07 依 `expands_window` 执行 |

## 验收标准

- [x] 状态转换测试（`cargo test -p buddy-ui page_state`：9 个）
- [x] 端到端：真实 engine + 磁盘 + mock 模型，覆盖发送 / 401 / 429 / 500 / 停止 / 设置 / 展开 / 补齐配置 / 配置失效（T25、T26）
- [x] 历史：重启读回最新一页，触顶经 engine 读取更早一页（T27）
- [x] 切换时窗口尺寸不变 —— 测试（T25：全程窗口尺寸与开窗时一致；拦截：切页时改窗口尺寸 → FAIL）
- [ ] 观感与流程 —— **需用户目检**（handoff §6.5 第 17 项）

## 证据

| 项 | 证据 |
|----|------|
| 单测 | `cargo test -p buddy-ui page_state`（9 个）：启动空态与相同页忽略、上一页记录与 `go_back`、设置叠加层的底层页、设置返回改道、配置副作用全表、流式落点、空态发送判定顺序、窗口展开判定、有效配置 |
| T25 | `cargo run -p buddy-app --example app_preview -- --selftest`：24 项检查。空态输入「你好」+ Enter → 流式页 → 对话页；助手正文与 mock 全文一致；盘上恰 2 条；提示词含 401 → 无 Key 页；`set_config` 补齐 → 对话页；429 / 500 → 界面生成的提示消息落盘；慢速流中 `Stop` → 1.5 秒内结束、无错误、留在对话页；设置往返；配置失效 → 空态；点「展开」→ 对话页；全程窗口尺寸不变 |
| T26 | 同上：空配置下空态发送 → 无 Key 页、无消息、草稿保留；点面板 → 设置 → 返回进对话页；只在内存补齐配置而盘上没有 → engine 拒绝发送，空占位被移除、报错、用户消息保留；写盘并更新后可发送；Provider 清空 → 空态 |
| T27 | 同上：预置 25 条后重启，起始页仍为空态；载入最新 10 条（`seed-15…24`）；触顶加载经 engine 读取 `seed-05…14`，再读最早 5 条；`has_more` 归零 |
| 拦截 | 见下「拦截验证」 |
| 目检反馈后补充（#17 首轮） | 手动预览原先是固定 560×480 的大窗口，空态看起来不是气泡；现手动模式加「窗口壳替身」`DemoShell`（示例内，非产品代码）：按 v1 `geometry.rs` 从 560×60 启动，离开紧凑页时展开为 750×500（设置 760×640），设置页返回紧凑页之前的页面时展开为对话尺寸；只在离开紧凑页时改尺寸（v1 `resizeWindowForPage`），v1 在 401 后从对话页切到无 Key 页时窗口**不会缩回**（面板悬在大窗口中间）；目检 #17 反馈「框体应一起缩小」，替身在「进入紧凑页」时缩回 560×60（**偏离 v1，待用户确认**；macOS 上 `resize` 保持左上角，气泡落在旧窗口顶部而非底边，底边锚定属 S07-06）。自检 T25 增补：空态 / 对话页小齿轮真实点击进设置、设置叠在对话页之上、`PageChanged` 事件序列（empty→streaming 展开，streaming→conversation 与 conversation→noapikey 不展开）；拦截：小齿轮不响应 → FAIL、不发切页事件 → FAIL |
| 回归 | `chat_preview --selftest` T11–T22、`pages_preview --selftest` T23–T24 全部 PASS |

### 拦截验证

逐项故意改坏，确认自检 FAIL，再还原（还原后三项全 PASS）：

| # | 改坏 | 结果 |
|---|------|------|
| 1 | `stream_finished` 忽略 401 标记 | T25 FAIL（「401 后进入无 Key 页」） |
| 2 | 不落盘界面生成的提示消息 | T25 FAIL（配额 / 提示消息落盘两项） |
| 3 | 设置返回不再对紧凑页改道 | T26 FAIL |
| 4 | 缺配置发送时不切无 Key 页 | T26 FAIL（三项） |
| 5 | 缺配置发送时清空草稿 | T26 FAIL（「草稿保留」） |
| 6 | 启动读最旧一页而非最新一页 | T27 FAIL（五项） |
| 7 | 忽略 engine 拒绝发送 | T26 FAIL（两项） |
| 8 | 无 Key 页补齐配置不进对话 | T25 FAIL（八项，后续流程连锁） |
| 9 | 每帧 `window.resize(700×500)` | T25 / T26 FAIL（「全程窗口尺寸不变」等）；只在设置页渲染时改一次的变体**没有**生效（`resize` 在绘制中调用一次未被平台采纳），不作为证据 |
| 10 | `Stop` 不调用 `stop_generation` | T25 FAIL（「停止在 1.5 秒内生效」） |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 设置页 | 只做占位 | 设置页归 S06-01；叠加 / 返回 / 底层页不卸载的转换逻辑已是真的，S06-01 替换占位即可 |
| 窗口尺寸 | 路由器不改窗口；发 `PageChanged` 事件，`expands_window` 给出判定 | 硬约束 6 的准确含义是「内容页之间保持用户尺寸；仅离开紧凑页时展开一次」（v1 基线 §3）。展开需要窗口壳，属 Phase 07。`docs/design/pages-and-states.md` 写的紧凑窗口 460×78 已过时，v1 实为 560×60 |
| 输入区共用 | 空态页与对话页渲染同一个 `Composer` 实体 | v1 草稿存在 `chatStore`，空态输入后点「展开」草稿不丢；切页时路由器切换 `standalone` |
| 启动页 | 总是空态 | v1 已知行为：即使有历史也不自动进入对话（`pages-and-states.md` Known Implementation Notes） |
| 启动加载 | 先 `preload` 再开窗 | `Conversation::with_history_page` 需要首屏数据；读盘为毫秒级。v1 是先开窗再异步填充，窗口壳（Phase 07）可改回 |
| 流式结束的落点 | 无论当前页都 `set_page(conversation / noapikey)` | 与 v1 `useStreaming` 逐字一致，包括一个 v1 特性：流式结束时若设置页正开着，会被切走。窗口壳接入托盘「设置…」后可能出现，如需改为「设置页开着就不动」是一处小改动（`PageState::stream_finished`） |
| 401 之外的错误 | 留在（或回到）对话页，提示消息落盘 | 与 v1 一致：配额 / 服务器 / 网络三类追加一条 assistant 提示并 `saveMessage` |
| 图标不继承颜色（目检 #17 发现） | `icons::icon()` 返回 `Icon` 元素，绘制时读取祖先文字颜色；门禁新增 S01-04-5d 禁止直接 `svg()` | GPUI 的 `Svg` 只读自己的样式，不继承父元素 `text_color`（`gpui/src/elements/svg.rs`）：此前输入区的设置 / 模型 / 发送图标、操作栏的「复制」「回到问题」图标、悬停变色全部没有画出来。用临时打开 gpui `test-support`（`Window::render_to_image`）把窗口渲染成图片确认，修复前后对比；`test-support` 会改 Cargo.lock（+85 行），故未提交，诊断代码已移除。**此前已通过的目检项里的图标（#4 复制勾、#12 思考块、#13 工具卡片状态、#14 操作栏）现在才真正显示，请顺带留意** |
| 已知未接 | 审批弹窗、`ask_user` 提问、模型选择、附件 | 分别归 S05-13 / S05-15 / S05-07；输入区上对应事件已留位置 |
| 移除 `add-provider` 页值 | 不迁移 | v1 遗留类型值，路由器无对应分支；添加 Provider 是设置页内部的侧滑层（S06-02） |

## 完成记录

- 日期：
- commit：
- 设计文档处置：待 `done` 时执行（`docs/design/pages-and-states.md` 由 S05-18 与 S10-03 共同退役）。已实现且 why 已迁入 `page_state.rs` 模块文档的段落：State vs Component、EmptyPage、NoApiKeyPage、State Flow、Known Implementation Notes；**仍需保留并标注**：ChatPage 的审批 / 提问（S05-13）、SettingsPage（S06）、Global Interactions（Esc / 失焦 / 全局热键 / 选中文本，S07）
