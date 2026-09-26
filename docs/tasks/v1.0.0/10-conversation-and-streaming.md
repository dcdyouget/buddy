# Task 10: Conversation & Streaming Pages

## 目标

实现多轮对话主窗口和流式生成状态的完整页面。

## 相关设计文档

- `docs/design/pages-and-states.md` — page-conversation 和 page-streaming 规格
- `docs/design/sse-and-api.md` — 流式事件名称和数据结构
- `docs/design/ipc-contract.md` — listen 事件规范
- `docs/design/component-mapping.md` — 组件映射

## 验收标准

### ConversationPage

- [ ] 720×620 frameless 窗口
- [ ] 顶部仅 ⚙ 设置按钮（无 traffic lights, 无 drag handle）
- [ ] 消息列表可滚动，用户气泡靠右（主色浅底）、AI 气泡靠左
- [ ] 输入栏：[B logo] [textarea] [× clear] [模型 pill] [↑ send]
- [ ] 发送后乐观切换 streaming；失败回滚
- [ ] 模型 pill 点击 → 弹出 ModelDropdown

### StreamingPage

- [ ] 与 conversation 同布局
- [ ] AI 消息末尾闪烁光标（CSS `@keyframes buddy-blink`）
- [ ] 发送按钮变为红色 Stop 按钮
- [ ] 输入栏状态文字：`{model_name} · Generating… {N} tokens`
- [ ] Stop 按钮 → 中断流，保留已收到内容，回到 conversation
- [ ] 流完成 → 自动回到 conversation

## 开发工作

| ID | Task | Component | Details |
|----|------|-----------|---------|
| D39 | ConversationPage | `src/pages/ConversationPage.tsx` | Main chat window layout |
| D40 | StreamingPage | `src/pages/StreamingPage.tsx` | Conversation + streaming indicators |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T05 | Render tests | Both pages render without crash |
| T07 | Streaming event simulation | Mock `listen` events, verify chatStore token append |
| T10 | E2E: conversation | Input → send → streaming → stop → continue |
| T14 | E2E: window | Esc closes, blur hides, streaming continues |

### T07 细节

```
1. Mock listen('stream-token') → emit "你" → chatStore.messages last item ends with "你"
2. Mock listen('stream-token') → emit "好" → chatStore.messages last item ends with "你好"
3. Mock listen('stream-done') → chatStore finalize → uiStore page = 'conversation'
4. Mock listen('stream-error', '401') → uiStore error set, navigate to noapikey
```
