# 第三方依赖许可证清单

> 生成方式见本文末「如何重新生成」。**本清单基于 `Cargo.lock` 实测统计，非人工罗列。**
> 最近更新：2026-09-11（S01-03）

---

## 1. 按分层归属

| 层 | 路径 | 许可证 | 引入的 GPL 依赖 |
|----|------|--------|----------------|
| 引擎层 | `crates/engine/` | **MIT** | **无** — 依赖树中 0 处 GPL |
| 界面层 | `crates/ui/` | **GPL-3.0-or-later** | 见 §3 |
| 入口 | `apps/buddy/` | **GPL-3.0-or-later** | 继承自 `crates/ui` |
| markdown 渲染 | `crates/markdown/` | **GPL-3.0-or-later** | 本身即 vendored zed `crates/markdown`（rev `290cbcb`），见 `crates/markdown/VENDOR.md` 与其 patch 文件 |
| 语法高亮 | `crates/syntax/` | **MIT** | 无 —— 移植自 Comet `crates/syntax`（MIT，原许可证 `crates/syntax/LICENSE-MIT-comet`） |

**引擎层的纯净性已被实测验证**（S00-08）：`cargo tree -p buddy-engine` 中
0 处 `gpui` / `theme` / `ui` / 其他 GPL crate。其直接依赖仅为
`tokio` / `reqwest` / `serde` / `serde_json` / `futures-util` / `async-trait` /
`thiserror` / `log` / `chrono` / `dom_query` / `base64` / `parking_lot`，
全部为 MIT / Apache-2.0。

---

## 2. 全量许可证分布（746 个包）

| 许可证表达式 | 包数 |
|-------------|------|
| `MIT OR Apache-2.0` | 339 |
| `MIT` | 146 |
| `Apache-2.0 OR MIT` | 69 |
| `MIT/Apache-2.0` | 32 |
| `Apache-2.0` | 31 |
| `Unicode-3.0` | 18 |
| `BSD-3-Clause` | 11 |
| `Zlib OR Apache-2.0 OR MIT` | 10 |
| `Unlicense OR MIT` | 8 |
| `Apache-2.0/MIT` | 8 |
| **`MPL-2.0`** | **7** |
| **`GPL-3.0-or-later`** | **7** |
| `MIT OR Apache-2.0 OR Zlib` | 6 |
| `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | 5 |
| `BSD-2-Clause` | 4 |
| `Zlib` | 4 |
| `MIT / Apache-2.0` | 3 |
| 其余（各 1-2 个，均为宽松许可） | 25 |
| 待工具确认 | 21 |

**占绝对多数的是 MIT / Apache-2.0 家族（约 720 个包）。**

---

## 3. GPL-3.0-or-later：7 个（**全部来自 zed，且全部在界面层**）

| crate | 用途 |
|-------|------|
| `theme` | 设计令牌（颜色 / 字体 / 语法主题） |
| `ui` | zed 组件库 |
| `component` | `ui` 的组件基础 |
| `icons` | `ui` 的图标资源 |
| `menu` | `ui` 的菜单组件 |
| `syntax_theme` | 语法高亮主题（`theme` 依赖） |
| `ui_macros` | `ui` 的过程宏 |

**来源**：`https://github.com/zed-industries/zed`，rev `290cbcb9cb6a5dcbe0060431a126ad19e743f2f4`
**许可证全文**：`LICENSE-GPL-3.0-or-later`（34 KB，取自同一 rev，与上游一致）

### zed 仓库中**未使用**的 GPL crate（供对照）

zed 的绝大多数 crate 都是 GPL-3.0-or-later，包括 `markdown` / `language` / `editor` /
`workspace` / `project` / `settings` / `fs` / `language_models` / `web_search` /
`terminal` / `collab` 等。

**本项目刻意避开了它们**：

| 避开的原因 | 替代方案 |
|-----------|---------|
| `language` 会拖回 `settings` 框架，并经 `tree-sitter`(zed fork) → `wasmtime`（需 cmake） | 自研 type stub（`S04-02` 将换成 Comet 的 MIT `syntax`） |
| `markdown` 依赖 `settings` / `theme_settings` / `mermaid_render` | **vendor 后 patch**（S00-06，闭包仅 +25 包） |
| `editor` / `workspace` / `project` 深度耦合 zed 应用骨架 | 不引入 |

---

## 4. MPL-2.0：7 个（**文件级 copyleft，义务不同于 GPL**）

| crate | 用途 | 是否修改 |
|-------|------|---------|
| `cbindgen` | 构建期工具（生成 C 头） | 否 |
| `cssparser` | CSS 解析（HTML 渲染链） | 否 |
| `cssparser-macros` | 同上（过程宏） | 否 |
| `dtoa-short` | 浮点格式化 | 否 |
| `dwrote` | Windows DirectWrite 绑定 | 否 |
| `option-ext` | `Option` 扩展 | 否 |
| `selectors` | CSS 选择器匹配 | 否 |

