# Buddy — Agent Conventions

> These rules apply to **ALL agents** working on this project. Violations will be rejected in code review.

---

## 1. General Principles

1. **Read `AGENTS.md` first** — it's the **项目权威入口** and document index（`CLAUDE.md` 只是指向它的薄指针，见规则 §1.7）
2. **不修改已退役/历史文档** — `docs/tasks/v1.0.0/` 是历史记录；设计文档实现完成后会被退役（见 `docs/specs/RULES.md` §7）
3. **Follow the docs** — 待实现内容由 `docs/specs/README.md`（spec 注册表）定义；外观由 `docs/design/design-tokens.md` 与 `docs/design/pages-and-states.md` 定义；Rust 架构由 `docs/design/rust-architecture.md` 定义
4. **YAGNI** — 只实现 spec 列出的内容，不做「以后可能用得上」的东西
5. **One file, one purpose** — max ~300 lines per file; split if larger
6. **Progressive disclosure** — keep files focused and scannable
7. **单一真相源** — 同一信息只在一处维护，其他地方用指针（见 `docs/specs/RULES.md` §11.2）
8. **路径引用必须验证存在**（见 `docs/specs/RULES.md` §11.1）

---

## 2. TypeScript Conventions

### Naming

| Thing | Convention | Example |
|-------|-----------|---------|
| Components | `PascalCase` | `MessageBubble.tsx` |
| Hooks | `camelCase`, `use` prefix | `useStreaming.ts` |
| Stores | `camelCase`, `Store` suffix | `configStore.ts` |
| Types/Interfaces | `PascalCase` | `AppConfig`, `ProviderConfig` |
| Files | Match default export | `GlassPanel.tsx` |
| Props types | `{Component}Props` | `CodeBlockProps` |

### Component Structure

```tsx
// 1. Imports (React → third-party → local)
import { useState } from 'react';
import { motion } from 'framer-motion';
import { useConfigStore } from '@/stores/configStore';
import { IconButton } from '@/components/shared/IconButton';

// 2. Types
interface MessageBubbleProps {
  message: Message;
  isStreaming?: boolean;
}

// 3. Component
export function MessageBubble({ message, isStreaming = false }: MessageBubbleProps) {
  // hooks
  // derived state
  // handlers
  // render
  return (/* JSX */);
}
```

### Styles

- **ALWAYS use CSS variables** — never hardcode colors, spacing, shadows, or radii
- Tailwind utility classes for layout (`flex`, `gap`, `p-4`, etc.)
- Custom styles via `style={{ ... }}` with CSS vars for brand-specific values
- Dark mode: the CSS var switching handles it; you only write one style

```tsx
// ✅ CORRECT
<div style={{ color: 'var(--text-primary)', borderRadius: 'var(--radius-md)' }}>

// ❌ WRONG
<div style={{ color: '#1D1D1F', borderRadius: '8px' }}>
```

### State Management (Zustand)

```tsx
// Store pattern: minimal, no boilerplate
import { create } from 'zustand';

interface ConfigState {
  config: AppConfig;
  setConfig: (config: AppConfig) => void;
  updateTheme: (theme: 'light' | 'dark') => void;
}

export const useConfigStore = create<ConfigState>((set) => ({
  config: defaultConfig,
  setConfig: (config) => set({ config }),
  updateTheme: (theme) => set((state) => ({
    config: { ...state.config, theme }
  })),
}));
```

### IPC Calls

```tsx
// All invoke calls go through stores, never in components directly
import { invoke } from '@tauri-apps/api/core';

// In a store action:
const config = await invoke<AppConfig>('get_config');
await invoke('save_config', { config: newConfig });
```

---

## 3. Rust Conventions

### Naming

| Thing | Convention | Example |
|-------|-----------|---------|
| Modules | `snake_case` | `commands.rs`, `api.rs` |
| Structs | `PascalCase` | `AppConfig`, `ModelInfo` |
| Enums | `PascalCase` | `Theme`, `MessageRole` |
| Functions | `snake_case` | `send_message`, `load_messages` |
| Constants | `SCREAMING_SNAKE_CASE` | `CHUNK_SIZE` |

### Error Handling

- Commands return `Result<T, String>` — the String is a user-facing Chinese error message
- Internal errors use a custom `ApiError` enum (in `api.rs`)
- Never `unwrap()` in command handlers — use `map_err(|e| format!("..."))` to convert to String
- Log errors with `eprintln!` for debugging

```rust
// ✅ CORRECT
#[tauri::command]
async fn get_config(app: tauri::AppHandle) -> Result<AppConfig, String> {
    storage::get_config(&app).map_err(|e| format!("读取配置失败: {}", e))
}

// ❌ WRONG
#[tauri::command]
async fn get_config() -> Result<AppConfig, String> {
    let data = std::fs::read_to_string("config.json").unwrap(); // panics!
}
```

### Module Organization

