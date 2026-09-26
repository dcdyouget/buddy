# Spec 注册表

> **唯一权威状态源。** 与 spec 文件头部的状态必须始终一致。
> 规则见 `docs/specs/RULES.md`。背景见 `docs/tasks/v2.0.0-gpui/`。
>
> 📌 **接手本项目请先读 [`handoff.md`](./handoff.md)** —— 当前在飞状态、
> 未提交改动的归属划分、阻塞点、以及 Phase 00 实测得出的全部坑。

## 进度总览

| Phase | 名称 | spec 数 | done | 准入条件 |
|-------|------|--------|------|---------|
| 00 | 可行性 Spike | **9** | **9** | — |
| 01 | 工程骨架与分层 | 6 | 4 | **Phase 00 Go ✅ 已满足** |
| 02 | 引擎层移植 | 9 | 2 | S01-01 |
| 03 | 主题与设计令牌 | 7 | 0 | S01-01 |
| 04 | Markdown 栈 | 9 | 0 | S01-01 |
| 05 | 聊天界面 | 18 | 0 | S03-*, S04-* |
| 06 | 设置界面 | 6 | 0 | S03-* |
| 07 | 应用外壳与窗口行为 | 13 | 0 | S01-01 |
| 08 | 更新与发布 | 11 | 0 | S07-* |
| 09 | 平台对齐（Windows） | 9 | 0 | macOS 全链路验收 |
| 10 | 测试与验收 | 7 | 0 | 与 02-09 并行 |
| | **合计** | **104** | **15** | |

> **准入条件是必要条件而非充分条件**：具体以各 spec 自身的「依赖」列为准（`RULES.md` §9.2）。
> 规范强制 **Phase 单调性**：Phase `NN` 的 spec 只能依赖 Phase ≤ `NN` 的 spec（`RULES.md` §9.1）。

> **表格列说明**：Phase 00 / 01 / 02 已展开为详细文件，因此多一列「文件」；其余 Phase 在启动时展开并补上该列。

---

## Phase 00 — 可行性 Spike

> **证伪优先。** S00-02 或 S00-03 失败 → 整体 No-Go，回退 Tauri。
> 拆解依据：`docs/tasks/v2.0.0-gpui/00-spike.md`

| ID | Spec | 依赖 | 状态 | 文件 |
|----|------|------|------|------|
| S00-01 | 抽取 zed theme + ui 并编译 | — | `done` | `phase-00/S00-01-extract-theme-ui.md` |
| S00-02 | Buddy 形态浮动面板（macOS）★ | S00-01 | `done` | `phase-00/S00-02-floating-panel.md` |
| S00-03 | 全局热键 + tray + autostart 集成 ★ | S00-01 | `done` | `phase-00/S00-03-hotkey-tray-autostart.md` |
| S00-04 | 毛玻璃双路径验证 | S00-02 | `done` | `phase-00/S00-04-backdrop.md` |
| S00-05 | 中文 IME 验证 | S00-01 | `done` | `phase-00/S00-05-ime.md` |
| S00-06 | markdown vendor + patch + 流式验证 | S00-01 | `done` | `phase-00/S00-06-markdown.md` |
| S00-07 | 大列表虚拟化验证 | S00-01 | `done` | `phase-00/S00-07-list.md` |
| S00-08 | 引擎层最小闭环 | S00-01 | `done` | `phase-00/S00-08-engine-loop.md` |
| S00-09 | Spike 结论与 Go/No-Go 决策 | S00-01..08 | `done` | `phase-00/S00-09-decision.md` |

---

## Phase 01 — 工程骨架与分层

> 拆解依据：`docs/tasks/v2.0.0-gpui/01-skeleton.md`

| ID | Spec | 依赖 | 状态 | 文件 |
|----|------|------|------|------|
| S01-01 | 三层 workspace 结构与工具链 | S00-09 ✅ | `done` | `phase-01/S01-01-workspace.md` |
| S01-02 | GPUI 依赖来源与 rev 锁定 | S01-01 | `done` | `phase-01/S01-02-gpui-source.md` |
| S01-03 | 许可证分层声明与 NOTICE | S01-01 | `done` | `phase-01/S01-03-licensing.md` |
| S01-04 | 防 GPL 污染 CI 断言 | S01-03 | `done` | `phase-01/S01-04-license-guard.md` |
| S01-05 | 迁移期目录与退路分支 | S01-01 | `doing` | `phase-01/S01-05-fallback.md` |
| S01-06 | 建立 v1 视觉与行为基线 | S01-01 | `todo` | `phase-01/S01-06-v1-baseline.md` |

