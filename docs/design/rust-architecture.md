# Rust Backend Architecture

> 实现入口：`src-tauri/src/lib.rs`

## Module Map

```text
lib.rs
├── commands.rs
├── providers/
│   ├── mod.rs
│   ├── openai_compatible.rs
│   ├── openai_composite_safe.rs
│   └── anthropic.rs
├── streaming.rs
├── tools/
│   ├── mod.rs
│   ├── builtin.rs
│   ├── image_generation.rs
│   └── websearch/
│       ├── mod.rs
│       ├── duckduckgo.rs
│       └── web_fetch.rs
├── mcp/
├── models/
│   ├── config.rs
│   ├── message.rs
│   ├── storage.rs
│   ├── mcp.rs
│   └── model_context.rs
├── storage.rs
├── hotkey.rs
├── tray.rs
├── window/
│   ├── events.rs
│   └── positioning.rs
└── platform/
    ├── macos.rs
    └── windows.rs
```

## Responsibilities

- `lib.rs`：注册 log、autostart、shell、global-shortcut 插件，初始化取消/审批/提问/热键状态，创建托盘并注册 IPC。日志同时输出到终端和系统应用日志目录，使用本地时区，单文件上限 5 MB，最多保留 3 份。
- `commands.rs`：配置、模型、历史、流式对话、停止生成、工具审批和用户提问命令。`send_message` 同时负责上下文裁剪、tool loop 与消息持久化。
- `providers/`：通过 `LlmProvider` 统一 OpenAI-compatible 与 Anthropic 的模型列表、测速和流式聊天。
- `streaming.rs`：定义 `ContentBlock`、统一 `StreamEvent` 与 `StreamEventEmitter`。
- `tools/`：工具定义、注册、执行策略；内置 `websearch` 聚合 Bing 中国与 DuckDuckGo 并读取排名靠前的网页；`generate_image` 仅为已开启生图能力的 OpenAI-compatible 模型注册，通用 Provider 使用 `/images/generations`，MiniMax 使用原生 `/image_generation` 与 `image-01`。工具图片与回传模型的文本结果分离。
- `mcp/`：MCP 客户端相关模块；配置结构已经接入 `AppConfig`，但对话注册表尚未注入 MCP tools。
- `storage.rs`：配置文件与每 100 条一个分块的消息存储；追加写使用进程内互斥锁。
- `hotkey.rs`：注册、注销和运行时更新全局快捷键。
- `tray.rs`、`window/`、`platform/`：托盘菜单、失焦隐藏、按光标所在屏幕定位、平台窗口效果。托盘“设置…”通过事件打开前端设置页，“开机自启”调用 autostart 插件并同步配置，“退出”结束应用。

## Managed State

| State | Purpose |
|---|---|
| `CancelState` | 当前生成的 `watch::Sender<bool>` |
| `ApprovalState` | 待审批 write tool + 本轮全部允许标记 |
| `QuestionState` | `ask_user` 等待回答的 oneshot |
| `HotkeyState` | 当前注册快捷键 |
| `SavedWindowPositions` | 窗口位置记录 |

## Registered Commands

`send_message`、`stop_generation`、`approve_tool_call`、`answer_tool_question`、`get_config`、`save_config`、`fetch_models`、`test_latency`、`load_messages`、`save_message`。

## Window Configuration

初始窗口为 460×78、无装饰、透明、可调整尺寸、最小 360×60、跨工作区、跳过任务栏；`alwaysOnTop=false`、`focus=false`。macOS 最低版本 12，并启用 private API 与毛玻璃效果。

## Main Dependencies

Tauri 2、tauri-plugin-log、reqwest 0.12、dom_query、tokio、serde、chrono、async-trait、thiserror、parking_lot、arboard、window-vibrancy，以及 macOS 窗口相关 objc2/core-graphics。
