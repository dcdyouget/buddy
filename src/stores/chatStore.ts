/**
 * chatStore.ts — 聊天状态管理
 *
 * 管理聊天会话的核心状态，包括：
 * - 消息数组（messages）：完整的对话历史
 * - 流式生成状态（isStreaming、streamingTokens）
 * - 输入草稿（draftInput）：用户正在编辑但尚未发送的文本
 * - 错误处理（error）
 * - 内容块（blocks）：区分文本和思考块的结构化内容
 *
 * 消息发送流程：
 * 1. sendMessage() 创建 user + assistant 消息，设置 isStreaming = true
 * 2. Rust 后端通过统一事件协议推送 StreamEvent
 * 3. useStreaming hook 监听 stream-event → 调用对应的 append* 方法
 * 4. 流式完成后调用 finalizeMessage() 收尾
 * 5. 用户可随时调用 stopGeneration() 中断生成
 *
 * 浏览器模式下使用 setInterval 模拟流式效果，方便前端调试。
 */

import { create } from 'zustand';
import type { Message, ContentBlock } from '@/types';
import { isBrowser, MOCK_MESSAGES } from '@/utils/mock';

/** ChatStore 状态和操作定义 */
interface ChatState {
  messages: Message[];            // 消息列表（按时间排序）
  draftInput: string;             // 输入框草稿文本
  isStreaming: boolean;           // 是否正在流式生成中
  streamingTokens: number;        // 当前流式生成已接收的 token 数
  streamingModelId: string | null; // 当前正在生成的模型 ID
  streamingBlocks: ContentBlock[]; // 流式中正在构建的内容块
  error: string | null;           // 最近的错误信息

  // ── 操作 ──
  setDraftInput: (text: string) => void;                        // 设置输入草稿
  sendMessage: (content: string, modelId: string) => Promise<void>; // 发送消息
  stopGeneration: () => Promise<void>;                          // 停止生成
  appendToken: (token: string) => void;                         // 追加 token（兼容旧格式）
  appendTextToken: (token: string) => void;                     // 追加文本 token
  appendThinkingToken: (token: string) => void;                 // 追加思考 token
  handleTextStart: (contentIndex: number) => void;              // 文本块开始
  handleTextDelta: (contentIndex: number, delta: string) => void; // 文本增量
  handleTextEnd: (contentIndex: number, content: string) => void; // 文本块结束
  handleThinkingStart: (contentIndex: number) => void;           // 思考块开始
  handleThinkingDelta: (contentIndex: number, delta: string) => void; // 思考增量
  handleThinkingEnd: (contentIndex: number, content: string) => void; // 思考块结束
  handleStreamDone: () => void;                                  // 流式完成
  handleStreamError: (reason: string, message: string) => void;  // 流式错误
  finalizeMessage: () => void;                                  // 完成流式消息（兼容）
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

/**
 * 从文本中解析 <think>...</think> 标签，转换为 ContentBlock 数组
 *
 * 用于流式渲染期间实时检测思考标签。与 thinkParser.ts 中的
 * parseThinkBlocks 逻辑一致，但直接返回 ContentBlock[] 格式。
 *
 * 规则：
 * - <think> 之前的内容 → text block
 * - <think>...</think> → thinking block (is_open=false)
 * - <think>... (无闭合) → thinking block (is_open=true)
 * - 嵌套 <think> 当作字面文本
 */
function parseThinkFromText(text: string): ContentBlock[] {
  const blocks: ContentBlock[] = [];
  const THINK_OPEN = '<think>';
  const THINK_CLOSE = '</think>';
  let i = 0;

  while (i < text.length) {
    const thinkOpen = text.indexOf(THINK_OPEN, i);

    if (thinkOpen === -1) {
      // 没有更多标签，剩余全是文本
      blocks.push({ type: 'text', content: text.substring(i) });
      break;
    }

    // <think> 之前的文本
    if (thinkOpen > i) {
      blocks.push({ type: 'text', content: text.substring(i, thinkOpen) });
    }

    // 查找闭合标签
    const thinkStart = thinkOpen + THINK_OPEN.length;
    const thinkClose = text.indexOf(THINK_CLOSE, thinkStart);

    if (thinkClose === -1) {
      // 无闭合 → 流式进行中
      blocks.push({ type: 'thinking', content: text.substring(thinkStart), is_open: true });
      break;
    }

    // 完整闭合
    blocks.push({
      type: 'thinking',
      content: text.substring(thinkStart, thinkClose),
      is_open: false,
    });
    i = thinkClose + THINK_CLOSE.length;
  }

  // 过滤空 text block
  return blocks.filter((b) => b.type !== 'text' || b.content.length > 0);
}

export const useChatStore = create<ChatState>((set, get) => ({
  // 浏览器模式预填充 mock 消息，方便 UI 调试
  messages: isBrowser ? [...MOCK_MESSAGES] : [],
  draftInput: '',
  isStreaming: false,
  streamingTokens: 0,
  streamingModelId: null,
  streamingBlocks: [],
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
      model_id: null,
      created_at: Math.floor(Date.now() / 1000),
    };

    // 构建空的助手消息，内容将在流式过程中逐步填充
    const assistantMessage: Message = {
      id: generateId(),
      role: 'assistant',
      content: '',
      blocks: [],
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
      streamingBlocks: [],
      error: null,
    });

    if (isBrowser) {
      // 浏览器 mock：模拟流式逐字输出
      const mockResponse =
        '这是一个模拟回复。在浏览器中运行时，Tauri invoke 不可用，所以这里展示的是假数据。\n\n在实际应用中，这里会通过 SSE 流式返回 AI 的真实回复。';
      let charIndex = 0;
      const streamInterval = setInterval(() => {
        if (charIndex < mockResponse.length) {
          const token = mockResponse[charIndex];
          get().appendTextToken(token);
          charIndex++;
        } else {
          clearInterval(streamInterval);
          get().finalizeMessage();
        }
      }, 30);
      return;
    }

    try {
      // Tauri 模式：调用 Rust 后端发起 SSE 请求
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('send_message', {
        messages: updatedMessages.slice(0, -1),
        modelId,
      });
    } catch (e) {
      set({
        isStreaming: false,
        error: String(e),
      });
    }
  },

