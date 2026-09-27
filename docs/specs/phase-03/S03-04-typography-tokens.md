# S03-04 字体与排版令牌迁移

> 状态: `done`
> Phase: 03
> 依赖: S03-01
> 阻塞: —
> 退役设计文档: `docs/design/design-tokens.md` §Typography / §Spacing / §Motion

## 目标

由生成器产出字号、行高、字距、字重、间距、动效时长与缓动曲线常量；字体栈原样保留顺序。

## 输入

- v1 实值：`--font-sans` 首选 `'Fira Code'`（等宽）、基础字重 650（S01-06 tokens.md）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S03-04-1 | 忠实迁移 | 不「修正」首选等宽字体与 650 字重 —— 这是 v1 真实观感 |
| S03-04-2 | em 单位 | 字距 `em` 以系数保存，渲染时乘字号 |
| S03-04-3 | 负延迟 | `--delay-streaming-char-age-*` 为负毫秒，用 `i32` |

## 验收标准

- [x] 字号 / 行高 / 字距 / 字重 / 间距 / 时长 / 缓动与 CSS 逐项一致 —— 对照 WebKit 读出的自定义属性值（强于生成器自检）
- [x] 字体栈顺序与 CSS 一致

## 证据

| 项 | 证据 |
|----|------|
| 常量对照 | `verify_tokens.py` 常量 **50 项**（`metrics` 间距 9 / 圆角 5 / 字号 7 / 字距 3 / 行高 3 / 字重 3；`motion` 时长 9 / 延迟 8 / 缓动 2；`fonts` 2）与 WebKit 读出的 `--x` 值逐项一致；`1.1s` 等秒单位换算为 1100 ms |
| 反证 | `--duration-fast` 120 → 121：报「常量 --duration-fast: 生成 121 vs WebKit `120ms`」，rc=1 |
| 排版角色 | `theme_system::typography` 5 个角色取自 v1 `global.css:1574-1578`；单测 `typography_roles_match_v1_classes`（title 20/600、h3 16/600、body 14/650/1.5、body-sm 13、caption 12、title 字距 -0.01em） |
| 忠实迁移 | `fonts::FONT_SANS` 首项 `Fira Code`、`FONT_WEIGHT_REGULAR = 650.0` 与 v1 一致，未「修正」 |
| 单测 | `cargo test -p buddy-ui` 5 passed |
| 提交 | `6d04e59`（角色与退役）、`db0945a`（生成） |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 排版角色来源 | v1 `.t-*` 类的实际定义 | `design-tokens.md` 的角色表含从未存在的 `t-display` / `t-overline`，且 body 字号与代码不符 |
| title / h3 字重 | 保留写死的 600 | v1 即如此；改用令牌会改变观感 |

## 完成记录

- 日期：2026-09-27
- commit：`6d04e59`
- 设计文档处置：`docs/design/design-tokens.md` **整份删除**（S03-02/03/04 共同替代），已登记台账
