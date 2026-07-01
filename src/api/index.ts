/**
 * api/index.ts — API 层统一导出
 *
 * 所有 Tauri invoke 调用都应通过此模块进行，
 * 不要在 stores/components/hooks 中直接调用 invoke。
 */

export { sendMessage, stopGeneration } from './chat';
export { getConfig, saveConfig } from './config';
export { loadMessages, saveMessage } from './storage';
export { fetchModels, testLatency } from './provider';
