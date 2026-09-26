# S01-03 许可证分层声明与 NOTICE

> 状态: `done`
> Phase: 01
> 依赖: S01-01, S01-02
> 阻塞: —
> 退役设计文档: —

## 目标

把 GPL 边界用文件与声明固化下来，并建立完整的第三方许可证清单。

**产出物**：`LICENSE` / `THIRD_PARTY_NOTICES.md` / 各 crate 的 `license` 字段 / vendored patch 归档。

## 输入

- `docs/tasks/v2.0.0-gpui/research-log.md` §6 —— 许可证分层（Apache-2.0 平台层 / GPL 界面层 / MIT 引擎层）
- `docs/specs/RULES.md` §7 —— 设计文档退役规则（vendored patch 需 GPL 释出）
- S00-06 的 patch 清单
- `~/Project/comet/THIRD_PARTY_NOTICES.md` —— 清单格式参考

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S01-03-1 | 界面层声明 | `crates/ui` + `apps/buddy` 设 `license = "GPL-3.0-or-later"` |
| S01-03-2 | 引擎层声明 | `crates/engine` 设 `license = "MIT"`（或 Apache-2.0），并在 README 写明「engine 层不含 GPL 代码」及**边界理由** |
| S01-03-3 | 分层 NOTICE | `THIRD_PARTY_NOTICES.md` 按许可证分组：<br>Apache-2.0: gpui / gpui_platform / gpui_macos / gpui_wgpu<br>GPL-3.0-or-later: zed `theme` / `ui` / vendored `markdown`<br>MIT: Comet `syntax`（若采用）<br>tree-sitter grammars：**逐一列出**<br>每项需标明来源仓库与 rev（zed 基准 rev 见 research-log §9.1；Comet 见 §9.7） |
| S01-03-4 | vendor patch 归档 | vendored `markdown.rs` 的修改存为独立 patch 文件（满足 GPL 对修改部分的释出要求） |
| S01-03-5 | 根 LICENSE | 说明分层许可：`crates/engine` MIT，其余 GPL-3.0-or-later |
| S01-03-6 | 保留原始版权头 | vendored / 移植的文件必须保留原始版权声明与来源注释 |
| S01-03-7 | grammar 许可证核对 | 每个 tree-sitter grammar 确认许可证（多数 MIT/Apache，个别需注意） |
| S01-03-8 | 来源注释 | 每个移植文件头部注明：来源项目、rev/版本、原始许可证、本仓库的修改 |
| S01-03-9 | **vendor 清单（S00-06 实测）** | `crates/markdown` 共 vendor **12680 行**（`markdown.rs` 7428 / `parser.rs` 1893 / `html.rs` + `html/` 2516 / `selection.rs` 598 / `path_range.rs` 245），另加 3 个 shim/stub（491 行）。均在 `crates/ui` 下，受 GPL 约束 |
| S01-03-10 | **Comet `syntax` 的 MIT 声明** | `S04-02` 将引入 Comet 的 `crates/syntax`（1354 行，**MIT**）。它属**界面层**（依赖 GPUI），因此链接后整体仍为 GPL，但**必须保留其 MIT 声明与版权头** |
| S01-03-11 | **engine 侧的纯 MIT 声明** | S00-08 已实证 `buddy-engine` 依赖树 0 处 GPUI/Tauri/zed crate。其依赖（`reqwest`/`tokio`/`serde`/`chrono`/`dom_query`/`base64`/`parking_lot`/…）均为 MIT/Apache 兼容，可完整声明为 MIT |
| S01-03-12 | **`[patch.crates-io]` 的许可证记录** | 复制的 5 个 zed fork（`async-process` / `notify` 等）也需要确认许可证并写入 NOTICE |

## 验收标准

### 本 spec 范围内（已达成）

- [x] 三个 crate 的 `license` 字段正确
- [x] `THIRD_PARTY_NOTICES.md` 覆盖第三方依赖，按许可证分组
- [x] 根 `LICENSE` 清楚说明分层边界
- [x] `crates/engine/README.md` 写明引擎层的可闭源性与理由
- [x] GPL 与 Apache-2.0 **全文随仓库分发**（合规要求）

