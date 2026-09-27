# S03-03 外观令牌迁移（不透明填充 + 圆角 + 阴影）

> 状态: `doing`
> Phase: 03
> 依赖: S03-01, S00-04
> 阻塞: —
> 退役设计文档: `docs/design/design-tokens.md` §Border Radius / §Shadows

## 目标

由同一生成器产出圆角刻度与阴影（CSS `box-shadow` 多层列表 → `ShadowSpec` 列表），并提供 `ShadowSpec` → `gpui::BoxShadow` 转换。

## 输入

- S00-04：窗口不透明实色 + 16px 圆角 + 无边框 + 无模糊；窗口级阴影已关

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S03-03-1 | 圆角 | 仅 `4/8/12/16/9999`（硬约束 3），生成器断言 |
| S03-03-2 | 阴影语义 | CSS `x y blur spread color [inset]`；GPUI `BoxShadow { offset, blur_radius, spread_radius, color }` 不支持 inset → inset 层单独标记，渲染侧决定 |
| S03-03-3 | 窗口阴影 | `--shadow-window*` 照常生成（数据完备），是否使用由 S07 决定（S00-04 已关窗口阴影） |

## 验收标准

- [ ] 圆角集合 ⊆ {4, 8, 12, 16, 9999}
- [ ] 每个阴影层数与各参数与 CSS 一致（生成器自检）
- [ ] inset 层被标记且未被当作外阴影使用

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
