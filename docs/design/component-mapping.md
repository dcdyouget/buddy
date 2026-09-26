# Component Mapping

> 以 `src/components/` 当前实现为准。

## Page Composition

```text
App
├── EmptyPage
│   ├── GlassPanel
│   ├── InputDock
│   └── ModelDropdown
├── NoApiKeyPage
│   └── GlassPanel
├── ChatPage
│   ├── GlassPanel
│   ├── IconButton(settings)
│   ├── MessageBubble[]
│   │   ├── StreamingMarkdown
│   │   ├── CodeBlock
│   │   ├── ThinkSection
│   │   ├── ToolSection
│   │   │   └── WebSearchSection
│   │   └── UserResponseInput
│   ├── InputDock
│   ├── ModelDropdown
│   ├── ApprovalModal
│   └── QuestionModal
└── SlideInPanel(SettingsPage)
    ├── GlassPanel
    ├── ThemeSetting
    ├── HotkeySetting
    │   ├── KbdRow
    │   └── HotkeyRecorder
    ├── ModelList
    │   └── ModelRow[]
    ├── FooterActions
    └── SlideInPanel(AddProviderPanel)
        ├── ProviderCard[]
        ├── StatusDot
        └── FooterActions
```

## Component Roles

| Component | Role |
|---|---|
| `GlassPanel` | 毛玻璃容器 |
| `IconButton` | lucide 图标按钮 |
| `SlideInPanel` | 通用右侧覆盖层与显隐动画 |
| `FooterActions` | 取消/确认双按钮 |
| `ApprovalModal` | write tool 单次允许、整轮允许或拒绝 |
| `QuestionModal` | `ask_user` 单选/多选/补充输入 |
| `InputDock` | 输入、模型入口、发送/停止、流式状态 |
| `MessageBubble` | user/assistant 消息编排与嵌套回应 |
| `StreamingMarkdown` | GFM Markdown 流式渲染 |
| `CodeBlock` | Prism 代码高亮与复制 |
| `ThinkSection` | thinking 内容折叠展示 |
| `ToolSection` | tool 调用状态、参数和结果；`websearch` 使用紧凑搜索状态块 |
| `UserResponseInput` | 对 assistant 问句的嵌套回应输入 |
| `ModelDropdown` | 已启用模型切换 |
| `ClearButton` | 清空输入 |
| `ThemeSetting` | light/dark 选择 |
| `HotkeySetting` / `HotkeyRecorder` | 快捷键展示与录制 |
| `ModelList` / `ModelRow` | 模型默认项、上下文窗口等配置 |
| `AddProviderPanel` / `ProviderCard` | Provider 选择、验证、模型导入 |

## Files

```text
src/components/
├── chat/
│   ├── ClearButton.tsx
│   ├── CodeBlock.tsx
│   ├── InputDock.tsx
│   ├── MessageBubble.tsx
│   ├── ModelDropdown.tsx
│   ├── StreamingMarkdown.tsx
│   ├── ThinkSection.tsx
│   ├── ToolSection.tsx
│   ├── WebSearchSection.tsx
│   └── UserResponseInput.tsx
├── settings/
│   ├── AddProviderPanel.tsx
│   ├── HotkeyRecorder.tsx
│   ├── HotkeySetting.tsx
│   ├── ModelList.tsx
│   ├── ModelRow.tsx
│   ├── ProviderCard.tsx
│   └── ThemeSetting.tsx
└── shared/
    ├── ApprovalModal.tsx
    ├── FooterActions.tsx
    ├── GlassPanel.tsx
    ├── IconButton.tsx
    ├── KbdRow.tsx
    ├── QuestionModal.tsx
    ├── SlideInPanel.tsx
    └── StatusDot.tsx
```

## UI Constraints Reflected in Code

窗口无系统 chrome；图标使用 lucide-react；主题色和大部分视觉值通过 CSS variables。当前 compact → content 路由切换会通过 `windowResize.ts` 设置一次预设尺寸。
