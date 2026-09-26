# Architecture Overview

> 本文描述当前实现；历史任务拆分见 `docs/tasks/v1.0.0/`。

## Project

Buddy 是 macOS / Windows 跨平台 AI 聊天工具：全局快捷键唤起无边框窗口，失焦或按 Esc 隐藏窗口，后台流式生成不因此停止。

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop | Tauri 2 / Rust |
| Frontend | React 18 / TypeScript / Vite 8 |
| UI | Tailwind CSS v4 / CSS variables / Framer Motion / lucide-react |
| State | Zustand 5 |
| HTTP & streaming | reqwest / tokio / SSE |
| Providers | OpenAI-compatible / Anthropic |
| Storage | Local JSON chunks |

## Runtime Architecture

```text
React pages/components
  ├─ configStore ── api/config.ts ───────┐
  ├─ chatStore ─── api/chat|storage.ts ──┼─ Tauri invoke / stream-event
  └─ uiStore + useStreaming              │
                                         ▼
commands.rs ── providers/ ── streaming.rs ── Provider API
     ├────── tools/ ── approval / ask_user
     ├────── storage.rs ── config + message chunks
     └────── hotkey / tray / window / platform
```

后端把不同 Provider 的响应统一为 `stream-event`。`send_message` 内部可执行多轮“模型 → tool → 结果 → 模型”循环；前端只维护一条会话流。

## Project Structure

```text
src/
├── api/                 # invoke 封装
├── components/
│   ├── chat/
│   ├── settings/
│   └── shared/
├── hooks/               # 流事件、平滑渲染、窗口拖动
├── pages/               # Empty / NoApiKey / Chat / Settings
├── stores/              # chat / config / ui
├── styles/
├── types/
└── utils/

src-tauri/src/
├── commands.rs
├── hotkey.rs
├── lib.rs
├── mcp/
├── models/              # config / message / storage / mcp / model_context
├── platform/            # macOS / Windows
├── providers/           # OpenAI-compatible / Anthropic
├── storage.rs
├── streaming.rs
├── tools/               # 内置工具与注册表
├── tray.rs
└── window/              # events / positioning
```

## Implemented Decisions

| Decision | Current behavior |
|---|---|
| 单会话 | UI 无会话列表；消息按时间顺序分块持久化 |
| Provider 适配 | `provider_type` 选择 OpenAI-compatible 或 Anthropic；`compat` 控制厂商差异 |
| 上下文裁剪 | 每轮按模型 `context_window` 的 70% 预算保留最近消息 |
| 工具调用 | 内置工具顺序执行；写操作需审批；最多 20 轮，连续 3 轮全失败则停止 |
| MCP | 配置模型与模块已存在，但当前 `send_message` 尚未把 MCP tools 注入注册表 |
| 窗口 | 初始 460×78，可调整；气泡可通过上箭头展开；compact 页首次进入内容页会展开；快捷键默认 `CmdOrCtrl+J` |
| 配置 | API Key、MCP headers/env 以明文 JSON 保存 |

## Hard Constraints

无系统窗口按钮；品牌色为 `#5B5FE9`；颜色、阴影、间距优先使用设计 token；图标只用 lucide-react；UI 文案为中文；Esc/失焦只隐藏窗口，不取消生成。

注意：项目原始约束写明“页面切换不改变窗口尺寸”，但当前 `windowResize.ts` 会在 `empty/noapikey → conversation/streaming/settings` 时设置预设尺寸。本文按实现记录，该行为是否保留需产品确认。
