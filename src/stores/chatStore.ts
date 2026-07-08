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
import type { Message, ContentBlock, PendingQuestion, ToolCall, ToolCallStatus } from '@/types';
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

  // P11: 当前等待回答的 ask_user 问题(后端 ToolQuestionRequired 事件触发)
  pendingQuestion: PendingQuestion | null;

  // P10: "回应模型问题"模式
  // 当模型在一次回答末尾提出问题时(启发式:文本以 ? 或 ？ 结尾),
  // 进入此状态。ChatPage 会用 UserResponseInput 替换 InputDock,
  // 用户填写的回应会以 parent_message_id 指向这条 assistant 消息,
  // 在 UI 上嵌套渲染在父消息内部。
  waitingForResponse: {
    parentMessageId: string;
    question: string;
  } | null;

  // P9: 当前流式轮次中正在进行的工具调用（按 id 索引）
  // 流式结束后会被合并到最后一条 assistant 消息的 tool_calls 字段中
  activeToolCalls: Record<string, ToolCall>;

  // P9: 平滑文本渲染 —— 后端推送的文本增量先入队缓冲
  // rAF 循环再从队头逐字消费到 streamingBlocks，避免突发的 SSE chunk
  // 导致 React 批量 re-render 产生的「一卡一卡」视觉
  pendingTextBuffer: string;

  // ── 操作 ──
  setDraftInput: (text: string) => void;
  sendMessage: (content: string, modelId: string, parentMessageId?: string) => Promise<void>;
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
  _ensureToolCallEntry: (id: string, name: string, status: ToolCallStatus) => ToolCall;
  _computeInsertAfterBlockIndex: () => number;
  handleToolCallStart: (id: string, name: string, contentIndex: number) => void;
  handleToolCallDelta: (id: string, argumentsDelta: string) => void;
  handleToolCallEnd: (id: string, name: string, args: string) => void;
  handleToolExecuting: (id: string, name: string) => void;
  handleToolResult: (id: string, name: string, content: string, isError: boolean) => void;
  handleToolApprovalRequired: (id: string, name: string, args: string, reason: string) => void;
  setToolApproval: (approval: ChatState['toolApproval']) => void;
  // P10: 回应模型问题的状态控制
  setWaitingForResponse: (wfr: ChatState['waitingForResponse']) => void;
  // P11: ask_user 问题的状态控制
  setPendingQuestion: (q: PendingQuestion | null) => void;
  answerPendingQuestion: (selected: number[], inputs?: string[], custom?: string) => Promise<void>;
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

/**
 * 在 messages 数组中从尾部向前查找最后一条 role === 'assistant' 的消息索引。
 * 返回 -1 表示没有 assistant 消息。
 *
 * 为什么需要这个：handleToolResult 会在数组末尾 push role='tool' 的消息,
 * 所以直接用 `messages.length - 1` 取到的可能是 tool 消息,
 * 而我们要把 streamingBlocks / tool_calls 写到真正的 assistant 消息上。
 */
