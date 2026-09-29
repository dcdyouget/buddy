# 交接说明（v2.0.0-gpui 迁移）

> ⚠️ **本文不是状态源。** 权威状态在 [`README.md`](./README.md)（spec 注册表）。
> 本文只记录**权威文档里没有的东西**：当前在飞状态、未提交改动的归属划分、以及交接边界。
>
> 最近更新：2026-09-27（Phase 00-02 完成；Phase 03 进行中）

---

## 1. 一句话状态

**Phase 00 / 01 / 02 全部完成；Phase 03（主题）4/7 完成、2 个待用户目检。**
**全栈闭包 770 包、毒性依赖零残留、完全不需要 fork GPUI、端到端已跑通（真实 API → SSE → markdown 渲染）。**

```
P00 可行性 Spike          9/9   ✅ 完成
P01 工程骨架与分层         6/6   ✅ 完成
P02 引擎层移植            9/9   ✅ 完成
P03 主题与设计令牌         4/7   ← S03-05/06 待目检，S03-07 进行中
P04 Markdown 栈          0/9
P05 聊天界面             0/18
P06 设置界面              0/6
P07 应用外壳与窗口行为      0/13
P08 更新与发布            0/11
P09 平台对齐（Windows）     0/9
P10 测试与验收             0/7
合计                    28/104
```

---

## 2. 必读顺序（新 agent 从这开始）

| 顺序 | 文件 | 为什么 |
|------|------|--------|
| 1 | `AGENTS.md` | 唯一权威入口：项目概览、**Agent 工作流**、Document Index、**Hard Constraints 1-10** |
| 2 | `docs/specs/RULES.md` | **Spec 工作规则**：粒度、状态机、完成定义、证据要求、设计文档退役、文档卫生 |
| 3 | `docs/specs/README.md` | **Spec 注册表与进度（唯一权威状态源）** |
| 4 | `docs/dev-environment.md` | **环境前置**（代理、三个 GPUI 硬前置、`[patch.crates-io]`、不要破坏 v1） |
| 5 | `docs/tasks/v2.0.0-gpui/research-log.md` | 调研证据（§9 环境配方、§10-§18 全部实测结论与风险登记） |
| 6 | 当前要做的 spec 文件 | `docs/specs/phase-NN/S<NN>-<NN>-*.md` |

**不要重读 `docs/design/*` 当前置真值** —— 它已被证明含两处「从未存在的意图」
（见 §7 与 research-log §18.4）。

---

## 3. Phase 00 的产出（9 个 spec，全部含可复核证据）

| Spec | 结论 | 产物 |
|------|------|------|
| S00-01 抽取 zed theme+ui | ✅ | 三个 GPUI 接入硬前置（research-log §10） |
| S00-02 浮动面板 | ✅ | `docs/evidence/s00-02/window-patch.rs` |
| S00-03 热键/tray/自启 | ✅ | `docs/evidence/s00-03/shell-integration.rs` |
| S00-04 毛玻璃 | ✅ **决定不用** | `docs/evidence/s00-04/window-appearance.rs` |
| S00-05 中文 IME | ✅ | `docs/evidence/s00-05/ime-input.rs` |
| S00-06 markdown vendor | ✅ 闭包 +25 包 | `docs/evidence/s00-06/`（shim/stub + 配方手册） |
| S00-07 大列表虚拟化 | ✅ 0.35% 行渲染/帧 | `docs/evidence/s00-07/list-integration.rs` |
| S00-08 引擎层闭环 | ✅ 端到端跑通 | `docs/evidence/s00-08/engine-integration.md` |
| S00-09 结论与决策 | ✅ **Go** | `docs/specs/phase-00/S00-09-decision.md` |

### 风险登记最终状态（9 项中 4 项消除、1 项降级）

| # | 风险 | 复评 |
|---|------|------|
| R1 | 窗口外壳 | **消除** |
| R2 | Windows `Floating` 静默 no-op | 中（macOS 已定 `PopUp`；Windows 待 S09-04） |
| R3 | **GPUI fork 维护成本** | **消除** |
| R4 | Windows 字体/文字渲染差异 | 中高（不变） |
| R5 | Win10 无 Mica | **消除** |
| R6 | Windows IMM32 输入法 | 中（macOS 已过） |
| R7 | **GPL 传染** | **可消除（已实证）** |
| R8 | 无法回退 | 低（S01-05：本地 tag `v1-final` 已建并实机验证；未推送） |
| R9 | 设计资产丢失 | 低（但暴露了新问题类） |

