# S00-09 Spike 结论与 Go/No-Go 决策

> 状态: `done`
> Phase: 00
> 依赖: S00-01 ~ S00-08（全部 `done`）
> 阻塞: —
> 退役设计文档: —

## 目标

汇总 Phase 00 全部结论，给出明确的 Go / No-Go 决策，并初始化 Phase 01。

## 硬性门槛核对

| Spec | 门槛 | 结果 |
|------|------|------|
| S00-02 | 「无边框 / 透明 / 非激活 / 全工作区 / 点击外部关闭」全部达成 | ✅ **通过** |
| S00-03 | 全局热键 + tray + autostart 与 GPUI 事件循环共存 | ✅ **通过** |

> 阈值未触发 → **无 No-Go 理由**。

---

## 1. 结论表

| Spec | 结论 | 关键证据 | 意外发现 |
|------|------|---------|---------|
| **S00-01** | ✅ 通过 | `theme` + `ui` + `gpui` 编译运行；闭包 30 zed crate / 693 包 | **三个非显然前置条件**（缺任一即失败）：`runtime_shaders`（否则需完整 Xcode）、`font-kit`（否则**完全没有文字**，静默）、自实现 `ThemeSettingsProvider`（否则开窗 panic）。另：github 必须走代理（直连 40min 未完成 vs 代理 865s） |
| **S00-02** | ✅ 通过 | styleMask `0x8b → 0x88`；硬约束 1/6/7 PASS | **窗口外壳不需要 fork**。`WindowKind::PopUp` 是唯一同时含 `NonactivatingPanel` + `CanJoinAllSpaces` 的取值；`WindowOptions` **无法**得到零装饰窗口；`acceptsFirstMouse` / `canBecomeKeyWindow` 已由 gpui 提供（推翻原计划「需手写」） |
| **S00-03** | ✅ 通过 | 热键 7 次 / tray 菜单 3 次 / GPUI 心跳 90 tick 交错；autostart 全流程 | **三者与 GPUI 共用事件循环**（无需另起）；`tray-icon` 未抢占 `NSApp` delegate；**`TrayIcon` 非 `Send`/`Sync`** 必须主线程 |
| **S00-04** | ✅ 通过 | `CABackdropLayer` 实测存在；圆角 `cornerRadius=16` 验证 | **产品决定不使用毛玻璃**（与 v1 一致）→ **完全不需要 fork**，R3/R5 消除。发现 **v1 本身从未实现毛玻璃**（`macos.rs:154` 明写不启用 vibrancy）。`--glass-outline` 白边框被用户否决 |
| **S00-05** | ✅ 通过 | 34 次组字更新 / 8 次中文提交 / **组字期间 Enter 0 次**到达应用 | **官方 `input.rs` 示例自带 4 个缺陷**（2 个致命：一装输入法就崩、多行 panic）。`shape_line` 拒绝 `\n`。Enter 三态由 gpui 平台层保证（前提是 `marked_text_range()` 正确实现） |
| **S00-06** | ✅ 通过 | **闭包 718 包（+25）**；流式 306 字符逐字无重排 | **必须复制 zed 的 `[patch.crates-io]`**（`[patch]` 不传递给下游）；**`language` 必须移除**（否则 `settings` 被拖回 + 需 cmake）。关键技巧：用 API 兼容 shim，patch 从 21 处降到 3 行 |
| **S00-07** | ✅ 通过 | **3.55 行渲染/帧 vs 1005 总行（0.35%）**；跟尾状态机自动打断/恢复 | **四个实现陷阱**（`flex_grow_1` 缺失→静默渲染 0 行；回调内碰 `ListState`→panic；Bottom 对齐必须 `measure_all()`，249ms/1000 行；`FollowMode` 需配显式 `scroll_to_end()`）。陷阱 1 掩盖陷阱 2 |
| **S00-08** | ✅ 通过 | 真实 API → 113 事件 → markdown 渲染；`had_stream_error = false` | **引擎移植只需 4 处改动**（5255 行）；**IPC 层消失**；**R7 实证可消除**（engine 依赖树 0 处 GPUI/Tauri/zed）；`Done.full_text` 含 ` thinking` 标签（差 15，精确对上） |

### 最终闭包

| 分层 | 包数 |
|------|------|
| GPUI 基线（S00-01） | 693 |
| + markdown 栈（S00-06） | +25 |
| + engine（S00-08：reqwest / tokio / …） | +52 |
| **全栈合计** | **770（+77）** |

**毒性依赖零残留**：`settings` 全家桶 / `lsp` / `rpc` / `wasmtime` / `tree-sitter` / `language` / `theme_settings` / `mermaid_render` / `editor` / `fs` —— 全部不在闭包内。

---

## 2. 风险复评