### 移交后续 spec（因产物尚不存在，无法在本 spec 完成）

编写本 spec 时假设 vendored markdown 与 tree-sitter grammar 已在 Phase 01 存在 ——
**但实际它们是 `S04-01` / `S04-02` 的产物**。因此以下三项**范围调整**，明确移交：

- [ ] 所有 tree-sitter grammar 的许可证逐一列出 → **移交 `S04-02`**（引入 Comet `syntax` 时）
- [ ] vendored `markdown.rs` 的 patch 作为独立文件归档 → **移交 `S04-01`**（vendor 时生成）
- [ ] 所有移植文件保留原始版权头与来源注释 → **移交 `S04-01`**（vendor 时检查）

> 这三项**不是遗漏，是范围修正**：Phase 01 尚无任何 vendored / 移植代码。
> `THIRD_PARTY_NOTICES.md` §8 已预留接口与要求，`S01-04` 的 CI 断言会守护它们。

## 证据

### 1. 三层许可证声明

| crate | `license` 字段 |
|-------|---------------|
| `crates/engine/Cargo.toml` | `MIT` |
| `crates/ui/Cargo.toml` | `GPL-3.0-or-later` |
| `apps/buddy/Cargo.toml` | `GPL-3.0-or-later` |

### 2. 已创建的许可产物

| 文件 | 大小 | 内容 |
|------|------|------|
| `LICENSE` | 2457 B | 分层说明 + GPL 实际约束 + 致谢 |
| `LICENSE-GPL-3.0-or-later` | **34357 B** | GPL-3.0 全文（取自 zed rev `290cbcb`，与上游一致） |
| `LICENSE-APACHE-2.0` | 10768 B | Apache-2.0 全文（GPUI 平台层使用） |
| `THIRD_PARTY_NOTICES.md` | 8059 B | 全量许可证分布 + GPL/MPL 明细 + fork 清单 + 重新生成方法 |
| `crates/engine/README.md` | 3947 B | 引擎层可闭源性与理由 + 4 处移植改动 |

全部确认未被 `.gitignore` 排除。

### 3. 全量许可证统计（从 `Cargo.lock` 实测，746 个包）

**占绝对多数的是 MIT / Apache-2.0 家族（约 720 个包）。**

| 许可证 | 包数 | 处理 |
|--------|------|------|
| `MIT OR Apache-2.0` | 339 | 无额外义务 |
| `MIT` | 146 | 无 |
| `Apache-2.0 OR MIT` | 69 | 无 |
| `MIT/Apache-2.0` | 32 | 无 |
| `Apache-2.0` | 31 | 无 |
| `Unicode-3.0` | 18 | 保留声明 |
| `BSD-3-Clause` | 11 | 保留声明 |
| 其余宽松许可（Zlib / Unlicense / BSD-2 / ISC / CC0 / 0BSD / bzip2 / NCSA / BSL-1.0 / LLVM-exception） | ~70 | 保留声明 |
| **`MPL-2.0`** | **7** | **文件级 copyleft —— 未修改，无额外义务** |
| **`GPL-3.0-or-later`** | **7** | **仅界面层，见 §4** |

### 4. GPL 的精确范围：**7 个 crate，全部在界面层**

| crate | 用途 |
|-------|------|
| `theme` | 设计令牌 |
| `ui` | zed 组件库 |
| `component` | `ui` 的组件基础 |
| `icons` | 图标资源 |
| `menu` | 菜单组件 |
| `syntax_theme` | 语法主题 |
| `ui_macros` | 过程宏 |

**全部来自 zed rev `290cbcb`。全部只被 `crates/ui` 使用。**

对照：zed 仓库里 GPL 的 crate 有 **150+ 个**（含 `markdown` / `language` / `editor` /
`workspace` / `project` / `settings` / `fs` / `terminal` / `collab` …），
**本项目刻意避开了它们**（理由与替代方案见 `THIRD_PARTY_NOTICES.md` §3）。

