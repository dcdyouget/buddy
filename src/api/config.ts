/**
 * config.ts — 配置相关 API 封装
 *
 * 封装 Tauri invoke 调用，统一处理浏览器/Tauri 环境切换。
 */

import { isBrowser } from '@/utils/mock';
import type { AppConfig } from '@/types';

async function invokeBackend<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isBrowser) return undefined as T;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

/** 从磁盘读取应用配置 */
export async function getConfig(): Promise<AppConfig> {
  return invokeBackend<AppConfig>('get_config');
}

/** 保存应用配置到磁盘 */
export async function saveConfig(config: AppConfig): Promise<void> {
  await invokeBackend('save_config', { config });
}