---

## 4. Phase 01 的进度

| Spec | 状态 | 说明 |
|------|------|------|
| S01-01 三层 workspace | ✅ `done` | 依赖方向机检通过；v1 未被破坏 |
| S01-02 GPUI 依赖锁定 | ✅ `done` | 31 个 zed crate 同一 rev；不 fork；干净构建 442 crate / 28s |
| S01-03 许可证分层 | ✅ `done` | GPL 全文入库；746 包许可统计 |
| S01-04 CI 断言 | ✅ `done` | 14 项检查 + **10 项拦截验证**全通过（S02-01 期间事后修正 4 处缺陷） |
| **S01-05 退路分支** | ✅ `done` | tag/分支仅本地，推送暂缓，见 §5 |
| S01-06 v1 基线采集 | ✅ `done` | 见 `docs/evidence/v1-baseline/README.md` |

### 已落地的工程骨架

```
Cargo.toml              ← workspace 根（resolver=2 + exclude src-tauri + [patch.crates-io]）
Cargo.lock              ← 已入库（可复现构建）
rust-toolchain.toml     ← channel = "1.95.0"
LICENSE                 ← 分层说明
LICENSE-GPL-3.0-or-later / LICENSE-APACHE-2.0   ← 全文
THIRD_PARTY_NOTICES.md  ← 746 包许可清单
docs/dev-environment.md ← 环境前置
scripts/check-discipline.py  ← 14 项纪律检查 + 10 项拦截验证
.github/workflows/discipline.yml

crates/engine/          ← buddy-engine（MIT，零 GPUI/Tauri/GPL）
crates/ui/              ← buddy-ui（GPL，重导出 gpui/theme 表面）
apps/buddy/             ← buddy-app（GPL，只依赖两个分层 crate）
```

---

## 5. S01-05 退路：已按选项 (a) 执行（2026-09-26）

| 引用 | 指向 | 内容 |
|------|------|------|
| commit `9cc244a` | — | 用户的 v1 改动单独提交（仅 `src/` `src-tauri/` `README.md`） |
| tag `v1-final` | `9cc244a` | 退路基准，不可移动 |
| 分支 `v1-fallback` | `9cc244a` | v1 热修用 |

S01-05 已 `done`：回退演练通过，用户实机运行确认可用。**tag 与分支仅在本地**——用户决定（2026-09-27）**整个重构完成后再推送**（必须带上 tag `v1-final`），推送时补做 GitHub 分支保护。

`promo-video/` 渲染成品已按用户要求删除。

## 6. S01-06 范围已调整（2026-09-26）

> **2026-09-26 用户决策：不截图、不录屏。** 视觉/行为验收由用户对照运行中的 v1 目检并反馈。
> 因此**不再需要屏幕录制权限**。

S01-06 剩余范围（均无需屏幕权限）：页面×状态清单（供目检对照）、令牌实测值、
性能基线、1000+ 消息长会话样本（构造数据，供性能对比）。
**真实数据目录快照已取消**（用户决策：开发阶段，不保存历史、不做历史数据迁移）。

**不可再生资产已转移**：视觉基准从「截图」变为「可构建运行的 v1」，
由 `S01-05` 退路 tag 保证 —— 所以 S01-05 的回退演练是硬性的。

---

## 6.5 ⏳ 待用户目检清单（攒齐后统一交给用户）

> 用户要求（2026-09-27）：能做的先做完，需要确认「效果是否 OK」的汇总到一起。确认后在此勾掉并回写对应 spec。
>
> **2026-09-28：下表 7 项全部由用户目检通过**，已回写各 spec（S03-05/06、S04-06..09 → `done`）。
> 第 2 项：v1 联网搜索在用户实际使用中正常 → research-log §19.6 的失败仅见于本机自动化测试环境。

