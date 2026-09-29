# S05-13 提问与审批卡片

> 状态: `blocked`
> Phase: 05
> 依赖: S05-08, S02-07
> 阻塞: 等待用户目检（`handoff.md` §6.5 第 19 项）
> 退役设计文档: `docs/design/pages-and-states.md` 的 ChatPage 审批 / 提问部分

## 目标

ask_user 提问卡（选项 / 补充输入 / 自定义回答）与工具审批浮层，回答经 engine 回传。

## 输入

- v1 `AskUserCard.tsx`、`QuestionPrompt.tsx`、`ApprovalModal.tsx`、`utils/askUserDisplay.ts`（及其测试）、`ToolSection.tsx` 对 ask_user 的分支、`chatStore.answerPendingQuestion`、`useStreaming.resolveApproval`、`.tool-question-*` / `.question-prompt*` / `.tool-interaction-*` 样式
- engine `ChatEngine::answer_tool_question(id, selected, inputs, custom)` / `approve_tool_call(id, approved, approve_all)`

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S05-13-1 | 请求-响应配对 | 事件里的调用 id 就是配对键：提问卡以工具调用 id 建立（`Transcript::ask_cards`），审批以 `Approval::id` 回传；engine 找不到对应槽位（已取消 / 超时）时提问卡报「提交失败，请重试」，审批只记日志（同 v1） |
| S05-13-2 | 提问卡 | `chat/ask_card.rs`：v1 `askUserDisplay` 的参数解析（问题只取最后一句、兼容 snake_case / camelCase）、`AskUserCard` 实体（选项单选替换 / 多选切换、选中带对勾、「需补充」+ 补充信息框、自定义回答框 Cmd / Ctrl+Enter 提交、确认 / 跳过、提交失败提示、「用户回应」）；展开的 ask_user 工具行显示它而不是调用参数 / 执行结果 |
| S05-13-3 | 审批浮层 | `chat/approval_panel.rs`：浮在输入区上方居中的面板（盾牌图标、工具名、参数预览、拒绝 / 本次都允许 / 允许、「Esc · 拒绝」）；Esc 拒绝由对话页处理（有审批时页面持有焦点） |
| S05-13-4 | 输入框 Enter 语义 | `TextArea::set_enter_mode`：`Chat`（默认）/ `NewlineOnEnter`（自定义回答框：Enter 换行、Cmd / Ctrl+Enter 提交）/ `SingleLine`（补充信息框，Enter 不做事） |
| S05-13-5 | 图标 | 新增 `CornerDownRight`、`Shield`、`ShieldCheck`、`ShieldX` |

## 验收标准

- [x] 回答 / 审批往返测试（mock 模型 + **真实 engine 工具循环**，T29）
- [x] 参数解析、提交条件、回答载荷（单测，含 v1 `askUserDisplay.test.ts` 的两个用例）
- [ ] 交互 —— **需用户目检**（handoff §6.5 第 19 项）

## 证据

| 项 | 证据 |
|----|------|
| 单测 | `cargo test -p buddy-ui ask_card`（8 个）：最后一句问题提取与标记清理、snake_case / camelCase 选项归一化（过滤空标签与非对象）、标题优先级、单选替换 / 多选切换、提交条件（含要求补充、仅空白、自定义单独可提交、提交中禁用）、回答载荷；`approval_panel`（1 个）：决定 → `(approved, approve_all)` |
| T29 | `cargo run -p buddy-app --example app_preview -- --selftest`：mock 模型让 engine 调用 `ask_user` / `create_file`。提问：出现待答问题与卡片；选「方案 B」出现补充框；未填补充时确认无效；填「更快」后确认 → engine 工具结果为 `User selected: 方案 B` + `User input: 更快`；再问一次点跳过 → 结果不含选项；再问一次只写自定义回答 → 结果含该回答。审批：写入类工具出现审批（工具名 `create_file`、原因含路径）；Esc → 拒绝，工具结果「用户拒绝执行」、文件未创建；再次审批，真实点击「允许」→ 文件内容为 `hello`；一次调用两个写入，真实点击「本次都允许」→ 只出现一次审批，两个文件都已创建 |
| 渲染核对 | 临时打开 gpui `test-support` 用 `Window::render_to_image` 看渲染结果（会改 `Cargo.lock`，未提交，已还原）：提问卡与审批浮层的层次、颜色、对勾、标记、按钮与 v1 设计一致 |
| 拦截 | 见下 |

