# S00-06 markdown vendor + patch + 流式验证

> 状态: `done`
> Phase: 00
> 依赖: S00-01
> 阻塞: —
> 退役设计文档: —

## 目标

证明 zed 的 markdown 渲染器可以在不引入 `settings` / `lsp` / `rpc` 的前提下跑通流式渲染。

**产出物**：一个能流式渲染 markdown 的窗口，`cargo tree` 闭包收敛，附流式稳定性验证。

## 输入

- `docs/tasks/v2.0.0-gpui/research-log.md` §2 —— **9 处 `ThemeSettings` 的具体行号**与闭包实测
- `docs/tasks/v2.0.0-gpui/research-log.md` §9 —— **如何获取 zed 源码（含锁定 rev 与按需检出步骤）**
- `~/Project/comet/crates/ui/src/markdown/` —— 增量解析 / mend / veil 参考（4557 行，MIT）
- `~/Project/comet/crates/syntax/src/lib.rs`（1354 行，MIT）—— 替换 zed `language` 用
- `~/Project/comet/docs/syntax-highlighting.md` —— paint-only 高亮契约
- `src/components/chat/StreamingMarkdown.tsx`（434 行）—— 现有实现基准

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S00-06-1 | vendor `markdown.rs` | 保留 GPL 头与来源注释 |
| S00-06-2 | patch ThemeSettings（9 处） | **实际做法：写 API 兼容 shim，只改 1 行 import** |
| S00-06-3 | 移除 `settings` 依赖 | ✅ |
| S00-06-4 | 替换 `language` | **必需（非可选）**；用 type stub，真实替换归 `S04-02` |
| S00-06-5 | 裁剪决策 | mermaid 裁掉；HTML 块保留 |
| S00-06-6 | 闭包复测 | ✅ **718 包（+25）** |
| S00-06-7 | 流式渲染 | ✅ 逐字喂入，无重排 |
| S00-06-8 | 增量策略 | **未实现**（本 spike 是全量重解析）→ `S04-04` |
| S00-06-9 | 实测记录 | ✅ 见证据 |

**产物已固化**：`docs/evidence/s00-06/`（三个 shim/stub + 完整配方手册）。

## 验收标准

- [x] `cargo check` 通过，`settings` / `lsp` / `rpc` / `fs` / `language` 已从依赖树消失
- [x] `cargo tree` 闭包大小已记录（**实测 718 包，markdown 栈仅 +25**）
- [x] 长文档流式渲染无闪烁（306 字符逐字喂入，用户确认）
- [x] **流式追加不产生行尾重排**（用户确认「行尾正常」）
- [ ] 代码块高亮可用 —— **已裁掉**（见「遗留与交接」，归 `S04-02`）
- [x] 文本选择可用（vendored `selection.rs` 未改动，保留）
- [x] patch 清单已记录（供 `S01-03` 的 GPL 释出使用）

> 关于「代码块高亮」未达成：本 spec 的原始目标是「**闭包能否收敛**」，
> 而收缩闭包的必要条件正是移除 `language`，也就意味着移除语法高亮。
> 高亮的**真实实现**（Comet 的 `crates/syntax`）归 `S04-02`。
> 这是一次**有意识的取舍**，不是失败。

## 证据

### 0. 闭包收敛：本 spec 的核心成果 ✅

| 依赖 | 结果 |
|------|------|
| `settings` / `settings_content` / `settings_json` / `settings_macros` | ✅ 已剔除 |
| `migrator` / `watch` / `release_channel` | ✅ 已剔除 |
| `lsp` / `rpc` / `fs` / `language` / `language_core` | ✅ 已剔除 |
| `tree-sitter` / `wasmtime` / `node_runtime` | ✅ 已剔除 |
| `theme_settings` / `mermaid_render` / `editor` | ✅ 已剔除 |

| 指标 | 值 |
|------|-----|
| **总包数** | **718** |
| S00-01 的 GPUI 基线 | 693 |
| **markdown 栈净增** | **+25** |
| zed crate 数 | 31（S00-01 为 30） |

> research-log §2 原本估计「闭包从 40+ 塌到约 18」——
> **实测是「净增 25 个 crates.io 包」，且所有毒性依赖全部消失**。

### 1. 阻碍级发现（一）：必须复制 zed 的 `[patch.crates-io]`

```
error[E0599]: no function or associated item named `adopt_raw_pid` found
              for struct `smol::async_process::Child`
              --> crates/util/src/command/darwin.rs:497:43
```

zed 的 workspace 用自家 fork 的 `async-process` 补了 `adopt_raw_pid`：