**第二批（Phase 05）：** 2026-09-28 第 8–11 项全部通过（#8 / #11 首轮反馈滚轮逐行，改为无极滚动后复核通过）。2026-09-29 第 12–15 项全部通过（S05-09 / S05-10 / S05-14 / S05-05 → `done`）；第 16 项通过（S05-16 → `done`）；第 17 项通过（S05-18 → `done`）；第 18 项通过（S05-15 → `done`）。**2026-09-29 发现并修复全局缺陷：GPUI 的 `svg` 不继承父元素颜色，此前所有放在带颜色元素里的图标（输入区设置 / 模型 / 发送、操作栏复制 / 回到问题等）都没有画出来；修复后图标才真正显示，已通过的 #4 / #12 / #13 / #14 等含图标的项请顺带复看。**

| # | 来源 | 看什么 | 怎么操作 | 预期 |
|---|------|--------|---------|------|
| 8 | S05-01 | 长列表滚动是否流畅；流式回复时是否贴底跟随 | `cargo run -p buddy-app --example chat_preview`（默认 1000 条；可加 `-- --messages 5000`）；用触控板 / 滚轮上下滚动；点「模拟流式回复」 | 滚动跟手、无卡顿；流式回复逐字出现且列表贴底。**行外观为占位（S05-08 起按 v1 渲染），本项只看滚动与跟随** |
| 9 | S05-08 | 消息行外观、流式态、错误态是否与 v1 一致 | 同上窗口，与 v1 对照；点「模拟流式回复」「模拟出错」；拖选用户消息文字并 Cmd+C | 用户气泡靠右（最宽 76%、淡品牌色底、左下角较小圆角、细阴影）；助手内容左对齐、消息之间留白与 v1 相同；流式时最新字渐显、末尾星标呼吸；出错时已显示的内容保留，底部出现红色提示条（可关闭）；用户消息文字可选择复制。思考块 / 工具行仍为占位（S05-09 / S05-10） |
| 10 | S05-06 | 中文输入法打字、回车规则、输入区外观 | 同上窗口底部的输入区：用拼音输入中文（组字时按 Enter 只上屏、不发送）；Enter / Shift+Enter 发送；Cmd+Enter 换行；多行时自动增高、超过约 5 行出现内部滚动；试试方向键、Option+方向键、Cmd+Z、双击选词、清除按钮；发送后流式中变为「mock-model · 生成中...」+ 红色停止按钮 | 与 v1 一致。已知差距：v1 悬停 / 聚焦时的流动渐变描边与跟随指针的光晕，这里以品牌色描边近似 |
| 11 | S05-04 | 流式跟随手感、回到底部按钮 | 同上窗口：发送后在流式中向上滚动（应停住不被拉回）；结束后右下角出现圆形「↓」按钮，点它平滑回到底部；再发一条时自动回到底部跟随 | 与 v1 一致。v1 跟随也是直接贴底（无弹簧），若觉得生硬请告诉我 |
| 12 | S05-09 | 思考块外观与折叠 | `chat_preview` 点「模拟思考与工具」 | 左侧蓝色竖边的折叠条；输出中「正在思考」+ 三个跳动的点 + 一道光泽扫过 + 最新思考；结束后「思考过程」+ 第一行；点击展开 / 收起 |
| 13 | S05-10 | 工具卡片外观与展开 | 同上 | 执行中卡片展开（调用参数），完成后收起并显示「已完成 / 失败」徽标；点开看参数与结果；长结果框内可单独滚动，不带动整个列表 |
| 14 | S05-14 | 回答下方操作栏 | `chat_preview` 任一已完成回答的下方 | 小胶囊条：「复制」「回到问题」、竖线分隔、右侧时间（等宽字）；点复制后变品牌色「已复制」约 1.6 秒后恢复，剪贴板内容为回答正文；点「回到问题」本轮提问滚到顶部；流式中不出现 | 与 v1 `MessageActions` 一致 |
| 15 | S05-05 | 历史分页与长历史滚动 | `cargo run -p buddy-app --example chat_preview -- --messages 5000`（目检模式按 v1 分页：先载最新 10 条） | 向上滚到接近顶部时顶端短暂出现「正在加载更早消息…」（模拟读取 150ms），随后更早的 10 条出现在上方，**正在看的内容不跳**；持续上滚可一直翻到第 1 条（「第一行 / 第二行 / 第三行」）；全程顺滑 | v1 同为每页 10 条、距顶 56px 触发 |
| 16 | S05-16 | 空态页与无 Key 页的外观与交互 | `cargo run -p buddy-app --example pages_preview`（两个 560×60 无标题栏窗口，Cmd+Q / Esc 退出；加 `-- --error` 看带错误条的空态页）；对照 v1 的空态与无 Key 状态 | 空态：透明底、只有一枚圆角气泡输入框（占满窗口）、顶部居中小胶囊「展开」按钮（悬停变品牌色 + 提示「展开对话」）；无 Key：整块面板、左侧品牌色钥匙图标、红色「请先设置 API Key」、右侧红色「设置 ›」，点击整块或聚焦后 Enter / 空格会在终端打印「前往设置」。已知差距：面板描边用默认边框色（v1 的白色半透明描边已按决定不迁移）；无窗口外壳 / 阴影 / 拖拽（Phase 07）；展开图标线宽 v1 为 1.8 |
| 17 | S05-18 | 页面切换与真实对话流程 | `cargo run -p buddy-app --example app_preview -- --mock`（560×480 无标题栏窗口，Cmd+Q / Esc 退出；提示词含「401」「429」「500」「慢」触发对应情形；`-- --no-key` 从空配置开始；不加参数用 v1 config 的真实模型，数据在沙盒 `target/buddy-app-preview/`）。依次试：空态输入并回车 → 流式 → 对话；发「401」→ 无 Key 页 → 点面板进设置占位页 → 「返回」；发「429」「500」看提示消息；发「慢」后点红色停止；空态点顶部小按钮「展开」；空态输入文字后展开，草稿仍在 | 页面切换时**窗口大小始终不变**（v1 会在离开紧凑页时展开窗口，那是 Phase 07 的窗口壳做的事，这里不做）；空态是窗口底部一枚圆角输入气泡，对话页是整块面板；流程与 v1 一致：401 进无 Key 页，429 / 500 / 网络错误留在对话页并多出一条提示消息，停止后无红色错误条。已知差距：设置页是占位（S06-01）；无审批 / 提问 / 模型选择 / 附件（S05-13 / S05-15 / S05-07） |
| 18 | S05-15 | 模型选择菜单 | `cargo run -p buddy-app --example app_preview -- --mock`（有 Mock / Mock 2 两个启用模型，另有一个未启用的 Hidden 不应出现）。点输入区右侧机器人图标（齿轮和发送之间）；试试：点选另一个模型；再打开看对勾是否在新模型上；按 Esc；点菜单外面；在气泡（560×60）里打开；发含「慢」的话，流式中看机器人图标是否消失 | 菜单是一个独立的小窗口，出现在按钮**上方**、右缘与主窗口对齐：宽约 272，每行两行字（模型名 + `Mock · 128K 上下文`），当前模型行淡蓝底 + 右侧品牌色对勾，悬停行凹陷底；点选后对勾移到新行，约 0.12 秒后菜单关闭，焦点回到主窗口；关掉重开 app 后仍是新选的模型（盘上已保存）；Esc、点击菜单外部都能关闭；流式中输入区不显示机器人图标。已知差距：v1 是系统原生菜单（文字列表 + 勾），这里是 v1 回退用的下拉外观；机器人图标的悬停动画未做；点击外部关闭的路径无自动化 |

