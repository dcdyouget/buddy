# v1 设计令牌实测值（S01-06-8）

> 生成：`python3 scripts/v1-baseline/extract_tokens.py`，来源 `v1-final:src/styles/global.css`（代码真值）。
> 对照：`e91bbc3:docs/design/design-tokens.md`（已退役）。浅色 139 个变量，深色覆盖 46 个。

## 与 design-tokens.md 的差异

### 值不一致（8，`var()` 已展开后比较）

| 令牌 | 主题 | design-tokens.md | global.css（真值） |
|---|---|---|---|
| `--bg-elevated` | 浅色/通用 | `rgba(255,255,255,0.72)` | `#FFFFFF` |
| `--bg-elevated` | 深色 | `rgba(48,46,51,0.74)` | `#302E33` |
| `--bg-surface` | 浅色/通用 | `rgba(247,245,242,0.82)` | `#FFFFFF` |
| `--bg-surface` | 深色 | `rgba(28,27,30,0.82)` | `#1C1B1E` |
| `--composer-surface` | 浅色/通用 | `rgba(252,251,250,0.86)` | `#FFFFFF` |
| `--composer-surface` | 深色 | `rgba(39,38,42,0.88)` | `#27262A` |
| `--panel-surface` | 浅色/通用 | `rgba(255,255,255,0.46)` | `#F5F4F2` |
| `--panel-surface` | 深色 | `rgba(255,255,255,0.045)` | `#302E33` |

### 文档声明但代码不存在（0）

无

### 代码存在但文档未声明（82）

`--code-bg`, `--code-border`, `--code-header-bg`, `--code-syntax-comment`, `--code-syntax-function`, `--code-syntax-keyword`, `--code-syntax-number`, `--code-syntax-operator`, `--code-syntax-property`, `--code-syntax-punctuation`, `--code-syntax-special`, `--code-syntax-string`, `--code-text`, `--composer-glow`, `--control-surface`, `--delay-streaming-char-age-1`, `--delay-streaming-char-age-2`, `--delay-streaming-char-age-3`, `--delay-streaming-char-age-4`, `--delay-streaming-char-age-5`, `--delay-streaming-char-age-6`, `--delay-streaming-char-age-7`, `--delay-streaming-char-age-8`, `--duration-streaming-char-settle`, `--duration-streaming-star-breathe`, `--duration-thinking-loader`, `--duration-thinking-sheen`, `--duration-tool-flow`, `--filter-streaming-star`, `--font-size-2xl`, `--font-size-base`, `--font-size-lg`, `--font-size-md`, `--font-size-sm`, `--font-size-xl`, `--font-size-xs`, `--glass-outline`, `--letter-spacing-base`, `--letter-spacing-tight`, `--letter-spacing-wide`, `--line-height-base`, `--line-height-relaxed`, `--line-height-tight`, `--markdown-accent`, `--markdown-accent-line`, `--markdown-accent-medium`, `--markdown-accent-soft`, `--markdown-accent-strong`, `--neutral-0`, `--neutral-100`, `--neutral-1000`, `--neutral-200`, `--neutral-300`, `--neutral-400`, `--neutral-50`, `--neutral-500`, `--neutral-600`, `--neutral-700`, `--neutral-800`, `--neutral-900`, `--shadow-composer`, `--shadow-floating-md`, `--shadow-floating-sm`, `--shadow-focus`, `--shadow-static`, `--shadow-streaming-char-settle`, `--shadow-window`, `--shadow-window-edge`, `--streaming-star-blue`, `--streaming-star-blue-soft`, `--streaming-star-white`, `--surface-highlight`, `--tool-ui-accent`, `--tool-ui-accent-medium`, `--tool-ui-accent-soft`, `--tool-ui-accent-strong`, `--tool-ui-action`, `--tool-ui-flow-highlight`, `--tool-ui-flow-secondary`, `--user-bubble-border`, `--window-inner-highlight`, `--window-outline`

## 完整令牌表（global.css）

