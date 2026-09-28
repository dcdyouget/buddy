# S05-10 工具调用分组（ToolGroup / Deferred）

> 状态: `blocked`
> Phase: 05
> 依赖: S05-08
> 阻塞: 等待用户目检（`handoff.md` §6.5 第 13 项）
> 退役设计文档: —

## 目标

工具调用按 v1 分组与插入位置显示，含运行中 / 成功 / 失败状态。

## 输入

- v1 `ToolSection.tsx`、`DeferredToolSection.tsx`、`MessageBubble.tsx`（`insertAfterBlockIndex` 分桶）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S05-10-1 | 插入位置 | 与 v1 分桶规则一致 |

## 验收标准

- [x] 分桶规则测试 —— `insert_after_buckets_match_v1`
- [ ] 观感 —— **需用户目检**（handoff §6.5 第 13 项）

## 证据

| 项 | 证据 |
|----|------|
| 实现 | `crates/ui/src/chat/tool_card.rs`、`code_block::compact_renderer`、列表中展开状态 / 卡内滚动 / 内层滚动让位；commit `6aa8ffa` |
| 分桶规则 | 行模型单测 `insert_after_buckets_match_v1`（S05-02） |
| 单测 | `metas_match_v1`、`summary_and_args_match_v1`、`default_expansion_matches_v1`、`details_use_longer_fence_when_needed`（发现并修正围栏长度缺陷） |
| T20 | 执行中卡片高 113px → 完成后自动收起 62px；点开 457.5px 且重测 1 行；滚轮落在长结果上：卡内 0 → -60px、列表不动 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 分组 | 每个调用一张卡片（v1 `DeferredToolSection` 同样逐个渲染） | 与 v1 一致 |
| 卡内滚动 | 详情最高 180px，滚轮在内层还能滚时让给内层 | v1 `canNestedScrollerConsume` |
| 网络搜索 / 图片生成 / 提问 | 暂按通用卡片显示 | 专门卡片在 S05-11 / S05-12 / S05-13 |
| 展开动画 | 不做（v1 160ms 高度渐变） | 次要；目检若在意再补 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
