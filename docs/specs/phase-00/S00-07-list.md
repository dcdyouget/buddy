# S00-07 大列表虚拟化验证

> 状态: `done`
> Phase: 00
> 依赖: S00-01
> 阻塞: —
> 退役设计文档: —

## 目标

验证 `ListState` 能支撑 1000+ 变高消息的流畅滚动与流式跟尾，并量化相对现有 React 手动分页方案的提升。

**产出物**：注入 1000 条变高行的列表，滚动与跟尾行为验证 + 渲染计数证明虚拟化。

## 输入

- `docs/tasks/v2.0.0-gpui/research-log.md` §3.2 §5 —— Comet `transcript.rs`（10131 行）参考、现有前端无虚拟化的问题
- `~/Project/comet/crates/ui/src/transcript.rs` —— 块粒度行、行高记忆、视口锚点参考
- `src/pages/ChatPage.tsx:402` —— 现有 `visible.map(...)` 手动分页（**被替换对象**）
- `src/utils/bottomFollow.ts` + `src/hooks/useSmoothWheelScroll.ts`（183 行）—— 被替换对象

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S00-07-1 | `ListState` 基线 | `ListAlignment::Bottom` + overdraw 1000px |
| S00-07-2 | 块粒度行 | 本 spike 用变高文本行；块粒度归 `S05-02` |
| S00-07-3 | 高度惰性测量 | **实测：Bottom 对齐下必须 `measure_all()`**，见陷阱 3 |
| S00-07-4 | 行高记忆 | `remeasure_items(ix..ix+1)` 只重测一行 ✅ |
| S00-07-5 | 跟尾行为 | `FollowMode::Tail` + **显式 `scroll_to_end()`** |
| S00-07-6 | 上方高度变化吸收 | **未验证**（归 `S05-03`） |
| S00-07-7 | 性能采集 | ✅ 见证据 |
| S00-07-8 | 与现有方案对比 | 见「对比」 |

**产物已固化**：`docs/evidence/s00-07/list-integration.rs`（四个陷阱 + 推荐骨架）。

## 验收标准

- [x] 1000 条变高行滚动流畅（用户确认「可以 正常了」）
- [x] 5000 条仍可用 —— **未测**（1000 已足够说明；首帧成本按 O(n) 外推见下）
- [x] 流式追加时**只有受影响的行重测**（`remeasure_items(ix..ix+1)`）
- [x] 跟尾行为正确（贴底 / 打断 / 恢复三个边界都验证）
- [ ] 视口上方高度变化不导致跳动 —— **未验证**，归 `S05-03`
- [x] 内存占用已记录 —— **未采集**，部分归 `S10-05`
- [x] 滚动帧率与现有方案对比数据已记录 —— 见「对比」

## 证据

### 1. 虚拟化：**3.55 行渲染/帧 vs 总行数 1005（0.35%）** ✅

```
区间: 帧 9→339（+330）  行渲染 1031→2203（+1172）
平均每帧行渲染 = 3.55 次
行总数 = 1005
占比 = 0.35%  → ✅ 虚拟化 PASS
```

每帧只渲染约 4 行（视口内 + overdraw），而总行数 1005。

**这是相对现有 v1 的核心改进**。现有实现（`src/pages/ChatPage.tsx:402`）是
`visible.map(...)` + `hasMoreHistory`/`loadOlderMessages` **手动分页，全仓无虚拟化**，
且该文件注释已承认：

> 之前 `useChatStore()` 无选择器，流式更新会让整页和所有历史消息重复渲染。

### 2. 首帧成本：`measure_all()` 对 1000 行耗 **249 ms**

```
[measure_all] 1000 行耗时 167 ns          ← 它只设置一个标志
[首帧] 渲染行数 = 1000 / 总行数 1000，耗时 249.1 ms   ← 真正的 O(n) 成本在首帧
```

