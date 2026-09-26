# Phase 04: Markdown 栈

## 目标

建立流式 markdown 渲染能力，替换现有的 `react-markdown` + `remark-gfm` + `prism-react-renderer` + `markdownNormalizer.ts` + `StreamingMarkdown.tsx` 整套。

**策略**：vendor zed 的 `markdown.rs`（已验证耦合很浅），patch 掉 `settings` / `language`，语法高亮采用 Comet 的 MIT `syntax` crate。

## 相关文档

- `docs/tasks/v2.0.0-gpui/research-log.md` §2 §3.2 — 依赖闭包实测与 Comet 参考实现
- `docs/design/sse-and-api.md` — 流式约定
- `docs/design/pages-and-states.md` — 消息渲染状态

## 验收标准

- [ ] `cargo tree` 中 markdown 相关闭包收敛到约 18 个 zed crate（不含 `settings` / `lsp` / `rpc` / `language`）
- [ ] 流式追加无闪烁、无行尾重排、滚动不跳
- [ ] 未闭合标记（`**bold` / `[link](partial`）在流式中不产生抖动
- [ ] GFM 覆盖：表格、任务列表、删除线、自动链接
- [ ] 代码块语言识别 + 高亮 + 复制
- [ ] 文本选择与复制可用
- [ ] 高亮**只改前景色**，绝不改变字体/字重/换行/高度/滚动几何

## 开发工作

### 4.1 vendor 与 patch

| ID | Task | Details |
|----|------|---------|
| D01 | vendor `markdown.rs` | 复制 zed `crates/markdown/src/markdown.rs`（7428 行）到 `crates/ui/src/markdown/`，**保留 GPL-3.0-or-later 头** |
| D02 | 记录来源 | 文件头注释写明 zed rev、日期、本仓库的 patch 清单 |
| D03 | patch ThemeSettings（9 处） | 具体行号见 `research-log.md` §2：194 / 197 / 198 / 201 / 202 / 205 / 206 / 211 / 212 / 213 / 216 / 217 / 218 / 226 / 227 / 279 / 280 / 283 / 290 / 291 / 407 / 408 / 409。**全部替换为自有 `Typography` 配置** |
| D04 | 移除 settings 依赖 | patch 后从 `Cargo.toml` 删除 `settings` / `theme_settings`，确认编译通过 |
| D05 | 移除 language 依赖 | 见 4.2 |
| D06 | 处理 `mermaid_render` | 决定是否保留 mermaid 支持（现有 Buddy 无此功能，可裁掉以减小闭包） |
| D07 | 处理 `html5ever` / `markup5ever_rcdom` | 若不需要渲染原始 HTML 可裁掉；**注意：裁掉意味着 HTML 按字面量显示** |
| D08 | 记录 patch 文件 | 生成的 patch 单独存为 `crates/ui/patches/zed-markdown-<rev>.patch`（满足 GPL 对修改部分的释出要求） |

### 4.2 语法高亮（替换 zed `language`）

| ID | Task | Details |
|----|------|---------|
| D09 | 引入 Comet `syntax`（MIT） | 移植 `~/Project/comet/crates/syntax/src/lib.rs`（1354 行），保留 MIT 头与来源注释 |
| D10 | 语言注册表 | 按现有 `CodeBlock.tsx` 支持的语言集建立 registry（`LanguageId` + 别名 + 扩展名） |
| D11 | grammar 依赖 | 选用 tree-sitter 官方 grammar，逐一确认许可证并写入 `THIRD_PARTY_NOTICES.md` |
| D12 | 替换 `language::` 调用点 | zed markdown 中 36 处（多在测试，4894 行以后），改为调用 Comet syntax 的接口 |
| D13 | `Rope` 依赖处理 | zed 的 `Rope` 用于源码操作；评估保留 zed `rope` crate 或换更轻实现 |
| D14 | `CharClassifier` 处理 | 词边界判定，需在 Comet syntax 或自有工具中提供等价能力 |
| D15 | 闭包复测 | `cargo tree` 确认 `lsp` / `rpc` / `fs` / `http_client` / `language` 全部消失 |

### 4.3 流式渲染

