/**
 * storage.ts — 消息存储相关 API 封装
 *
 * 封装 Tauri invoke 调用，统一处理浏览器/Tauri 环境切换。
 */

import { isBrowser } from '@/utils/mock';
import type { Message } from '@/types';

async function invokeBackend<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isBrowser) return undefined as T;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

/** 分页加载历史消息 */
export async function loadMessages(offset: number = 0, limit: number = 100): Promise<Message[]> {
  return invokeBackend<Message[]>('load_messages', { offset, limit });
}

/** 获取已持久化消息总数，用于从末尾分页加载。 */
export async function getMessageCount(): Promise<number> {
  return invokeBackend<number>('get_message_count');
}

/** 将单条消息持久化到本地存储 */
export async function saveMessage(message: Message): Promise<void> {
  await invokeBackend('save_message', { message });
}