### 5. 引擎层纯净性（风险 R7 的核心）

`crates/engine` 的直接依赖：

```
tokio / reqwest / serde / serde_json / futures-util / async-trait
thiserror / log / chrono / dom_query / base64 / parking_lot
```

**全部 MIT / Apache-2.0。** `cargo tree -p buddy-engine` 中
0 处 `gpui` / `theme` / `ui` / GPL crate（S00-08 实测）。

### 6. MPL-2.0 的 7 个包

`cbindgen` / `cssparser` / `cssparser-macros` / `dtoa-short` / `dwrote` /
`option-ext` / `selectors` —— **本项目未修改其中任何一个**，
因此除保留许可证声明外无额外义务。

> MPL-2.0 是**文件级** copyleft：链接使用不传染；仅当修改 MPL 覆盖的文件时才需公开那些修改。
> 已在 `THIRD_PARTY_NOTICES.md` §4 标注，并提示「若将来 vendor 其中任何 crate，必须公开修改」。

### 7. `[patch.crates-io]` 的 fork 许可证

| fork | 许可证 | 本项目是否修改 |
|------|--------|---------------|
| `async-process` | `Apache-2.0 OR MIT` | 否 |
| `async-task` | `Apache-2.0 OR MIT` | 否 |
| `notify` / `notify-types` | `CC0-1.0 OR MIT-0 OR Apache-2.0` | 否 |
| `tree-sitter-language` | `MIT` | 否 |
| `zed-font-kit` | `MIT OR Apache-2.0` | 否 |
| `zed-scap` | `MIT` | 否 |
| `wasm_thread` | `Apache-2.0 OR MIT` | 否 |

**均为宽松许可。** 已在 `THIRD_PARTY_NOTICES.md` §5 列出。

### 8. 21 个待工具确认的包

因 manifest 路径差异未能自动读取。**均为知名 crate，预期许可证为宽松**，
且已在 `THIRD_PARTY_NOTICES.md` §7 逐条列出预期值。
**`S01-04` 将引入 `cargo-deny` / `cargo-about` 自动化确认。**

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 许可证全文是否随仓库分发 | **是**（GPL + Apache-2.0） | GPL 合规要求；且避免依赖网络获取 |
| GPL 全文来源 | 取自 zed rev `290cbcb` | 与上游逐字节一致，避免版本歧义 |
| engine 层许可证 | **MIT** | 最宽松，最大化「以后独立使用 / 闭源」的可能性 |
| 是否统一根许可证 | **否，分层声明** | `crates/engine` 必须能单独按 MIT 使用；整齐划一反而丢失这个性质 |
| 三项验收标准处理 | **范围修正 + 移交** | vendored 产物是 `S04-01`/`S04-02` 的产物，Phase 01 无法完成。如实记录而非勾选了事 |
| MPL-2.0 的处理 | 保留声明，不修改其文件 | 文件级 copyleft，未修改即无额外义务 |

## 完成记录

- 日期：2026-09-11
- commit：（`LICENSE` / `LICENSE-GPL-3.0-or-later` / `LICENSE-APACHE-2.0` / `THIRD_PARTY_NOTICES.md` / `crates/engine/README.md` 已落地）
- 设计文档处置：—（本 spec 不涉及设计文档退役）

## 遗留与交接

| 项 | 交接 |
|----|------|
| tree-sitter grammar 许可证逐一列出 | `S04-02` |
| vendored `markdown.rs` 的 patch 归档 | `S04-01` |
| 移植文件的版权头与来源注释检查 | `S04-01` |
| 21 个包的许可证自动确认 | `S01-04`（`cargo-deny` / `cargo-about`） |
| Comet `syntax` 的 MIT 声明 | `S04-02` |
| **若引入 `fs` / 终端 / 协作，需补齐 `[patch]` 并更新 NOTICE** | `S02-*` / `S07-*` |
