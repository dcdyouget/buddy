# S05-06 Composer 文本输入与 IME

> 状态: `blocked`
> Phase: 05
> 依赖: S00-05, S03-04
> 阻塞: 等待用户目检（`handoff.md` §6.5 第 10 项：中文输入法实测与输入区外观）
> 退役设计文档: —

## 目标

多行输入框：Enter 发送、Shift+Enter 换行、中文组字期间 Enter 只上屏候选；自动增高；外观与 v1 一致。

## 输入

- v1 `src/components/chat/InputDock.tsx`
- S00-05 `docs/evidence/s00-05/ime-input.rs`（官方示例的 4 个缺陷已修）与 research-log §14

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S05-06-1 | 键盘三态 | Enter / Shift+Enter 发送、Cmd/Ctrl+Enter 换行、组字中 Enter 不发送（**按 v1 代码更正**，见决策记录） |
| S05-06-2 | 自动增高 | 最小 / 最大高度取 v1 |
| S05-06-3 | 发送 / 停止 | 流式中按钮变停止，禁止发送 |

## 验收标准

- [x] 键盘三态测试 —— `chat_preview` T16（经真实键位绑定分发按键）
- [ ] 中文输入法实测 —— **需用户目检**（handoff §6.5 第 10 项）

## 证据

| 项 | 证据 |
|----|------|
| 实现 | `crates/ui/src/text_area.rs`（通用多行输入框）、`crates/ui/src/chat/composer.rs`（v1 InputDock）；commit `c2299d6` |
| T16 | 组字中（标记文本 "ni"）Enter 不发送；上屏「你」后 Enter 发送 "你" 并清空；Cmd+Enter 插入换行不发送；Shift+Enter 发送（v1 同）。拦截：去掉组字守卫 → 组字中 Enter 误发送，FAIL |
| T17（S00-05 缺陷 4 回归，交接自 S00-05） | 「你好」后组字 "dian"、输入法光标 1..1 → 选区 7..7（正确）；按整段内容做 UTF-16 换算的旧写法得 9..9 → FAIL（该值在长度内，防御性 clamp 掩盖不了；首版用例用 "d" 时被 clamp 掩盖、拦截未失败 → 已加强）；组字中 Cmd+C 不崩溃 |
| 单测 | `can_send_matches_v1`（v1 `canSend` 全部分支）、`utf16_round_trip`、`boundaries_respect_graphemes_and_words` |
| S00-05 四个缺陷 | ① 不断言，placeholder 时按位置换算；② 组字选区相对插入点；③ `shape_text` 多行 + 软换行；④ 见 T17（根因已定位） |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 键盘语义 | Enter、Shift+Enter 发送；Cmd/Ctrl+Enter 换行 | v1 `InputDock.tsx` `handleKeyDown` 只把 Cmd/Ctrl+Enter 当换行（其余 Enter 一律 `preventDefault` 发送）；任务文档与 S00-05 写的「Shift+Enter 换行」与 v1 代码不符，以代码为准 |
| 输入框实现 | 自写 `TextArea` | 本 rev 无可复用的多行输入（zed `editor` 过重）；功能对齐 `<textarea>`：软换行、自动增高（v1 最高 120px）、视觉行移动、按词编辑、鼠标选择、剪贴板（粘贴保留换行）、撤销 / 重做、光标闪烁 |
| 缺陷 4 根因 | `new_selected_range_utf16` 相对 `new_text`，应在 `new_text` 内换算 | 官方示例按整段内容换算，插入点前有多字节字符时偏移错误；S00-05 只做了 clamp、根因未查清，本 spec 定位并以 T17 锁定 |
| 外描边色 | `--border-default` | v1 用 `--glass-outline`，该令牌在 S00-04 被用户否决（白色描边观感）、未迁移 |
| 悬停 / 聚焦光环 | 品牌色内描边（2px）近似 | v1 为带遮罩的动画渐变描边 + 跟随指针的径向光晕，GPUI 无对应能力 → 目检 |
| 待其他 spec | 图片按钮与附件 → S05-07；模型按钮 → S05-15；设置 → S05-18 | 按 spec 划分；按钮已在位 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
