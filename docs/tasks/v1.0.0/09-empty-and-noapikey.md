# Task 09: Empty & NoApiKey Pages

## 目标

实现首次唤出的空态页面和未配置 API Key 的警告页面。

## 相关设计文档

- `docs/design/pages-and-states.md` — page-empty 和 page-noapikey 规格
- `docs/design/design-tokens.md` — 颜色/圆角/阴影
- `docs/design/component-mapping.md` — 组件映射

## 验收标准

### EmptyPage

- [ ] 520px 宽毛玻璃 pill，居中悬浮
- [ ] 内容：[B logo] [input] [× clear] [sparkles model pill ▾] [↑ send 按钮]
- [ ] 输入框有内容时，清除按钮(×)显示；无内容时隐藏
- [ ] 发送按钮在有内容时激活态（主色），无内容时禁用态（灰色）
- [ ] 底部提示行：`Enter 发起提问 · Shift+Enter 换行`
- [ ] Hover 时 pill 阴影加深 (`--shadow-floating-sm` → `--shadow-floating-md`)
- [ ] Enter / 点击发送：检查无 API Key → 跳转 noapikey

### NoApiKeyPage

- [ ] 同尺寸（520×60），同毛玻璃 pill
- [ ] 内容：[B logo] [⚠ alert-triangle 红] [红字"请先设置 API Key"] [红 CTA "设置 →"]
- [ ] 整条 pill 可点击 → 跳转 settings
- [ ] 无隐私提示文字

## 开发工作

| ID | Task | Component | Details |
|----|------|-----------|---------|
| D37 | EmptyPage | `src/pages/EmptyPage.tsx` | Input pill with all interactions |
| D38 | NoApiKeyPage | `src/pages/NoApiKeyPage.tsx` | Red warning pill |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T05 | Render tests | Both pages render without crash |
| T08 | E2E: empty → noapikey | Fresh install → type text → Enter → verify noapikey shown |
