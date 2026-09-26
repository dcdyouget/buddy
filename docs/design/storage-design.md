# Storage Design

## Layout

数据目录由 `app.path().app_data_dir()` 决定，应用标识为 `com.buddy.chat`。

```text
{app_data_dir}/
├── config.json
├── manifest.json
├── chunk_001.json
├── chunk_002.json
├── attachments/
│   ├── upload-*.png
│   └── generated-*.png
└── ...
```

## Config

```json
{
  "theme": "light",
  "hotkey": "CmdOrCtrl+J",
  "providers": [],
  "models": [],
  "selected_model_id": "",
  "auto_start": false,
  "allowed_paths": [],
  "mcp_servers": []
}
```

Provider 内可含明文 `api_key` 与 `compat`；MCP 配置可含明文 `env`/`headers`。文件缺失或 JSON 损坏时返回默认配置并记录警告。写入为 pretty JSON，当前不是原子临时文件替换。

## Message Chunks

每个 chunk 最多 100 条消息，格式为 `{ id, messages }`；`manifest.json` 保存 `{ chunks: [{file,count}], total_messages }`。消息支持 user/assistant/tool、content blocks、tool calls/results 和 `parent_message_id`。

图片二进制不写入消息 JSON。用户上传和模型生成的图片都会保存到
`attachments/`，消息中的 `ImageAttachment` 只保存绝对路径、名称和 MIME 类型。
附件文件被外部删除后，历史消息仍保留原路径，前端显示“图片已删除”。

## Append

`append_message` 用进程内全局 mutex 串行化写入：

1. 读取 manifest。
2. 最后一个 chunk 已满则创建下一个编号。
3. 读取完整 chunk；追加消息；重写完整文件。
4. 更新 chunk count 与 `total_messages`；重写 manifest。

若 mutex poisoned 会恢复 guard 继续写。若已有 chunk JSON 损坏，当前实现记录警告并以空 chunk 重写该文件。

## Load

`load_messages(offset, limit)` 从最早消息开始计算 offset，根据 manifest 跨 chunk 读取，返回按时间正序排列的结果。缺失或损坏的 chunk 被跳过；manifest 缺失或损坏视为空历史。

前端启动默认调用 `loadMessages(0, 100)`，当前没有实现滚动向上继续分页；历史超过 100 条时只加载首批 100 条。

## Persistence Ownership

- `send_message`：后端保存新 user message、每轮 assistant message 和 tool result。
- `save_message`：保留给前端显式追加单条消息。
- 配置更新：`save_config` 保存完整快照，并触发热键重新注册。

## Current Limits

单会话、无删除/清空磁盘命令、无跨进程文件锁、无事务/原子替换、无加密。前端 `clearMessages()` 只清内存，不删除 chunk 文件。
