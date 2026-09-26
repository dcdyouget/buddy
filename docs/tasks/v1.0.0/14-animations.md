# Task 14: Animations

## 目标

实现所有界面动效。

## 相关设计文档

- `docs/design/design-tokens.md` — motion tokens (`--ease-standard`, `--duration-fast/normal/slow`)
- `docs/design/pages-and-states.md` — streaming cursor 和 model dropdown 的具体动画描述

## 验收标准

- [ ] 窗口首次出现：scale(0.95) → scale(1) + opacity 0→1（spring）
- [ ] 新消息气泡：slideUp(10px) + fadeIn（200ms）
- [ ] 页面切换：AnimatePresence fade + slide
- [ ] Streaming AI 消息末尾闪烁光标（CSS `@keyframes buddy-blink` 1s）
- [ ] ModelDropdown：从 model pill 位置 scale 弹出，带箭头指示器
- [ ] 按钮 hover/active 状态过渡（120ms ease-standard）
- [ ] 尊重 `prefers-reduced-motion`：关闭所有动画

## 开发工作

| ID | Task | Details |
|----|------|---------|
| D54 | Window enter/exit | Wrap root with `<motion.div>` scale+opacity spring |
| D55 | Message slide-in | `<motion.div>` per message, `initial={{ opacity: 0, y: 10 }}`, `animate={{ opacity: 1, y: 0 }}` |
| D56 | Page transitions | `AnimatePresence mode="wait"` + `motion.div` fade/slide |
| D57 | Blinking cursor | CSS class `.buddy-cursor` with `@keyframes buddy-blink` |
| D58 | Model dropdown | `<motion.div>` with `initial={{ scale: 0.95, opacity: 0 }}`, `animate={{ scale: 1, opacity: 1 }}` |

## 测试工作

无独立测试。通过 E2E 和手动审查验证动画是否流畅。

### T05 覆盖

- 组件渲染不因 motion 配置崩溃
- AnimatePresence 在有/无子元素时均正常