### 拦截验证

逐项故意改坏，确认 T29 FAIL，再还原：

| # | 改坏 | 结果 |
|---|------|------|
| 1 | 回答载荷丢掉补充输入 | FAIL（「engine 收到选项与补充输入」） |
| 2 | 不检查要求补充的必填项 | FAIL（「要求补充却没填 → 确认无效」等两项） |
| 3 | 回答后不清除待答问题 | **仍 PASS** —— engine 随后发出的工具结果同样会清除待答状态，`clear_question` 是与 v1 一致的冗余保险，无独立自动化证据 |
| 4 | Esc 不拒绝 | FAIL（Esc 拒绝、回合结束等三项） |
| 5 | 审批不回传 engine | FAIL（八项：engine 一直等待，回合无法结束） |
| 6 | 「本次都允许」不带 all 标志 | FAIL（「后续写入不再询问」「两个文件都已创建」） |
| 7 | 不渲染审批浮层 | FAIL（点「允许」无法清除审批等五项） |
| 8 | 跳过却带上选项 | FAIL（「跳过的结果不含选项」） |

**测试事故（已处理）**：首版 mock 把 engine 附加在用户文本后的上下文文字也当成路径，让真实的 `create_file` 在仓库根目录写出了 10 个 5 字节的 `hello` 文件（`Current`、`and`、`date`……）。已确认内容后删除；mock 现在只取紧跟触发词、以 `/` 开头的绝对路径。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 提问卡状态放在卡片实体里，由列表按工具调用 id 持有 | `Transcript::ask_cards` | 卡片有输入框（实体）和选择状态，列表行是每帧重建的元素，放不下；卡片状态变化（选择、输入换行）通过观察触发所在行重测高度 |
| 回答同步返回 | `answer_tool_question` 是同步方法，成功即清除待答问题 | v1 是 `await` 后清除；这里没有异步间隙，`提交中` 状态实际不可见，但保留判定（提交中禁用）以与 v1 逻辑一致 |
| 审批决定后无条件关闭浮层 | 与 v1 `dismiss` 一致 | engine 找不到槽位（流式已被取消）时，浮层也应消失；失败只记日志 |
| Esc 的作用域 | 对话页：有审批时页面取得焦点，按键在页面上处理 | v1 是 `window` 级 keydown。GPUI 按焦点路径分发，浮层出现时输入区处于「生成中」形态无文本框，抢焦点无副作用 |
| 补充信息框用单行 `TextArea` | `EnterMode::SingleLine` | v1 是 `<input>`；不新造单行输入组件 |
| 自定义回答框高度 | 2 行起、最高 6 行后框内滚动 | v1 `<textarea rows=2 resize:vertical>`，用户可拖拽；GPUI 无拖拽缩放，取自动增高近似 |
| 审批浮层的纵向位置 | 底边距窗口底 64px（`space-12 + space-4`）、顶边至少留 48px | 同 v1 `.tool-interaction-layer`；v1 的 `100vh` 上限在此为「父容器高度」 |
| 多个待答问题 | 不支持并发（`ChatState::question` 是单值） | 同 v1 `pendingQuestion`；engine 的工具循环本就顺序执行 |
| 审批 `reason` 展示 | 原样显示 engine 给的文本（`{工具} 调用 {参数 JSON}`） | 同 v1 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：待 `done` 时执行 —— 删除 `docs/design/pages-and-states.md` 的 ChatPage 审批 / 提问部分（why 已写入 `ask_card.rs` / `approval_panel.rs` 模块文档），并登记
