# S04-05 半截标记修补（mend）

> 状态: `done`
> Phase: 04
> 依赖: S04-04
> 阻塞: —
> 退役设计文档: —

## 目标

流式显示层自动补全未闭合的 `**` / `*` / `_` / `~~` / 反引号 / `[link](`，结算时以正式解析替换。

## 输入

- Comet `mend.rs`（MIT，413 行）
- v1 `src/utils/markdownNormalizer.ts`（230 行）—— **读后更正**：它不是流式修补，而是两项与流式无关的规范化：
  ① 中文标点旁 `**` 的 CommonMark 强调边界失效 → 在加粗内容两端插零宽空格守卫（渲染时去掉）；
  ② AI 把纯文本 / 无语言代码块的结束围栏粘在正文末尾 → 拆开。**两项都是 v2 必做**

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S04-05-1 | 行为对齐 v1 | 以 `markdownNormalizer.ts` 已处理的情形为必做集，Comet 额外情形择优 |

## 验收标准

- [x] 修补单测覆盖 v1 normalizer 的全部情形
- [x] ~~半截标记流式中无抖动 —— 需用户目检~~ **范围更正并移交**：v1 没有半截标记修补（见决策记录），
  本项实际要比对的是「v2 流式显示与 v1 一致」，只能在真实消息行上看 → 移交 `S05-08`（消息行流式态）目检；
  v1 用例「未闭合的流式围栏在闭合前保持纯文本」（`StreamingMarkdown.test.tsx:245`）移交 `S04-06`

## 证据

| 项 | 证据 |
|----|------|
| 实现 | `crates/ui/src/markdown/normalize.rs`（逐函数对应 v1 `src/utils/markdownNormalizer.ts`@`v1-final`）；commit `c956292` |
| 单测 | `cargo test -p buddy-ui normalize` → **6 passed** |
| v1 用例对应 | v1 规范化相关用例全部在 `v1-final:src/components/chat/StreamingMarkdown.test.tsx`：`:27` 中文加粗 → `chinese_strong_next_to_punctuation`；`:44` 英文加粗 → `english_strong_still_works`；`:57` 代码内不规范化 → `code_is_untouched`；`:103` 粘连结束围栏 → `attached_plain_fence_is_repaired`（原文照搬，emoji 换为文字以满足硬约束 4） |
| 补充用例 | `language_fences_are_not_rewritten`（带语言代码块里的 ``` 字符串不拆）；`escaped_and_triple_stars_are_left_alone` |
| 反证 | 加粗：原文直接解析 → 输出含字面 `**`，测试断言失败；围栏：原文直接解析只得 1 个标题（第二个被代码块吞掉），测试内断言 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 不实现 Comet `mend.rs` 流式半截标记修补 | **不做** | 迁移以 v1 行为为准；v1 流式期间半截 `**` 会短暂显示为字面星号，闭合后变粗体。加修补会改变用户已习惯的观感且带来「先粗后撤回」风险。若 S05-08 目检认为需要，再单独开 spec |
| 接入方式 | 每批对**完整文本**规范化后 `Markdown::replace` | 规范化需要完整行与成对定界符；上游每次追加本就全量重解析（S04-04：5000 行 9 ms），`replace` 不增成本。v1 分稳定 / 不稳定两段分别规范化只是 React 的记忆化优化，段界在段落边界，结果等价 |
| 零宽空格守卫 | 显示 / 复制时去掉（`strip_guards`） | 与 v1 `StreamingMarkdown.tsx:282` 一致；复制带出零宽空格会污染用户粘贴内容 → `S04-09` 验收 |
| 按字节查找定界符 | 可行 | `*`/`` ` ``/`~` 为 ASCII，UTF-8 多字节序列不含 ASCII 字节，切片边界必落在字符边界 | |

## 完成记录

- 日期：2026-09-27
- commit：`c956292`
- 设计文档处置：无