---

## Phase 02 — 引擎层移植

> 高复用、低风险。可与 Phase 07 并行。
> 拆解依据：`docs/tasks/v2.0.0-gpui/02-engine.md`

| ID | Spec | 依赖 | 状态 | 文件 |
|----|------|------|------|------|
| S02-01 | providers 移植（openai_compatible / anthropic） | S02-02, S02-03 | `doing` | `phase-02/S02-01-providers.md` |
| S02-02 | tools 移植（builtin / image_generation / websearch / file_tools） | S02-03 | `done` | `phase-02/S02-02-tools.md` |
| S02-03 | models 与流式数据类型移植 | S01-01 | `done` | `phase-02/S02-03-models.md` |
| S02-04 | storage 移植与数据目录 | S02-03 | `todo` | `phase-02/S02-04-storage.md` |
| S02-05 | 流式取消语义与事件契约 | S02-01 | `todo` | `phase-02/S02-05-stream-cancel.md` |
| S02-06 | tokio / GPUI 执行器桥接 | S02-05 | `todo` | `phase-02/S02-06-executor-bridge.md` |
| S02-07 | 工具调用与审批的请求-响应配对 | S02-05 | `todo` | `phase-02/S02-07-tool-approval.md` |
| S02-08 | IPC 层作废（命令 → engine API 覆盖表） | S02-01..S02-07 | `todo` | `phase-02/S02-08-ipc-retire.md` |
| S02-09 | 引擎测试迁移与契约测试 | S02-01..S02-04 | `todo` | `phase-02/S02-09-engine-tests.md` |

---

## Phase 03 — 主题与设计令牌

> 拆解依据：`docs/tasks/v2.0.0-gpui/03-theme.md`

| ID | Spec | 依赖 | 状态 |
|----|------|------|------|
| S03-01 | Theme 结构、Appearance 与全局安装 | S01-01 | `todo` |
| S03-02 | 颜色令牌迁移（品牌色 / 状态色 / 中性阶） | S03-01 | `todo` |
| S03-03 | 外观令牌迁移（不透明填充 + 圆角 + 阴影） | S03-01, S00-04 | `todo` |
| S03-04 | 字体与排版令牌迁移 | S03-01 | `todo` |
| S03-05 | 平台字体栈与 TextRenderingMode | S03-04 | `todo` |
| S03-06 | 主题切换与系统外观跟随 | S03-01 | `todo` |
| S03-07 | 令牌完备性与硬约束校验（脚本） | S03-02..S03-05 | `todo` |

---

## Phase 04 — Markdown 栈

> 拆解依据：`docs/tasks/v2.0.0-gpui/04-markdown.md`

| ID | Spec | 依赖 | 状态 |
|----|------|------|------|
| S04-01 | vendor markdown.rs 并 patch 掉 ThemeSettings（9 处） | S01-02 | `todo` |
| S04-02 | 引入 Comet syntax 替换 zed language | S04-01 | `todo` |
| S04-03 | 闭包收敛与依赖清理（含 mermaid / html 裁剪决策） | S04-02 | `todo` |
| S04-04 | 块粒度增量解析与后台合并 | S04-03 | `todo` |
| S04-05 | 半截标记修补（mend） | S04-04 | `todo` |
| S04-06 | 流式渐显（veil） | S04-04 | `todo` |
| S04-07 | 代码块与语法高亮 | S04-02 | `todo` |
| S04-08 | GFM 元素（表格 / 任务列表 / 删除线 / 链接 / 图片） | S04-03 | `todo` |
| S04-09 | 文本选择与复制 | S04-03 | `todo` |

---

## Phase 05 — 聊天界面

> 拆解依据：`docs/tasks/v2.0.0-gpui/05-chat-ui.md`

