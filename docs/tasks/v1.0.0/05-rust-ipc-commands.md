# Task 05: Rust IPC Commands

## 目标

实现所有 `#[tauri::command]` 处理函数，连接前端调用到 Rust 后端模块。

## 相关设计文档

- `docs/design/ipc-contract.md` — 完整 invoke/listen 契约
- `docs/design/rust-data-models.md` — 数据结构
- `docs/design/sse-and-api.md` — API 调用方式

## 验收标准

- [ ] `send_message` 参数校验通过后启动流式请求，错误/取消正确处理
- [ ] `stop_generation` 发送 cancel 信号到正在运行的流
- [ ] `get_config` 返回磁盘上的 `AppConfig`
- [ ] `save_config` 校验并持久化配置
- [ ] `fetch_models` 调用 API 并返回模型列表
- [ ] `test_latency` 调用 API 并返回延迟
- [ ] `load_messages` 正确分页加载历史消息
- [ ] 所有命令返回 `Result<T, String>`，String 为用户可读中文错误

## 开发工作

| ID | Task | Details |
|----|------|---------|
| D14 | `send_message` | 接收 messages + modelId → 查 provider 信息 → 调用 api::stream_chat |
| D15 | `stop_generation` | 向当前活跃的 cancel watch channel 发信号 |
| D16 | `get_config` | 调用 storage::get_config |
| D17 | `save_config` | 校验 config → storage::save_config |
| D18 | `fetch_models` | 调用 api::fetch_models |
| D19 | `test_latency` | 调用 api::test_latency |
| D20 | `load_messages` | offset + limit → storage::load_messages |

`send_message` 需要管理 cancel channel 的生命周期——用 `OnceLock<watch::Sender<bool>>` 或 `Mutex<Option<...>>` 存储在 app state 中。

## 测试工作

本任务的测试通过 Task 10-11（conversation/streaming E2E）间接覆盖。IPC 命令的正确性体现在后续任务中。
