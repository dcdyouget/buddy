# Task 13: App Shell & Theme

## 目标

实现 App.tsx 页面状态机、流式事件监听 hook、主题切换逻辑。

## 相关设计文档

- `docs/design/pages-and-states.md` — 状态机图和进入规则
- `docs/design/design-tokens.md` — 主题切换机制
- `docs/design/ipc-contract.md` — 事件名称

## 验收标准

### App.tsx

- [ ] 根据 `uiStore.currentPage` 渲染对应页面
- [ ] 页面切换使用 `AnimatePresence` 动画包裹
- [ ] 进入规则：providers 为空 → noapikey；否则 → 上次页面
- [ ] 全局 Esc 监听：隐藏窗口（不停止流式）
- [ ] 全局 `listen` 设置：stream-token, stream-done, stream-error, stream-cancelled

### useStreaming Hook

- [ ] 封装所有 `listen` 事件注册/注销
- [ ] 组件卸载时自动清理 listener

### Theme

- [ ] `html.classList.toggle('dark')` 切换主题
- [ ] 主题变更持久化到 `configStore`
- [ ] 应用启动时从 config 读取主题并应用
- [ ] CSS variable 自动完成所有颜色切换（light ↔ dark）

## 开发工作

| ID | Task | File | Details |
|----|------|------|---------|
| D51 | App.tsx | `src/App.tsx` | Page router + event listeners + theme init |
| D52 | useStreaming | `src/hooks/useStreaming.ts` | Event registration/cleanup |
| D53 | Theme toggle | `src/App.tsx` or store action | `toggle('dark')` + persist |

## 测试工作

本任务通过 E2E 测试覆盖（T08-T15）。