| ID | Spec | 依赖 | 状态 |
|----|------|------|------|
| S05-01 | Transcript 虚拟列表骨架（ListState） | S03-01 | `todo` |
| S05-02 | 块粒度行模型与稳定 id | S05-01 | `todo` |
| S05-03 | 行高记忆与测量 | S05-02 | `todo` |
| S05-04 | 跟尾弹簧与回到底部 | S05-03 | `todo` |
| S05-05 | 历史加载与滚动（替换手动分页） | S05-04 | `todo` |
| S05-06 | Composer 文本输入与 IME | S00-05, S03-04 | `todo` |
| S05-07 | Composer 附件（粘贴 / 拖拽 / 选择 / 草稿） | S05-06 | `todo` |
| S05-08 | 消息行渲染（用户 / 助手 / 流式态 / 错误态） | S04-04, S05-02 | `todo` |
| S05-09 | 思考块 | S05-08 | `todo` |
| S05-10 | 工具调用分组（ToolGroup / Deferred） | S05-08 | `todo` |
| S05-11 | 网络搜索卡片 | S05-10 | `todo` |
| S05-12 | 图片生成卡片 | S05-10 | `todo` |
| S05-13 | 提问与审批卡片 | S05-08, S02-07 | `todo` |
| S05-14 | 消息操作（复制 / 重试 / 编辑） | S05-08 | `todo` |
| S05-15 | 模型选择器 | S05-06, S02-01 | `todo` |
| S05-16 | 空态与无 Key 态 | S05-06 | `todo` |
| S05-17 | 状态实体拆分与细粒度通知 | S05-01 | `todo` |
| S05-18 | 页面状态机 | S05-16 | `todo` |

---

## Phase 06 — 设置界面

> 拆解依据：`docs/tasks/v2.0.0-gpui/06-settings-ui.md`

| ID | Spec | 依赖 | 状态 |
|----|------|------|------|
| S06-01 | 设置页骨架与控件集 | S03-01 | `todo` |
| S06-02 | Provider 卡片与新增流程 | S06-01, S02-01 | `todo` |
| S06-03 | 模型列表与上下文配置 | S06-02 | `todo` |
| S06-04 | MCP 配置界面 | S06-01, S02-03 | `todo` |
| S06-05 | 热键录制交互 | S06-01, S00-03 | `todo` |
| S06-06 | 主题设置界面 | S06-01, S03-06 | `todo` |

> 原 S06-07（更新设置界面）已移至 **S08-11**，原 S06-08（自启与数据目录设置）已移至 **S07-13**。
> 原因：它们的后端产出在 Phase 07/08，留在 Phase 06 会造成跨 Phase 前向依赖，违反 `RULES.md` §9.1 的单调性不变式。
> 本次调整发生于注册表建立初期、尚无任何实现，因此直接重编号而非标记废弃。

---

## Phase 07 — 应用外壳与窗口行为

> 第二高风险区。准入依赖 Phase 00 的 S00-02 / S00-03。
> 拆解依据：`docs/tasks/v2.0.0-gpui/07-shell.md`

| ID | Spec | 依赖 | 状态 |
|----|------|------|------|
| S07-01 | 窗口创建工厂与配置 | S01-01 | `todo` |
| S07-02 | 无装饰 / 去阴影 / acceptsFirstMouse | S00-02 | `todo` |
| S07-03 | 全局热键唤起与切换 | S00-03 | `todo` |
| S07-04 | Esc / 点击外部关闭（不断流） | S07-02 | `todo` |
| S07-05 | 置顶与全工作区可见 | S07-02 | `todo` |
| S07-06 | 多显示器与窗口定位 | S07-01 | `todo` |
| S07-07 | 窗口拖动与选择隔离 | S07-02 | `todo` |
| S07-08 | 入场动画与减弱动效 | S07-02 | `todo` |
| S07-09 | tray 图标与菜单 | S00-03 | `todo` |
| S07-10 | 开机自启 | S00-03 | `todo` |
| S07-11 | 单实例与生命周期（无窗口存活 / 休眠唤醒） | S07-01 | `todo` |
| S07-12 | 窗口行为自检模式（探测静默 no-op） | S07-02 | `todo` |
| S07-13 | 自启与数据目录设置 UI | S07-01, S07-10 | `todo` |

---

## Phase 08 — 更新与发布

> 拆解依据：`docs/tasks/v2.0.0-gpui/08-release.md`

