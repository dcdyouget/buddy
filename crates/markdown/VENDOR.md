# vendored zed markdown

| 项 | 值 |
|----|-----|
| 上游 | https://github.com/zed-industries/zed `crates/markdown/src` |
| rev | `290cbcb9cb6a5dcbe0060431a126ad19e743f2f4`（与 workspace 中全部 zed crate 同一 rev，见 S01-02） |
| vendor 日期 | 2026-09-27（S04-01；方案与 shim 来自 S00-06） |
| 许可证 | GPL-3.0-or-later（上游 `crates/markdown/Cargo.toml` 的 `license` 字段）；原文件版权与内容未改动 |
| Buddy 修改 | [`patches/zed-markdown-290cbcb.patch`](./patches/zed-markdown-290cbcb.patch)（上游 `src/` → 本目录 `src/` 的完整 diff，满足 GPL 修改释出） |

## 修改清单

| 文件 | 修改 | 原因 |
|------|------|------|
| `markdown.rs` | `language` / `settings` / `theme_settings` 的 4 处 `use` 改指本地模块；新增 `mod` 声明、`install_theme_settings`、`pub mod syntax` 重导出（共 43 行差异） | 去掉会拖入 zed settings 框架与需 cmake 的 `language`；向 buddy-ui 暴露高亮类别与语言注册表 |
| `markdown.rs`（S04-07） | **接通 `CodeBlockRenderer::Custom`**：上游本 rev 该分支为空实现（`render` / `transform` 从不调用，全仓无调用方），缩进代码块也强制走 Default。改为：`render` 产出外层容器、代码作为其子节点并套 `style.code_block`，结束时调用 `transform` | Buddy 代码块需 v1 的头部栏（语言标签 + 「复制」文字按钮），Default 只有悬浮图标按钮 |
| `theme_settings_shim.rs` | **新增**（Buddy 编写） | 提供与 `theme_settings::ThemeSettings` 同名同 API 的替代；字体 / 字号由 buddy-ui 按 Buddy 令牌传入 |
| `language_stub.rs` | **新增**（Buddy 编写） | 替代 `language`：S04-02 起经 `buddy-syntax`（Comet，MIT）做 tree-sitter 高亮，只放行 v1 有高亮的语言，类别归并为 9 类（`SYNTAX_CATEGORIES`） |
| `mermaid.rs` | **整体替换为 stub**（原 1836 行 → 100 行） | 原实现依赖 node / wasm；Buddy（v1）无 mermaid 功能 |
| `parser.rs` / `selection.rs` / `path_range.rs` / `html.rs` / `html/*` | **未改动**（与上游逐字节一致） | — |

## 依赖层面的修改（`Cargo.toml`，不在 `src/` patch 内）

| 上游依赖 | Buddy | 原因 |
|---------|-------|------|
| `language` / `settings` / `theme_settings` / `mermaid_render` | 移除 | 由 `src/` 内 shim / stub 替代（见上表） |
| `util`（zed） | 改为本地 [`util-shim/`](./util-shim)（包名 `buddy-md-util`，**lib 名仍为 `util`**，Apache-2.0） | 源码中 `use util::…` 无需改动；zed `util` 只被本 crate 使用却拖入约 21 个包（S04-03 实测 818 → 797）。替身重导出 `gpui_util`（zed `util` 本就从它重导出 `maybe!` / `ResultExt`），并逐字复制 `generate_heading_slug` |
| — | 新增 `buddy-syntax` | 语法高亮（S04-02） |

## 复核方法

```bash
Z=~/.cargo/git/checkouts/zed-*/290cbcb/crates/markdown
cp -R $Z/src /tmp/md && (cd /tmp/md && patch -p1 < <本目录>/patches/zed-markdown-290cbcb.patch)
diff -r /tmp/md <本目录>/src    # 无输出 = 上游原文 + patch 与 vendored 逐字节一致
```

`scripts/check-discipline.py` 的「S04-03 GPL patch 同步」自动执行上述复核（拦截验证 14 保证其有效）。

## 不在此 crate 中做的事

- **不改 vendored 原文**来消除 warning 或适配风格（`dead_code` 在 `Cargo.toml` `[lints]` 放行；上游单元测试 `test = false`）。
- 上游单元测试依赖 zed 测试设施与真实 `language`，不编译；Buddy 的 markdown 测试写在 `crates/ui`。
