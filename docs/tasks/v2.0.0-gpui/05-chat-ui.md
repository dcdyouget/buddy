# Phase 05: 聊天界面

## 目标

把聊天页从 React 移植到 GPUI，并顺手解决现有实现的两个已知问题：**无虚拟化**、**流式导致整页重渲染**。

## 相关文档

- `docs/design/pages-and-states.md` — 7 个页面 + 状态机
- `docs/design/component-mapping.md` — 设计 → 组件映射
- `docs/evidence/v1-baseline/` — v1 实机基线（由 S01-06 采集）
- `docs/tasks/v2.0.0-gpui/research-log.md` §3.2 §5 — Comet 参考实现、现有前端问题
- `docs/tasks/v2.0.0-gpui/03-theme.md` — 主题令牌
- `docs/tasks/v2.0.0-gpui/04-markdown.md` — markdown 渲染

## 验收标准

- [ ] 1000 条历史消息下滚动流畅，且有真虚拟化（非手动分页）
- [ ] 流式追加只重渲染受影响的块，不触发整页重绘
- [ ] Enter 发送 / Shift+Enter 换行；**中文组字期间 Enter 不发送**
- [ ] 点击外部关闭窗口时**不打断流式**（硬约束 7）
- [ ] 窗口尺寸在页面/状态切换时不变（硬约束 6）
- [ ] 所有图标用 svg，无 emoji（硬约束 4）
- [ ] 全部 UI 文案为中文（硬约束 10）

## 组件移植清单

| 现有文件 | 行数 | 目标 |
|---------|------|------|
| `src/pages/ChatPage.tsx` | 530 | 拆为 `ChatView` + `Transcript` |
| `src/pages/EmptyPage.tsx` | 186 | `EmptyView` |
| `src/pages/NoApiKeyPage.tsx` | — | `NoApiKeyView` |
| `src/components/chat/InputDock.tsx` | 529 | `Composer` |
| `src/components/chat/MessageBubble.tsx` | 319 | `MessageRow` |
| `src/components/chat/LiveMessageBubble.tsx` | — | 并入 `MessageRow` 的流式态 |
| `src/components/chat/ToolSection.tsx` | 338 | `ToolGroup` |
| `src/components/chat/DeferredToolSection.tsx` | — | `DeferredToolGroup` |
| `src/components/chat/WebSearchSection.tsx` | 264 | `WebSearchGroup` |
| `src/components/chat/GenerateImageSection.tsx` | 261 | `ImageGenGroup` |
| `src/components/chat/ThinkSection.tsx` | 197 | `ThinkBlock` |
| `src/components/chat/AskUserCard.tsx` | 263 | `AskUserCard` |
| `src/components/chat/QuestionPrompt.tsx` | — | `QuestionPrompt` |
| `src/components/chat/MessageActions.tsx` | 160 | `MessageActions` |
| `src/components/chat/AttachmentImage.tsx` | — | `AttachmentThumb` |
| `src/components/chat/ModelDropdown.tsx` | 190 | `ModelPicker` |
| `src/components/chat/ClearButton.tsx` | — | 并入工具栏 |
| `src/components/shared/ApprovalModal.tsx` | 242 | `ApprovalModal` |
| `src/components/shared/GlassPanel.tsx` | — | `glass_panel()` 助手 |
| `src/components/shared/IconButton.tsx` | — | `icon_button()` 助手 |
| `src/components/shared/KbdRow.tsx` | — | `kbd_row()` 助手 |
| `src/components/shared/StatusDot.tsx` | — | `status_dot()` |
| `src/components/shared/FooterActions.tsx` | — | 并入对应视图 |

需删除的 hook（被 GPUI 原生能力替代）：

| 现有 | 行数 | 替代 |
|------|------|------|
| `src/hooks/useSmoothWheelScroll.ts` | 183 | GPUI 原生滚动 |
| `src/hooks/useSmoothTextRenderer.ts` | 200 | Phase 04 的 veil / 增量解析 |
| `src/utils/bottomFollow.ts` | — | `ListState(Bottom)` + 自有弹簧 |
| `src/stores/chatStore.ts` | 1447 | GPUI `Entity` |

## 开发工作

### 5.1 Transcript（核心）

| ID | Task | Details |
|----|------|---------|
| D01 | `ListState` 骨架 | `ListState::new(n, ListAlignment::Bottom, overdraw)` |
| D02 | 块粒度行模型 | 一行 = 一个 markdown 块 / 一个工具组 / 一张卡片，**不是一条消息** |
| D03 | 稳定行 id | 形如 `msgId#blockId`；乐观回显与持久化后必须同 id（避免闪烁） |
| D04 | 活跃回合不拆分 | 流式中的回合保持单行，持久化后再拆分（对齐 Comet 做法） |
| D05 | 行高记忆 | 键 = (行 id, 内容长度, 宽度)；流式 token 只重测一行 |
| D06 | 视口上方高度变化吸收 | 上方行高变化时保持滚动锚点，不跳动 |
| D07 | 跟尾弹簧 | 贴底 + 前馈追踪流式增长；用户上滑立即打断；回到 70px 带内重新吸附 |
| D08 | 回到底部按钮 | 复现现有「回到底部」按钮及其出现阈值 |
| D09 | 历史加载 | 替换现有 `hasMoreHistory` / `loadOlderMessages` 手动分页；改为虚拟化 + 按需加载 |
| D10 | 滚动条 | 隐藏式滚动条，与玻璃风格一致 |