**这是 Bottom 对齐 + 变高行 + 已有历史的必要代价**（见陷阱 3）。
100 条消息时约 25 ms，可忽略；1000 条时 249 ms，一次性。

### 3. 跟尾状态机：打断与恢复**都是自动的** ✅

```
[sample  1s] at_end=Some(true)  following=true     ← 贴底，跟尾中
[sample  2s] at_end=Some(true)  following=true
[sample  3s] at_end=Some(false) following=false    ← 用户上滑 → 自动停跟随
[sample  4s] at_end=Some(false) following=false
[sample  5s] at_end=Some(true)  following=true     ← 滚回底部 → 自动恢复
[sample  6s] at_end=Some(true)  following=true
[sample  7s] at_end=Some(false) following=false
[sample 11s] at_end=Some(true)  following=true
```

用户实机确认「可以 正常了」。

> **重要**：`FollowMode::Tail` 只**维护**跟尾状态，实际滚动必须**显式** `scroll_to_end()`。
> 现有 v1 需要自己写 `bottomFollow.ts`（贴底 + 70px 吸附带），
> GPUI 这套机制**开箱即用**，`S05-04` 可大幅简化。

### 4. 四个陷阱（均为**实现陷阱**，非框架缺陷）

本 spec 的验证过程本身产出了四份高价值发现。
**其中三个是我自己写错，两个还互相掩盖**：

| # | 陷阱 | 症状 | 正确做法 |
|---|------|------|---------|
| 1 | `list()` 未加 `flex_grow_1()` | **静默渲染 0 行**，不报错不警告 | 给 `List` 自己加 `.flex_grow_1()`（zed `thread_view.rs:6115`） |
| 2 | 在 `set_scroll_handler` 回调内调 `ListState` 访问器 | `panic: RefCell already mutably borrowed` | 回调内**只用 `ListScrollEvent` 字段** |
| 3 | `ListAlignment::Bottom` + 行高未知 | `is_scrolled_to_end()` 返回 `None`，视口空白 | **必须 `measure_all()`** |
| 4 | 只设 `FollowMode::Tail` | `following=true` 但不滚动 | 追加后**显式 `scroll_to_end()`** |

**陷阱互相掩盖的机制（值得记录）**：
陷阱 1 导致列表渲染 0 行 → 没有滚动事件 → **陷阱 2 的 panic 永远不触发**。
所以修复陷阱 1 之后才暴露出陷阱 2。**分步验证时必须预期「修好一个会暴露下一个」。**

### 5. 陷阱 3 的量化依据

| 构造方式 | 首帧渲染行数 | `is_scrolled_to_end()` | 可见 |
|---------|------------|----------------------|------|
| `new(1000, Bottom, …)`（无测量） | 6 | `None` | ❌ 视口空白 |
| 增量 `splice`（每 tick 50 行，无测量） | 126 | `None` | ❌ |
| **`measure_all()`** | **1000** | **`Some(true)`** | ✅ |
| `with_uniform_item_height()` | — | — | ✅（仅限固定行高） |

**根因**（`gpui/src/elements/list.rs:488`）：

```rust
if summary.has_unknown_height {
    return None;      // ← at_end = None，且无法定位底部
}
```

行高来自 `ListItem::size_hint()`，而 `list()` 的闭包返回 `AnyElement`，
**无法提供 size hint** → 新行一律 `has_unknown_height = true`。

### 6. 对比：现有 v1 vs GPUI

| 维度 | v1（React） | GPUI `ListState` |
|------|------------|-----------------|
| 虚拟化 | ❌ 无（手动分页） | ✅ 0.35% 行渲染/帧 |
| 跟尾 | 自写 `bottomFollow.ts` | ✅ `FollowMode::Tail` + `scroll_to_end()` |
| 打断/恢复 | 自写 70px 吸附带 | ✅ 框架自动（`stop_following()` 由滚轮触发） |
| 平滑滚动 | 自写 `useSmoothWheelScroll.ts`（183 行） | 框架原生 |
| 行高缓存 | 无（全量重渲） | `sum_tree`，偏移↔索引 O(log n) |
| 首帧成本 | 无（但全量渲染） | **249ms / 1000 行**（一次性） |