| # | 来源 | 看什么 | 怎么操作 | 预期 |
|---|------|--------|---------|------|
| 1 | S02-06 | v2 窗口能用真实模型流式对话 | `cargo run -p buddy-app --example chat_bridge` → 点「发送示例问题」；再点一次后立刻点「停止生成」 | 回复文字逐步出现；状态行「结束：Done」、显示首批事件毫秒数；停止后状态「结束：Error(Aborted): 用户取消」。对话写在 `target/buddy-dev-data/`，**不进 v1 历史** |
| 2 | S02-09 | v1 联网搜索是否在实际使用中也失败 | 在 v1 里让模型「搜索一下今天的新闻」 | 若搜索报错/无结果，属 v1 既有问题（research-log §19.6），决定是否另开修复 |
| 4 | S04-07 | 代码块外观与复制反馈是否与 v1 一致 | `cargo run -p buddy-app --example markdown_preview` → 对照 v1 中同样的代码块；点各块右上角「复制」后粘贴；点「切换到深色」再看一遍 | 圆角边框 + 淡蓝底；头部有下边框，语言标签为胶囊（纯文本 / 无语言块无标签，行高更松）；「复制」点击后变绿显示「已复制」、对勾弹一下、约 2 秒复原；粘贴内容与代码一致（列表内代码无多余缩进）；超长行横向滚动不换行。已知差距：hover 变色无渐变；横向滚动条仅滚动时出现 |
| 5 | S04-08 | 标题 / 列表 / 任务 / 引用 / 表格 / 分隔线 / 链接 / 图片 / 原始 HTML 的观感是否与 v1 一致 | 同上窗口（`markdown_preview`）顶部的 GFM 样例；对照 v1 中发送相同 markdown 的回复；点链接 | h1–h3 左侧渐变竖条（h1 另有底部渐隐线）；无序为带光圈小圆点、有序为圆形序号徽章；任务项为复选框；引用左侧主色竖边 + 淡渐变底；表格圆角外框、表头淡底主色字；分隔线两端渐隐；链接主色、点击用浏览器打开（`相对链接` 打开 `https://example.com/path`）；头像图片显示、失败图片显示占位；`<kbd>` 等原样显示。已知差距见 S04-08 决策记录（行内代码无边框、无悬停效果等） |
| 6 | S04-09 | 消息文字能否正常拖选、双击选词、三击选行；选区颜色是否可接受；复制粘贴结果 | 同上窗口；拖选一段含加粗 / 列表 / 表格的内容，Cmd+C 后粘贴到备忘录或表格软件 | 选区为淡品牌色（用户已决定用品牌色）；粘贴后段落间有空行、表格列以制表符分隔、无多余不可见字符 |
| 7 | S04-06 | 流式渐显与星标是否与 v1 一致 | `cargo run -p buddy-app --example streaming_preview`；与 v1 中实际对话的流式输出对照；点「重新播放」；输出中切到别的窗口再切回 | 约 50 字/秒逐步出现，积压时加速；最新约 9 个字从偏白、半透明过渡到正常颜色；最后一个字后有蓝色四角星呼吸（输出停顿时明显）；段落刚结束时星标单独成行；代码块未写完时无「复制」按钮；切走再切回时积压内容立即出现。已知差距：字符无光晕（v1 有白蓝发光）、星标为单色近似 |
| 3 | S03-05 / S03-06 | v2 主题的颜色、字体、圆角、阴影观感是否与 v1 一致；浅深切换是否无闪白 | `cargo run -p buddy-app --example theme_preview` → 对照同时打开的 v1（`git worktree add ../buddy-v1 v1-final && cd ../buddy-v1 && npm run tauri dev`）；点「切换到深色 / 浅色」 | 字体为 Fira Code（中文为苹方）、正文字重偏粗（v1 为 650）；色块与 v1 同名颜色一致；切换瞬间整窗换色、无白闪。已知差距：标题字距 GPUI 不支持（约 -0.2px/字） |

