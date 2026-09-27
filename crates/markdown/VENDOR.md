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
| `theme_settings_shim.rs` | **新增**（Buddy 编写） | 提供与 `theme_settings::ThemeSettings` 同名同 API 的替代；字体 / 字号由 buddy-ui 按 Buddy 令牌传入 |
| `language_stub.rs` | **新增**（Buddy 编写） | 替代 `language`：S04-02 起经 `buddy-syntax`（Comet，MIT）做 tree-sitter 高亮，只放行 v1 有高亮的语言，类别归并为 9 类（`SYNTAX_CATEGORIES`） |
| `mermaid.rs` | **整体替换为 stub**（原 1836 行 → 100 行） | 原实现依赖 node / wasm；Buddy（v1）无 mermaid 功能 |
| `parser.rs` / `selection.rs` / `path_range.rs` / `html.rs` / `html/*` | **未改动**（与上游逐字节一致） | — |

## 复核方法

```bash
Z=~/.cargo/git/checkouts/zed-*/290cbcb/crates/markdown
cp -R $Z/src /tmp/md && (cd /tmp/md && patch -p1 < <本目录>/patches/zed-markdown-290cbcb.patch)
diff -r /tmp/md <本目录>/src    # 无输出 = 上游原文 + patch 与 vendored 逐字节一致
```

## 不在此 crate 中做的事

- **不改 vendored 原文**来消除 warning 或适配风格（`dead_code` 在 `Cargo.toml` `[lints]` 放行；上游单元测试 `test = false`）。
- 上游单元测试依赖 zed 测试设施与真实 `language`，不编译；Buddy 的 markdown 测试写在 `crates/ui`。
