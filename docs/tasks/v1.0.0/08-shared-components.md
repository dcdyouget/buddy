# Task 08: Shared Components

## 目标

创建可复用的基础 UI 组件。

## 相关设计文档

- `docs/design/design-tokens.md` — Token 使用规范
- `docs/design/component-mapping.md` — 组件列表和 Props

## 验收标准

- [ ] `GlassPanel` 渲染毛玻璃容器，设置 `data-tauri-drag-region`
- [ ] `IconButton` 28×28 圆形按钮，hover 状态
- [ ] `FooterActions` 双按钮（取消 + 确定），颜色语义正确
- [ ] `StatusDot` 绿/黄/红三种颜色
- [ ] `KbdRow` 快捷键组合展示（macOS 符号 / Windows 符号）
- [ ] 所有组件只使用 design token，不硬编码颜色

## 开发工作

| ID | Task | Component | Details |
|----|------|-----------|---------|
| D32 | GlassPanel | `src/components/shared/GlassPanel.tsx` | `.surface-glass` + `data-tauri-drag-region` |
| D33 | IconButton | `src/components/shared/IconButton.tsx` | lucide icon, size prop, hover/active states |
| D34 | FooterActions | `src/components/shared/FooterActions.tsx` | onCancel, onConfirm, confirmLabel, cancelLabel |
| D35 | StatusDot | `src/components/shared/StatusDot.tsx` | kind: success/warning/error |
| D36 | KbdRow | `src/components/shared/KbdRow.tsx` | keys: string[], platform-aware modifier symbols |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T05 | Component render tests | Each shared component renders without crash, props work correctly |

使用 `vitest` + `@testing-library/react`：

1. `GlassPanel` 渲染 children + 有 `data-tauri-drag-region`
2. `IconButton` 点击触发 onClick
3. `FooterActions` 取消/确认按钮各自触发回调
4. `StatusDot` 三种 kind 渲染不同颜色
5. `KbdRow` macOS 下 ⌘ 符号正确，Windows 下 Ctrl 符号正确