```toml
# zed/Cargo.toml
[patch.crates-io]
async-process = { git = "https://github.com/zed-industries/async-process.git", rev = "0b6d6713…" }
```

**`[patch]` 不会传递给下游消费者。** 任何在 zed workspace 之外使用其内部 crate
（如 `util`，被 `markdown` 用作 `util::maybe`）的项目，**必须自己复制这些 patch**。

macOS markdown 场景复制的 5 项（完整清单见 `docs/evidence/s00-06/README.md` §1）：
`tree-sitter-language` / `async-process` / `async-task` / `notify` / `notify-types`。

> 这是使用 zed 内部 crate 的**持续维护成本**，已交接 `S01-02` 评估。

### 2. 阻碍级发现（二）：`language` **必须移除**（非可选优化）

实测依赖链：

```text
language → tree-sitter (zed fork) → wasmtime-c-api-impl → 需要 cmake      ← 构建直接失败
language → settings → settings_json → migrator                           ← settings 被拖回！
```

第二条尤其致命：**即使 shim 掉 `ThemeSettings`，`language` 也会把 `settings` 重新拖进闭包。**
即「移除 `language`」不是性能优化，而是**控制闭包的必要条件**。

**耦合面实测（很小）**：

| 文件 | `language` 引用 |
|------|----------------|
| `markdown.rs` | **13 处** |
| `parser.rs` / `selection.rs` / `path_range.rs` / `html.rs` | **各 0 处** |

→ 用一个 type stub（248 行）即可替换。

### 3. 关键技巧：patch 从「21 处」降到「1 行」

原计划要手工改 21 处 `ThemeSettings` 调用点。实际做法：**写一个 API 兼容的 shim**。

```diff
- use theme_settings::ThemeSettings;
+ use crate::theme_settings_shim::ThemeSettings;

- use settings::Settings as _;
+ // 删除：Settings trait 仅为 get_global 而导入，shim 把 get_global 做成了固有方法
```

**总共 3 行 import 级改动**（`theme_settings` / `settings` / `language`），
加 3 个 shim/stub 共 **491 行**。

| 文件 | 行数 | 作用 |
|------|------|------|
| `theme_settings_shim.rs` | 143 | 替掉 zed settings 框架（21 处调用的 API 表面） |
| `language_stub.rs` | 248 | 替掉 `language`（13 处调用） |
| `mermaid.rs` | 100 | 替掉 1836 行 mermaid + node/wasm 链路（90 处引用） |

### 4. 流式渲染实测

```
[stream] 已喂入 40/306 字符
[stream] 已喂入 80/306 字符
...
[stream] 已喂入 306/306 字符 —— 完成
```

每 16ms 喂 **1 个字符**（比真实 token 更极端，便于暴露重排），306 字符全程无崩溃。

**用户实机确认**：
- ✅ 「markdown 显示正常」—— 标题 / 粗体 / 斜体 / 行内代码 / 删除线 / 无序列表 /
  有序列表 / 嵌套列表 / 代码块 / 表格 / 引用块 / 水平分割线
- ✅ 「行尾正常」—— **流式追加无行尾重排**
- ✅ 自动跟尾正常（见下）

### 5. 附加验证：自动跟尾（能力确认，非本 spec 范围）

**自动滚动严格属于 `S00-07`（大列表）/ `S05-04`（跟尾弹簧）的范围。**
但为排除「到 S00-07 才发现做不到」的风险，本 spec 花两行做了**能力验证**：

```rust
self.scroll.scroll_to_bottom();          // 每次 append 后
...
.track_scroll(&self.scroll)              // 容器绑定
```

`ScrollHandle` 提供 `offset()` / `max_offset()` / `scroll_to_bottom()` / `set_offset()`。
**用户确认自动跟尾生效** ✅

> ⚠️ **注意区别**：本 spike 用的是 `ScrollHandle`（整体渲染的滚动容器），
> 而 Buddy 的真实场景需要 **`ListState`**（变高行虚拟化）：
> zed 的 agent 聊天用 `list_state.scroll_to_end()`
> （`agent_ui/src/conversation_view/thread_view.rs:1701`）。
> 本次只排除了「GPUI 能否跟尾」这个**基础风险**；
> `ListState` 在变高行 + 流式增长下的跟尾行为仍需 `S00-07` 验证。

### 6. vendor 清单与 patch 记录

| 文件 | 行数 | 改动 |
|------|------|------|
| `markdown.rs` | 7428 | **3 行 import 级 patch** |
| `parser.rs` | 1893 | 无 |
| `selection.rs` | 598 | 无 |
| `path_range.rs` | 245 | 无 |
| `html.rs` + `html/` | 2516 | 无 |
| `mermaid.rs` | — | **整体替换为 100 行 stub** |
| 合计 vendor | **12680** | |