| ID | Spec | 依赖 | 状态 |
|----|------|------|------|
| S08-01 | 更新器选型与清单格式 | S07-11 | `todo` |
| S08-02 | 下载 / sha256 校验 / 验签 | S08-01 | `todo` |
| S08-03 | macOS bundle 替换安装 | S08-02 | `todo` |
| S08-04 | Windows 安装流程 | S08-02 | `todo` |
| S08-05 | 静默检查与空闲调度 | S08-01 | `todo` |
| S08-06 | 打包脚本（macOS app + dmg） | S08-03 | `todo` |
| S08-07 | 打包脚本（Windows） | S08-04 | `todo` |
| S08-08 | 签名与公证 | S08-06 | `todo` |
| S08-09 | CI 流水线与版本一致性守卫 | S08-06, S08-07 | `todo` |
| S08-10 | GPL 合规产出（源码链接 / 随包许可 / patch 归档） | S01-03 | `todo` |
| S08-11 | 更新设置界面与手动检查入口 | S08-05, S06-01 | `todo` |

---

## Phase 09 — 平台对齐（Windows）

> **准入：macOS 全链路验收通过。**
> 拆解依据：`docs/tasks/v2.0.0-gpui/09-windows.md`

| ID | Spec | 依赖 | 状态 |
|----|------|------|------|
| S09-01 | 渲染差异盘点与圆角 / 阴影对齐 | — | `todo` |
| S09-03 | 字体与文字渲染标定 | S09-01 | `todo` |
| S09-04 | WindowKind 语义复核与静默失效排查 | S07-12 | `todo` |
| S09-05 | 点击外部 / Esc / 尺寸不变复核 | S09-04 | `todo` |
| S09-06 | DPI 与多显示器 | S09-04 | `todo` |
| S09-07 | 输入法矩阵测试（含第三方输入法） | S05-06 | `todo` |
| S09-08 | tray / 热键 / 自启的 Windows 实现 | S07-03, S07-09, S07-10 | `todo` |
| S09-09 | Windows 打包与更新 | S08-07, S08-04 | `todo` |
| S09-10 | 平台差异清单与接受标准 | S09-01, S09-03..S09-07 | `todo` |

---

## Phase 10 — 测试与验收

> 与 Phase 02-09 并行。拆解依据：`docs/tasks/v2.0.0-gpui/10-testing.md`

| ID | Spec | 依赖 | 状态 |
|----|------|------|------|
| S10-01 | 分层测试基础设施（L1-L7） | S01-01 | `todo` |
| S10-02 | 硬约束验证清单（AGENTS.md 1-10） | S05-18, S07-05 | `todo` |
| S10-03 | 页面视觉验收与 v1 基线比对 | S01-06, S06-01, S05-18 | `todo` |
| S10-04 | 端到端关键路径 | S05-18, S07-13 | `todo` |
| S10-05 | 性能基线与新旧对比 | S05-05 | `todo` |
| S10-06 | 稳定性测试（长跑 / 异常 / 存储原子性） | S02-04 | `todo` |
| S10-07 | 验收报告与已知限制 | S10-01..S10-06 | `todo` |

---

## 状态变更记录

> 每次状态变更在此追加一行（便于回溯）。