### 5.2 Composer（输入区）

| ID | Task | Details |
|----|------|---------|
| D11 | 多行文本输入 | 以 zed `examples/input.rs` 为起点，接入 Phase 00 S05 的 IME 验证结果 |
| D12 | 自动增高 | 复现现有 auto-grow（有最小/最大高度） |
| D13 | 键盘语义 | Enter 发送、Shift+Enter 换行；IME 组字期间 Enter 只提交候选词 |
| D14 | 草稿保持 | 页面/状态切换后草稿不丢（现有 `draftInput` 行为） |
| D15 | 图片附件 | 粘贴、拖拽、选择；草稿态 `draftImages` 行为保留 |
| D16 | 附件缩略图与删除 | 复现 `AttachmentImage.tsx` 与现有删除交互 |
| D17 | 发送 / 停止 形态切换 | 复现现有 Send → Stop 的按钮切换 |
| D18 | 流式中禁止发送 | 复现现有约束（`isStreaming` 时禁发 + 禁切模型） |

### 5.3 消息与工具渲染

| ID | Task | Details |
|----|------|---------|
| D19 | `MessageRow` | 用户/助手两种形态；助手走 Phase 04 markdown |
| D20 | 流式态 | 光标、缓冲、token 计数 |
| D21 | `ThinkBlock` | 思考块折叠/展开（复现 `ThinkSection.tsx`） |
| D22 | `ToolGroup` | 工具调用分组展示，含状态（运行中/成功/失败） |
| D23 | `DeferredToolGroup` | 延迟工具区 |
| D24 | `WebSearchGroup` | 搜索来源卡片、favicon、点击打开 |
| D25 | `ImageGenGroup` | 生成中占位、进度、结果图、保存 |
| D26 | 错误态 | 复现 `error` 展示与重试 |
| D27 | `MessageActions` | 复制、重试、编辑等，hover 显现 |

### 5.4 提问与审批

| ID | Task | Details |
|----|------|---------|
| D28 | `AskUserCard` | 复现选项/输入/多题分页交互 |
| D29 | `QuestionPrompt` | 内联提问提示 |
| D30 | `ApprovalModal` | 工具执行审批弹窗；注意这是窗内浮层，不是新窗口 |
| D31 | 请求-响应配对 | 与 Phase 02 D14 / D15 的 engine 侧打通；窗口关闭后重开的恢复语义 |

### 5.5 模型与配置交互

| ID | Task | Details |
|----|------|---------|
| D32 | `ModelPicker` | 复现 `ModelDropdown.tsx` + 现有原生菜单 `openNativeModelMenu` 的交互 |
| D33 | 模型列表数据源 | 接 Phase 02 的 provider 模型列表 |

### 5.6 状态管理

| ID | Task | Details |
|----|------|---------|
| D34 | 状态拆分 | 把 `chatStore.ts`（1447 行）拆为多个 `Entity`，避免单一大状态 |
| D35 | 细粒度通知 | 流式更新只 `notify` 受影响的实体/行，**严禁全量重绘** |
| D36 | 页面状态机 | 按 `docs/design/pages-and-states.md` 实现 chat / empty / no-api-key / settings 的切换 |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T01 | 行高记忆测试 | 断言流式 append 只触发一行重测 |
| T02 | 跟尾行为测试 | 贴底 / 上滑打断 / 70px 重吸附的边界 |
| T03 | 键盘语义测试 | Enter / Shift+Enter / IME 组字期间 Enter 的三态 |
| T04 | 草稿保持测试 | 切页再切回，草稿与附件完整 |
| T05 | 大列表性能 | 1000 / 5000 条消息的滚动帧率与内存，与旧 React 实现对比 |
| T06 | 消息往返测试 | 发送 → 流式 → 持久化 → 重开，内容与行 id 一致 |
| T07 | 窗口交互测试 | 流式中关闭窗口，断言流式未被中断且重开后内容完整（硬约束 7） |
| T08 | 视觉回归 | 对照 v1 基线（`docs/evidence/v1-baseline/`，由 S01-06 采集） |

## 备注

D35（细粒度通知）是解决现有「流式整页重渲染」问题的关键，应作为验收重点。
现有 `ChatPage.tsx` 的注释已明确记录该问题，迁移中必须验证已消除。