## 7. 必须知道的坑（Phase 00 实测得出，勿重踩）

### 7.1 环境（三个硬前置，缺任一即失败）

| # | 条件 | 缺失后果 |
|---|------|---------|
| 1 | `gpui_platform` 必含 **`runtime_shaders`** | 构建失败，需完整 Xcode 的 MetalToolchain |
| 2 | `gpui_platform` 必含 **`font-kit`** | **完全没有文字**，且**静默**（只一条 WARN） |
| 3 | 必须在 `theme::init` 后装 `ThemeSettingsProvider` | 开窗 panic |

**另有**：github 必须走代理（直连 40min 未完成 vs 代理 865s）；必须复制 zed 的
`[patch.crates-io]`（当前保留 2 项，另 3 项需在 `S04-02` / `S04-04` 加回）。
**全部细节见 `docs/dev-environment.md`。**

### 7.2 诊断纪律

**任何 GPUI 显示异常，先 `RUST_LOG=debug` 看 gpui 自身日志。** 两个静默失败（无文字、渲染 0 行）都靠这个发现。

### 7.3 「仪表盘可以在窗口空白时显示 PASS」

S00-07 实测：自动化计数器显示「虚拟化 PASS / at_end=true」，而窗口**一片空白**。
三个真 bug（`list()` 缺 `flex_grow_1`、回调内碰 `ListState`、没调 `scroll_to_end()`）
都是**用户一句「我没看到内容」**才挖出来的。

→ **需要眼睛的验收，不得仅凭自身仪表盘标 `done`。**
→ 这正是 `S01-04` 要求「每项检查都要做拦截验证」的由来（8 项全部验证过）。

### 7.4 官方示例不可信