function findLastAssistantIdx(messages: Message[]): number {
  for (let i = messages.length - 1; i >= 0; i--) {
    if (messages[i].role === 'assistant') return i;
  }
  return -1;
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
  activeToolCalls: {},
  waitingForResponse: null,
  pendingQuestion: null,
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
  sendMessage: async (content: string, modelId: string, parentMessageId?: string) => {
    const { messages } = get();

    // 构建用户消息
    const userMessage: Message = {
      id: generateId(),
      role: 'user',
      content,
      model_id: null,
      created_at: Math.floor(Date.now() / 1000),
      parent_message_id: parentMessageId,
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
      activeToolCalls: {},
      toolApproval: null,
      waitingForResponse: null, // 一旦开始新一轮,清除回应模式
      pendingQuestion: null, // 清除任何挂起的 ask_user 问题
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
      const toSend = updatedMessages.slice(0, -1);
      // ── 诊断: dump 所有 tool 相关消息 ──
      console.warn(`[chatStore.sendMessage] 准备发送 ${toSend.length} 条消息:`);
      toSend.forEach((m, i) => {
        const tc = m.tool_calls ? ` tool_calls=[${m.tool_calls.map(t => t.id).join(',')}]` : '';
        const tcid = m.tool_call_id ? ` tool_call_id=${m.tool_call_id}` : '';
        if (m.role === 'assistant' && m.tool_calls) console.warn(`  [${i}] assistant id=${m.id}${tc}`);
        if (m.role === 'tool') console.warn(`  [${i}] tool id=${m.id} name=${m.tool_name}${tcid} err=${m.is_error} content=${m.content.slice(0,80)}`);
      });
      const { sendMessage: sendMsg } = await import('@/api/chat');
      await sendMsg(toSend, modelId);
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
    // 多轮情况下 text_start 会被多次调用,只在该索引位置还没有 block 时才创建
    // 否则会覆盖前序轮的累积内容(导致"只显示最后一条"bug)
    if (blocks.length > contentIndex) {
      return;
    }
    while (blocks.length <= contentIndex) {
      blocks.push({ type: 'text', content: '' });
    }
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

  /** 文本块结束 */
  handleTextEnd: (contentIndex: number, content: string) => {
    if (contentIndex === 0) {
      // OpenAI 单块模式:每个 turn 都会发 text_end(idx=0, &full_response)
      // 其中 content 是「当前 turn」的文本,不是累积全文。
      // 不能整体重置 streamingBlocks(会丢失前序 turn 的内容),
      // 改为 push 一个空文本块作为轮次分隔,让 smoothTextDelta 把下一轮的字符
      // 追加到新 block 而不是污染前序轮。
      const blocks = [...get().streamingBlocks];
      blocks.push({ type: 'text', content: '' });
      set({ streamingBlocks: blocks });
    } else {
      // Anthropic 多块模式:每个 text_end 只更新对应索引的 block
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
    const { messages, streamingBlocks, activeToolCalls } = get();
    const updated = [...messages];
    // 找到最后一条 assistant 消息 —— 它才是 streamingBlocks / tool_calls 的目标。
    // 注意: handleToolResult 会在 messages 末尾 push role='tool' 的消息,
    // 所以 messages 的最后一条可能是 tool 消息而不是 assistant。
    const lastAssistantIdx = findLastAssistantIdx(updated);
    let lastAssistant: Message | null = null;
    if (lastAssistantIdx >= 0) {
      lastAssistant = { ...updated[lastAssistantIdx] } as Message;
      // 闭合所有 thinking block,并去掉尾部由 text_end 推入的空文本分隔块
      const closedBlocks = streamingBlocks.map((b: ContentBlock) =>
        b.type === 'thinking' ? { ...b, is_open: false } : b,
      );
      while (closedBlocks.length > 0) {
        const last = closedBlocks[closedBlocks.length - 1];
        if (last.type === 'text' && last.content === '') {
          closedBlocks.pop();
        } else {
          break;
        }
      }
      lastAssistant.blocks = closedBlocks;
      // 把累积的 tool_calls 合并到最后一条 assistant 消息
      const toolCallsList = Object.values(activeToolCalls);
      if (toolCallsList.length > 0) {
        lastAssistant.tool_calls = toolCallsList;
      }
      updated[lastAssistantIdx] = lastAssistant;
    }

    // P10: 启发式判断模型是否在"提问" —— 本轮没有 tool_call,文本以 ? 或 ？ 结尾
    // 如果是,把状态切到 waitingForResponse,ChatPage 会切换到 UserResponseInput
    let waitingForResponse: ChatState['waitingForResponse'] = null;
    if (lastAssistant) {
      const hadToolCalls = (lastAssistant.tool_calls?.length ?? 0) > 0;
      if (!hadToolCalls) {
        const text = (lastAssistant.content || '').trim();
        if (text.endsWith('?') || text.endsWith('？')) {
          waitingForResponse = {
            parentMessageId: lastAssistant.id,
            question: text,
          };
        }
      }
    }

    set({
      messages: updated,
      isStreaming: false,
      streamingTokens: 0,
      streamingModelId: null,
      streamingBlocks: [],
      pendingTextBuffer: '',
      toolApproval: null,
      activeToolCalls: {},
      waitingForResponse,
      // pendingQuestion 由 answerPendingQuestion 自身清空,这里不动
      // (后端通过 tool_result 事件来,在此之前 modal 一直挂着)
    });
  },

  /** 流式错误：先清缓冲再重置状态 */
  handleStreamError: (_reason: string, message: string) => {
    get().flushTextBuffer();
    // 错误时把仍未完成的 tool_calls 也合并到消息（标记 error），便于用户回看
    const { messages, activeToolCalls } = get();
    let updated = messages;
    const lastAssistantIdx = findLastAssistantIdx(messages);
    if (Object.keys(activeToolCalls).length > 0 && lastAssistantIdx >= 0) {
      const lastAssistant = { ...messages[lastAssistantIdx] } as Message;
      lastAssistant.tool_calls = Object.values(activeToolCalls).map((tc) => ({
        ...tc,
        status: tc.status === 'done' ? 'done' : 'error',
        result: tc.result ?? message,
        is_error_result: tc.is_error_result ?? true,
      }));
      updated = [...messages];
      updated[lastAssistantIdx] = lastAssistant;
    }
    set({
      messages: updated,
      isStreaming: false,
      streamingTokens: 0,
      streamingModelId: null,
      streamingBlocks: [],
      pendingTextBuffer: '',
      error: message,
      activeToolCalls: {},
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
  // 状态由 activeToolCalls (id -> ToolCall) 维护,流式结束后在 handleStreamDone
  // 时合并到最后一条 assistant 消息的 tool_calls 字段,以便持久化/重渲染

  /** 找到/创建当前流式最后一条 assistant 消息的 tool_calls 数组,并返回其引用 */
  _ensureToolCallEntry: (id: string, name: string, status: ToolCallStatus): ToolCall => {
    const active = { ...get().activeToolCalls };
    if (!active[id]) {
      active[id] = { id, name, arguments: '', status };
    } else {
      active[id] = { ...active[id], name, status };
    }
    set({ activeToolCalls: active });
    return active[id];
  },

  /**
   * 计算 tool_call 应当插入到哪个 block 之后。
   * 跳过尾部由 text_end 推入的空文本块(它们是轮次分隔符,不算内容 block)。
   * 如果没有 block 或全是空 block,返回 -1(渲染时放在所有 block 之后)。
   */
  _computeInsertAfterBlockIndex: (): number => {
    const blocks = get().streamingBlocks;
    for (let i = blocks.length - 1; i >= 0; i--) {
      const b = blocks[i];
      if (b.type === 'text' && b.content !== '') return i;
      if (b.type === 'thinking' && b.content !== '') return i;
    }
    return -1;
  },

  handleToolCallStart: (id: string, name: string, _contentIndex: number) => {
    get()._ensureToolCallEntry(id, name, 'calling');
    // 记录内联位置:tool_call 应当插入到哪个 block 之后
    const insertAfter = get()._computeInsertAfterBlockIndex();
    const active = { ...get().activeToolCalls };
    if (active[id]) {
      active[id] = { ...active[id], insertAfterBlockIndex: insertAfter };
      set({ activeToolCalls: active });
    }
  },

  handleToolCallDelta: (id: string, argumentsDelta: string) => {
    const active = { ...get().activeToolCalls };
    const prev = active[id];
    if (!prev) return; // 兜底:start 缺失时直接忽略 delta
    active[id] = { ...prev, arguments: (prev.arguments || '') + argumentsDelta };
    set({ activeToolCalls: active });
  },

  handleToolCallEnd: (id: string, name: string, args: string) => {
    const active = { ...get().activeToolCalls };
    const prev = active[id];
    if (prev) {
      active[id] = { ...prev, name, arguments: args, status: 'calling' };
    } else {
      // start 事件丢失,直接以终态插入
      active[id] = { id, name, arguments: args, status: 'calling' };
    }
    set({ activeToolCalls: active });
  },

  handleToolExecuting: (id: string, name: string) => {
    get()._ensureToolCallEntry(id, name, 'executing');
  },

  handleToolResult: (id: string, name: string, content: string, isError: boolean) => {
    console.warn(`[chatStore] tool_result id=${id} name=${name} err=${isError} len=${content.length} msgs_before=${get().messages.length}`);

    // 用一个 set + 一个本地变量,防止两次 set 的间隙被其他事件插入
    const state = get();
    const active = { ...state.activeToolCalls };
    const prev = active[id];
    if (prev) {
      active[id] = { ...prev, name, status: isError ? 'error' : 'done', result: content, is_error_result: isError };
    } else {
      active[id] = { id, name, arguments: '', status: isError ? 'error' : 'done', result: content, is_error_result: isError };
    }

    const toolMsg: Message = {
      id: 'tool-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 9),
      role: 'tool' as const,
      content,
      model_id: null,
      created_at: Math.floor(Date.now() / 1000),
      tool_call_id: id,
      tool_name: name,
      is_error: isError,
    };
    set({
      activeToolCalls: active,
      messages: [...state.messages, toolMsg],
    });
    console.warn(`[chatStore] tool_result done msgs_after=${get().messages.length}`);
  },

  handleToolApprovalRequired: (id: string, name: string, args: string, reason: string) => {
    set({ toolApproval: { id, name, arguments: args, reason } });
  },

  setToolApproval: (approval) => {
    set({ toolApproval: approval });
  },

  /** 设置/清除"等待用户回应"状态 */
  setWaitingForResponse: (wfr) => {
    set({ waitingForResponse: wfr });
  },

  /** 设置当前等待回答的 ask_user 问题 */
  setPendingQuestion: (q) => {
    set({ pendingQuestion: q });
  },

  /**
   * 用户回答 ask_user:把答案发给后端,清空本地状态
   * 后端的 send_message 阻塞 await 会收到 answer 并继续执行
   * @param selected  选中的选项索引(单选/多选)
   * @param inputs    对应 selected 中每个选项的补充输入(可选)
   * @param custom    自定义文本回答(可选)
   */
  answerPendingQuestion: async (selected, inputs, custom) => {
    const { pendingQuestion } = get();
    if (!pendingQuestion) return;
    if (isBrowser) {
      // 浏览器 mock 直接清空
      set({ pendingQuestion: null });
      return;
    }
    try {
      const { answerToolQuestion } = await import('@/api/chat');
      await answerToolQuestion({
        id: pendingQuestion.id,
        selected,
        inputs: inputs ?? [],
        custom: custom ?? null,
      });
    } catch (e) {
      console.error('[Buddy] 回答 ask_user 失败:', e);
    } finally {
      set({ pendingQuestion: null });
    }
  },
  clearMessages: () => {
    set({ messages: [], draftInput: '', error: null, activeToolCalls: {}, toolApproval: null });
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
