/**
 * types/index.ts — TypeScript 类型定义
 *
 * 与 Rust 后端的 models.rs / streaming.rs 数据模型一一对应。
 * 定义整个应用中使用的所有核心类型，包括：
 * - 主题类型 (Theme)
 * - 提供商配置 (ProviderConfig) + 提供商类型 (ProviderType)
 * - 模型信息 (ModelInfo)
 * - 消息结构 (Message) + 内容块 (ContentBlock)
 * - 流式事件 (StreamEvent)
 * - 页面状态 (PageState)
 * - 应用配置 (AppConfig)
 * - 提供商预设 (ProviderPreset + PROVIDER_PRESETS)
 *
 * 来源：docs/design/rust-data-models.md
 */

/** 主题类型：亮色 / 暗色 */
export type Theme = 'light' | 'dark';

/** 提供商类型：决定使用哪种 API 适配器 */
export type ProviderType = 'openai_compatible' | 'anthropic';

/** 兼容性配置：适配不同厂商的 API 差异 */
export interface CompatConfig {
  thinking_format?: string;              // 思考参数格式: openai/deepseek/openrouter/qwen/together/zai
  max_tokens_field?: string;             // max token 字段名: max_tokens/max_completion_tokens
  supports_stream_options_usage?: boolean; // 是否支持 stream_options.include_usage
  supports_reasoning_effort?: boolean;   // 是否支持 reasoning_effort
  supports_store?: boolean;              // 是否支持 store 字段
  supports_developer_role?: boolean;     // 是否支持 developer 角色
  supports_temperature?: boolean;        // 是否支持 temperature 参数
  supports_long_cache_retention?: boolean; // 是否支持长缓存保留
}

/** 服务提供商配置 */
export interface ProviderConfig {
  id: string;                    // 提供商标识
  name: string;                  // 展示名称，显示在 UI 中
  base_url: string;              // API 基础地址
  api_key: string;               // API 密钥
  enabled_model_ids: string[];   // 用户在此提供商下启用的模型 ID 列表
  provider_type: ProviderType;   // 提供商类型，决定 API 适配器
  /** 兼容性配置（可选），用于适配不同厂商的 API 差异 */
  compat?: CompatConfig;
}

/** 模型信息 */
export interface ModelInfo {
  id: string;                    // 模型标识
  provider_id: string;           // 所属提供商 ID
  display_name: string;          // 显示名称
  context_window: number;        // 上下文窗口大小（token 数）
  latency_ms: number | null;     // 测速延迟（毫秒），null 表示未测速
}

/** 消息角色：用户 / 助手 */
export type MessageRole = 'user' | 'assistant';

/** 内容块类型（对应 Rust ContentBlock） */
export type ContentBlock =
  | { type: 'text'; content: string }
  | { type: 'thinking'; content: string; is_open: boolean };

/** 聊天消息 */
export interface Message {
  id: string;                    // UUID v4 唯一标识
  role: MessageRole;             // 发送者角色
  content: string;               // 消息正文（兼容旧格式）
  /** 结构化内容块（v2.0 新增，用于区分文本和思考块） */
  blocks?: ContentBlock[];
  model_id: string | null;       // 使用的模型 ID（用户消息为 null）
  created_at: number;            // 创建时间，Unix 时间戳（秒）
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
  | { event: 'error'; reason: string; message: string; partial_text: string };

/** 页面状态枚举 — 驱动整个应用的页面路由 */
export type PageState =
  | 'empty'          // 空状态：尚未配置 provider 或模型
  | 'noapikey'       // 无 API Key：已选模型但未配置密钥
  | 'conversation'   // 对话页：展示消息列表
  | 'streaming'      // 流式生成中：实时显示 AI 回复
  | 'settings'       // 设置页：管理 provider、模型、快捷键
  | 'add-provider';  // 添加提供商子页面

/** 应用全局配置 — 对应 Rust 后端的 AppConfig */
export interface AppConfig {
  theme: Theme;                  // 主题
  hotkey: string;                // 全局快捷键
  providers: ProviderConfig[];   // 已配置的服务提供商列表
  models: ModelInfo[];           // 已知的模型信息列表
  selected_model_id: string;     // 当前选中的模型 ID
  auto_start: boolean;           // 是否开机自启
}

// ─── 提供商预设 ────────────────────────────────────────

/** 内置提供商预设结构 */
export interface ProviderPreset {
  id: string;             // 提供商标识
  name: string;           // 展示名称
  base_url: string;       // API 基础地址
  icon_letter: string;    // 提供商图标的首字母
  provider_type: ProviderType; // 提供商类型
  compat?: CompatConfig;  // 默认兼容性配置
}

/** 内置提供商预设列表（含 compat 默认值） */
export const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    id: 'deepseek', name: 'DeepSeek', base_url: 'https://api.deepseek.com/v1', icon_letter: 'D',
    provider_type: 'openai_compatible',
    compat: { thinking_format: 'deepseek', supports_store: false, supports_developer_role: false },
  },
  {
    id: 'minimax', name: 'MiniMax', base_url: 'https://api.minimaxi.com/v1', icon_letter: 'M',
    provider_type: 'openai_compatible',
    compat: { supports_stream_options_usage: false },
  },
  {
    id: 'glm', name: 'GLM (智谱)', base_url: 'https://open.bigmodel.cn/api/paas/v4', icon_letter: 'G',
    provider_type: 'openai_compatible',
    compat: { thinking_format: 'qwen', supports_stream_options_usage: false },
  },
  {
    id: 'kimi', name: 'Kimi (月之暗面)', base_url: 'https://api.moonshot.cn/v1', icon_letter: 'K',
    provider_type: 'openai_compatible',
    compat: { supports_store: false },
  },
  {
    id: 'mimo', name: 'MiMo (小米)', base_url: 'https://api.xiaomimimo.com/v1', icon_letter: 'X',
    provider_type: 'openai_compatible',
    compat: { thinking_format: 'deepseek', supports_store: false },
  },
  {
    id: 'anthropic', name: 'Anthropic', base_url: 'https://api.anthropic.com', icon_letter: 'A',
    provider_type: 'anthropic',
    compat: { supports_temperature: true, supports_long_cache_retention: true },
  },
];