`crates/gpui/examples/input.rs` **自带 4 个缺陷**（2 个致命）。只能当骨架。
详见 `docs/evidence/s00-05/ime-input.rs`。

### 7.5 图层/属性探测必须延迟复探

S00-04 首探在首次合成之前，得到「无 `CABackdropLayer`」的**错误结论**，
差点误判为「macOS 26 打坏模糊、需要 fork」。延迟到 t≈1s 后图层才出现。
→ 适用 `S07-12` / `S09-01` / `S09-04`。

### 7.6 文档里的意图 ≠ 既成事实

| 实例 | 真相 |
|------|------|
| `docs/design/prototypes/` | AGENTS.md + CLAUDE.md + tasks/v1.0.0 层层引用 → **路径从未存在** |
| 「毛玻璃」 | AGENTS.md Design Philosophy 写着 → v1 `macos.rs:154` **明写不启用 vibrancy** |
| 「`ThemeSettings` 需 patch 9 处」 | 实际 1 行 import + shim |
| 「`acceptFirstMouse` 需手写」 | gpui 已硬编码 `YES` |
| 「`theme`+`ui` 闭包 12 个」 | 实为 30 个 |

→ 规则立于 `RULES.md` §11。**首次接触任何文档结论时，先验证再采用。**

### 7.7 其他已知陷阱（详见 research-log 对应节）

| 陷阱 | 位置 |
|------|------|
| `Done.full_text` 含 ` thinking` 标签（差 15 字符），不可当显示文本 | §17.5 |
| 内联 think 字符数在 `StreamOutcome` 里拿不到 | §17.6 |
| tokio 任务不能捕获 `Rc`/`Cell`；future 需 `'static` | §17.7 |
| `ListState` 四个陷阱（`flex_grow` / 回调借用 / `measure_all` / `scroll_to_end`） | §16.3 |
| `shape_line` 拒绝 `\n`，多行需拆逻辑行 | §14.2 |
| Enter 三态由 gpui 平台层保证，前提是 `marked_text_range()` 正确 | §14.4 |
| 渲染层 clamp 会掩盖语义层越界（`copy()` 会 panic） | §14.5 |
| `WindowKind::Floating` 不给全工作区；`PopUp` 才是正解 | §11.1 |
| **每帧路径上不得枚举系统字体**（`all_font_names()` 走 CoreText，单次数十 ms）；字体用 `fonts::ui_font` / `mono_font`（已缓存）。曾致整窗重绘 620ms | S03-05 决策记录 |

---

## 8. 下一批任务（按可自动化程度排序）

| 顺序 | Spec | 需要用户？ |
|------|------|-----------|
| 1 | ~~S01-05 退路分支~~ | ✅ done（推送暂缓） |
| 2 | **S01-06 v1 基线采集** | ❌ 剩余部分全自动（见 §6） |
| 3 | ~~`S02-03` / `S02-02` / `S02-01` / `S02-04` / `S02-05`~~ | ✅ done |
| 4 | `S02-07` 对话编排（`send_message` 迁入 engine：工具循环、审批/提问、持久化、终态事件） | ❌ 全自动（**范围大于原标题**，见 spec S02-07-5/6） |
| 5 | `S02-06` tokio/GPUI 桥接 | ❌ 全自动 |
| 6 | `S02-09` 引擎测试迁移 | ❌ 全自动 |
| 7 | `S02-08` 命令覆盖表 | ❌ 全自动 |
| 11 | `S03-01` ~ `S03-07` 主题 | 🟡 部分需视觉对照 |
| 12 | `S04-*` markdown | 🟡 部分需视觉对照 |

### 8.1 跨 spec 移交（展开对应 Phase 的 spec 文件时**必须并入**其验收标准）

