# S03-02 颜色令牌迁移（品牌色 / 状态色 / 中性阶 / 表面 / 语义色）

> 状态: `done`
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

- [x] 生成的每个颜色与 CSS 真值逐个一致 —— 改为**全量**对照 WebKit 实算（比抽查更强）
- [x] 抽查 `color-mix` 结果与浏览器算法一致 —— 全量覆盖，含纯色混合 / 与 `transparent` 混合 / 嵌套
- [x] 品牌色 `#5B5FE9` 唯一（硬约束 2）

## 证据

| 项 | 证据 |
|----|------|
| 生成 | `python3 scripts/theme/gen_tokens.py` → `crates/ui/src/theme_system/tokens.rs` 560 行：`Palette` 79 个颜色 × 浅/深、`Shadows`、`metrics` / `motion` / `fonts`；源 139 个变量 = 生成 138 + 排除 1（`--glass-outline`，S00-04 用户否决），生成器内 `assert` 保证 |
| 独立真值对照 | `python3 scripts/theme/verify_tokens.py`：离屏 WKWebView（与 v1 同引擎）对每个令牌 `getComputedStyle`，**158 个颜色值（79 × 浅/深）全部一致，最大 RGB 偏差 0.000/255**。验证对象是已生成的 `tokens.rs` 本身（解析产物，不复用生成器） |
| 反证 | 把深色 `--code-syntax-keyword` 的 r 篡改 +2/255 → 报「不一致 1」并定位该令牌，rc=1；恢复后 `--check` 一致 |
| 手算抽查 | 浅色 `code_syntax_keyword` = `color-mix(#2563EB 86%, #222120)`：R = (37×0.86+34×0.14)/255 = 0.14345，与生成值一致；深色 `markdown-accent-soft` alpha 0.13 与源 `13%` 一致 |
| 品牌色 | 生成器断言 `--buddy-primary` / `-500` = `#5B5FE9`；单测 `of_selects_the_matching_token_set` 断言浅深一致 |
| 单测 | `cargo test -p buddy-ui`：4 passed |
| 提交 | `db0945a` |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 真值 | `v1-final` 的 `global.css` | `design-tokens.md` 有 8 处与代码不符（S01-06） |
| 生成而非手抄 | `gen_tokens.py` + `--check` | 139 个变量含大量嵌套 color-mix；生成器同时是完备性校验 |
| 独立验证 | WKWebView 实算 | 生成器与校验若共用解析逻辑会同错；WebKit 是 v1 实际渲染引擎 |
| 颜色常量类型 | `gpui::Rgba`（`const` 构造） | 零运行时成本；需要时 `Hsla::from` |

## 完成记录

- 日期：2026-09-27
- commit：`db0945a`
- 设计文档处置：`design-tokens.md` 颜色段已被代码取代；整份删除随 S03-04（三者最后完成者）
