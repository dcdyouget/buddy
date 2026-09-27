# S04-06 流式渐显（veil）

> 状态: `blocked`
> Phase: 04
> 依赖: S04-04
> 阻塞: 等待用户目检（`handoff.md` §6.5 第 7 项：流式渐显与星标观感是否与 v1 一致）
> 退役设计文档: —

## 目标

新到文字以纯绘制层 alpha 渐显，不改布局；尊重减弱动效。

## 输入

- Comet `veil.rs`（MIT，508 行）
- v1 流式行为用例（`v1-final:src/components/chat/StreamingMarkdown.test.tsx`）：`:151` 最新字符渐显且星标在下一位置、`:199` 星标锚定列表最后可见字符、`:214` 首字符前显示呼吸星标、`:230` 代码内容不加字符过渡、`:245` 未闭合流式围栏闭合前保持纯文本（S04-05 移交）
- v1 令牌：`--delay-streaming-char-age-1..8`（-32..-256ms）、`--duration-streaming-char-settle` 260ms（`theme_system::tokens::motion`）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S04-06-1 | 参数 | 取 v1 令牌，不另定 |
| S04-06-2 | 减弱动效 | 系统开启时关闭渐显（v1 `WindowEntrance.tsx` 用 `prefers-reduced-motion`） |

## 验收标准

- [x] 渐显不改变布局（同文本有无 veil 高度相同）—— T08
- [ ] 观感与 v1 一致 —— **需用户目检**（handoff §6.5 第 7 项）

## 证据

| 项 | 证据 |
|----|------|
| 实现 | `crates/ui/src/markdown/streaming.rs`（`Pacer` / `partition` / `tail` / `settle_progress` / `star_breath` / `star_element` / `decorate`）、`crates/ui/src/accessibility.rs`、`assets/icons/streaming-star.svg`；`code_block::renderer(.., streaming)`；commit `89d90d8` |
| vendored 补丁 | `MarkdownDecorations::veil` / `overlay`；patch 同步通过；`VENDOR.md` 已登记 |
| 单测（15） | 节奏：v1 `useSmoothTextRenderer.test.tsx` 5 例逐一对应 + 速率（50 字/秒、积压 16 字上限、卡顿后重新排期）；尾段：v1 `StreamingMarkdown.test.tsx` `:151` `:199` `:214` `:230` `:245` 逐一对应 + 无新字不落定 + 守卫不计；时序：延迟 32ms/级、260ms、星标 1.1s 周期端点 |
| 减弱动效 | `accessibility` 单测：与 `defaults read com.apple.universalaccess reduceMotion` 一致 |
| T08 | `streaming_preview -- --selftest`：无效果 601.5px = 全部落定起点 + 行内星标 601.5px；反证：单独星标 613.5px；落定查询 528 次、星标构建 3 次（证明效果确实作用） |
| T08 拦截 | 关掉 vendored overlay 布局 → 星标构建 0 次 → FAIL，退出码 1；已恢复 |
| T09 | 模拟网络流式（固定种子）全程：最多落定 9 字、出现过行内与单独星标；结束后源文本 707 字节 == 规范化样例，星标 `Hidden`、无落定 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 范围 | 含节奏器（v1 `useSmoothTextRenderer`） | 渐显的「新字符」由节奏器批次定义；Phase 05 无对应 spec |
| 实现方式 | 只改文字颜色（逐字运行）+ 星标为叠加层 | 不改排版（T08）；GPUI 文本运行只有颜色，没有不透明度 → 以颜色 alpha 表达 v1 的 `opacity` |
| 光晕 | 字符光晕（v1 `text-shadow`）**不做**；星标光晕以圆形 BoxShadow 近似 | GPUI 文本无阴影；星标为单色 SVG 蒙版，v1 的白蓝渐变填充与 drop-shadow 只能近似 → 目检 |
| 星标占位 | 叠加、不占行内宽度 | v1 星标为 12px 行内块，处于行尾时可能提前折行；叠加避免每批跳动，且星标只在最后一个字符后出现 |
| 未闭合围栏 | 不显示复制按钮；**仍高亮** | v1 用纯文本块；高亮不改变排版（S04-02 T03），闭合时不跳变 |
| 守卫字符 | 不计入 9 字 | 零宽，占位只会让可见的落定少一个；v1 会计入（差异不可见） |
| 减弱动效 | 读 macOS `NSWorkspace.accessibilityDisplayShouldReduceMotion`（每帧实时） | GPUI 无接口；`objc` 0.2 已在闭包中（gpui_macos），`Cargo.lock` 仅多一条依赖边；Windows 在 Phase 09 |
| 星标呼吸 | 每批重新开始（v1 以 phase-a/b 交替类名重启动画） | 与 v1 一致：流式快时星标停在收缩态，停顿时才呼吸 |

## 完成记录

- 日期：（待目检通过后填写）
- commit：`89d90d8`
- 设计文档处置：无
