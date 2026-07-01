/**
 * provider.ts — 服务提供商相关 API 封装
 *
 * 封装 Tauri invoke 调用，统一处理浏览器/Tauri 环境切换。
 */

import { isBrowser } from '@/utils/mock';
import type { ModelInfo } from '@/types';

async function invokeBackend<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isBrowser) return undefined as T;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

/** 从提供商获取可用模型列表 */
export async function fetchModels(
  baseUrl: string,
  apiKey: string,
  providerType: string,
): Promise<ModelInfo[]> {
  return invokeBackend<ModelInfo[]>('fetch_models', {
    baseUrl,
    apiKey,
    providerType,
  });
}

/** 测试指定模型的 API 延迟（毫秒） */
export async function testLatency(
  baseUrl: string,
  apiKey: string,
  modelId: string,
  providerType: string,
): Promise<number> {
  return invokeBackend<number>('test_latency', {
    baseUrl,
    apiKey,
    modelId,
    providerType,
  });
}