| ID | Task | Details |
|----|------|---------|
| D16 | 块粒度增量解析 | 只从最后一个未稳定的顶层块边界重解析（对齐 Comet `parser.rs` 思路） |
| D17 | 后台解析 + 合并 | 解析放 `background_spawn`，在途时新内容到达则合并重跑（zed `markdown.rs:928` 的 coalescing 模式） |
| D18 | 半截标记修补 | 移植 Comet `mend.rs`（413 行）思路：显示层自动补全未闭合的 `**` / `*` / `_` / `~~` / 反引号 / `[link](` |
| D19 | 链接占位 | 流式中的 URL 用哨兵值（Comet 用 `PENDING_LINK_URL`），不得触发真实跳转 |
| D20 | setext 闪烁修正 | 流式中单独一行 `-` / `--` / `=` 不得被误判为 setext 下划线导致段落变标题 |
| D21 | 渐显效果 | 移植 `veil.rs`（508 行）思路：**纯绘制层** alpha，不改布局，故永不 reflow |
| D22 | 尊重减弱动效 | 系统「减弱动态效果」开启时关闭渐显 |
| D23 | 结算语义 | 流结束后用正式解析结果替换显示层解析，允许一次性翻转但不得抖动 |

### 4.4 渲染元素

| ID | Task | Details |
|----|------|---------|
| D24 | 段落 / 标题 | 各级标题的字号与行高取自 Phase 03 |
| D25 | 行内代码 | 背景 wash + 圆角（沿用现有 `inline_code` 风格） |
| D26 | 代码块 | 等宽字体、不换行（高度 = 行数 × 行高，使高亮与布局解耦）、横向滚动条 |
| D27 | 代码块复制按钮 | 复现 `CodeBlock.tsx`（229 行）的复制交互 |
| D28 | 列表 / 引用 | 嵌套列表、引用块 |
| D29 | 表格 | 表格渲染 + 列宽策略（参考 Comet `table_columns`） |
| D30 | 图片 | 渲染 + 加载失败占位；与 Phase 02 D21 的附件路径打通 |
| D31 | 链接 | 点击打开系统浏览器 |
| D32 | 删除线 / 任务列表 / 脚注 | GFM 补充项 |
| D33 | 文本选择 | 复现现有 `MessageBubble` 的选择行为，含跨块选择 |

### 4.5 与现有实现的对照

现有前端需被替换的资产：

| 现有文件 | 行数 | 处理 |
|---------|------|------|
| `src/components/chat/StreamingMarkdown.tsx` | 434 | 替换 |
| `src/utils/markdownNormalizer.ts` | 230 | 逻辑并入 D18 / D19 / D20 |
| `src/components/chat/CodeBlock.tsx` | 229 | 替换 |
| `src/components/chat/PlainCodeBlock.tsx` | — | 替换 |
| `src/components/chat/MessageActions.tsx` | 160 | 归 Phase 05 |
| `react-markdown` / `remark-gfm` / `prism-react-renderer` | — | 移除依赖 |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T01 | 解析器单测 | 各类 markdown 结构的解析结果断言 |
| T02 | mender 单测 | 半截标记的修补结果逐个断言（对照 Comet `mend.rs` 的已知取舍） |
| T03 | 高亮不变式测试 | **断言高亮不改变布局**：同文本带/不带高亮，测量高度必须相同 |
| T04 | 流式稳定性测试 | 逐字符喂入长文档，断言无重排（记录每次 append 后的行数与总高） |
| T05 | GFM 覆盖测试 | 表格 / 任务列表 / 删除线 / 自动链接 |
| T06 | 性能测量 | 长文档（>5000 行）解析与渲染耗时 |
| T07 | 对照测试 | 同一 markdown 源在旧 React 实现与新实现的渲染结果对比 |

## 已知取舍（需记录决策）

| 项 | 取舍 |
|----|------|
| mermaid | 现有 Buddy 无此功能。若裁掉，含 mermaid 代码块按普通代码渲染 |
| 原始 HTML | 若裁掉 `html5ever`，HTML 按字面量显示而非渲染 |
| 数学公式 | 现有 Buddy 无。Comet 也未实现 |
| 半截标记修补的近似性 | Comet `mend.rs` 明确接受近似：`2**3` 会短暂加粗 `3`；完成后由正式解析纠正 |