| 日期 | Spec | 变更 | 备注 |
|------|------|------|------|
| — | — | 注册表建立 | 初始 104 个 spec |
| — | S01-06 | 新增 | 建立 v1 视觉与行为基线。原方案打算「对照 `docs/design/prototypes/` 验收」，但审计发现该目录**根本不存在**；真实视觉基准只能是当前 v1 Tauri 应用的实机渲染，必须在 UI 移植前采集 |
| — | S06-07 → S08-11 | 重编号 | 消除跨 Phase 前向依赖（原依赖 S08-05），违反 `RULES.md` §9.1 |
| — | S06-08 → S07-13 | 重编号 | 消除跨 Phase 前向依赖（原依赖 S07-10），并改名为「自启与数据目录设置 UI」 |
| — | S10-03 | 调整 | 依赖由 `S06-01,S05-18` 改为 `S01-06,S06-01,S05-18`；验收基准由不存在的 prototypes 改为 v1 基线截图 |
| — | S10-04 | 调整 | 依赖 `S06-08` → `S07-13` |
| 2026-09-10 | S00-01 | `todo` → `doing` → `done` | 实测通过。产出三个非显然前置条件（`runtime_shaders` / `font-kit` / 自实现 `ThemeSettingsProvider`）、闭包实测值 30（原估 12 为下界）、代理为硬前置。已回写 research-log §9.4 / §8 / §10 |
| 2026-09-10 | S00-02 | `todo` → `doing` → `done` | 实测通过。**窗口外壳不需要 fork GPUI**；`PopUp` 是唯一含非激活面板+全工作区的 kind；`WindowOptions` 无法得到零装饰（objc2 必需）；`acceptsFirstMouse` 已由 gpui 提供。硬约束 1/6/7 均 PASS。产物固化到 `docs/evidence/s00-02/window-patch.rs`。已回写 research-log §11 |
| 2026-09-10 | S00-03 | `todo` → `doing` → `done` | **证伪条件未触发**：三者与 GPUI 共用事件循环，无需另起。tray 未抢占 `NSApp` delegate；`TrayIcon` 非 `Send/Sync` 必须主线程；objc2 版本无分裂；autostart 全流程通过。闭包 +50（693→743）。产物固化到 `docs/evidence/s00-03/shell-integration.rs`。已回写 research-log §12（含 R1 降级） |
| 2026-09-10 | S00-04 | `todo` → `doing` → `done` | **产品决定不使用毛玻璃**（与 v1 一致）。最终外观：不透明实色 + 16px 圆角 + 无边框 + 无模糊。**不需要 fork** → R3 消除。发现 v1 本身从未实现毛玻璃（`macos.rs:154`）。`--glass-outline` 白边框被用户否决。产物固化到 `docs/evidence/s00-04/window-appearance.rs`。已回写 research-log §13 |
| 2026-09-10 | **S09-02** | **删除** | 原为「毛玻璃三选一与 Win10 回退」。产品不使用系统 backdrop → 前提消失，风险 R5 消除。Phase 09 由 10 spec 减为 9，总数 105 → 104 |
| 2026-09-10 | S03-03 | 改名 | 「玻璃与阴影令牌迁移」→「**外观**令牌迁移（不透明填充 + 圆角 + 阴影）」，不再涉及 blur 参数标定 |
| 2026-09-10 | S09-01 / S09-10 | 依赖调整 | S09-02 删除后修正依赖引用，避免悬空 |
| 2026-09-11 | S00-05 | `todo` → `doing` → `done` | 中文 IME 全流程通过。**发现官方 `input.rs` 示例自带 4 个缺陷**（2 个致命：一装输入法就崩、多行 panic）。实现多行渲染（`shape_line` 拒绝 `\n`）。Enter 三态由 gpui 平台层保证。产物固化到 `docs/evidence/s00-05/ime-input.rs`。已回写 research-log §14 |
| 2026-09-11 | S00-06 | `todo` → `doing` → `done` | **闭包收敛达成：718 包（基线 693，markdown 栈仅 +25）**。两个阻碍级发现：① 必须复制 zed 的 `[patch.crates-io]`（`[patch]` 不传递给下游）；② **`language` 必须移除**（否则 `settings` 被拖回 + 需 cmake）。关键技巧：用 API 兼容 shim，patch 从 21 处降到 1 行。流式渲染与行尾稳定性已验证。代价：语法高亮裁掉 → `S04-02`。产物固化到 `docs/evidence/s00-06/`。已回写 research-log §15 |
| 2026-09-11 | S00-07 | `todo` → `doing` → `done` | **虚拟化 3.55 行/帧 vs 1005 总行（0.35%）**。跟尾状态机自动打断/恢复。**四个实现陷阱**（`flex_grow_1` 缺失→静默渲染 0 行；回调内碰 ListState→panic；Bottom 对齐必须 `measure_all()`，249ms/1000 行；`FollowMode` 需配显式 `scroll_to_end()`），且陷阱 1 掩盖陷阱 2。v1 的 `bottomFollow.ts` + `useSmoothWheelScroll.ts` + 手动分页可整体删除。产物固化到 `docs/evidence/s00-07/list-integration.rs`。已回写 research-log §16 |
| 2026-09-11 | S00-08 | `todo` → `doing` → `done` | **端到端跑通**：v1 真实配置 → `create_provider` → 真实 API → SSE → 113 事件 → markdown 渲染。**引擎移植只需 4 处改动**（5255 行）。**IPC 层消失**（`commands.rs` 1868 行不再需要）。**风险 R7 实证可消除**（engine 依赖树 0 处 GPUI/Tauri/zed）。四个陷阱：`Done.full_text` 含 ` thinking` 标签（差 15 字符，精确对上）；内联 think 字符数 `StreamOutcome` 拿不到；tokio 任务不能捕获 `Rc`/`Cell`；future 需 `'static`。产物固化到 `docs/evidence/s00-08/`。已回写 research-log §17 |
| 2026-09-11 | **S00-09** | `todo` → `doing` → `done` | **决策：Go**。两个硬门槛通过；**9 项风险中 4 项消除（R1/R3/R5/R7）、1 项降级（R2）**；端到端跑通；**完全不需要 fork GPUI**；闭包 770 包（+77）毒性依赖零残留。**Phase 00 完成（9/9）**，Phase 01 准入已满足。已回写 research-log §18 |
| 2026-09-11 | S01-01 | `todo` → `doing` → `done` | 三层 workspace 落地（engine MIT / ui GPL / app GPL）。**依赖方向机检通过**（engine 树 0 处 GPUI）。**v1 未被破坏**（`exclude` + `cargo check` 回归验证）。GPUI 表面从 `buddy-ui` 重导出（app 不直接依赖 zed）。两个 feature 硬前置已生效，运行时 0 panic、0 文字警告。新增 `docs/dev-environment.md` |
| 2026-09-11 | S01-02 | `todo` → `doing` → `done` | **依赖来源定为 B（全部同一 zed git rev）**：四者中三个未发布，只能 git。**31 个 zed crate 同一 rev**（版本号不同但 rev 一致）。**平台层全部 Apache-2.0**，GPL 仅 `theme`/`ui`。干净目录 `--locked` 构建通过（**442 crate**，28.49s，关键 crate 全部编译）。**不 fork**（R3 消除） |
| 2026-09-11 | S01-03 | `todo` → `doing` → `done` | 三层许可证声明（engine MIT / ui·app GPL）。**GPL 全文随仓库分发**（34KB，取自 zed rev）。**全量许可统计 746 包**（MIT/Apache 家族约 720；**GPL 精确为 7 个且全在 UI 层**；MPL-2.0 7 个未修改）。**3 项验收标准范围修正并移交** `S04-01`/`S04-02`（vendored 产物尚不存在）|
| 2026-09-11 | S01-04 | `todo` → `doing` → `done` | `scripts/check-discipline.py`：**14 项检查全部通过 + 8 项拦截验证全部通过**。CI 集成（含拦截验证）。修正一处测试意图（注入 `gpui` 而非 `buddy-ui`，否则走循环依赖报错路径）。收窄文档路径检查到 `docs/`（附理由）。**顺带发现 `[patch.crates-io]` 5 项中 3 项未被使用**，已移除并注明何时加回。完整许可证扫描交接 `S08-09` |
| 2026-09-26 | S01-06 | 范围调整（仍为 `todo`） | **用户决策：不截图、不录屏**，视觉/行为验收改为用户对照运行中的 v1 目检。取消 S01-06-4/5/6/7，保留清单、令牌、性能、长会话、数据快照。不可再生资产由截图转为「可运行的 v1」→ 由 S01-05 退路 tag 保证。S10-03 的比对基准相应改为「用户对照 v1 目检」，展开 Phase 10 时落实 |
| 2026-09-26 | S01-05 | `todo` → `doing` | 用户选 (a)：v1 改动单独提交 `9cc244a` → tag `v1-final` + 分支 `v1-fallback`。回退演练通过（tsc / vite build / vitest 99 / cargo build / cargo test 166）。**待用户**：推送与分支保护、v1 运行目检。用户另决定不做历史数据迁移 |
| 2026-09-26 | Phase 02 | 展开 9 个 spec 文件 + 依赖/范围修正 | 依源码实测修正依赖：`models/message.rs:16` ↔ `streaming.rs:22` 互相引用 → streaming 数据类型与 emitter 去 Tauri 并入 **S02-03**；S02-02/S02-04 依赖 S02-03；S02-01 依赖 S02-02+S02-03。**S02-05** 改名「流式取消语义与事件契约」（去 Tauri 已随 S02-03 完成，剩余真正未验证项是取消）。**S02-08** 改为「覆盖表 + 作废契约」，**不删 v1 的 `commands.rs`**（会破坏 v1）。S02-04 取消旧数据回归（用户决策） |
| 2026-09-26 | S02-03 | `todo` → `doing` → `done` | models/mcp 与 v1 逐字节一致；streaming 仅 emitter 改 channel（25 行差异）。测试 29/29，与 v1 同模块数量一致。engine 0 warning、0 处 tauri/gpui。commit `1f50d80` |
| 2026-09-26 | S02-02 | `todo` → `doing` → `done` | tools 15 文件与 v1 逐字节一致；测试 81（ignored 8）与 v1 一致。commit `32204d8` |
