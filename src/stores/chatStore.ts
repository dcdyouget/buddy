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
import { parseThinkBlocks } from '@/utils/thinkParser';

/** ChatStore 状态和操作定义 */
interface ChatState {
  messages: Message[];
  draftInput: string;
  isStreaming: boolean;
  streamingTokens: number;
  streamingModelId: string | null;
  streamingBlocks: ContentBlock[];
  error: string | null;

  // P8: Tool 审批状态
  toolApproval: {
    id: string;
    name: string;
    arguments: string;
    reason: string;
  } | null;

  // P9: 平滑文本渲染 —— 后端推送的文本增量先入队缓冲
  // rAF 循环再从队头逐字消费到 streamingBlocks，避免突发的 SSE chunk
  // 导致 React 批量 re-render 产生的「一卡一卡」视觉
  pendingTextBuffer: string;

  // ── 操作 ──
  setDraftInput: (text: string) => void;
  sendMessage: (content: string, modelId: string) => Promise<void>;
  stopGeneration: () => Promise<void>;
  appendToken: (token: string) => void;
  appendTextToken: (token: string) => void;
  appendThinkingToken: (token: string) => void;
  handleTextStart: (contentIndex: number) => void;
  handleTextDelta: (contentIndex: number, delta: string) => void;
  handleTextEnd: (contentIndex: number, content: string) => void;
  handleThinkingStart: (contentIndex: number) => void;
  handleThinkingDelta: (contentIndex: number, delta: string) => void;
  handleThinkingEnd: (contentIndex: number, content: string) => void;
  // P8: Tool handlers
  handleToolCallStart: (id: string, name: string, contentIndex: number) => void;
  handleToolCallDelta: (id: string, argumentsDelta: string) => void;
  handleToolCallEnd: (id: string, name: string, args: string) => void;
  handleToolExecuting: (id: string, name: string) => void;
  handleToolResult: (id: string, name: string, content: string, isError: boolean) => void;
  handleToolApprovalRequired: (id: string, name: string, args: string, reason: string) => void;
  setToolApproval: (approval: ChatState['toolApproval']) => void;
  handleStreamDone: () => void;
  handleStreamError: (reason: string, message: string) => void;
  finalizeMessage: () => void;
  // P9: 平滑渲染
  feedTextDelta: (delta: string) => void;
  smoothTextDelta: (count: number) => void;
  flushTextBuffer: () => void;
  saveMessage: (message: Message) => Promise<void>;
  loadMessages: (offset?: number, limit?: number) => Promise<void>;
  clearMessages: () => void;
  setMessages: (messages: Message[]) => void;
  setError: (error: string | null) => void;
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
 * 适配器：将 parseThinkBlocks() 的 ContentSegment[] 转换为 ContentBlock[]
 *
 * thinkParser.ts 返回 { type: 'think', content, isOpen } 格式，
 * 此处统一转换为 store 使用的 { type: 'thinking', content, is_open } 格式。
 */
function parseThinkFromText(text: string): ContentBlock[] {
  const segments = parseThinkBlocks(text);
  return segments.map((seg) => {
    if (seg.type === 'think') {
      return { type: 'thinking' as const, content: seg.content, is_open: seg.isOpen };
    }
    return { type: 'text' as const, content: seg.content };
  });
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
  toolApproval: null,
  pendingTextBuffer: '',
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
      pendingTextBuffer: '',
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
      const { sendMessage: sendMsg } = await import('@/api/chat');
      await sendMsg(updatedMessages.slice(0, -1), modelId);
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
      const { stopGeneration: stop } = await import('@/api/chat');
      await stop();
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

  /** 思考块开始 —— 追加到 blocks 末尾而非按 content_index 覆盖 */
  handleThinkingStart: (_contentIndex: number) => {
    const blocks = [...get().streamingBlocks];
    const lastBlock = blocks[blocks.length - 1];
    // 如果最后一个已经是 open 的 thinking block，重置它
    if (lastBlock && lastBlock.type === 'thinking' && lastBlock.is_open) {
      blocks[blocks.length - 1] = { type: 'thinking', content: '', is_open: true };
    } else {
      blocks.push({ type: 'thinking', content: '', is_open: true });
    }
    set({ streamingBlocks: blocks });
  },

  /** 追加思考 delta —— 在 blocks 末尾找最后一个 open 的 thinking block */
  handleThinkingDelta: (_contentIndex: number, delta: string) => {
    const blocks = [...get().streamingBlocks];
    // 从后往前找最后一个 open 的 thinking block
    let found = false;
    for (let i = blocks.length - 1; i >= 0; i--) {
      const b = blocks[i];
      if (b.type === 'thinking' && b.is_open) {
        blocks[i] = { type: 'thinking', content: b.content + delta, is_open: true };
        found = true;
        break;
      }
    }
    if (!found) {
      blocks.push({ type: 'thinking', content: delta, is_open: true });
    }
    set({
      streamingBlocks: blocks,
      streamingTokens: get().streamingTokens + 1,
    });
  },

  /** 思考块结束 */
  handleThinkingEnd: (_contentIndex: number, content: string) => {
    const blocks = [...get().streamingBlocks];
    for (let i = blocks.length - 1; i >= 0; i--) {
      const b = blocks[i];
      if (b.type === 'thinking' && b.is_open) {
        blocks[i] = { type: 'thinking', content, is_open: false };
        set({ streamingBlocks: blocks });
        return;
      }
    }
    blocks.push({ type: 'thinking', content, is_open: false });
    set({ streamingBlocks: blocks });
  },

  /** 流式完成：先清缓冲，再将 streamingBlocks 附加到消息 */
  handleStreamDone: () => {
    get().flushTextBuffer();
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
      pendingTextBuffer: '',
      toolApproval: null,
    });
  },

  /** 流式错误：先清缓冲再重置状态 */
  handleStreamError: (_reason: string, message: string) => {
    get().flushTextBuffer();
    set({
      isStreaming: false,
      streamingTokens: 0,
      streamingModelId: null,
      streamingBlocks: [],
      pendingTextBuffer: '',
      error: message,
    });
  },

  // ── P9: 平滑文本渲染 ────────────────────────────

  /** 将 text_delta 入队到缓冲，等待 rAF 循环逐字消费 */
  feedTextDelta: (delta: string) => {
    set((s) => ({ pendingTextBuffer: s.pendingTextBuffer + delta }));
  },

  /** rAF 每帧调用：从缓冲区取 count 个字符，追加到 streamingBlocks 和 messages[last].content */
  smoothTextDelta: (count: number) => {
    const { pendingTextBuffer, messages, streamingBlocks } = get();
    if (pendingTextBuffer.length === 0) return;

    const chars = pendingTextBuffer.slice(0, count);
    const rest = pendingTextBuffer.slice(count);

    // 更新最后一条消息的 content（用于持久化）
    const updated = [...messages];
    const lastMsg = { ...updated[updated.length - 1] } as Message;
    lastMsg.content = (lastMsg.content || '') + chars;
    updated[updated.length - 1] = lastMsg;

    // 增量追加到 streamingBlocks（不完全 parseThinkFromText，避免 O(n²)）
    const blocks = [...streamingBlocks];
    const lastBlock = blocks[blocks.length - 1];
    if (lastBlock && lastBlock.type === 'text') {
      blocks[blocks.length - 1] = { ...lastBlock, content: lastBlock.content + chars };
    } else {
      if (lastBlock && lastBlock.type === 'thinking' && lastBlock.is_open) {
        blocks[blocks.length - 1] = { ...lastBlock, is_open: false };
      }
      blocks.push({ type: 'text', content: chars });
    }

    set({
      pendingTextBuffer: rest,
      messages: updated,
      streamingBlocks: blocks,
      streamingTokens: get().streamingTokens + 1,
    });
  },

  /** 将缓冲区剩余字符全部推入（done/error 前调用，防止丢字） */
  flushTextBuffer: () => {
    const { pendingTextBuffer } = get();
    if (pendingTextBuffer.length === 0) return;
    get().smoothTextDelta(pendingTextBuffer.length);
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
      const { saveMessage: save } = await import('@/api/storage');
      await save(message);
    } catch (e) {
      console.error('[Buddy] 保存消息失败:', e);
    }
  },

  /** 从磁盘加载历史消息，自动补全缺失的 blocks */
  loadMessages: async (offset = 0, limit = 100) => {
    if (isBrowser) return;
    try {
      const { loadMessages: load } = await import('@/api/storage');
      const history = await load(offset, limit);
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


  // ── P8: Tool 事件处理 ────────────────────────────────
  // 注意: 不再将 tool 状态文本拼入 assistant.content，防止污染对话历史
  // 导致模型看到 "[create_file 结果]:" 等文本后产生幻觉、不再调用工具
  handleToolCallStart: (_id: string, _name: string, _contentIndex: number) => {
    // 工具调用状态由 toolApproval + UI 层展示
  },

  handleToolCallDelta: (_id: string, _argumentsDelta: string) => {
    // 增量实时:暂时只更新 toolApproval 不刷 UI(后端循环速度很快)
  },

  handleToolCallEnd: (_id: string, _name: string, _args: string) => {
    // tool_call 参数完整
  },

  handleToolExecuting: (_id: string, _name: string) => {
    // 后端开始执行
  },

  handleToolResult: (_id: string, name: string, content: string, isError: boolean) => {
    // 工具结果以独立 tool 消息插入，保持与后端一致的数据结构
    const toolMsg: Message = {
      id: 'tool-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 9),
      role: 'tool' as const,
      content,
      model_id: null,
      created_at: Math.floor(Date.now() / 1000),
      tool_call_id: _id,
      tool_name: name,
      is_error: isError,
    };
    set({ messages: [...get().messages, toolMsg] });
  },

  handleToolApprovalRequired: (id: string, name: string, args: string, reason: string) => {
    set({ toolApproval: { id, name, arguments: args, reason } });
  },

  setToolApproval: (approval) => {
    set({ toolApproval: approval });
  },
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
