# S03-01 Theme 结构、Appearance 与全局安装

> 状态: `done`
> Phase: 03
> 依赖: S01-01
> 阻塞: —
> 退役设计文档: —

## 目标

在 `crates/ui/src/theme_system/` 建立 Buddy 自己的主题体系：`Appearance { Light, Dark }`、`Theme`（按外观选取生成的令牌）、`Theme::install(appearance, cx)` 全局安装与 `cx.buddy_theme()` 读取。

## 输入

- v1 主题只有 `light` / `dark`（`src/types/index.ts:12`、engine `models::Theme`），**无「跟随系统」**
- zed 的 `theme` crate 仍由 `init_theme` 安装（zed `ui` 组件依赖它）；Buddy 界面只读 Buddy 主题

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S03-01-1 | 模块位置 | `crates/ui/src/theme_system/`（`lib.rs` 模块规划中的名字），避免与 zed `theme` crate 重名 |
| S03-01-2 | 令牌来源 | 令牌数据由 S03-02 生成器产出（`tokens.rs`），本 spec 只提供结构与安装 |
| S03-01-3 | 全局 | `impl Global for Theme`；`BuddyTheme` trait 给 `App` 提供 `buddy_theme()` |
| S03-01-4 | 与配置互转 | `Appearance` ↔ engine `models::Theme`（持久化沿用 v1 `config.json` 的 `theme` 字段） |

## 验收标准

- [x] `Theme::install` 后 `cx.buddy_theme().appearance` 与安装值一致 —— 在真实 App 中自检（非单测，见决策）
- [x] 再次 install 另一外观，读取值随之改变 —— 同上
- [x] `Appearance` ↔ `models::Theme` 双向转换（单测）

## 证据

| 项 | 证据 |
|----|------|
| 结构 | `crates/ui/src/theme_system/mod.rs`：`Appearance`、`Theme { appearance, colors, shadows }`、`Theme::of` / `install`、`set_appearance`、`BuddyTheme::buddy_theme()`、`ShadowSpec`、`typography` |
| 真实 App 自检 | `cargo run -p buddy-app --example theme_preview -- --selftest`：`PASS S03-01 安装浅色后读回 Light`；`PASS S03-06 切换后读回 Dark`；`RESULT: PASS`，exit 0 |
| 单测 | `appearance_round_trips_with_config_theme`、`of_selects_the_matching_token_set`（`--bg-canvas` 浅 #F3F1EE / 深 #181719）等，`cargo test -p buddy-ui` 8 passed |
| 提交 | `db0945a`、`8cadce6` |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 模块名 | `theme_system` | 与 zed `theme` crate 区分；沿用 `lib.rs` 模块规划 |
| 与 zed 主题关系 | 两者并存，Buddy 界面只读 Buddy 主题 | zed `ui` 组件依赖 zed 主题；读 zed 配色会偏离 v1 |
| 安装类测试方式 | 预览程序 `--selftest`（真实 App） | gpui `test-support` 会拉入 wayland / x11 / proptest，仅为两个断言不值得 |

## 完成记录

- 日期：2026-09-27
- commit：`db0945a`、`8cadce6`
- 设计文档处置：—