**MPL-2.0 是文件级 copyleft**：
- 链接使用**不产生**传染义务
- 仅当你**修改了 MPL 覆盖的文件**时，才需要公开那些修改

**本项目未修改上述任何一个**，因此除保留许可证声明外无额外义务。

> ⚠️ 若将来需要修改其中任何 crate（通常需 vendor），必须公开修改后的文件。

---

## 5. 通过 `[patch.crates-io]` 固定的 zed fork

**S00-06 阻碍级发现**：zed 的 workspace 用自家 fork 覆盖了若干 crates.io 依赖，
而 **`[patch]` 不会传递给下游消费者** —— 不复制这些 patch 会编译失败
（`adopt_raw_pid` 找不到）。完整清单与「未复制项及理由」见
`docs/evidence/s00-06/README.md` §1。

| fork | 上游 | 许可证 | 本项目是否修改 |
|------|------|--------|---------------|
| `async-process` | smol-rs/async-process | `Apache-2.0 OR MIT` | 否 |
| `async-task` | smol-rs/async-task | `Apache-2.0 OR MIT` | 否 |
| `notify` / `notify-types` | notify-rs/notify | `CC0-1.0 OR MIT-0 OR Apache-2.0` | 否 |
| `tree-sitter-language` | tree-sitter/tree-sitter | `MIT` | 否 |

传递引入的其他 zed fork（非 `[patch]`，由 zed crate 自身引用）：

| fork | 许可证 |
|------|--------|
| `zed-font-kit`（`font-kit` fork） | `MIT OR Apache-2.0` |
| `zed-scap`（`scap` fork） | `MIT` |
| `wasm_thread` | `Apache-2.0 OR MIT` |

**均为宽松许可，且本项目未修改它们。**

---

### Buddy 的 macOS 热键补丁（2026-10-03）

`vendor/global-hotkey/` 来自 crates.io `global-hotkey 0.8.0`，许可证为 `Apache-2.0 OR MIT`，保留 `LICENSE-APACHE`、`LICENSE-MIT` 及源码版权声明。本地只修改 Carbon 注册 options 为独占，避免外部占键时静默成功；注册事务 / 事件桥 API 不变，其他平台源码不变。来源、字节级差异及复核方法见该目录的 `VENDOR.md`。此补丁只作用于 v2 workspace；v1 的独立 Cargo manifest 不受影响。

## 6. `[patch]` 未复制的项及理由

| 未复制 | 原因 |
|--------|------|
| `windows-capture` | 仅 Windows；当前 macOS 阶段不需要 |
| `calloop` | 仅 Linux |
| `livekit` / `libwebrtc` / `webrtc-sys` | 协作 / 音频功能，Buddy 不需要 |
| `scratch` | zed 的 corgi 构建工具**本地 path 依赖**，无法复制；不在本项目闭包内 |

> ⚠️ **若将来引入 `fs` / 终端 / 协作等功能，需按需补齐并更新本清单。**

---

## 7. 待工具确认（21 个）

以下包在本次统计中因 manifest 路径差异未能自动读取，**均为知名 crate，
许可证已知为宽松**，但**应由工具确认**（`S01-04` 引入 `cargo-deny` / `cargo-about`）：

| crate | 预期许可证 |
|-------|-----------|
| `openssl` / `openssl-sys` / `openssl-macros` / `openssl-probe` / `openssl-src` | `Apache-2.0` |
| `schannel` | `MIT` |
| `vcpkg` | `MIT OR Apache-2.0` |
| `anstyle-wincon` | `MIT OR Apache-2.0` |
| `wasm-streams` | `MIT OR Apache-2.0` |
| `jiff-static` | `Unlicense OR MIT` |
| `once_cell_polyfill` | `MIT OR Apache-2.0` |
| `derive_refineable` / `perf`（zed） | `Apache-2.0`（已实测 `derive_refineable` = `Apache-2.0`） |
| `buddy-engine` / `buddy-ui` / `buddy-app` | 本仓库自己的 crate，见 §1 |

---

## 8. 本仓库对 GPL 代码的修改（GPL 要求释出）

GPL-3.0-or-later 要求：**对 GPL 代码的修改必须以 GPL 释出。**

本项目当前**尚未 vendor 任何 zed GPL 代码**（`S04-01` 将 vendor `crates/markdown`）。
届时：

- 修改会以**独立 patch 文件**归档（`crates/ui/patches/zed-<crate>-<rev>.patch`）
- `S00-06` 已实测：对 `markdown.rs` 的修改**仅 3 行 import 级**
- vendored 文件的原始版权头与来源注释**必须保留**

