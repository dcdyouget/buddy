# S03-01 Theme 结构、Appearance 与全局安装

> 状态: `doing`
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

- [ ] `Theme::install` 后 `cx.buddy_theme().appearance` 与安装值一致（单测）
- [ ] 再次 install 另一外观，读取值随之改变（单测）
- [ ] `Appearance` ↔ `models::Theme` 双向转换（单测）

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