- One file per concern
- `lib.rs` — setup only (plugins, command registration, tray, hotkey), no business logic
- `commands.rs` — IPC handlers, thin wrappers that delegate to other modules
- `api.rs` — HTTP client, SSE parsing, no Tauri dependency
- `storage.rs` — file I/O, needs `AppHandle` for data dir path
- `models.rs` — pure data structs, no logic
- `hotkey.rs` — global shortcut management

---

## 4. Component Patterns

### File Organization

```
src/components/
├── chat/                    # Conversation-related
│   ├── MessageBubble.tsx
│   ├── CodeBlock.tsx
│   ├── InputDock.tsx
│   ├── ModelPicker.tsx
│   └── ModelDropdown.tsx
├── settings/                # Settings-related
│   ├── ModelRow.tsx
│   ├── HotkeyRecorder.tsx
│   └── ProviderCard.tsx
└── shared/                  # Reusable across pages
    ├── GlassPanel.tsx
    ├── IconButton.tsx
    ├── FooterActions.tsx
    ├── StatusDot.tsx
    └── KbdRow.tsx
```

### Component Size

- Max ~200 lines per component
- If longer, extract sub-components or hooks
- Shared components go in `shared/`
- Page-specific components go in their group folder

---

## 5. Design Token Usage

> 注：本节原引用 `buddy-design/colors_and_type.css`，**该路径从未存在**（审计确认）。
> 设计令牌的唯一真值来源是 **`docs/design/design-tokens.md`**（辅以 `src/styles/global.css` 的实际定义）。
> 仓库中曾有一个同名文件在 `.design/animation-preview/`（Trae 工具生成的动效预览实验，前缀 `--ap-`、主色 `#5B8DEF`），**已删除**：其令牌与 `global.css` 重复，且**设计以代码为准**。

v1（Tauri）经 CSS 变量映射到 Tailwind config；v2（GPUI）映射到 `Theme`（见 `docs/specs/phase-01/` 与 `docs/tasks/v2.0.0-gpui/03-theme.md`）。

### Icons

- **不使用 emoji 作为 UI 图标**
- v1：`lucide-react`（`import { Send, Settings, X } from 'lucide-react'`）
- v2（GPUI）：svm 资源 + `Theme` 颜色；**不使用 lucide-react**
- Size: 14px for inline, 16px for standalone buttons
- Color: inherit from parent

### Typography

- Use semantic class names: `t-body`, `t-caption`, `t-h3`, etc.
- Monospace: `t-mono` class for code, model names
- Never set `font-family` inline — use the classes

### Spacing

- Use Tailwind spacing scale (`p-4`, `gap-2`, `mt-6`)
- Custom values use `var(--space-N)`
- Grid gaps and flex gaps consistent throughout

### Colors (CRITICAL)

```
NEVER:
  color: '#5B5FE9'            ← hardcoded
  background: '#FFFFFF'       ← hardcoded
  rgba(0, 0, 0, 0.06)         ← hardcoded

ALWAYS:
  color: 'var(--buddy-primary)'
  background: 'var(--bg-surface)'
  border: '1px solid var(--border-subtle)'
```

---

## 6. Git Conventions

- **Branch**: `main` for development; feature branches optional for agents
- **Commits**: descriptive, Chinese OK for messages
- **NO commits** unless explicitly asked
- **NO force push** to main

---

## 7. Testing Requirements

- Rust: `#[cfg(test)]` module at bottom of each `.rs` file
- React: `vitest` + `@testing-library/react` for component render tests
- E2E: manual smoke test checklist (SPEC.md T08-T18)
- Before marking a task complete: verify it works

---

## 8. What NOT to Do

1. ❌ Do NOT add traffic-light buttons, drag handles, or window chrome
2. ❌ Do NOT hardcode colors, spacing, radii, or shadows（v1 用 CSS 变量，v2 用 `Theme`）
3. ❌ Do NOT use emoji as UI icons
4. ❌ Do NOT implement features not in a spec（spec 注册表：`docs/specs/README.md`）
5. ❌ Do NOT 修改已退役的设计文档（已于 `docs/specs/design-deletions.md` 登记的删除不得回滚）
6. ❌ Do NOT change window size on page transitions
7. ❌ Do NOT use `unwrap()` or `expect()` in Tauri command handlers
8. ❌ Do NOT add new dependencies without updating the relevant spec
9. ❌ Do NOT create files outside the defined project structure
10. ❌ Do NOT make API calls from React components — go through stores → IPC → Rust（v1）
    ❌ v2（GPUI）：不得在渲染函数中直接发网络请求 —— 走 `Entity` + 后台执行器

---

## 9. Quick Reference

```
Task to do → Check SPEC.md for task ID
UI question → Check FRONTEND_DESIGN.md
Rust question → Check BACKEND_DESIGN.md
Style question → Check this file
Still unsure → Ask, don't guess
```