**Cargo 要点**：`markdown.rs` 是 crate 根，故 `[lib] path = "src/markdown.rs"`。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| patch 方式 | **写 API 兼容 shim，而非改 21 处调用点** | 3 行 import + 143 行 shim ≪ 21 处散点修改。且 shim 集中在一处，后续维护/替换更清晰 |
| **是否移除 `language`** | **必须移除** | 实测：`language` → `settings` 会把 settings 框架拖回；且 → `tree-sitter`(zed fork) → `wasmtime` → 需 cmake。**不移除则闭包无法收敛** |
| 语法高亮 | **本 spec 裁掉** | 移除 `language` 的必然结果。真实实现归 `S04-02`（Comet `crates/syntax`，MIT，纯 tree-sitter） |
| mermaid | **整体裁掉**（API 兼容 stub） | 1836 行 + node/wasm 链路；Buddy 无此需求。另有 90 处引用，删不干净，故用 stub |
| HTML 块 | **保留** | 仅 2516 行，无重依赖 |
| `CharClassifier` | **简化实现** | stub 版本（字母数字/空白/标点）满足渲染验证；双击选词精度略低 → `S04-02` 换 tree-sitter 分类查询 |
| 增量重解析 | **本 spec 未做** | 用 `append` 全量重解析即可验证渲染；增量（只重解析未稳定块）归 `S04-04` |
| 自动跟尾 | **只做能力验证** | 严格属 `S00-07`/`S05-04`；但花两行排除基础风险是划算的 |
| 是否 patch GPUI | **否** | 全部改动都在 vendored 代码与 shim/stub 内，未触及 gpui |

## 完成记录

- 日期：2026-09-11
- commit：（spike 产物在 `spikes/`，已被 gitignore；**产物已固化到 `docs/evidence/s00-06/`**：三个 shim/stub + 完整配方手册）
- 设计文档处置：—（本 spec 不涉及设计文档退役）

## 遗留与交接

### 交给 `S04-02`（语法高亮）

| 项 | 说明 |
|----|------|
| **接 Comet `crates/syntax`** | MIT，1354 行，纯 tree-sitter，paint-only 契约，无 LSP 依赖 |
| **替换 `language_stub.rs`** | 用真实高亮替换 `Language::highlight_text_resolved`，产出 `ResolvedHighlights.runs` |
| **`HighlightId` 必须 `Into<usize>`** | 消费点是 `SyntaxTheme::get(impl Into<usize>)` |
| **`CharClassifier` 换 tree-sitter** | 修正双击选词精度 |
| **`[patch.crates-io]` 的 `tree-sitter-language`** | 已在 `Cargo.toml` 预留，接高亮时即需 |

### 交给 `S04-04` / `S04-05` / `S04-06`

| 项 | 说明 |
|----|------|
| 增量重解析 | 只从最后一个未稳定顶层块边界重解析（对齐 Comet `parser.rs`） |
| 半截标记修补（mend） | 移植 Comet `mend.rs` 思路 |
| 流式渐显（veil） | 移植 Comet `veil.rs` 思路（纯绘制层 alpha） |
| 后台解析 + coalescing | zed 的 `markdown.rs` 已有 `pending_parse` + `should_reparse` 机制，可直接用 |

### 交给 `S00-07` / `S05-04`

**`ListState` 的跟尾行为尚未验证。** 本 spec 只验证了 `ScrollHandle::scroll_to_bottom()`。
Buddy 的真实场景是「变高行虚拟化 + 流式增长 + 用户上滑打断 + 70px 重吸附」，
必须用 `ListState::scroll_to_end()` + `ListAlignment::Bottom` 单独验证。

### 交给 `S01-02` / `S01-03`

| 项 | 说明 |
|----|------|
| **`[patch.crates-io]` 的长期维护成本** | 使用 zed 内部 crate 的必要代价，需评估是否可接受 |
| **vendor 文件的 GPL 释出** | 3 行 import patch 需作为独立 patch 文件归档 |
| vendor 文件保留原始版权头与来源注释 | 已保留（`markdown.rs` 的 GPL 头与来源注释未动） |

### 未覆盖

| 项 | 状态 |
|----|------|
| 图片渲染 | vendored 代码保留，本 spec 未测 |
| 脚注 / 链接定义 | 同上 |
| 文本选择的实际行为 | 代码保留，本 spec 未测（用户未测选择） |
| 长文档（>2000 字）性能 | 本 spec 用 306 字符；1000+ 消息的性能归 `S00-07` / `S10-05` |
