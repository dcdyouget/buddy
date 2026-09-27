# S04-06 流式渐显（veil）

> 状态: `todo`
> Phase: 04
> 依赖: S04-04
> 阻塞: —
> 退役设计文档: —

## 目标

新到文字以纯绘制层 alpha 渐显，不改布局；尊重减弱动效。

## 输入

- Comet `veil.rs`（MIT，508 行）
- v1 令牌：`--delay-streaming-char-age-1..8`（-32..-256ms）、`--duration-streaming-char-settle` 260ms（`theme_system::tokens::motion`）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S04-06-1 | 参数 | 取 v1 令牌，不另定 |
| S04-06-2 | 减弱动效 | 系统开启时关闭渐显（v1 `WindowEntrance.tsx` 用 `prefers-reduced-motion`） |

## 验收标准

- [ ] 渐显不改变布局（同文本有无 veil 高度相同）
- [ ] 观感与 v1 一致 —— **需用户目检**

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
