# Rust Data Models

> **部分退役（RULES §7.2）**
> 已实现：S02-03（`models/` 与 `streaming.rs` 已迁入 `crates/engine/src/`，与 v1 逐字节一致，仅 emitter 改为 channel）。
> 未实现：S02-02（tools）、S02-01（providers）—— S02-01 完成后整份删除。

> Rust 源：`src-tauri/src/models/`、`src-tauri/src/streaming.rs`；前端镜像：`src/types/index.ts`。

## AppConfig

```rust
pub struct AppConfig {
    pub theme: Theme,                    // light | dark
    pub hotkey: String,                  // 默认 CmdOrCtrl+J
    pub providers: Vec<ProviderConfig>,
    pub models: Vec<ModelInfo>,
    pub selected_model_id: String,
    pub auto_start: bool,
    pub allowed_paths: Vec<String>,      // write tool 路径白名单；空数组=不限制
    pub mcp_servers: Vec<McpServerConfig>,
}
```

新增字段使用 `serde(default)` 兼容旧配置。保存时若 `selected_model_id` 非空，必须能在 `models` 中找到。

## ProviderConfig

```rust
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub enabled_model_ids: Vec<String>,
    pub provider_type: String,           // openai_compatible | anthropic
    pub compat: Option<CompatConfig>,
}
```

`CompatConfig` 当前包含：

- `thinking_format`
- `max_tokens_field`
- `supports_stream_options_usage`
- `supports_reasoning_effort`
- `supports_store`
- `supports_developer_role`
- `supports_temperature`
- `supports_tools`

缺省 `provider_type` 为 `openai_compatible`。前端还声明了 `supports_long_cache_retention`，Rust 当前未持久化该字段。

## Message and Content

```rust
pub struct Message {
    pub id: String,
    pub role: MessageRole,               // user | assistant | tool
    pub content: String,
    pub images: Vec<ImageAttachment>,           // user=输入图片，tool=生成结果图片
    pub blocks: Option<Vec<ContentBlock>>,
    pub model_id: Option<String>,
    pub created_at: u64,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub is_error: Option<bool>,
    pub parent_message_id: Option<String>,
}

pub struct ImageAttachment {
    pub id: String,
    pub name: String,
    pub media_type: String,
    pub path: String,       // 持久化字段，指向 app_data_dir/attachments
    pub data_url: String,   // 仅导入、Provider 请求和旧记录迁移时临时使用
}

pub enum ContentBlock {
    Text { content: String },
    Thinking { content: String, is_open: bool },
}

pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,               // 原始 JSON 字符串
}
```

`parent_message_id` 只影响前端嵌套显示，消息仍作为独立 user message 进入模型上下文。前端的 tool `status/result/insertAfterBlockIndex` 是运行时展示字段，不属于 Rust 持久化模型。

## ModelInfo

`id`、`api_model_id: Option<String>`、`provider_id`、`display_name`、`context_window: u32`、`latency_ms: Option<u32>`、`supports_vision: bool`、`supports_image_generation: bool`。配置中的 `id` 为 `provider_id::原始模型ID`，避免跨服务同名模型冲突；`api_model_id` 显式保存原始 API 模型名，`raw_model_id` 优先读取该字段，缺失时将完整的旧 `id` 作为原始模型名，不按分隔符猜测。旧配置通过 `normalize_model_ids` 幂等迁移，同时更新启用列表和默认选择，保留密钥和模型能力。可选原始模型名和两个能力字段均使用 `serde(default)` 兼容旧配置；生图工具仅对 OpenAI-compatible Provider 生效。未知模型的上下文窗口由后端映射逻辑回退为 128000。

## MCP

`McpServerConfig` 包含 `id/name/enabled/transport`，stdio 的 `command/args/env`，SSE 的 `url/headers`，以及 `timeout_secs`（默认 30）和 `auto_reconnect`（默认 true）。传输类型为 `stdio | sse`。

## Storage

```rust
pub struct Manifest {
    pub chunks: Vec<ChunkMeta>,
    pub total_messages: u64,
}
pub struct ChunkMeta { pub file: String, pub count: u32 }
pub struct ChatChunk { pub id: String, pub messages: Vec<Message> }
```

## Wire Rules

- serde 字段默认 snake_case；`QuestionOption` 单独使用 camelCase。
- 时间戳为 Unix 秒。
- user message ID 由前端生成；后端持久化的多轮 assistant/tool message 使用时间戳组合 ID，因此并非所有 ID 都是 UUID。
- `blocks`、tool 字段与 `parent_message_id` 均兼容旧消息缺失。
