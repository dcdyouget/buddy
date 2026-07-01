/**
 * chat.ts — 聊天相关 API 封装
 *
 * 封装 Tauri invoke 调用，统一处理浏览器/Tauri 环境切换。
 * 浏览器模式下为 no-op，Tauri 环境下动态导入 invoke 函数。
 */

import { isBrowser } from '@/utils/mock';
import type { Message } from '@/types';

async function invokeBackend<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isBrowser) return undefined as T;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

/** 发起流式 AI 对话请求 */
export async function sendMessage(messages: Message[], modelId: string): Promise<void> {
  await invokeBackend('send_message', { messages, modelId });
}

/** 停止当前正在进行的流式生成 */
export async function stopGeneration(): Promise<void> {
  await invokeBackend('stop_generation');
}
