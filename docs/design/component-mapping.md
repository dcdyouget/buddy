# Component Mapping — 设置与窗口外壳

> 聊天 / 空态 / 无 Key 页的组件映射已由 Phase 05 实现，相关段落已退役；理由与行为以 `crates/ui/src/chat/` 模块文档和对应 spec 决策记录为准。
> 设置覆盖层、分组与共用控件由 S06-01 替代；新增 Provider 的预设与完整保存流程由 S06-02 替代；模型列表、上下文及能力编辑由 S06-03 替代，理由见对应 spec，代码在 `crates/ui/src/settings/`；本文件仅保留未实现的设置子组件（Phase 06）与窗口外壳（Phase 07）。v1 参考代码仍保留，以 `src/components/` 当前实现为准。

## Settings Composition

```text
SettingsView 子项（骨架见代码）
├── ThemeSetting
└── HotkeySetting
    ├── KbdRow
    └── HotkeyRecorder
```

## Remaining Component Roles

| Component | Role |
|---|---|
| `GlassPanel` | 窗口面板容器；实色与无装饰约束见 `AGENTS.md` |
| `ThemeSetting` | light / dark 选择 |
| `HotkeySetting` / `HotkeyRecorder` / `KbdRow` | 快捷键展示与录制 |

## Remaining Files

```text
src/components/
├── settings/
│   ├── HotkeyRecorder.tsx
│   ├── HotkeySetting.tsx
│   └── ThemeSetting.tsx
└── shared/
    ├── GlassPanel.tsx
    └── KbdRow.tsx
```