| 令牌 | 浅色（`:root`，展开后） | 深色（`html.dark`，展开后；空 = 同浅色） |
|---|---|---|
| `--bg-canvas` | `#F3F1EE` | `#181719` |
| `--bg-elevated` | `#FFFFFF` | `#302E33` |
| `--bg-overlay` | `rgba(28, 25, 23, 0.38)` | `rgba(0, 0, 0, 0.55)` |
| `--bg-sunken` | `rgba(43, 40, 38, 0.055)` | `rgba(255, 255, 255, 0.065)` |
| `--bg-surface` | `#FFFFFF` | `#1C1B1E` |
| `--border-default` | `rgba(43, 40, 38, 0.10)` | `rgba(255, 255, 255, 0.13)` |
| `--border-strong` | `rgba(43, 40, 38, 0.16)` | `rgba(255, 255, 255, 0.19)` |
| `--border-subtle` | `rgba(43, 40, 38, 0.065)` | `rgba(255, 255, 255, 0.085)` |
| `--buddy-primary` | `#5B5FE9` |  |
| `--buddy-primary-100` | `#E2E3FC` |  |
| `--buddy-primary-200` | `#C6C7F8` |  |
| `--buddy-primary-300` | `#A9ABF3` |  |
| `--buddy-primary-400` | `#8D8FEE` |  |
| `--buddy-primary-50` | `#F1F1FE` |  |
| `--buddy-primary-500` | `#5B5FE9` |  |
| `--buddy-primary-600` | `#4A4ED4` |  |
| `--buddy-primary-700` | `#3D40B0` |  |
| `--buddy-primary-800` | `#2F328A` |  |
| `--buddy-primary-900` | `#1F2160` |  |
| `--code-bg` | `color-mix(in srgb, #2563EB 4%, #FFFFFF)` | `#121114` |
| `--code-border` | `color-mix( in srgb, #2563EB 20%, #FFFFFF )` | `color-mix( in srgb, #2563EB 30%, #121114 )` |
| `--code-header-bg` | `color-mix( in srgb, #2563EB 7%, #FFFFFF )` | `color-mix( in srgb, #2563EB 12%, #121114 )` |
| `--code-syntax-comment` | `color-mix( in srgb, #2563EB 18%, #7D7976 )` | `color-mix( in srgb, #2563EB 22%, #918D8A )` |
| `--code-syntax-function` | `color-mix( in srgb, #2563EB 68%, #16A34A )` | `color-mix( in srgb, #2563EB 48%, #16A34A )` |
| `--code-syntax-keyword` | `color-mix( in srgb, #2563EB 86%, #222120 )` | `color-mix( in srgb, #2563EB 62%, #FFFFFF )` |
| `--code-syntax-number` | `color-mix( in srgb, #D97706 78%, #222120 )` | `color-mix( in srgb, #D97706 66%, #FFFFFF )` |
| `--code-syntax-operator` | `color-mix( in srgb, #2563EB 52%, #222120 )` | `color-mix( in srgb, #2563EB 36%, #FFFFFF )` |
| `--code-syntax-property` | `color-mix( in srgb, #2563EB 74%, #222120 )` | `color-mix( in srgb, #2563EB 58%, #FFFFFF )` |
| `--code-syntax-punctuation` | `#5E5B59` | `#B7B3B0` |
| `--code-syntax-special` | `color-mix( in srgb, #D97706 66%, #2563EB )` | `color-mix( in srgb, #D97706 52%, #FFFFFF )` |
| `--code-syntax-string` | `color-mix( in srgb, #16A34A 64%, #2563EB )` | `color-mix( in srgb, #16A34A 58%, #FFFFFF )` |
| `--code-text` | `color-mix( in srgb, #2563EB 10%, #222120 )` | `color-mix( in srgb, #2563EB 18%, #FFFFFF )` |
| `--composer-glow` | `color-mix(in srgb, #2563EB 52%, #FFFFFF)` | `color-mix(in srgb, #2563EB 68%, #5B5FE9)` |
| `--composer-surface` | `#FFFFFF` | `#27262A` |
| `--control-surface` | `#FFFFFF` | `#302E33` |
| `--delay-streaming-char-age-1` | `-32ms` |  |
| `--delay-streaming-char-age-2` | `-64ms` |  |
| `--delay-streaming-char-age-3` | `-96ms` |  |
| `--delay-streaming-char-age-4` | `-128ms` |  |
| `--delay-streaming-char-age-5` | `-160ms` |  |
| `--delay-streaming-char-age-6` | `-192ms` |  |
| `--delay-streaming-char-age-7` | `-224ms` |  |
| `--delay-streaming-char-age-8` | `-256ms` |  |
| `--duration-fast` | `120ms` |  |
| `--duration-normal` | `200ms` |  |
| `--duration-slow` | `320ms` |  |
| `--duration-streaming-char-settle` | `260ms` |  |
| `--duration-streaming-star-breathe` | `1.1s` |  |
| `--duration-thinking-loader` | `1.2s` |  |
| `--duration-thinking-sheen` | `3.4s` |  |
| `--duration-tool-flow` | `2.8s` |  |
| `--ease-spring` | `cubic-bezier(0.34, 1.56, 0.64, 1)` |  |
| `--ease-standard` | `cubic-bezier(0.2, 0.0, 0, 1)` |  |
| `--field-surface` | `rgba(43, 40, 38, 0.045)` | `rgba(255, 255, 255, 0.055)` |
| `--filter-streaming-star` | `drop-shadow(0 0 2px #FFFFFF) drop-shadow(0 0 5px color-mix( in srgb, #2563EB 82%, #FFFFFF )) drop-shadow(0 0 9px color-mix( in srgb, #2563EB 52%, transparent ))` |  |
| `--font-mono` | `'Fira Code', 'JetBrains Mono', 'SF Mono', 'Menlo', 'Consolas', monospace` |  |
| `--font-sans` | `'Fira Code', 'JetBrains Mono', 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', 'PingFang SC', 'Hiragino Sans GB', 'Microsoft YaHei', sans-serif` |  |
| `--font-size-2xl` | `24px` |  |
| `--font-size-base` | `13px` |  |
| `--font-size-lg` | `16px` |  |
| `--font-size-md` | `14px` |  |
| `--font-size-sm` | `12px` |  |
| `--font-size-xl` | `20px` |  |
| `--font-size-xs` | `11px` |  |
| `--font-weight-emphasis` | `750` |  |
| `--font-weight-heading` | `800` |  |
| `--font-weight-regular` | `650` |  |
| `--glass-outline` | `#E9E7E4` | `rgba(255, 255, 255, 0.19)` |
| `--letter-spacing-base` | `0` |  |
| `--letter-spacing-tight` | `-0.01em` |  |
| `--letter-spacing-wide` | `0.02em` |  |
| `--line-height-base` | `1.5` |  |
| `--line-height-relaxed` | `1.65` |  |
| `--line-height-tight` | `1.25` |  |
| `--markdown-accent` | `color-mix(in srgb, #2563EB 72%, #7D7976)` | `color-mix(in srgb, #2563EB 58%, #FFFFFF)` |
| `--markdown-accent-line` | `color-mix(in srgb, color-mix(in srgb, #2563EB 72%, #7D7976) 38%, transparent)` | `color-mix(in srgb, color-mix(in srgb, #2563EB 58%, #FFFFFF) 44%, transparent)` |
| `--markdown-accent-medium` | `color-mix(in srgb, color-mix(in srgb, #2563EB 72%, #7D7976) 24%, transparent)` | `color-mix(in srgb, color-mix(in srgb, #2563EB 58%, #FFFFFF) 28%, transparent)` |
| `--markdown-accent-soft` | `color-mix(in srgb, color-mix(in srgb, #2563EB 72%, #7D7976) 10%, transparent)` | `color-mix(in srgb, color-mix(in srgb, #2563EB 58%, #FFFFFF) 13%, transparent)` |
| `--markdown-accent-strong` | `color-mix(in srgb, #2563EB 82%, #222120)` | `color-mix(in srgb, #2563EB 42%, #FFFFFF)` |
| `--neutral-0` | `#FFFFFF` |  |
| `--neutral-100` | `#F5F4F2` |  |
| `--neutral-1000` | `#121110` |  |
| `--neutral-200` | `#E9E7E4` |  |
| `--neutral-300` | `#D4D1CD` |  |
| `--neutral-400` | `#A6A29D` |  |
| `--neutral-50` | `#FBFAF9` |  |
| `--neutral-500` | `#7D7976` |  |
| `--neutral-600` | `#5E5B59` |  |
| `--neutral-700` | `#454240` |  |
| `--neutral-800` | `#302E2D` |  |
| `--neutral-900` | `#222120` |  |
| `--panel-surface` | `#F5F4F2` | `#302E33` |
| `--primary-tint-soft` | `#F1F1FE` | `rgba(112, 117, 255, 0.18)` |
| `--primary-tint-strong` | `#E2E3FC` | `rgba(112, 117, 255, 0.30)` |
| `--radius-full` | `9999px` |  |
| `--radius-lg` | `12px` |  |
| `--radius-md` | `8px` |  |
| `--radius-sm` | `4px` |  |
| `--radius-xl` | `16px` |  |
| `--shadow-composer` | `0 14px 32px rgba(35, 29, 24, 0.14), 0 3px 9px rgba(35, 29, 24, 0.06)` | `0 14px 32px rgba(0, 0, 0, 0.35), 0 3px 9px rgba(0, 0, 0, 0.18)` |
| `--shadow-floating-md` | `0 22px 64px rgba(35, 29, 24, 0.16), 0 2px 10px rgba(35, 29, 24, 0.05)` |  |
| `--shadow-floating-sm` | `0 8px 24px rgba(35, 29, 24, 0.09), 0 1px 2px rgba(35, 29, 24, 0.04)` |  |
| `--shadow-focus` | `0 0 0 3px color-mix(in srgb, #5B5FE9 10%, transparent)` |  |
| `--shadow-static` | `0 1px 2px rgba(35, 29, 24, 0.04)` |  |
| `--shadow-streaming-char-settle` | `0 0 2px #FFFFFF, 0 0 7px color-mix( in srgb, #2563EB 82%, #FFFFFF ), 0 0 13px color-mix( in srgb, #2563EB 52%, transparent )` |  |
| `--shadow-window` | `0 30px 90px rgba(35, 29, 24, 0.22), 0 10px 34px rgba(35, 29, 24, 0.12), 0 1px 0 rgba(255, 255, 255, 0.55) inset` | `0 30px 90px rgba(0, 0, 0, 0.48), 0 10px 34px rgba(0, 0, 0, 0.34), 0 1px 0 rgba(255, 255, 255, 0.08) inset` |
| `--shadow-window-edge` | `0 2px 7px rgba(35, 29, 24, 0.24), 0 1px 2px rgba(35, 29, 24, 0.12)` | `0 2px 7px rgba(0, 0, 0, 0.58), 0 1px 2px rgba(0, 0, 0, 0.32)` |
| `--space-1` | `4px` |  |
| `--space-10` | `40px` |  |
| `--space-12` | `48px` |  |
| `--space-2` | `8px` |  |
| `--space-3` | `12px` |  |
| `--space-4` | `16px` |  |
| `--space-5` | `20px` |  |
| `--space-6` | `24px` |  |
| `--space-8` | `32px` |  |
| `--state-error` | `#DC2626` |  |
| `--state-info` | `#2563EB` |  |
| `--state-success` | `#16A34A` |  |
| `--state-warning` | `#D97706` |  |
| `--streaming-star-blue` | `color-mix( in srgb, #2563EB 82%, #FFFFFF )` |  |
| `--streaming-star-blue-soft` | `color-mix( in srgb, #2563EB 52%, transparent )` |  |
| `--streaming-star-white` | `color-mix( in srgb, #FFFFFF 92%, #2563EB )` |  |
| `--surface-highlight` | `transparent` | `rgba(255, 255, 255, 0.045)` |
| `--text-muted` | `#5E5B59` | `#B7B3B0` |
| `--text-on-primary` | `#FFFFFF` |  |
| `--text-primary` | `#222120` | `#F7F6F5` |
| `--text-tertiary` | `#7D7976` | `#918D8A` |
| `--tool-ui-accent` | `color-mix(in srgb, #2563EB 72%, #7D7976)` | `color-mix(in srgb, #2563EB 58%, #FFFFFF)` |
| `--tool-ui-accent-medium` | `color-mix(in srgb, color-mix(in srgb, #2563EB 72%, #7D7976) 24%, transparent)` | `color-mix(in srgb, color-mix(in srgb, #2563EB 58%, #FFFFFF) 28%, transparent)` |
| `--tool-ui-accent-soft` | `color-mix(in srgb, color-mix(in srgb, #2563EB 72%, #7D7976) 10%, transparent)` | `color-mix(in srgb, color-mix(in srgb, #2563EB 58%, #FFFFFF) 13%, transparent)` |
| `--tool-ui-accent-strong` | `color-mix(in srgb, #2563EB 82%, #222120)` | `color-mix(in srgb, #2563EB 42%, #FFFFFF)` |
| `--tool-ui-action` | `#2563EB` |  |
| `--tool-ui-flow-highlight` | `color-mix( in srgb, #2563EB 48%, #FFFFFF )` |  |
| `--tool-ui-flow-secondary` | `color-mix( in srgb, #2563EB 68%, #16A34A )` |  |
| `--user-bubble` | `rgba(91, 95, 233, 0.075)` | `rgba(91, 95, 233, 0.19)` |
| `--user-bubble-border` | `rgba(91, 95, 233, 0.14)` | `rgba(169, 171, 243, 0.22)` |
| `--window-inner-highlight` | `rgba(255, 255, 255, 0.72)` | `rgba(255, 255, 255, 0.12)` |
| `--window-outline` | `color-mix(in srgb, #A6A29D 58%, #FFFFFF)` | `color-mix(in srgb, #D4D1CD 68%, #FFFFFF)` |
