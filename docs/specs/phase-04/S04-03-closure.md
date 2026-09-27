# S04-03 闭包收敛与依赖清理（mermaid / html 裁剪决策）

> 状态: `done`
> Phase: 04
> 依赖: S04-02
> 阻塞: —
> 退役设计文档: —

## 目标

确定 mermaid（stub）与原始 HTML 渲染（`html5ever`）的去留，量化 markdown 栈的依赖增量。

## 输入

- S00-06：闭包 +25 包（718）；v1 无 mermaid、无原始 HTML 渲染（react-markdown 默认不渲染 HTML）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S04-03-1 | mermaid | 保持 stub（v1 无此功能） |
| S04-03-2 | 原始 HTML | 以 v1 行为为准：react-markdown 把 HTML 转义为文本 → `parse_html = false` |
| S04-03-3 | zed `util` | 本地替身，vendored 源码不改 |
| S04-03-4 | 未用语法包 | feature 化，默认不编译 |

## 验收标准

- [x] 依赖增量有 `cargo tree` 数值证据
- [x] 两项裁剪决策记录 v1 实际行为依据

## 证据

| 项 | 证据 |
|----|------|
| 原始 HTML（v1 实测） | 用 v1 的 `react-markdown` 10.1.0 + `remark-gfm`（无 rehype-raw）在 Node 中 `renderToStaticMarkup`：`行内 <b>粗</b> 与 <br> 换行` → `<p>行内 &lt;b&gt;粗&lt;/b&gt; 与 &lt;br&gt; 换行</p>`；块级 `<div align="center">…</div>` → 转义文本。**HTML 显示为字面文字、`<br>` 不换行** |
| HTML 决策落地 | vendored `MarkdownOptions.parse_html` 默认 `false`（上游测试 `test_br_tag_not_a_hard_break_without_parse_html`：`<br>` 保留为 InlineHtml 文本）；`html5ever` 仍随 `html` 模块编译（上游 `parser.rs` 引用，不改源码），运行时不走渲染路径 —— 实际显示效果交 S04-08 预览目检 |
| mermaid | 保持 S00-06 stub（v1 无此功能；`render_mermaid_diagrams` 默认 false） |
| util 替身 | `cargo tree -i util` → 仅 buddy-markdown 使用；替身重导出 `gpui_util` + 逐字复制 `generate_heading_slug`（与 zed `util/src/markdown.rs:4-17` diff 无差异）；**`Cargo.lock` 818 → 797（−21）**；vendored `src/` 零改动 |
| 未用 patch | `cargo tree -i async-process` 为空 → 移除该 `[patch.crates-io]` 项（注释写明何时加回） |
| 语法包裁剪 | 编译产物测量：25 个语法静态库合计 32.8 MB，其中 v1 无高亮的 10 个（c-sharp 5.2 / php 2.2 / ruby 2.1 / bash 1.5 / java 0.5 / make / containerfile / nix / lua / toml 各 ≤0.2）≈ **12 MB** → 移入 `extra-languages`（默认关）；默认构建 `cargo tree -p buddy-app` 中这 10 个包命中 **0**；测试构建开启 feature，Comet 测试 18 + 8 通过 |
| GPL patch 同步 | 新检查「S04-03 GPL patch 同步」（上游 + patch == vendored），拦截验证 14（改 parser.rs 不更新 patch → FAIL）；纪律检查 17 项 / 拦截验证 14 项全部通过 |
| 提交 | `6c55b0c` |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| HTML 处理 | 解析关闭，`html5ever` 仍编译 | v1 实测为转义文本；裁掉 `html5ever` 需改上游 `parser.rs`，收益仅数个包，不值得破坏「vendored 原文不改」 |
| util | 本地替身（lib 名 `util`） | 零源码改动即可去掉 21 个包 |
| 语法包 | feature 化默认关 —— **2026-09-27 用户决定：启用**（buddy-markdown 打开 `extra-languages`，commit `06057e1`） | 约 12 MB 静态数据（未压缩），对照 < 10 MB 安装包目标 → 最终包体积在 Phase 08 实测，超标时再议 |
| 测试 | dev-dependency 自引用开启 feature | 保持 Comet 原测试集完整 |

## 完成记录

- 日期：2026-09-27
- commit：`6c55b0c`
- 设计文档处置：—
