# S03-03 外观令牌迁移（不透明填充 + 圆角 + 阴影）

> 状态: `done`
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
| S03-03-2 | 阴影语义 | CSS `x y blur spread color [inset]` → `ShadowSpec` → GPUI `BoxShadow`。~~GPUI 不支持 inset~~ **更正**：本 rev `BoxShadow` 有 `inset: bool`（`gpui/src/style.rs:359`），直接映射 —— 编写本 spec 时未查源码即下结论，已按实测修正 |
| S03-03-3 | 窗口阴影 | `--shadow-window*` 照常生成（数据完备），是否使用由 S07 决定（S00-04 已关窗口阴影） |

## 验收标准

- [x] 圆角集合 ⊆ {4, 8, 12, 16, 9999}
- [x] 每个阴影层数与各参数与 CSS 一致 —— 改为对照 WebKit 独立实算（强于生成器自检）
- [x] inset 层被标记且未被当作外阴影使用 —— `ShadowSpec::to_box_shadow` 保留 `inset`，单测断言

## 证据

| 项 | 证据 |
|----|------|
| 圆角 | `tokens::metrics::RADIUS_{SM,MD,LG,XL,FULL}` = 4 / 8 / 12 / 16 / 9999；`gen_tokens.py` 对每个 `--radius-*` 断言 ∈ {4,8,12,16,9999}，否则生成失败 |
| 阴影对照 | `verify_tokens.py`：`box-shadow` / `filter` 计算值逐层比对，**18 项（9 令牌 × 浅/深）全部一致**，含 `--shadow-window` 第 3 层 inset、`--filter-streaming-star` 三层 drop-shadow |
| 反证 | 深色 `--shadow-composer` 第 1 层 blur 32 → 33：报「阴影 dark --shadow-composer 第 1 层」不一致，rc=1；恢复后通过 |
| inset | 单测 `shadow_spec_keeps_every_field_including_inset`：第 3 层 `to_box_shadow().inset == true`、offset.y = 1、alpha 0.55；第 1 层为外阴影 |
| 不透明填充 | 表面色为不透明实色（S03-02 已验证；与 S00-04 结论一致） |
| 提交 | `be5bb81`（校验）、`db0945a`（生成与转换） |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 窗口阴影令牌 | 照常生成，不在本 spec 使用 | 数据完备；S00-04 已决定窗口不加系统阴影，是否用 CSS 窗口阴影由 S07 定 |
| inset | 直接映射 GPUI `BoxShadow::inset` | 源码实测支持；无需另行处理 |

## 完成记录

- 日期：2026-09-27
- commit：`be5bb81`
- 设计文档处置：`design-tokens.md` §Border Radius / §Shadows 已由代码取代；整份删除随 S03-04
