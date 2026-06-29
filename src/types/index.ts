/**
 * types/index.ts — TypeScript 类型定义
 *
 * 与 Rust 后端的 models.rs 数据模型一一对应。
 * 定义整个应用中使用的所有核心类型，包括：
 * - 主题类型 (Theme)
 * - 提供商配置 (ProviderConfig)
 * - 模型信息 (ModelInfo)
 * - 消息结构 (Message)
 * - 页面状态 (PageState)
 * - 应用配置 (AppConfig)
 * - 提供商预设 (ProviderPreset + PROVIDER_PRESETS)
 *
 * 来源：docs/design/rust-data-models.md
 */

/** 主题类型：亮色 / 暗色 */
export type Theme = 'light' | 'dark';

/** 服务提供商配置 */
export interface ProviderConfig {
  id: string;                    // 提供商标识，如 "deepseek" | "minimax" | "glm" | "kimi" | "custom"
  name: string;                  // 展示名称，显示在 UI 中
  base_url: string;              // API 基础地址，如 "https://api.deepseek.com/v1"
  api_key: string;               // API 密钥，明文存储于 JSON 文件（v1.0.0）
  enabled_model_ids: string[];   // 用户在此提供商下启用的模型 ID 列表
}

/** 模型信息 */
export interface ModelInfo {
  id: string;                    // 模型标识，如 "deepseek-chat"
  provider_id: string;           // 所属提供商 ID，如 "deepseek"
  display_name: string;          // 显示名称，如 "DeepSeek-Chat"
  context_window: number;        // 上下文窗口大小（token 数），如 128000
  latency_ms: number | null;     // 测速延迟（毫秒），null 表示未测速
}

/** 消息角色：用户 / 助手 */
export type MessageRole = 'user' | 'assistant';

/** 聊天消息 */
export interface Message {
  id: string;                    // UUID v4 唯一标识
  role: MessageRole;             // 发送者角色
  content: string;               // 消息正文
  model_id: string | null;       // 使用的模型 ID（用户消息为 null）
  created_at: number;            // 创建时间，Unix 时间戳（秒）
}

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
  hotkey: string;                // 全局快捷键，如 "CmdOrCtrl+J"
  providers: ProviderConfig[];   // 已配置的服务提供商列表
  models: ModelInfo[];           // 已知的模型信息列表
  selected_model_id: string;     // 当前选中的模型 ID
  auto_start: boolean;           // 是否开机自启
}

// ─── 提供商预设 ────────────────────────────────────────

/** 内置提供商预设结构 */
export interface ProviderPreset {
  id: string;          // 提供商标识
  name: string;        // 展示名称
  base_url: string;    // API 基础地址
  icon_letter: string; // 提供商图标的首字母（用于圆形头像）
}

/** 内置提供商预设列表
 *  用户添加提供商时可以直接选择这些预设，无需手动填写 API 地址 */
export const PROVIDER_PRESETS: ProviderPreset[] = [
  { id: 'deepseek', name: 'DeepSeek', base_url: 'https://api.deepseek.com/v1', icon_letter: 'D' },
  { id: 'minimax', name: 'MiniMax', base_url: 'https://api.minimaxi.com/v1', icon_letter: 'M' },
  { id: 'glm', name: 'GLM (智谱)', base_url: 'https://open.bigmodel.cn/api/paas/v4', icon_letter: 'G' },
  { id: 'kimi', name: 'Kimi (月之暗面)', base_url: 'https://api.moonshot.cn/v1', icon_letter: 'K' },
];
