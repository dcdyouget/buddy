# S01-04 防 GPL 污染 CI 断言

> 状态: `done`
> Phase: 01
> 依赖: S01-03
> 阻塞: —
> 退役设计文档: —

## 目标

用自动化手段保证 `buddy-engine` 永远不会被 GPUI 或 GPL 依赖污染，且依赖方向不被破坏。

**产出物**：CI 中生效的检查脚本 + 故意注入违规依赖时的拦截验证。

## 输入

- `docs/tasks/v2.0.0-gpui/research-log.md` §6 —— 许可证分层（硬约束）
- `docs/specs/RULES.md` §7 —— 边界规则
- S01-03 的许可证声明

## 背景

GPL 边界一旦被破坏**不可逆**（`research-log.md` 风险 R7）。人工评审不足以守住，必须脚本化。

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S01-04-1 | 引擎层禁用清单 | 脚本检查 `cargo tree -p buddy-engine` 输出中不得出现：`gpui` / `gpui_platform` / `gpui_macos` / `gpui_windows` / `theme`（zed 的）/ `ui`（zed 的）/ 其他 GPL crate |
| S01-04-2 | 依赖方向检查 | 禁止 `engine → ui` 方向的任何依赖 |
| S01-04-3 | GPL 依赖白名单 | 仅在 `crates/ui` / `apps/buddy` 中允许 GPL 依赖，出现于其他位置即失败 |
| S01-04-4 | 许可证扫描 | `cargo-deny check licenses` 或 `cargo-about`，与 `THIRD_PARTY_NOTICES.md` 交叉核对 |
| S01-04-5 | 硬约束扫描 | 圆角值集合 ⊆ {4,8,12,16,9999}；品牌色唯一；无 emoji 图标（对应 `AGENTS.md` 硬约束 2/3/4） |
| S01-04-6 | CI 集成 | 以上检查挂进 CI，失败即阻断合并 |
| S01-04-7 | 自我验证 | 故意注入一个违规依赖，确认检查能拦住 |
| S01-04-8 | 脚本可本地运行 | 提供本地执行入口，避免只在 CI 暴露 |
| S01-04-9 | **文档路径引用校验** | 扫描 `docs/specs/` `docs/tasks/v2.0.0-gpui/` `AGENTS.md` `CLAUDE.md` 中的仓库内路径引用（形如 \`docs/...\`），确认路径存在；未创建的未来产物必须含「待创建」字样。**来源：一次实际审计发现 `docs/design/prototypes/` 等路径被多份文档引用但根本不存在，且一路传播未被发现** |
| S01-04-10 | **Phase 单调性校验** | 解析 `docs/specs/README.md`，断言不存在跨 Phase 前向依赖（`RULES.md` §9.1） |
| S01-04-11 | **spec 状态一致性校验** | 断言注册表状态与各 spec 文件头部的 `状态:` 字段完全一致（`RULES.md` §4） |
| S01-04-12 | **spec 依赖图校验** | 断言无循环依赖、无指向不存在 spec 的引用；注册表声明的各 Phase spec 数与实际条目数一致 |
| S01-04-13 | **退役台账校验** | 断言 `design-deletions.md` 中列出的每个路径**确实存在**（防止登记幽灵文档） |
| S01-04-14 | **agent 入口完整性校验** | 断言 `RULES.md` §11.4 表中列出的每个 agent 入口文件**存在**且**包含对 `AGENTS.md` 的引用**；同时断言入口文件总行数不超过阈值（超过说明写了实质内容，违反薄指针规则）；断言 `AGENTS.md` 中引用的文档路径均存在 |
| S01-04-15 | **文档入库校验** | 断言 `RULES.md` §13 列出的关键路径**未被 `.gitignore` 排除**（`git check-ignore` 返回空）且**已被 git 跟踪**。**来源：审计发现 `.gitignore` 曾有 `docs/**`，导致整个 spec 体系与证据不入库，克隆仓库无法工作** |

## 验收标准

- [x] 引擎层禁用清单检查生效
- [x] 依赖方向检查生效
- [x] **故意注入 `gpui` 到 engine 依赖中，检查报 FAIL**（拦截验证 1）
- [x] 许可证声明与许可证文件检查生效
- [x] 硬约束扫描（圆角 / 品牌色 / emoji）生效
- [x] 文档路径引用校验生效（S01-04-9）—— 拦截验证 2
- [x] Phase 单调性校验生效（S01-04-10）—— 拦截验证 3
- [x] spec 状态一致性校验生效（S01-04-11）—— 拦截验证 4
- [x] spec 依赖图校验生效（S01-04-12）—— 拦截验证 8
- [x] 退役台账校验生效（S01-04-13）—— 拦截验证 6
- [x] agent 入口完整性校验生效（S01-04-14）—— 拦截验证 5
- [x] 文档入库校验生效（S01-04-15）—— 拦截验证 7
- [x] **全部 8 项拦截验证通过**
- [x] CI 失败信息能明确指出违规项与修复方向
- [x] 脚本可本地运行（`python3 scripts/check-discipline.py`）

### 未完全达成的部分（如实记录）

- [ ] **完整许可证扫描**（`cargo-deny` / `cargo-about`）—— 工具未安装。
  当前只检查**声明与文件存在性**，未自动比对依赖树的全部许可证。
  `THIRD_PARTY_NOTICES.md` §7 的 **21 个包仍待工具确认**。
  → 交接 `S08-09`（CI 流水线）一并引入。

## 证据

### 1. 检查器

`scripts/check-discipline.py`（Python 3，无需额外依赖）。

```bash
$ python3 scripts/check-discipline.py
全部 14 项通过
```

**14 项检查**：

| 分组 | 检查 |
|------|------|
| 架构边界 | engine 禁用清单 / 依赖方向 / engine 许可证声明 / 界面层许可证声明 / 许可证文件 |
| 硬约束 | 圆角刻度 / 品牌色 / 无 emoji 图标 |
| 文档纪律 | 路径引用 / spec 依赖图 / spec 状态一致性 / 退役台账 / agent 入口 / 文档入库 |

### 2. 拦截验证：**8 项全部通过** ✅

```bash
$ python3 scripts/check-discipline.py --self-test
OK   拦截验证 1：注入 `gpui` 到 engine → 禁用清单命中 ✅
OK   拦截验证 2：注入幽灵路径 → 检查报 FAIL ✅
OK   拦截验证 3：注入跨 Phase 前向依赖 → 能检出 ✅
OK   拦截验证 4：注册表状态造假 → 检查报 FAIL ✅
OK   拦截验证 5：入口文件超长 → 检查报 FAIL ✅
OK   拦截验证 6：台账登记幽灵路径 → 检查报 FAIL ✅
OK   拦截验证 7：把 docs/specs 加入 .gitignore → 检查报 FAIL ✅
OK   拦截验证 8：注入悬空依赖 → 能检出 ✅
拦截验证全部 8 项通过 —— 检查确实有效
```

**这一步是本 spec 最重要的验收项。** 一个从未失败过的检查等于没有检查
（S00-07 的教训：自动化仪表盘显示 `PASS`，而窗口其实是空白的）。

自测通过 `try/finally` 保证注入被回滚。实测验证：
`.gitignore` 无 `docs/specs/` 残留、台账无幽灵行残留、`cargo check --workspace` 仍通过。

### 3. 一处测试意图的修正（值得记录）

拦截验证 1 最初注入的是 `buddy-ui`（而非 `gpui`）：

```
error: cyclic package dependency: package `buddy-engine` depends on itself
```

检查**确实报了 FAIL**，但走的是 cargo 的**循环依赖**报错路径，
**没有测到真正要测的「禁用清单」逻辑**。

改为注入 `gpui` 后，确认命中禁用清单：

```
FAIL S01-04-1 engine 层禁用清单
     engine 依赖树里出现了禁止项：
       - gpui
     修复方向：
       · 若来自 gpui / theme / ui → 违反 GPL 分层，必须把这些代码移到 buddy-ui
       · 若来自 tauri → v2 已移除 Tauri，不应残留
```

> **教训**：拦截验证必须确认「失败的原因就是被测的那个原因」，
> 而不是「碰巧也失败了」。

### 4. 一处检查范围的收窄（附理由）

文档路径检查**只检查 `docs/` 下的引用**，不检查 `crates/**` / `src/**`。

**理由**：`crates/xxx` 形式在文档里有三种可能，检查器无法区分：

| 情况 | 例子 | 应否报错 |
|------|------|---------|
| ① 外部仓库路径 | `crates/gpui_apple/build.rs`（zed 源码树） | 否 |
| ② Buddy 的未来路径 | `crates/ui/src/markdown/`（`S04-01` 才创建） | 否 |
| ③ 真的写错了 | — | 是 |

而第 ③ 种**由编译器更可靠地发现**（Rust 代码路径写错会编译失败）。
收窄到 `docs/` 既覆盖了真实缺陷类型（`docs/design/prototypes/` 那类幽灵路径），
又不产生误报。

### 5. 检查器抓到的真实问题

首次运行时 `S01-04-9` 报出 **1 处真实缺陷**：

```
docs/tasks/v2.0.0-gpui/10-testing.md:87 → docs/evidence/v1-baseline
```

该行引用了 `S01-06` 的未来产物但**未标注「待创建」**。已修正。
（另有 23 处首次报出，经分类后确认为检查器误报，见 §4。）

### 6. CI 集成

`.github/workflows/discipline.yml`：

```yaml
- name: 纪律检查（全部断言）
  run: python3 scripts/check-discipline.py
- name: 拦截验证（确认检查确实会失败）
  run: python3 scripts/check-discipline.py --self-test
```

**拦截验证也进 CI** —— 否则检查可能在某次重构后静默失效。

> 注：CI 侧网络可达，无需代理；本机开发需代理，见 `docs/dev-environment.md` §2。

### 7. 顺带发现：`[patch.crates-io]` 有 3 项未被使用

首次运行 `cargo tree` 时 cargo 警告：

```
warning: patch `notify v9.0.0-rc.4 (…)` was not used in the crate graph
warning: patch `notify-types v2.1.0 (…)` was not used in the crate graph
warning: patch `tree-sitter-language v0.1.8 (…)` was not used in the crate graph
```

**S00-06 时期声明的 5 项 patch，当前只有 2 项（`async-process` / `async-task`）被使用。**

已在根 `Cargo.toml` 中暂时移除这 3 项，并**注明何时必须加回**：

| patch | 何时需要 |
|-------|---------|
| `tree-sitter-language` | `S04-02` 引入 tree-sitter 语法时 |
| `notify` / `notify-types` | `S04-04` 后台解析、`S07-*` 文件监听时 |

> ⚠️ 到那时**必须加回**，否则会因 API 不匹配而编译失败。
> 已在 `Cargo.toml` 注释与 `THIRD_PARTY_NOTICES.md` §5 两处标注。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 检查器实现语言 | **Python 3** | 无额外依赖、可读性好、CI 与本地都能跑。Rust `xtask` 属过度工程 |
| 文档路径检查范围 | **只查 `docs/`** | `crates/**` 无法区分「外部仓库 / 未来产物 / 真写错」，而第 3 种由编译器覆盖（见证据 §4） |
| 拦截验证是否进 CI | **是** | 否则检查可能在重构后静默失效 |
| 拦截验证注入什么 | **必须命中被测逻辑** | 首次注入 `buddy-ui` 导致循环依赖报错，虽 FAIL 但测错了东西（见证据 §3） |
| `[patch]` 未使用项 | **暂时移除 + 注明何时加回** | 保留只制造警告噪音；但必须记录加回时机，否则将来会踩坑 |
| 完整许可证扫描 | **本 spec 未做** | `cargo-deny` 未安装；已在 `THIRD_PARTY_NOTICES.md` §7 留下 21 个待确认项，交接 `S08-09` |

## 完成记录

- 日期：2026-09-11
- commit：（`scripts/check-discipline.py` + `.github/workflows/discipline.yml` 已落地）
- 设计文档处置：—（本 spec 不涉及设计文档退役）

## 遗留与交接

| 项 | 交接 |
|----|------|
| **完整许可证扫描**（`cargo-deny` / `cargo-about`） | `S08-09`（CI 流水线）；`THIRD_PARTY_NOTICES.md` §7 的 21 项待确认 |
| `tree-sitter-language` patch 加回 | `S04-02` |
| `notify` / `notify-types` patch 加回 | `S04-04` / `S07-*` |
| 品牌色逐值校验（硬约束 2） | `S03-02`（拿到实际色值后） |
| `crates/` 路径引用的校验 | 依赖编译器；若将来需要，可加 `cargo metadata` 交叉检查 |
| Windows / Linux 侧的 CI matrix | `S08-09` / `S09-*` |

## 事后修正（2026-09-26，随 S02-01 `89f92b8`）

| 缺陷 | 发现方式 | 修正 |
|------|---------|------|
| S01-04-13 要求台账**所有**路径存在，但「退役记录」段登记的恰是已删除文件 → 第一次真实退役（`rust-data-models.md`）即报错 | S02-01 删除设计文档 | 按段校验：现存段须存在；「退役记录」段须**已删除且在 git 历史中出现过**（后者保住拦截验证 6 对幽灵路径的拦截） |
| S01-04-9 对已退役路径的历史引用（spec 头 `退役设计文档` 字段）误报 | 同上 | 已退役路径在 spec / tasks 中豁免；在 `AGENTS.md` / `CLAUDE.md` 中仍报错（RULES §7.5） |
| S01-04-15 `git check-ignore` 不报告**已跟踪**路径 → 文档入库（`e91bbc3`）后，往 `.gitignore` 加 `docs/specs` 不再被拦（新 spec 文件会被静默忽略） | **拦截验证 7 由 OK 变 FAIL** | 加 `--no-index` |
| CI 浅克隆无 git 历史 | 推理（新检查依赖 `git log`） | workflow `fetch-depth: 0` |

新增拦截验证 9（入口残留已退役路径）、10（登记退役但文件仍在），**共 10 项全部通过**。
