# S03-02 颜色令牌迁移（品牌色 / 状态色 / 中性阶 / 表面 / 语义色）

> 状态: `doing`
> Phase: 03
> 依赖: S03-01
> 阻塞: —
> 退役设计文档: `docs/design/design-tokens.md` §Brand / §State / §Theme-bound（与 S03-03、S03-04 共同负责，最后完成者删除）

## 目标

写生成器 `scripts/theme/gen_tokens.py`：读 **`v1-final:src/styles/global.css`**（代码真值），按 CSS 规范计算 `color-mix(in srgb …)`（预乘 alpha），生成 `crates/ui/src/theme_system/tokens.rs` 中浅 / 深两套 `Palette`。

## 输入

- `docs/evidence/v1-baseline/tokens.md`（S01-06）：139 个变量；`design-tokens.md` 与代码不一致 8 处（表面色），**以代码为准**
- 03-theme.md 称 `design-tokens.md` 为唯一真值 —— 已被 S01-06 证伪

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S03-02-1 | 生成而非手抄 | 139 个变量手抄必然出错；生成器同时是 S03-07 完备性校验的基础 |
| S03-02-2 | color-mix 语义 | CSS Color 5：各分量按预乘 alpha 插值后反预乘；`transparent` = `rgba(0,0,0,0)` |
| S03-02-3 | 排除项 | `--glass-outline`：S00-04 用户实机否决（「四边白光」）→ 不生成，列入排除表并注明理由 |
| S03-02-4 | 常量 | `gpui::Rgba` 字段为 `pub f32` → `const` 构造，零运行时成本 |

## 验收标准

- [ ] 生成的每个颜色与 CSS 真值逐个一致（生成器自检：把生成值回转 `#RRGGBBAA` 与独立计算对比）
- [ ] 抽查 `color-mix` 结果与浏览器算法一致（至少 3 个：纯色混合、与 `transparent` 混合、嵌套）
- [ ] 品牌色 `#5B5FE9` 唯一（硬约束 2）

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