| 来源 | 移交给 | 内容 |
|------|--------|------|
| S04-05 | S05-08 | 流式消息每批对完整文本 `markdown::normalize::normalize_markdown` 后 `replace`；**目检**：流式中半截 `**` 的观感与 v1 一致（v1 会短暂显示字面星号，v2 不做 mend） |
| ~~S04-05~~ | ~~S04-09~~ | ~~复制时去掉零宽空格守卫~~ —— **已完成**（S04-09 `copy_text`；显示时守卫为零宽，无需处理） |
| ~~S04-07~~ | ~~S04-06~~ | ~~复制动画遵守减弱动效~~ —— **已完成**（S04-06 `accessibility::prefers_reduced_motion`） |
| S04-06 | S05-08 | 流式消息行：增量经 `streaming::Pacer`（`push` / 每帧 `tick`；窗口失焦 `hide`、聚焦 `show`，做法见 `examples/streaming_preview.rs`）；每批 `normalize_markdown` 后 `replace`；每帧 `streaming::tail` + `streaming::decorate`，`Star::Standalone` 时在消息末尾放 `star_element`；有动画时 `request_animation_frame`。结束 / 出错前 `Pacer::flush` 防丢字 |
| S04-06 | S05-17 | v1 的流式事件队列（文本未放完前，工具调用 / 结束事件排队等待，`chatStore._drainStreamEventQueue`）属状态层，不在 `Pacer` 内 |
| S04-07 / S04-08 | S05-08 | 助手消息用 `MarkdownElement::new(md, markdown::message_style(..)).code_block_renderer(code_block::renderer(md.downgrade(), streaming)).on_url_click(\|u, _, cx\| gfm::open_link(&u, cx)).image_resolver(\|u, _\| gfm::image_source(u))`；应用须 `with_assets(icons::Assets)`、`chat_bridge::init` 后 `http::install` |

**Phase 02（9 个 spec）可完全自动化** —— S00-08 已证明引擎层可脱离 Tauri 独立工作，
且 `providers/` / `models/` 的移植改动量已实测（4 处）。

---

## 9. 可直接复用的已验证代码

**`spikes/` 目录被 `.gitignore` 排除，会被清理。** 但以下产物**已固化到仓库**：

| 产物 | 内容 |
|------|------|
| `docs/evidence/s00-02/window-patch.rs` | 零装饰 / 去阴影 / 显隐 / 层级 / 全工作区（objc2） |
| `docs/evidence/s00-03/shell-integration.rs` | 热键 + tray + autostart 集成（含 `TrayIcon` 非 `Send/Sync` 的处置） |
| `docs/evidence/s00-04/window-appearance.rs` | 最终外观配置（不透明 + 16px 圆角） |
| `docs/evidence/s00-05/ime-input.rs` | 官方 `input.rs` 的 4 个缺陷修法 + 多行 `MultiLine` 实现 |
| `docs/evidence/s00-06/` | `theme_settings_shim.rs` / `language_stub.rs` / `mermaid.rs` + 完整 vendor 配方 |
| `docs/evidence/s00-07/list-integration.rs` | `ListState` 四个陷阱 + 推荐骨架 |
| `docs/evidence/s00-08/engine-integration.md` | 引擎去 Tauri 化规范 + 四个陷阱 |

`S02-01` ~ `S02-05` 可直接从 `s00-08` 的产物出发（已实测的 4 处改动）。

---

## 10. 交接纪律（接手方必须遵守）

1. **状态只在 `docs/specs/README.md` 维护。** 不另立状态源。
2. **完成一个 spec 立即更新注册表**（不批量补记）。状态同时写在 spec 文件头与注册表，必须一致。
3. **证据必须可复核**，不接受「OK」「已完成」。见 `RULES.md` §6。
4. **设计文档实现完成后必须删除**并登记 `docs/specs/design-deletions.md`。见 `RULES.md` §7。
5. **同一时刻最多 2 个 spec 处于 `doing`。**
6. **不允许跳过 `doing` 直接 `done`。**
7. **提交一律 `scripts/gate.sh && git commit …`**（纪律检查 16 项 + 拦截验证 13 项 + workspace / v1 编译）。**不要手写检查链**：曾两次用 `;` 连接导致检查失败后仍提交。
8. **不要破坏 v1**：改 workspace 后必须 `cd src-tauri && cargo check` 验证
   （根 `Cargo.toml` 的 `exclude = ["src-tauri"]` 是必需的）。
9. **需要眼睛验收的，停下来问用户。** 见 §7.3。

---

## 11. 文档规模概览（供接手方判断信息密度）

| 目录 | 文件数 | 行数 |
|------|-------|------|
| `docs/specs/` | 19 | 3654 |
| `docs/tasks/v2.0.0-gpui/` | 13 | 2632 |
| `docs/evidence/` | 10 | 2341 |
| `crates/` + `apps/` | 7 | 398 |
| `scripts/` | 4 | 1103 |