| # | 风险 | 原等级 | **复评后** | 依据 |
|---|------|-------|-----------|------|
| R1 | 窗口外壳（含 tray/热键冲突） | 高 | **消除** | S00-02 证明不需要 fork；S00-03 证明 tray 不抢 delegate、三者共用事件循环 |
| R2 | Windows `WindowKind::Floating` 静默 no-op | 高 | **中**（降级） | macOS 侧已定 `PopUp` 为正确取值；Windows 侧仍需 `S09-04` 真机验证 |
| R3 | GPUI fork 维护成本 | 中高 | **消除** | S00-04 定不 fork → 外壳与外观均用运行时 objc2 达成 |
| R4 | Windows 字体与文字渲染差异 | 中高 | **中高（不变）** | 与玻璃无关的部分仍在（PingFang SC vs Microsoft YaHei）。归 `S03-05` / `S09-03` |
| R5 | Win10 无 Mica / Mica 采样壁纸 | 中 | **消除** | 不使用系统 backdrop，差异不存在。`S09-02` 已删除 |
| R6 | Windows IMM32 输入法长尾 | 中 | **中**（macOS 侧已过） | `S00-05` 验证 macOS 可用；Windows 第三方输入法待 `S09-07` |
| R7 | GPL 传染失控 | 中 | **可消除（已实证）** | S00-08 验证 engine 依赖树 0 处 GPUI/Tauri/zed。`S01-04` 的 CI 断言有实施基础 |
| R8 | 无法回退 | 中 | **中（不变）** | 仍需 `S01-05` 建立退路分支；严守「macOS 先行」 |
| R9 | 既有设计资产丢失 | 低 | **低，但暴露一类新问题** | `docs/design/prototypes/` 与「毛玻璃」**两例「设计意图被当成既成事实」**。已在 `RULES.md` §11 立规则 |

**净效果：9 项风险中 4 项消除、1 项降级。**

---

## 3. 决策：**Go**

### 理由

1. **两个硬性门槛全部通过**（S00-02 / S00-03），无任何 No-Go 触发条件
2. **9 项风险中 4 项消除、1 项降级**，剩余项均已定位到具体 spec
3. **完全不需要 fork GPUI** —— 这是最大的成本项，原本预估「必然需要」，实际证明不需要
4. **闭包可控**：770 包（+77），且毒性依赖零残留
5. **端到端已跑通**：真实 API → SSE → markdown 渲染（S00-08）
6. **代价清楚且可接受**：裁掉语法高亮（`S04-02` 用 Comet MIT 方案补回）

### 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| **Go / No-Go** | **Go** | 两个硬门槛通过；4 项风险消除；端到端跑通 |
| **是否 patch GPUI fork** | **不 fork** | 窗口外壳（S00-02/03）与外观（S00-04）全部用运行时 objc2 达成；唯一的 fork 理由（窗内 backdrop blur）产品不需要 |
| GPUI 依赖来源 | **纯 zed git rev**（同一 rev） | 四者必须同 rev；见 `S01-02` |
| 是否继续回退预案 | **保留** | `S01-05` 建立退路分支，覆盖到首次正式发布之后 |

---

## 4. 交给 Phase 01+ 的关键输入

Phase 00 期间发现的事项，按归属分发：

| 发现 | 归属 |
|------|------|
| **必须复制 zed 的 `[patch.crates-io]`**（5 项，macOS markdown 场景） | `S01-01` / `S01-02` |
| 三个 GPUI 接入硬前置（`runtime_shaders` / `font-kit` / `ThemeSettingsProvider`） | `S01-01` / `S01-02` |
| 代理是硬前置（github 直连不可用） | `S01-01`（开发者环境说明） |
| vendor 文件的 GPL 释出（3 行 import patch） | `S01-03` |
| engine 分层实证（0 处 GPUI/Tauri）→ CI 断言 | `S01-04` |
| 退路分支（GPL 一旦发布不可回退） | `S01-05` |
| v1 视觉基线必须在 UI 移植前采集 | `S01-06`（**不可再生资产**） |
| `Done.full_text` 含 think 标签；内联 think 统计 | `S02-05` / `S05-08` / `S05-09` |
| tokio 任务不能捕获 `Rc`/`Cell`；future 需 `'static` | `S02-06` |
| `ListState` 四个陷阱；`measure_all` 249ms/1000 行 | `S05-01` ~ `S05-04` |
| `shape_line` 拒绝 `\n`；官方 `input.rs` 4 个缺陷 | `S05-06` |
| **图层/属性探测必须延迟复探** | `S07-12` / `S09-01` / `S09-04` |
| **最终验收须用「外壳 + 外观」完整窗口** | `S10-03` |
| Windows 侧仍需验证：`Floating` 语义 / 字体 / IME | `S09-04` / `S09-03` / `S09-07` |

---

## 5. Windows 前评估

**结论：维持「macOS 先行，Windows 第二阶段」**，理由：

