# S04-01 vendor zed markdown 并以 shim 替换 settings / language / mermaid

> 状态: `done`
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

- [x] `cargo check -p buddy-markdown` 通过、0 warning（lib 目标）
- [x] `cargo tree` 中无 `settings` / `language` / `lsp` / `rpc` / `fs` / `mermaid_render` / `wasmtime`
- [x] 除 import 级改动外 vendored 文件与 zed rev 逐字节一致（`diff` 证据）
- [x] 分层检查：engine 依赖树不含该 crate；许可证声明 GPL

## 证据

| 项 | 证据 |
|----|------|
| 编译 | `scripts/gate.sh` 全部通过（含 `cargo check -p buddy-markdown --lib` 0 warning） |
| 禁用依赖 | `cargo tree -p buddy-markdown -e normal` 中 `settings / language / lsp / rpc / fs / mermaid_render / wasmtime / theme_settings / node_runtime` 命中 **0** |
| 逐字节一致 | `parser.rs` / `selection.rs` / `path_range.rs` / `html.rs` / `html/*` 对 `~/.cargo/git/checkouts/zed-*/290cbcb/crates/markdown/src` `cmp` / `diff -r` 无差异；`markdown.rs` 差异 37 行（4 处 `use` + mod 声明 + `install_theme_settings`） |
| GPL patch 可还原 | 上游 `src/` 复制后 `patch -p1 < patches/zed-markdown-290cbcb.patch`，`diff -r` 与 vendored 目录**无输出**（2100 行改动，涉及 4 个文件） |
| 依赖增量 | `Cargo.lock` 752 → **788（+36）**；新增主要来自 zed `util`（`async_zip`、`rust-embed`、`nix`、`globset`…）→ S04-03 评估是否以 shim 替换 `util` |
| 分层 / 许可证 | `check-discipline.py` S01-04-1 engine 禁用清单 0 命中；S01-04-4b「GPL-3.0-or-later ×3（ui / app / markdown）」 |
| 字体接入 | `buddy_ui::markdown::init`：ui = `fonts::ui_font`（本机 Fira Code），code = `fonts::mono_font`，字号 14 / 13（取自 v1 `MessageBubble.tsx:234`、`CodeBlock.tsx:206`） |
| 提交 | `7424f9a` |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 位置 | 独立 crate `crates/markdown` | markdown.rs 是上游 crate 根、大量 `crate::` 路径；独立后 patch 边界清晰 |
| shim 参数化 | `install_theme_settings(cx, ui_font, buffer_font, sizes)` | 避免 markdown crate 依赖 buddy-ui（会成环）；由 buddy-ui 传令牌 |
| warning 处理 | `Cargo.toml` `[lints]` + feature 声明，不改 vendored 源 | 保持上游原文一致，patch 最小 |
| 上游单测 | `test = false`，gate 只 check lib | 依赖 zed 测试设施与被 stub 的 `language` |
| emoji 扫描 | 排除 vendored crate | 仅测试字符串含 ☕，非界面图标（硬约束 4 针对 Buddy 界面） |
| 新增 36 包许可 | 交 S08-09 全量扫描 | 与 S01-03 的遗留一致 |

## 完成记录

- 日期：2026-09-27
- commit：`7424f9a`
- 设计文档处置：—
