# Design Tokens

> Source: 本项目设计令牌的唯一真值来源。
> **注**：本文件原头部引用 `buddy-design/colors_and_type.css`，经审计确认**该路径从未存在**。仓库中唯一的同名文件是 `.design/animation-preview/colors_and_type.css`（Trae 设计工具生成的动效预览实验），前缀 `--ap-`、主色 `#5B8DEF`，**非本项目令牌**；该目录已于审计后**删除**（见 `docs/specs/design-deletions.md`）。
> **本文件是设计令牌的唯一真值来源。**
> Usage: Tailwind CSS v4 theme config + CSS custom properties

## Brand Colors

| Token | Value | Usage |
|-------|-------|-------|
| `--buddy-primary` | `#5B5FE9` | Primary buttons, focus borders, active states |
| `--buddy-primary-50` | `#F1F1FE` | Light tint backgrounds |
| `--buddy-primary-100` | `#E2E3FC` | Stronger tint |
| `--buddy-primary-200` | `#C6C7F8` | |
| `--buddy-primary-300` | `#A9ABF3` | |
| `--buddy-primary-400` | `#8D8FEE` | |
| `--buddy-primary-500` | `#5B5FE9` | Base |
| `--buddy-primary-600` | `#4A4ED4` | Hover/active |
| `--buddy-primary-700` | `#3D40B0` | |
| `--buddy-primary-800` | `#2F328A` | |
| `--buddy-primary-900` | `#1F2160` | |

## State Colors (semantic only)

| Token | Value | Usage |
|-------|-------|-------|
| `--state-success` | `#16A34A` | Connected, speed normal |
| `--state-warning` | `#D97706` | Quota low |
| `--state-error` | `#DC2626` | No API key, connection failed |
| `--state-info` | `#2563EB` | Info banner |

## Theme-bound Tokens

| Token | Light | Dark |
|-------|-------|------|
| `--bg-canvas` | `#F3F1EE` | `#181719` |
| `--bg-surface` | `rgba(247,245,242,0.82)` | `rgba(28,27,30,0.82)` |
| `--bg-elevated` | `rgba(255,255,255,0.72)` | `rgba(48,46,51,0.74)` |
| `--bg-sunken` | `rgba(43,40,38,0.055)` | `rgba(255,255,255,0.065)` |
| `--bg-overlay` | `rgba(28,25,23,0.38)` | `rgba(0,0,0,0.55)` |
| `--composer-surface` | `rgba(252,251,250,0.86)` | `rgba(39,38,42,0.88)` |
| `--panel-surface` | `rgba(255,255,255,0.46)` | `rgba(255,255,255,0.045)` |
| `--field-surface` | `rgba(43,40,38,0.045)` | `rgba(255,255,255,0.055)` |
| `--user-bubble` | `rgba(91,95,233,0.075)` | `rgba(91,95,233,0.19)` |
| `--text-primary` | `#222120` | `#F7F6F5` |
| `--text-muted` | `#5E5B59` | `#B7B3B0` |
| `--text-tertiary` | `#7D7976` | `#918D8A` |
| `--text-on-primary` | `#FFFFFF` | `#FFFFFF` |
| `--border-subtle` | `rgba(43,40,38,0.065)` | `rgba(255,255,255,0.085)` |
| `--border-default` | `rgba(43,40,38,0.10)` | `rgba(255,255,255,0.13)` |
| `--border-strong` | `rgba(43,40,38,0.16)` | `rgba(255,255,255,0.19)` |
| `--primary-tint-soft` | `var(--buddy-primary-50)` | `rgba(112,117,255,0.18)` |
| `--primary-tint-strong` | `var(--buddy-primary-100)` | `rgba(112,117,255,0.30)` |

## Typography

```css
--font-sans: 'Fira Code', 'JetBrains Mono', 'Inter', -apple-system,
             BlinkMacSystemFont, 'Segoe UI', 'PingFang SC',
             'Hiragino Sans GB', 'Microsoft YaHei', sans-serif;
--font-mono: 'Fira Code', 'JetBrains Mono', 'SF Mono', 'Menlo', 'Consolas', monospace;
--font-weight-regular: 650;
--font-weight-emphasis: 750;
--font-weight-heading: 800;
```

| Role | Class | Size |
|------|-------|------|
| display | `t-display` | 24px |
| title | `t-title` | 20px |
| h3 | `t-h3` | 16px |
| body | `t-body` | 14px |
| body-sm | `t-body-sm` | 13px |
| caption | `t-caption` | 12px |
| overline | `t-overline` | 11px |

## Spacing (4px base)

```
--space-1=4 · --space-2=8 · --space-3=12 · --space-4=16 · --space-5=20
--space-6=24 · --space-8=32 · --space-10=40 · --space-12=48
```

## Border Radius

```
--radius-sm=4 · --radius-md=8 · --radius-lg=12 · --radius-xl=16 · --radius-full=9999
```

## Shadows

| Token | Usage |
|-------|-------|
| `--shadow-static` | Input boxes, list items, chips (alpha ≤ 0.05 light) |
| `--shadow-floating-sm` | Settings inner floats |
| `--shadow-floating-md` | Main window, slide-in panels |

## Motion

```css
--ease-standard: cubic-bezier(0.2, 0.0, 0, 1);
--ease-spring:   cubic-bezier(0.34, 1.56, 0.64, 1);
--duration-fast:   120ms;
--duration-normal: 200ms;
--duration-slow:   320ms;
```

## Frosted Glass

> **已删除（2026-09-10）**：本段原有的 `.surface-glass { backdrop-filter: blur(..) saturate(180%) }`
> 是**设计意图，从未在 v1 中实际使用**（v1 为实色界面，`src-tauri/src/platform/macos.rs:154`）。
> 产品已决定不使用毛玻璃（见 `docs/specs/phase-00/S00-04-backdrop.md`），故按
> 「代码即设计」原则删除，不留用不上的设计意图。
>
> **窗口外观的最终配置**：不透明实色面板 + 16px 圆角（CALayer）+ 无边框 + 无模糊。
> 规范化产物：`docs/evidence/s00-04/window-appearance.rs`。

## Theme Switching

```ts
document.documentElement.classList.toggle('dark');
// CSS variables auto-remap. Persisted to config.json.
```

## Rule

**NEVER hardcode colors, spacing, radii, or shadows.** Always use CSS variables. The only exception is `rgba(220,38,38,0.12)` for the red warning tint.
