# Task 12: Settings & Add Provider Pages

## 目标

实现设置面板和添加模型子面板的完整功能。

## 相关设计文档

- `docs/design/pages-and-states.md` — page-settings 和 page-add-provider 规格
- `docs/design/component-mapping.md` — Segmented, ModelRow, ProviderCard, SlideInPanel
- `docs/design/sse-and-api.md` — Provider 预设信息

## 验收标准

### SettingsPage

- [ ] 760×640 frameless 窗口
- [ ] 主题选择：浅色 / 深色 toggle（v1.0.0 二选一）
- [ ] 快捷键展示 + [重新录制] 按钮
- [ ] 模型范围列表：每行 checkbox + 名称 + 描述 + 延迟 + [设为默认]
- [ ] 未启用模型行灰度显示
- [ ] [+ 添加模型] 虚线按钮 → SlideInPanel → AddProviderPage
- [ ] Footer: [取消] [确定]

### AddProviderPage

- [ ] 从右侧滑入覆盖 settings
- [ ] 顶部：[← 返回设置] [✕ 关闭]
- [ ] 标题行："添加模型 · 配置 Provider · 获取可用模型"
- [ ] Provider 预设卡片 2×2 网格（DeepSeek/MiniMax/GLM/Kimi）
- [ ] 选中预设 → 自动填入 Base URL
- [ ] "自定义 OpenAI 兼容服务"链接
- [ ] API Key 输入框 + 显示/隐藏切换 + 获取 Key 链接
- [ ] [获取模型列表] 主按钮 → 调用 fetch_models → 显示可用模型预览
- [ ] [测速] 次按钮 → 调用 test_latency → 显示延迟
- [ ] 可用模型预览：checkbox 列表，默认全选
- [ ] Footer: [取消] [✓ 添加] → 选中的模型加入 config

## 开发工作

| ID | Task | Component | Details |
|----|------|-----------|---------|
| D46 | SettingsPage | `src/pages/SettingsPage.tsx` | Full settings panel |
| D47 | ModelRow | `src/components/settings/ModelRow.tsx` | Checkbox + info + set-default |
| D48 | HotkeyRecorder | `src/components/settings/HotkeyRecorder.tsx` | Display + record button + conflict |
| D49 | AddProviderPage | `src/pages/AddProviderPage.tsx` | Provider config form |
| D50 | ProviderCard | `src/components/settings/ProviderCard.tsx` | Provider preset card |
| — | Segmented | `src/components/shared/Segmented.tsx` | (part of shared components) |
| — | SlideInPanel | `src/components/shared/SlideInPanel.tsx` | (part of shared components) |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T05 | Render tests | Settings and AddProvider render correctly |
| T09 | E2E: add provider flow | Empty → noapikey → settings → add provider → select DeepSeek → fill key → fetch models → select models → save → back to conversation |
| T12 | E2E: hotkey record | Settings → record new shortcut → verify it works |
| T13 | E2E: theme toggle | Settings → dark mode → verify CSS class + persistence |
