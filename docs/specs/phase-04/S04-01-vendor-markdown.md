# S04-01 vendor zed markdown 并以 shim 替换 settings / language / mermaid

> 状态: `doing`
> Phase: 04
> 依赖: S01-02
> 阻塞: —
> 退役设计文档: —

## 目标

新建 `crates/markdown`（`buddy-markdown`，lib 名 `zed_markdown`，GPL-3.0-or-later）：vendor zed rev `290cbcb` 的 `crates/markdown`，以 S00-06 验证过的 3 个 shim 替掉 `settings` / `language` / `mermaid_render`；shim 的字体与字号取自 Buddy 排版令牌（S03），并随 crate 存 patch 文件。

## 输入

- `docs/evidence/s00-06/README.md`（配方）；本地 `spikes/s00-06-markdown/`（= zed 原文 + 4 处 import patch + shim，已逐文件 diff 核对）
- 注册表原标题「patch ThemeSettings（9 处）」为旧估计；S00-06 实测为 1 行 import + shim

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S04-01-1 | 独立 crate | markdown.rs 是 crate 根且大量 `crate::` 路径；独立 crate 让 patch 边界清晰、便于 GPL 修改释出 |
| S04-01-2 | 来源记录 | 各 vendored 文件保留原版权头；crate 顶部注释写 zed rev / 日期 / patch 清单 |
| S04-01-3 | patch 文件 | `crates/markdown/patches/zed-markdown-290cbcb.patch`（原文 → vendor 的 diff），满足 GPL 修改释出（D08） |
| S04-01-4 | shim 读 Buddy 令牌 | `ThemeSettings` shim 的 ui / buffer 字体取 `theme_system::fonts`，字号取 `typography`，而非 zed 默认 |
| S04-01-5 | `[patch.crates-io]` | 按 S00-06 §1 补回所需项（async-process 等），并在根 Cargo.toml 注明来源 |

## 验收标准

- [ ] `cargo check -p buddy-markdown` 通过、0 warning
- [ ] `cargo tree` 中无 `settings` / `language` / `lsp` / `rpc` / `fs` / `mermaid_render` / `wasmtime`
- [ ] 除 3 处 import 外 vendored 文件与 zed rev 逐字节一致（`diff` 证据）
- [ ] 分层检查：engine 依赖树不含该 crate；许可证声明 GPL

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
