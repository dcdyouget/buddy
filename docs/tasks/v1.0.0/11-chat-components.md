# Task 11: Chat Components

## 目标

创建对话相关的可复用组件：消息气泡、代码块、输入框、模型选择器、模型下拉。

## 相关设计文档

- `docs/design/pages-and-states.md` — conversation / streaming / model-dropdown 页面细节
- `docs/design/component-mapping.md` — 组件 Props 定义
- `docs/design/design-tokens.md` — 样式 token

## 验收标准

### MessageBubble

- [ ] User 消息：右对齐，主色浅底 + 主色描边，圆角 `8px 8px 8px 4px`
- [ ] AI 消息：左对齐，纯文本或 Markdown 渲染，无气泡背景
- [ ] 支持嵌入 `CodeBlock` 子组件

### CodeBlock

- [ ] 深色背景 (`neutral-1000`)，圆角 `8px`
- [ ] 语言标注 + 一键复制按钮（右上角）
- [ ] 复制后显示 "Copied!" 反馈，2s 后恢复
- [ ] 语法高亮（使用 prism-react-renderer）
- [ ] 横向滚动支持长代码行

### InputDock

- [ ] [B logo] [textarea auto-resize] [× clear] [模型 pill] [send/stop]
- [ ] textarea 默认单行，Shift+Enter 换行，Enter 发送
- [ ] × clear 仅在 textarea 有内容时显示
- [ ] Streaming 状态下：textarea 隐藏，显示状态文字 + Stop 按钮

### ModelPicker

- [ ] 显示当前模型名 + sparkles icon + chevron
- [ ] 点击 → 打开 ModelDropdown

### ModelDropdown

- [ ] 320px 宽浮层，右对齐
- [ ] 标题行："切换默认模型" + "共 N 个已启用"
- [ ] 每行：Provider 首字母图标 + 模型名 + 描述 + 延迟
- [ ] 当前默认行高亮（浅靛底 + "默认"徽章）
- [ ] 点击任意行 → 设置默认 → 关闭
- [ ] Esc → 关闭

## 开发工作

| ID | Task | Component | Details |
|----|------|-----------|---------|
| D41 | MessageBubble | `src/components/chat/MessageBubble.tsx` | User/AI styles |
| D42 | CodeBlock | `src/components/chat/CodeBlock.tsx` | Highlight + copy |
| D43 | InputDock | `src/components/chat/InputDock.tsx` | Logo + textarea + actions |
| D44 | ModelPicker | `src/components/chat/ModelPicker.tsx` | Trigger pill |
| D45 | ModelDropdown | `src/components/chat/ModelDropdown.tsx` | 320px popover |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T05 | Render tests | Each component renders; user/AI role styles differ |
| T11 | E2E: model switch | Dropdown open → select → pill updates → verify config |