  /** 停止当前的流式生成 */
  stopGeneration: async () => {
    if (isBrowser) return;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('stop_generation');
    } catch (e) {
      console.error('Failed to stop generation:', e);
    }
  },

  /**
   * 追加 token 到当前最后一条消息（兼容旧 stream-token 格式）
   * 直接追加到 content 字符串末尾，同时更新 text block。
   */
  appendToken: (token: string) => {
    get().appendTextToken(token);
  },

  /**
   * 追加文本 token 到当前最后一条消息
   * 同时更新 content 字符串（向后兼容）和 blocks 数组。
   */
  appendTextToken: (token: string) => {
    const { messages } = get();
    const updated = [...messages];
    const lastMsg = { ...updated[updated.length - 1] } as Message;
    lastMsg.content = (lastMsg.content || '') + token;

    // 更新 blocks：找到最后一个 text block 或创建新的
    const blocks = [...(lastMsg.blocks || [])];
    const lastBlock = blocks[blocks.length - 1];
    if (lastBlock && lastBlock.type === 'text') {
      // 追加到现有 text block
      blocks[blocks.length - 1] = {
        ...lastBlock,
        content: lastBlock.content + token,
      };
    } else {
      // 如果上一个 block 是 thinking 且未闭合，先闭合它
      if (lastBlock && lastBlock.type === 'thinking' && lastBlock.is_open) {
        blocks[blocks.length - 1] = { ...lastBlock, is_open: false };
      }
      // 创建新的 text block
      blocks.push({ type: 'text', content: token });
    }
    lastMsg.blocks = blocks;

    updated[updated.length - 1] = lastMsg;
    set({
      messages: updated,
      streamingTokens: get().streamingTokens + 1,
    });
  },

  /**
   * 追加思考 token 到当前最后一条消息
   * 思考内容不加入 content 字符串，仅存入 blocks。
   */
  appendThinkingToken: (token: string) => {
    get().handleThinkingDelta(0, token);
  },

  /** 初始化/重置 streamingBlocks */
  handleTextStart: (contentIndex: number) => {
    const blocks = [...get().streamingBlocks];
    while (blocks.length <= contentIndex) {
      blocks.push({ type: 'text', content: '' });
    }
    blocks[contentIndex] = { type: 'text', content: '' };
    set({ streamingBlocks: blocks });
  },

  /** 追加文本 delta，自动检测 <think> 标签并分配正确的块类型 */
  handleTextDelta: (_contentIndex: number, delta: string) => {
    const { messages } = get();

    // 更新 content 字符串（用于持久化）
    const updated = [...messages];
    const lastMsg = { ...updated[updated.length - 1] } as Message;
    lastMsg.content = (lastMsg.content || '') + delta;
    updated[updated.length - 1] = lastMsg;

    // 从完整 content 中解析 <think> 标签，重建 blocks
    const newBlocks = parseThinkFromText(lastMsg.content);

    set({
      messages: updated,
      streamingBlocks: newBlocks,
      streamingTokens: get().streamingTokens + 1,
    });
  },

  /** 文本块结束：用 parseThinkFromText 重建 blocks，防止覆盖 thinking 块 */
  handleTextEnd: (contentIndex: number, content: string) => {
    // 对于 content_index=0（OpenAI 兼容格式的唯一块），从完整内容解析 blocks
    // 对于 Anthropic 的多块格式，每个 text_end 只涉及对应索引的块
    if (contentIndex === 0) {
      // 单块模式：用 parseThinkFromText 解析完整响应
      const newBlocks = parseThinkFromText(content);
      set({ streamingBlocks: newBlocks });
    } else {
      // 多块模式：保持现有 blocks，只更新对应索引
      const blocks = [...get().streamingBlocks];
      while (blocks.length <= contentIndex) {
        blocks.push({ type: 'text', content: '' });
      }
      blocks[contentIndex] = { type: 'text', content };
      set({ streamingBlocks: blocks });
    }
  },

  /** 思考块开始 */
  handleThinkingStart: (contentIndex: number) => {
    const blocks = [...get().streamingBlocks];
    while (blocks.length <= contentIndex) {
      blocks.push({ type: 'thinking', content: '', is_open: true });
    }
    blocks[contentIndex] = { type: 'thinking', content: '', is_open: true };
    set({ streamingBlocks: blocks });
  },

  /** 追加思考 delta */
  handleThinkingDelta: (contentIndex: number, delta: string) => {
    const blocks = [...get().streamingBlocks];
    while (blocks.length <= contentIndex) {
      blocks.push({ type: 'thinking', content: '', is_open: true });
    }
    const block = blocks[contentIndex];
    if (block.type === 'thinking') {
      blocks[contentIndex] = {
        ...block,
        content: block.content + delta,
        is_open: true,
      };
    }
    set({
      streamingBlocks: blocks,
      streamingTokens: get().streamingTokens + 1,
    });
  },

  /** 思考块结束 */
  handleThinkingEnd: (contentIndex: number, content: string) => {
    const blocks = [...get().streamingBlocks];
    while (blocks.length <= contentIndex) {
      blocks.push({ type: 'thinking', content: '', is_open: false });
    }
    blocks[contentIndex] = { type: 'thinking', content, is_open: false };
    set({ streamingBlocks: blocks });
  },

  /** 流式完成：将 streamingBlocks 附加到消息 */
  handleStreamDone: () => {
    const { messages, streamingBlocks } = get();
    const updated = [...messages];
    const lastMsg = { ...updated[updated.length - 1] } as Message;
    lastMsg.blocks = streamingBlocks.map((b: ContentBlock) =>
      b.type === 'thinking' ? { ...b, is_open: false } : b,
    );
    updated[updated.length - 1] = lastMsg;
    set({
      messages: updated,
      isStreaming: false,
      streamingTokens: 0,
      streamingModelId: null,
      streamingBlocks: [],
    });
  },

  /** 流式错误：重置状态 */
  handleStreamError: (_reason: string, message: string) => {
    set({
      isStreaming: false,
      streamingTokens: 0,
      streamingModelId: null,
      streamingBlocks: [],
      error: message,
    });
  },

  /** 流式生成完成后的收尾工作 */
  finalizeMessage: () => {
    const { messages } = get();
    // 闭合所有未完成的 thinking block
    const updated = [...messages];
    const lastMsg = { ...updated[updated.length - 1] } as Message;
    if (lastMsg.blocks) {
      lastMsg.blocks = lastMsg.blocks.map((b: ContentBlock) =>
        b.type === 'thinking' && b.is_open ? { ...b, is_open: false } : b,
      );
    }
    // 如果没有 blocks 但有 content，从 content 解析 <think> 标签
    if ((!lastMsg.blocks || lastMsg.blocks.length === 0) && lastMsg.content) {
      lastMsg.blocks = parseThinkFromText(lastMsg.content);
    }
    updated[updated.length - 1] = lastMsg;

    set({
      messages: updated,
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

  /** 从磁盘加载历史消息，自动补全缺失的 blocks */
  loadMessages: async (offset = 0, limit = 100) => {
    if (isBrowser) return;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const history = await invoke<Message[]>('load_messages', { offset, limit });
      if (history && history.length > 0) {
        // 为没有 blocks 的消息从 content 中解析 blocks
        const withBlocks = history.map((msg) => {
          if (msg.role === 'assistant' && (!msg.blocks || msg.blocks.length === 0) && msg.content) {
            return { ...msg, blocks: parseThinkFromText(msg.content) };
          }
          return msg;
        });
        set({ messages: withBlocks });
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