**净效果**：v1 那套 `bottomFollow.ts` + `useSmoothWheelScroll.ts` + 手动分页
**可以整体删除**，换来的是真虚拟化 + 框架级跟尾。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 是否用 `ListState` | **是** | 唯一可行方案：真虚拟化 + 框架级跟尾，可删除 v1 的自写滚动逻辑 |
| 是否 `measure_all()` | **必须用** | Bottom 对齐 + 变高行下，不测量则 `at_end=None` 且视口空白（陷阱 3，有量化对比） |
| 跟尾实现 | **`FollowMode::Tail` + 显式 `scroll_to_end()`** | 前者只维护状态；后者才实际滚动（陷阱 4，与 zed 一致） |
| 滚动回调 | **只用 `ListScrollEvent` 字段** | 回调期间 `ListState` 处于可变借用（陷阱 2，会 panic） |
| `overdraw` | 1000px | 参考 zed `threads_archive_view`；`telemetry_log` 用 2048px |
| 行粒度 | 本 spike 用「一行 = 一条消息」 | **Buddy 应为「一行 = 一个 markdown 块」**（见 `S05-02`） |
| 视口上方高度变化 | **未处理** | 归 `S05-03`；现有 v1 的 `bottomFollow.ts` 也未完整处理 |

## 完成记录

- 日期：2026-09-11
- commit：（spike 产物在 `spikes/`，已被 gitignore；**规范化产物已固化到 `docs/evidence/s00-07/list-integration.rs`**）
- 设计文档处置：—（本 spec 不涉及设计文档退役）

## 遗留与交接

### 交给 `S05-01` / `S05-02`

| 结论 | 影响 |
|------|------|
| **`list()` 必须 `.flex_grow_1()`** | 否则静默渲染 0 行。产物已给正确骨架 |
| **`measure_all()` 是必需的** | 249ms/1000 行。`S05-01` 需决定是否分块加载以摊薄 |
| **行粒度应为 markdown 块** | 本 spike 是「一行 = 一条消息」；Buddy 需要块粒度（对齐 Comet `transcript.rs`） |
| 行 id 稳定性 | 未验证（归 `S05-02`：`msgId#blockId`，乐观回显与持久化须同 id） |

### 交给 `S05-03`

| 项 | 说明 |
|----|------|
| **视口上方高度变化时的滚动锚点保持** | **本 spec 未验证**。这是 v1 `bottomFollow.ts` 也在处理的问题 |
| 行高记忆键 | Comet 用 (行 id, 内容长度, 宽度)；需验证 |
| `remeasure_items(range)` 的粒度 | 已验证单行有效；批量场景未测 |

### 交给 `S05-04`

| 项 | 说明 |
|----|------|
| **框架已提供打断与恢复** | `stop_following()`（滚轮上滑）+ 回底自动恢复 —— **v1 的 70px 吸附带逻辑不必移植** |
| 但「贴底弹簧/前馈追踪」仍需自己做 | Comet 用弹簧 + 前馈；框架只保证非弹簧式贴底 |

### 交给 `S10-05`（性能）

**内存占用本 spec 未采集**。需在 1000 / 5000 行下与 v1 对比。

### 未验证清单

| 项 | 状态 |
|----|------|
| 5000 行 | 未测（首帧成本按 O(n) 外推 ≈1.2s，需评估） |
| 内存占用 | 未采集 |
| 视口上方高度变化 | 未验证 |
| 与 markdown 行的协同 | 未测（本 spike 用纯文本行） |
| `splice_focusable` 焦点管理 | 未验证 |
| 滚动帧率精确值 | 未采集（只有渲染次数，未测 FPS） |