**当前状态**：无 vendored GPL 代码，故无 patch 需释出。

---

## 9. 如何重新生成本清单

本清单的 §2 数据由脚本从 `Cargo.lock` + 本地 registry / git checkout 的 `Cargo.toml` 提取：

```bash
# 1. 列出所有 zed 来源的 crate 及其 rev（验证 rev 一致性）
python3 - <<'PY'
import re, collections
t = open('Cargo.lock').read()
pkgs = re.findall(r'\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"\n(?:source = "([^"]+)")?', t)
zed = [(n, v, s) for n, v, s in pkgs if s and 'zed-industries/zed' in s]
print(collections.Counter(re.search(r'rev=([0-9a-f]+)', s).group(1) for _, _, s in zed))
PY

# 2. 统计许可证分布（读每个包的 Cargo.toml 的 license 字段）
#    完整脚本见 git 历史中的 S01-03 提交
```

**`S01-04` 将把这一步自动化**（`cargo-deny` 或 `cargo-about`），并加入 CI，
断言本清单与依赖树一致。

---

## 10. 许可证全文索引

| 许可证 | 全文位置 |
|--------|---------|
| GPL-3.0-or-later | 本仓库 `LICENSE-GPL-3.0-or-later` |
| Apache-2.0 | 本仓库 `LICENSE-APACHE-2.0` |
| MIT / BSD / ISC / MPL-2.0 / Unicode-3.0 / Zlib 等 | 各依赖包内自带的 `LICENSE*` 文件（随 `~/.cargo/registry` 分发） |

> GPL 与 Apache-2.0 全文随仓库分发是本项目的合规要求；
> 其余许可证的全文由各依赖包自身携带。

---

## 附：Phase 04 新增依赖（S04-01 / S04-02，2026-09-27）

> 上文 §2 的 746 包统计截至 S01-03；此后新增项在此列出，全量重统计交 `S08-09`。

| 来源 | 新增包数 | 许可证 |
|------|---------|--------|
| S04-01 vendored markdown（主要经 zed `util`：`async_zip`、`rust-embed`、`nix`、`globset` 等） | 36（`Cargo.lock` 752 → 788） | 待 S08-09 逐项统计 |
| S04-02 语法高亮：`tree-sitter` 0.26.11、`tree-sitter-highlight` 0.26.11、`tree-sitter-language` 0.1.8 及 27 个语法包（rust / javascript / typescript / python / go / json / bash / toml-ng / md / html / css / yaml / c / cpp / c-sharp / java / kotlin-ng / swift / ruby / php / sequel / lua / nix / make / containerfile）、`streaming-iterator` | 30（788 → 818） | **全部 MIT**（`streaming-iterator` 为 MIT OR Apache-2.0）；均未修改 |
| S04-03 收敛：zed `util` 换为本地替身 `crates/markdown/util-shim`（Apache-2.0，内容取自 zed `util` / `gpui_util`）；10 个语法包（bash / toml-ng / c-sharp / java / ruby / php / lua / nix / make / containerfile）改为可选 feature `extra-languages`，默认构建不含 | −21（818 → 797）；默认产物中另少 10 个语法包 | 替身为 Apache-2.0；其余不变 |

| S04-07 起图标：`crates/ui/assets/icons/*.svg`（`streaming-star.svg` 除外，为 Buddy 自绘）取自 Lucide —— 由 `scripts/icons/lucide_svg.py` 从 v1 所用 `lucide-react` 1.28.0 的图形数据逐字生成（清单见 `crates/ui/src/icons.rs` 的 `icons!` 声明） | 0（资源文件，非 crate） | **ISC**（Copyright (c) 2026 Lucide Icons and Contributors；许可证全文见 `node_modules/lucide-react/LICENSE` / https://lucide.dev/license）；未修改图形 |

## 附：Phase 07 托盘与登录项依赖（2026-10-03）

- `tray-icon` 0.24.2（MIT OR Apache-2.0）及 `muda` 0.19.3（Apache-2.0 OR MIT）：原生托盘与菜单；关闭默认 GTK / libxdo features，当前范围为 macOS，Windows 另验。
- `auto-launch` 0.6.0（MIT）：使用 LaunchAgent 模式；其 macOS 依赖 `smappservice-rs` 0.1.3（MIT）虽参与编译，本产品不调用 SMAppService 注册模式。
- PNG 解码复用闭包已有 `image` 0.25.10，不引入第二套图像库。上述依赖均位于 UI 层，engine 未新增依赖。
- 版本以 `Cargo.lock` 为准；许可证取本地 registry 中对应版本的 Cargo.toml，原许可证随上游 crate 分发。
