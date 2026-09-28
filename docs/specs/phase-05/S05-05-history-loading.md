# S05-05 历史加载与滚动（替换手动分页）

> 状态: `blocked`
> Phase: 05
> 依赖: S05-04
> 阻塞: 等待用户目检（`handoff.md` §6.5 第 15 项）
> 退役设计文档: —

## 目标

按需加载更早消息，插入到列表顶部时视口不跳；「正在加载更早消息…」提示与 v1 一致。

## 输入

- v1 `ChatPage.tsx`：距顶 ≤56px 触发 `loadOlderMessages`、保持 scrollTop 偏移；engine `load_messages` / `get_message_count`

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S05-05-1 | 按需加载 | 触顶加载一批，`splice` 到顶部并保持锚点 |
| S05-05-2 | 分页状态 | `ChatState::history`（offset / has_more / loading）、`begin_load_older` / `finish_load_older`，每页 10 条（v1 `HISTORY_PAGE_SIZE`）；失败保持原分页（v1 只记日志） |
| S05-05-3 | 读取 | `Conversation::with_history_page` + `HistoryLoader`（engine `load_messages(offset, limit)`；接线在 S05-18） |
| S05-05-4 | 触发 | 每帧判断距顶 ≤56px（v1 `scrollTop <= 56`）；触发时取消平滑滚轮（v1 同）；并入后的那一帧新行未测量，不据此再次触发 |
| S05-05-5 | 保持视口 | GPUI `splice` 在视口首行落入替换区间时把位置重置到区间起点、行内偏移清零；按与锚点无关的行键（`rows::stable_key`）找回原首行并还原偏移 |

## 验收标准

- [x] 加载更早消息后可见内容不移动（测试：T22 + 单测）
- [ ] 5000 条历史滚动 —— **需用户目检**（handoff §6.5 第 15 项）

## 证据

| 项 | 证据 |
|----|------|
| 单测 | `state::tests::history_paging_matches_v1`、`rows::tests::prepending_history_keeps_stable_keys` |
| T22 | `chat_preview --selftest`：5001 条只载最新 10 条（首条为助手消息）；真实滚轮滚到顶 → 加载中（读取由闸门挂起）→ 放行后并入 10 条：视口首行由 `head-a4991#0.0` 改锚为 `u4990#0.0` 且行内偏移不变，可见用户消息屏幕位置 167.5px 不变，只加载一次（offset 4991 → 4981） |
| 拦截 | 去掉按行键还原 → 首行跳到最早一页、连续加载到 offset 4731，T22 FAIL；`head` 锚不带消息 id → 两页开头行 id 相同、diff 误判，T22 FAIL；去掉「并入后跳过一帧」→ 一次触顶加载两页，T22 FAIL |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 触发时机 | 每帧判断，而非只在滚动事件里 | v1 内容不足一屏时无法滚动、永远不加载更早历史；这里会自动补到超过一屏 |
| 「正在加载更早消息…」 | 叠在列表顶部（`bg-surface` 底） | 插成列表行会被视口保持挪到视口外，看不到；本地读取通常只有几毫秒 |
| 行 id 的 `head` 锚 | 带首条消息 id（`head-{id}`） | 各页开头都是助手消息时 id 相同，diff 会把不同消息当同一行 |
| 未读判断 | 记最后一条可见消息 id（v1 记条数） | 顶部并入历史会让条数增加，但不是新消息 |
| `measure_all` 首帧成本（S05-01 移交） | 由分页控制：首屏只测 10 条，之后每次并入只测新增行 | |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
