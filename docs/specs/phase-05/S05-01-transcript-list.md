# S05-01 Transcript 虚拟列表骨架（ListState）

> 状态: `doing`
> Phase: 05
> 依赖: S03-01
> 阻塞: —
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

- [ ] 1000 条消息下只有可见行被布局（计数断言）
- [ ] 滚动流畅 —— **需用户目检**

## 证据

| 项 | 证据 |
|----|------|
| | |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| | | |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
