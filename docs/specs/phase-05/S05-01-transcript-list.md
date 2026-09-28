# S05-01 Transcript 虚拟列表骨架（ListState）

> 状态: `blocked`
> Phase: 05
> 依赖: S03-01
> 阻塞: 等待用户复核（handoff §6.5 第 8 / 11 项）：首轮目检反馈「向上滚动是逐行的，应为无极滚动」，已按 v1 `useSmoothWheelScroll` 修正
> 退役设计文档: —

## 目标

消息列表用 GPUI `ListState` 真虚拟化（只布局可见行 + 预渲染区），替代 v1 的整列表渲染。

## 输入

- v1 `src/pages/ChatPage.tsx`（整列表渲染、`no-scrollbar` 隐藏滚动条、上下内边距 space-3 / space-2）
- S00-07 `docs/evidence/s00-07/list-integration.rs` 与 research-log §16（`ListState` 四个陷阱）
- Comet `crates/ui/src/transcript.rs`（MIT，行模型与 splice 做法参考）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S05-01-1 | ListState 骨架 | `ListAlignment::Bottom`，overdraw 取 Comet 320px；`list()` 必须 `flex_grow` |
| S05-01-2 | 隐藏式滚动条 | 与 v1 `no-scrollbar` 一致：不显示滚动条，仍可滚动 |

## 验收标准

- [x] 1000 条消息下只有可见行被布局（计数断言）—— T11
- [ ] 滚动流畅 —— **需用户目检**（handoff §6.5 第 8 项）

## 证据

| 项 | 证据 |
|----|------|
| 实现 | `crates/ui/src/chat/transcript.rs`（`ListState` Bottom + `measure_all` + `FollowMode::Tail`、`cx.processor` 渲染行、按 diff splice / remeasure、正文行 markdown 实体按行 id 复用） |
| T11 虚拟化 | `chat_preview -- --selftest`：1000 条 / 5000 条消息，滚动 60 帧平均每帧布局 **7.0 行** |
| T13 重绘 | 1000 条：debug 2.6 ms；5000 条：debug 3.2 ms / release 0.34 ms |
| 首帧成本 | `measure_all` 首帧前布局全部已加载行：启动到首帧 debug 1000 条 339 ms / 5000 条 1027 ms；release 100 / 1000 / 5000 条 = 199 / 195 / 298 ms（与 S00-07 预警一致） |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 首帧成本 | 接受 `measure_all`，由 S05-05 分页加载控制已加载行数 | v1 首次只加载最近 10 条（`HISTORY_PAGE_SIZE`），更早的按需加载；按页加载后首帧只布局一页 |
| 行外观 | 本 spec 只做占位 | 按 v1 渲染由 S05-08..S05-12 分行型完成 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
