/**
 * chat.ts — 聊天相关 API 封装
 *
 * 封装 Tauri invoke 调用，统一处理浏览器/Tauri 环境切换。
 * 浏览器模式下为 no-op，Tauri 环境下动态导入 invoke 函数。
 */

import { isBrowser } from '@/utils/mock';
import type { ImageAttachment, Message } from '@/types';

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

/** 将图片复制到应用数据目录，返回只包含本地路径的附件。 */
export async function saveChatImage(
  image: ImageAttachment,
): Promise<ImageAttachment> {
  // 后端 save_chat_image 要求 data_url 必填；类型上可选是因为持久化后的附件
  // 只有 path 没有 data_url。这里显式校验，避免把 undefined 传过去。
  if (!image.data_url) {
    throw new Error(`图片缺少数据内容: ${image.name}`);
  }
  return invokeBackend('save_chat_image', {
    name: image.name,
    mediaType: image.media_type,
    dataUrl: image.data_url,
  });
}

/** 删除已保存的聊天图片附件（移除未发送的输入框图片时清理孤儿文件）。 */
export async function deleteChatImage(path: string): Promise<boolean> {
  return invokeBackend('delete_chat_image', { path });
}

/**
 * 回答 ask_user tool 的提问
 * @param id  tool_call.id
 * @param selected 选中的选项索引(单选/多选)
 * @param inputs   对应 selected 中每个选项的补充输入(可选)
 * @param custom   用户输入的自定义回答(可选)
 */
export async function answerToolQuestion(args: {
  id: string;
  selected: number[];
  inputs?: string[];
  custom: string | null;
}): Promise<void> {
  await invokeBackend('answer_tool_question', args);
}
