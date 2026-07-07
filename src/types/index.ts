/**
 * types/index.ts — TypeScript 类型定义
 *
 * 所有前端与 Rust 后端共享的数据类型。
 * 注意：这里定义的 TS 类型需要与 Rust struct 对齐，
 * 包括序列化字段名（用 snake_case）和字段结构。
 *
 * P8 新增：Tool 协议相关类型 (ToolCall, tool StreamEvent variants, MCP server config)
 */

/** 主题类型：亮色 / 暗色 */
export type Theme = 'light' | 'dark';

/** 提供商类型：决定使用哪种 API 适配器 */
export type ProviderType = 'openai_compatible' | 'anthropic';

/** 兼容性配置：适配不同厂商的 API 差异 */
export interface CompatConfig {
  thinking_format?: string;
  max_tokens_field?: string;
  supports_stream_options_usage?: boolean;
  supports_reasoning_effort?: boolean;
  supports_store?: boolean;
  supports_developer_role?: boolean;
  supports_temperature?: boolean;
  supports_long_cache_retention?: boolean;
  /** P8: 部分厂商不支持 tool calling，可设为 false */
  supports_tools?: boolean;
}

/** 服务提供商配置 */
export interface ProviderConfig {
  id: string;
  name: string;
  base_url: string;
  api_key: string;
  enabled_model_ids: string[];
  provider_type: ProviderType;
  compat?: CompatConfig;
}

/** 模型信息 */
export interface ModelInfo {
  id: string;
  provider_id: string;
  display_name: string;
  context_window: number;
  latency_ms: number | null;
}

/** 消息角色：用户 / 助手 / 工具 */
export type MessageRole = 'user' | 'assistant' | 'tool';

/** 内容块类型（对应 Rust ContentBlock） */
export type ContentBlock =
  | { type: 'text'; content: string }
  | { type: 'thinking'; content: string; is_open: boolean };

/** 工具调用（对应 Rust ToolCall） */
export interface ToolCall {
  id: string;
  name: string;
  arguments: string; // JSON string
}

/** MCP server 传输方式 */
export type McpTransport = 'stdio' | 'sse';

/** MCP server 配置（对应 Rust McpServerConfig） */
export interface McpServerConfig {
  id: string;
  name: string;
  enabled: boolean;
  transport: McpTransport;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  headers?: Record<string, string>;
  timeout_secs: number;
  auto_reconnect: boolean;
}

/** 聊天消息 */
export interface Message {
  id: string;
  role: MessageRole;
  content: string;
  blocks?: ContentBlock[];
  model_id: string | null;
  created_at: number;
  /** 工具调用（assistant 消息时） */
  tool_calls?: ToolCall[];
  /** 关联的工具调用 ID（tool 消息时） */
  tool_call_id?: string;
  /** 工具名（tool 消息时，调试/显示用） */
  tool_name?: string;
  /** 工具执行是否出错（tool 消息时） */
  is_error?: boolean;
}

/** 流式事件类型（对应 Rust StreamEvent） */
export type StreamEvent =
  | { event: 'start' }
  | { event: 'text_start'; content_index: number }
  | { event: 'text_delta'; content_index: number; delta: string }
  | { event: 'text_end'; content_index: number; content: string }
  | { event: 'thinking_start'; content_index: number }
  | { event: 'thinking_delta'; content_index: number; delta: string }
  | { event: 'thinking_end'; content_index: number; content: string }
  | { event: 'done'; reason: string; full_text: string }
  | { event: 'error'; reason: string; message: string; partial_text: string }
  // P4 Tool 协议事件
  | { event: 'tool_call_start'; id: string; name: string; content_index: number }
  | { event: 'tool_call_delta'; id: string; arguments_delta: string }
  | { event: 'tool_call_end'; id: string; name: string; arguments: string }
  | { event: 'tool_executing'; id: string; name: string }
  | { event: 'tool_result'; id: string; name: string; content: string; is_error: boolean }
  | { event: 'tool_approval_required'; id: string; name: string; arguments: string; reason: string }
  | { event: 'turn_end'; tool_calls_pending: number };

/** 页面状态枚举 — 驱动整个应用的页面路由 */
export type PageState =
  | 'empty'
  | 'noapikey'
  | 'conversation'
  | 'streaming'
  | 'settings'
  | 'add-provider';

/** 应用全局配置 — 对应 Rust 后端的 AppConfig */
export interface AppConfig {
  theme: Theme;
  hotkey: string;
  providers: ProviderConfig[];
  models: ModelInfo[];
  selected_model_id: string;
  auto_start: boolean;
  /** write 类 tool 可写的路径白名单（空 = 不限制） */
  allowed_paths: string[];
  /** MCP server 配置列表 */
  mcp_servers: McpServerConfig[];
}

// ─── 提供商预设 ────────────────────────────────────────

/** 内置提供商预设结构 */
export interface ProviderPreset {
  id: string;
  name: string;
  base_url: string;
  provider_type: ProviderType;
  compat?: CompatConfig;
}

/** 内置提供商预设列表（含 compat 默认值） */
export const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    id: 'deepseek',
    name: 'DeepSeek',
    base_url: 'https://api.deepseek.com',
    provider_type: 'openai_compatible',
    compat: { thinking_format: 'deepseek' },
  },
  {
    id: 'openai',
    name: 'OpenAI',
    base_url: 'https://api.openai.com/v1',
    provider_type: 'openai_compatible',
    compat: { max_tokens_field: 'max_completion_tokens' },
  },
  {
    id: 'anthropic',
    name: 'Anthropic',
    base_url: 'https://api.anthropic.com',
    provider_type: 'anthropic',
    compat: {},
  },
  {
    id: 'openrouter',
    name: 'OpenRouter',
    base_url: 'https://openrouter.ai/api/v1',
    provider_type: 'openai_compatible',
    compat: { thinking_format: 'openrouter' },
  },
  {
    id: 'glm',
    name: 'GLM / 智谱',
    base_url: 'https://open.bigmodel.cn/api/paas/v4',
    provider_type: 'openai_compatible',
    compat: {},
  },
  {
    id: 'minimax',
    name: 'MiniMax',
    base_url: 'https://api.minimax.chat/v1',
    provider_type: 'openai_compatible',
    compat: {},
  },
  {
    id: 'moonshot',
    name: 'Moonshot / 月之暗面',
    base_url: 'https://api.moonshot.cn/v1',
    provider_type: 'openai_compatible',
    compat: {},
  },
  {
    id: 'qwen',
    name: 'Qwen / 通义千问',
    base_url: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    provider_type: 'openai_compatible',
    compat: { thinking_format: 'qwen' },
  },
  {
    id: 'zhipu',
    name: 'Zhipu / 智谱',
    base_url: 'https://open.bigmodel.cn/api/paas/v4',
    provider_type: 'openai_compatible',
    compat: {},
  },
];
