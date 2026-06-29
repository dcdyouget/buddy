/**
 * chatStore.ts — 聊天状态管理
 *
 * 管理聊天会话的核心状态，包括：
 * - 消息数组（messages）：完整的对话历史
 * - 流式生成状态（isStreaming、streamingTokens）
 * - 输入草稿（draftInput）：用户正在编辑但尚未发送的文本
 * - 错误处理（error）
 *
 * 消息发送流程：
 * 1. sendMessage() 创建 user + assistant 消息，设置 isStreaming = true
 * 2. Rust 后端通过 SSE 逐个推送 token
 * 3. useStreaming hook 监听 stream-token 事件 → 调用 appendToken()
 * 4. 流式完成后调用 finalizeMessage() 收尾
 * 5. 用户可随时调用 stopGeneration() 中断生成
 *
 * 浏览器模式下使用 setInterval 模拟流式效果，方便前端调试。
 */

import { create } from 'zustand';
import type { Message } from '@/types';
import { isBrowser, MOCK_MESSAGES } from '@/utils/mock';

/** ChatStore 状态和操作定义 */
interface ChatState {
  messages: Message[];          // 消息列表（按时间排序）
  draftInput: string;           // 输入框草稿文本
  isStreaming: boolean;         // 是否正在流式生成中
  streamingTokens: number;      // 当前流式生成已接收的 token 数
  streamingModelId: string | null; // 当前正在生成的模型 ID
  error: string | null;         // 最近的错误信息

  // ── 操作 ──
  setDraftInput: (text: string) => void;                        // 设置输入草稿
  sendMessage: (content: string, modelId: string) => Promise<void>; // 发送消息
  stopGeneration: () => Promise<void>;                          // 停止生成
  appendToken: (token: string) => void;                         // 追加 token（流式）
  finalizeMessage: () => void;                                  // 完成流式消息
  saveMessage: (message: Message) => Promise<void>;             // 持久化单条消息
  loadMessages: (offset?: number, limit?: number) => Promise<void>; // 加载历史消息
  clearMessages: () => void;                                    // 清空消息
  setMessages: (messages: Message[]) => void;                   // 设置完整消息列表
  setError: (error: string | null) => void;                     // 设置错误信息
}

/**
 * 生成 UUID v4 格式的唯一 ID
 * 用于给每条消息分配唯一标识
 */
function generateId(): string {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === 'x' ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

export const useChatStore = create<ChatState>((set, get) => ({
  // 浏览器模式预填充 mock 消息，方便 UI 调试
  messages: isBrowser ? [...MOCK_MESSAGES] : [],
  draftInput: '',
  isStreaming: false,
  streamingTokens: 0,
  streamingModelId: null,
  error: null,

  /** 设置输入框草稿文本（用于接收外部选中的文本） */
  setDraftInput: (text: string) => {
    set({ draftInput: text });
  },

  /**
   * 发送消息到 AI
   * 1. 创建 user 消息和空的 assistant 消息
   * 2. 设置 isStreaming = true 进入流式状态
   * 3. 浏览器模式：使用 setInterval 逐字输出 mock 回复
   * 4. Tauri 模式：调用 Rust 后端 send_message 命令
   */
  sendMessage: async (content: string, modelId: string) => {
    const { messages } = get();

    // 构建用户消息
    const userMessage: Message = {
      id: generateId(),
      role: 'user',
      content,
      model_id: null, // 用户消息不关联模型
      created_at: Math.floor(Date.now() / 1000),
    };

    // 构建空的助手消息，内容将在流式过程中逐步填充
    const assistantMessage: Message = {
      id: generateId(),
      role: 'assistant',
      content: '', // 空内容，等待 token 追加
      model_id: modelId,
      created_at: Math.floor(Date.now() / 1000),
    };

    // 将两条消息追加到消息列表末尾
    const updatedMessages = [...messages, userMessage, assistantMessage];
    set({
      messages: updatedMessages,
      draftInput: '', // 发送后清空输入框
      isStreaming: true,
      streamingTokens: 0,
      streamingModelId: modelId,
      error: null,
    });

    // 用户消息由 Rust 后端在 send_message 命令中自动持久化，前端无需额外处理

    if (isBrowser) {
      // 浏览器 mock：模拟流式逐字输出
      const mockResponse =
        '这是一个模拟回复。在浏览器中运行时，Tauri invoke 不可用，所以这里展示的是假数据。\n\n在实际应用中，这里会通过 SSE 流式返回 AI 的真实回复。';
      let charIndex = 0;
      const streamInterval = setInterval(() => {
        if (charIndex < mockResponse.length) {
          const token = mockResponse[charIndex];
          get().appendToken(token); // 每次追加一个字符
          charIndex++;
        } else {
          // 所有字符输出完毕，结束流式
          clearInterval(streamInterval);
          get().finalizeMessage();
        }
      }, 30); // 30ms 间隔模拟流式效果
      return;
    }

    try {
      // Tauri 模式：调用 Rust 后端发起 SSE 请求
      const { invoke } = await import('@tauri-apps/api/core');
      // 发送除最后一条（空 assistant 消息）外的消息历史作为上下文
      await invoke('send_message', {
        messages: updatedMessages.slice(0, -1),
        modelId,
      });
    } catch (e) {
      // 发送失败时清除流式状态并记录错误
      set({
        isStreaming: false,
        error: String(e),
      });
    }
  },

  /** 停止当前的流式生成 */
  stopGeneration: async () => {
    if (isBrowser) return; // 浏览器模式不需要
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('stop_generation');
    } catch (e) {
      console.error('Failed to stop generation:', e);
    }
  },

  /**
   * 追加 token 到当前最后一条消息（assistant 消息）的 content 末尾
   * 这是流式更新的核心方法，由 useStreaming hook 在接收到 stream-token 事件时调用
   */
  appendToken: (token: string) => {
    const { messages } = get();
    const updated = [...messages];
    const lastMsg = { ...updated[updated.length - 1] }; // 浅拷贝最后一条消息
    lastMsg.content += token; // 追加 token 到内容末尾
    updated[updated.length - 1] = lastMsg;
    set({
      messages: updated,
      streamingTokens: get().streamingTokens + 1, // token 计数 +1
    });
  },

  /** 流式生成完成后的收尾工作（AI 回复由 Rust 后端自动持久化） */
  finalizeMessage: () => {
    set({
      isStreaming: false,
      streamingTokens: 0,
      streamingModelId: null,
    });
  },

  /** 将单条消息持久化到 Rust 后端存储 */
  saveMessage: async (message: Message) => {
    if (isBrowser) return;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('save_message', { message });
    } catch (e) {
      console.error('[Buddy] 保存消息失败:', e);
    }
  },

  /** 从磁盘加载历史消息 */
  loadMessages: async (offset = 0, limit = 100) => {
    if (isBrowser) return;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const history = await invoke<Message[]>('load_messages', { offset, limit });
      if (history && history.length > 0) {
        set({ messages: history });
      }
    } catch (e) {
      console.error('[Buddy] 加载历史消息失败:', e);
    }
  },

  /** 清空所有消息和草稿 */
  clearMessages: () => {
    set({ messages: [], draftInput: '', error: null });
  },

  /** 设置完整的消息列表（用于加载历史对话） */
  setMessages: (messages: Message[]) => {
    set({ messages });
  },

  /** 设置错误信息并停止流式状态 */
  setError: (error: string | null) => {
    set({ error, isStreaming: false });
  },
}));