| 项 | 状态 |
|----|------|
| macOS 侧全部验证 | ✅ 8/8 通过 |
| Windows 侧验证 | ❌ **零验证**（本机无 Windows；Comet 也不支持 Windows） |
| 已知 Windows 特有风险 | `Floating` 静默 no-op（R2）；字体/文字渲染差异（R4）；IMM32 输入法长尾（R6） |
| Comet 的参考价值 | **对 Windows 为零**（CI 仅 macOS + Linux） |

**不改变原计划**：`S09-*` 仍在 macOS 全链路验收通过后启动。理由见 `research-log` 风险 R8 ——
若一开始双平台并行，一旦 macOS 阶段发现成本失控，将无法退回 Tauri。

---

## 6. Phase 01 初始化

Phase 01 的 6 个 spec 状态已为 `todo`，现在正式展开为可执行。

| Spec | 状态 | 依赖 | 备注 |
|------|------|------|------|
| S01-01 三层 workspace 结构与工具链 | `todo` | **S00-09 Go** ✅ 已满足 | **已补充** `[patch.crates-io]` 与三个硬前置 |
| S01-02 GPUI 依赖来源与 rev 锁定 | `todo` | S00-01 ✅ | fork 决策已定：**不 fork** |
| S01-03 许可证分层声明与 NOTICE | `todo` | S01-01 | **已补充** vendor 文件与 patch 释出要求 |
| S01-04 防 GPL 污染 CI 断言 | `todo` | S01-03 | 15 项校验已定义（含 Phase 00 发现驱动的 7 项） |
| S01-05 迁移期目录与退路分支 | `todo` | S01-01 | 退路需覆盖到首次正式发布之后 |
| S01-06 建立 v1 视觉与行为基线 | `todo` | S01-01 | **不可再生，应最先执行** |

### 建议的执行顺序

```
S01-06（v1 基线采集）← 不可再生，最先做
S01-01（workspace 骨架）
S01-02（GPUI 依赖锁定）  ← 需 S01-01
S01-03（许可证分层）      ← 需 S01-01
S01-04（CI 断言）         ← 需 S01-03
S01-05（退路分支）        ← 可并行
```

> **`S01-06` 必须最先执行**：v1 的 UI 会在 Phase 05 起被替换，
> 而「7 个页面原型」经审计确认**根本不存在**，v1 的实机渲染是唯一视觉基准。
> 一旦开始改 UI，这个基准就永久丢失。

---

## 证据

| 项 | 证据 |
|----|------|
| 结论表 | 8 个 spec 的「证据」段（每项均含可复核材料） |
| 硬门槛 | S00-02 / S00-03 的验收标准全部勾选 |
| 闭包 | `Cargo.lock` 解析：770 包；毒性依赖 grep 零命中 |
| 风险复评 | 本文 §2，每项附依据来源 |
| 端到端 | S00-08 的日志（113 事件，`had_stream_error = false`）+ 用户确认 |
| 产物 | `docs/evidence/` 下 5 个 spec 的产物，共 2341 行 |

## 完成记录

- 日期：2026-09-11
- commit：（本 spec 无代码产物；结论见本文）
- 设计文档处置：**未删除任何设计文档**。Phase 00 是验证阶段，不涉及实现退役。
  `docs/design/*` 将在对应实现 spec 完成后按 `RULES.md` §7 逐份退役。

## 遗留与交接

### Phase 00 未做的事（如实记录）

| 项 | 说明 |
|----|------|
| **Windows 零验证** | 本机无 Windows。所有 Windows 结论来自源码阅读，非实测 |
| 内存占用 | 未在任何 spec 采集 → `S10-05` |
| 滚动 FPS 精确值 | S00-07 只测了渲染次数，未测 FPS |
| 取消生成 | S00-08 未实测 → `S02-05` |
| 工具调用链路 | S00-08 传空 `tools` → `S02-02` / `S02-07` |
| providers 既有测试 | 未运行 → `S02-09` |
| 软换行（markdown 长行折行） | S00-06 未实现 |
| 视口上方高度变化的滚动锚点 | S00-07 未验证 → `S05-03` |
| 主题切换时模糊不丢失 | S00-04 不适用（不用系统模糊） |

### 一个需要延续的纪律

Phase 00 反复暴露同一类问题：**「文档里的意图被当成了既成事实」**。

| 实例 | 真相 |
|------|------|
| `docs/design/prototypes/` | AGENTS.md 索引 + CLAUDE.md + tasks/v1.0.0 层层引用，**路径从未存在** |
| 「毛玻璃」 | AGENTS.md Design Philosophy 写着，但 v1 `macos.rs:154` 明写**不启用 vibrancy** |
| 「`ThemeSettings` 要手工 patch 9 处」 | 实际只需 1 行 import + shim |
| 「`acceptFirstMouse` 需手写」 | gpui 已提供 |

已在 `RULES.md` §11 立规则（路径引用必须验证、单一真相源）。
**Phase 01+ 首次接触任何文档结论时，应先验证再采用。**
