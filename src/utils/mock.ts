/**
 * mock.ts — 浏览器模式 Mock 数据与工具函数
 *
 * 当应用在浏览器中运行时（非 Tauri 环境），Tauri API 不可用，
 * 需要提供 mock 数据来支撑前端独立开发和调试。
 *
 * 主要提供：
 * - isBrowser：检测当前是否运行在浏览器中
 * - MOCK_CONFIG：模拟的全局配置（含 DeepSeek 预设）
 * - MOCK_MESSAGES：模拟的对话历史（含代码和文字回复）
 */

import type { AppConfig, Message } from '@/types';

/**
 * 判断当前是否运行在普通浏览器中（非 Tauri 环境）
 * Tauri 会在 window 上注入 __TAURI_INTERNALS__ 对象
 */
export const isBrowser = typeof window !== 'undefined' && !(window as any).__TAURI_INTERNALS__;

/**
 * Mock 全局配置
 * 包含预配置的 DeepSeek 提供商和两个模型，方便在浏览器模式下
 * 直接看到完整的 UI 效果而无需实际 API Key
 */
export const MOCK_CONFIG: AppConfig = {
  theme: 'light',
  hotkey: 'CmdOrCtrl+J',            // 默认快捷键：macOS 为 Cmd+J，Windows 为 Ctrl+J
  selected_model_id: 'deepseek-chat', // 默认选中聊天模型
  auto_start: false,                 // 默认不开机自启
  providers: [
    {
      id: 'deepseek',
      name: 'DeepSeek',
      base_url: 'https://api.deepseek.com/v1',
      api_key: 'sk-mock-xxxxxxxxxxxxxxxx', // 模拟 API Key
      enabled_model_ids: ['deepseek-chat'],
    },
  ],
  models: [
    {
      id: 'deepseek-chat',
      provider_id: 'deepseek',
      display_name: 'DeepSeek-Chat',
      context_window: 128000,
      latency_ms: 320, // 模拟延迟
    },
    {
      id: 'deepseek-reasoner',
      provider_id: 'deepseek',
      display_name: 'DeepSeek-Reasoner',
      context_window: 64000,
      latency_ms: 580,
    },
  ],
};

/**
 * Mock 对话历史
 * 包含四轮对话（两问两答），展示代码块和富文本回复效果
 */
export const MOCK_MESSAGES: Message[] = [
  {
    id: 'msg-1',
    role: 'user',
    content: '你好，请用 Python 写一个快速排序',
    model_id: null, // 用户消息不关联模型
    created_at: Math.floor(Date.now() / 1000) - 120, // 2 分钟前
  },
  {
    id: 'msg-2',
    role: 'assistant',
    content: `好的，这是一个 Python 快速排序实现：

\`\`\`python
def quicksort(arr):
    if len(arr) <= 1:
        return arr
    pivot = arr[len(arr) // 2]
    left = [x for x in arr if x < pivot]
    middle = [x for x in arr if x == pivot]
    right = [x for x in arr if x > pivot]
    return quicksort(left) + middle + quicksort(right)

# 示例
print(quicksort([3, 6, 8, 10, 1, 2, 1]))
\`\`\`

时间复杂度 **O(n log n)**，最坏情况 O(n²)。`,
    model_id: 'deepseek-chat',
    created_at: Math.floor(Date.now() / 1000) - 110,
  },
  {
    id: 'msg-3',
    role: 'user',
    content: '能再解释一下时间复杂度吗？',
    model_id: null,
    created_at: Math.floor(Date.now() / 1000) - 60, // 1 分钟前
  },
  {
    id: 'msg-4',
    role: 'assistant',
    content: `当然！快速排序的时间复杂度取决于 **pivot 的选择**：

- **最好情况 O(n log n)**：每次 pivot 都恰好把数组分成两半，递归深度 log n
- **平均情况 O(n log n)**：随机 pivot 的期望性能
- **最坏情况 O(n²)**：每次 pivot 都是最大/最小值（如已排序数组取第一个元素）

实际使用中通常随机选 pivot 或用三数取中法来避免最坏情况。`,
    model_id: 'deepseek-chat',
    created_at: Math.floor(Date.now() / 1000) - 50,
  },
];
