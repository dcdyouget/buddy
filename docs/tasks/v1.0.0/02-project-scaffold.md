# Task 02: Project Scaffold

## 目标

初始化 Tauri 2 + React + TypeScript 项目骨架，配置所有依赖和工具链。

## 相关设计文档

- `docs/design/overview.md` — 项目结构、技术栈、硬约束

## 验收标准

- [ ] `npm run tauri dev` 可启动空白窗口
- [ ] 项目结构符合 `docs/design/overview.md` 中的 layout
- [ ] Tailwind CSS v4 可正常使用，design token 配置完成
- [ ] 所有前端依赖安装完成且无报错
- [ ] 所有 Rust 依赖编译通过
- [ ] Tauri 窗口配置为无边框 + 透明 + 置顶

## 开发工作

| ID | Task | Details |
|----|------|---------|
| D01 | Init Tauri 2 + React + TS | `npm create tauri-app@latest`, select React + TS + Vite |
| D02 | Configure Tailwind CSS v4 | Import design tokens from `buddy-design/colors_and_type.css` as Tailwind theme |
| D03 | Install frontend deps | `framer-motion`, `zustand`, `lucide-react`, `react-markdown`, `prism-react-renderer` |
| D04 | Configure Tauri window | Edit `tauri.conf.json`: `decorations: false`, `transparent: true`, `alwaysOnTop: true`, `visibleOnAllWorkspaces: true`, `skipTaskbar: true` |
| D05 | Add Rust deps | Add to `Cargo.toml`: `reqwest`, `serde`, `serde_json`, `tokio`, `global-hotkey`, `tauri-plugin-autostart`, `uuid`, `chrono` |
| D06 | Create file structure | Create all directories and empty files per `docs/design/overview.md` |

## 测试工作

本任务为脚手架搭建。验收标准通过手动验证：

- `npm run tauri dev` 成功启动
- Rust 编译无警告
- Frontend HMR 正常
