# Phase 06: 设置界面

## 目标

把设置相关页面从 React 移植到 GPUI。

## 相关文档

- `docs/design/pages-and-states.md` — 设置页规格
- `docs/design/component-mapping.md` — 设计 → 组件映射
- `docs/tasks/v2.0.0-gpui/03-theme.md` — 主题令牌
- `docs/tasks/v2.0.0-gpui/07-shell.md` — 全局热键录制（依赖 Phase 07 的 S03 结论）

## 验收标准

- [ ] 设置页切换不改变窗口尺寸（硬约束 6）
- [ ] API Key 以明文 JSON 存储（硬约束 9），不做系统钥匙串
- [ ] 热键录制可捕获裸键组合并正确生效
- [ ] 更新检查与安装流程可用（与 Phase 08 打通）
- [ ] 全部文案中文（硬约束 10），图标用 svg（硬约束 4）

## 组件移植清单

| 现有文件 | 行数 | 目标 |
|---------|------|------|
| `src/pages/SettingsPage.tsx` | 148 | `SettingsView` |
| `src/components/settings/AddProviderPanel.tsx` | 649 | `AddProviderPanel` |
| `src/components/settings/ModelList.tsx` | — | `ModelList` |
| `src/components/settings/ModelRow.tsx` | 290 | `ModelRow` |
| `src/components/settings/ProviderCard.tsx` | — | `ProviderCard` |
| `src/components/settings/HotkeyRecorder.tsx` | 175 | `HotkeyRecorder`（依赖 S03） |
| `src/components/settings/HotkeySetting.tsx` | — | `HotkeySetting` |
| `src/components/settings/ThemeSetting.tsx` | — | `ThemeSetting`（依赖 Phase 03 D16） |
| `src/components/settings/UpdateSetting.tsx` | 210 | `UpdateSetting`（依赖 Phase 08） |
| `src/components/shared/SlideInPanel.tsx` | — | `slide_in_panel()` |
| `src/stores/configStore.ts` | 260 | `ConfigEntity` |

## 开发工作

### 6.1 设置页骨架

| ID | Task | Details |
|----|------|---------|
| D01 | `SettingsView` | 分组式布局，复现现有信息层级 |
| D02 | 分组与分隔 | 复用 Phase 05 的 `glass_panel()` 与分隔线 |
| D03 | 设置项控件集 | 开关、下拉、文本输入、按钮的统一封装 |
| D04 | 滑动面板 | `slide_in_panel()` 复现 `SlideInPanel.tsx`（用于新增 Provider 等） |
| D05 | 页面切换不改变窗口尺寸 | 与 Phase 07 D0x 联动验证（硬约束 6） |
| D06 | 滚动与键盘导航 | 设置页滚动、Tab 焦点顺序 |

### 6.2 Provider 与模型

| ID | Task | Details |
|----|------|---------|
| D07 | `ProviderCard` | 展示名称、状态、模型数量、启用开关 |
| D08 | `AddProviderPanel` | 复现 649 行的新增流程：预设、Base URL、API Key、模型列表 |
| D09 | 拉取模型列表 | 接 Phase 02 的 `fetch models` 能力 + 速度测试 |
| D10 | `ModelList` / `ModelRow` | 模型行的启用、删除、上下文长度、能力标记 |
| D11 | 模型上下文配置 | 现有 `models/model_context.rs`（288 行）对应的 UI |
| D12 | MCP 配置 | 现有 `models/mcp.rs` 对应的配置界面 |
| D13 | 校验与错误提示 | 无效 URL / Key 的即时反馈 |

### 6.3 热键

| ID | Task | Details |
|----|------|---------|
| D14 | 热键录制交互 | 复现 `HotkeyRecorder.tsx`：进入录制态、拦截按键、显示组合、取消 |
| D15 | 裸键拦截 | GPUI 中拦截无修饰键的按键（避免触发全局快捷键） |
| D16 | 冲突检测 | 与系统/其他应用热键冲突时的提示 |
| D17 | 保存与即时生效 | 修改后立即重新注册全局热键（依赖 Phase 07） |
| D18 | 默认热键 | 保持与现有版本一致，避免用户升级后失效 |

### 6.4 主题与外观

| ID | Task | Details |
|----|------|---------|
| D19 | `ThemeSetting` | 浅色 / 深色 / 跟随系统的切换 UI |
| D20 | 即时预览 | 切换即时生效（依赖 Phase 03 D16-D19） |

### 6.5 更新与通用

| ID | Task | Details |
|----|------|---------|
| D21 | `UpdateSetting` | 当前版本、检查更新、下载进度、安装、重启（依赖 Phase 08） |
| D22 | 开机自启 | 复现现有 autostart 开关（依赖 Phase 07 S03-5） |
| D23 | 数据目录入口 | 打开数据目录（现有能力） |
| D24 | 关于信息 | 版本、许可证声明（GPL 需要可访问的源码链接） |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T01 | 配置往返测试 | 写入 → 重启 → 读取，配置完整且格式与旧版本兼容 |
| T02 | 明文存储断言 | 断言 API Key 未写入系统钥匙串（硬约束 9） |
| T03 | 热键录制测试 | 各类组合键、非法输入、取消流程 |
| T04 | 尺寸不变测试 | 进出设置页窗口尺寸不变（硬约束 6） |
| T05 | 模型拉取测试 | 有效/无效 Key 两条路径 |
| T06 | 视觉回归 | 对照 v1 基线（`docs/evidence/v1-baseline/`，由 S01-06 采集） |

## 备注

`AddProviderPanel.tsx`（649 行）与 `ModelRow.tsx`（290 行）是设置页中最大的两个组件，且含较多表单交互，建议优先拆分子模块以符合 `docs/CONVENTIONS.md` 的「单文件 ≤300 行」规则。
