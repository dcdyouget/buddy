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
import type {
  Message,
  ContentBlock,
  ImageAttachment,
  PendingQuestion,
  ToolCall,
  ToolCallStatus,
} from '@/types';
import { isBrowser, MOCK_MESSAGES } from '@/utils/mock';
import { parseThinkBlocks } from '@/utils/thinkParser';

/** ChatStore 状态和操作定义 */
interface ChatState {
  messages: Message[];
  /** 当前已加载历史中最早一条的全局偏移量。 */
  historyOffset: number;
  hasMoreHistory: boolean;
  isLoadingHistory: boolean;
  draftInput: string;
  draftImages: ImageAttachment[];
  isStreaming: boolean;
  streamingTokens: number;
  streamingModelId: string | null;
  streamingBlocks: ContentBlock[];
  /** 最近一次写入正文的字符数，用于前端逐字炫光。 */
  streamingRevealCount: number;
  /** 每次正文写入递增，确保连续字符批次重新播放动画。 */
  streamingRevealRevision: number;
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

  // P9: 当前流式轮次中正在进行的工具调用（按 id 索引）
  // 流式结束后会被合并到最后一条 assistant 消息的 tool_calls 字段中
  activeToolCalls: Record<string, ToolCall>;

  // P9: 平滑文本渲染 —— 后端推送的文本增量先入队缓冲
  // rAF 循环再从队头逐字消费到 streamingBlocks，避免突发的 SSE chunk
  // 导致 React 批量 re-render 产生的「一卡一卡」视觉
  pendingTextBuffer: string;
  /** 文本仍在逐字显示时，延后提交 text_end，避免尾部被一次性冲出。 */
  pendingTextEnd: { contentIndex: number; content: string } | null;
  /** 后端已结束，但前端仍在消费逐字动画队列。 */
  streamDonePending: boolean;

  // ── 操作 ──
  setDraftInput: (text: string) => void;
  addDraftImages: (images: ImageAttachment[]) => Promise<void>;
  removeDraftImage: (id: string) => void;
  clearDraftImages: () => void;
  sendMessage: (content: string, modelId: string) => Promise<void>;
  stopGeneration: () => Promise<void>;
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
  handleToolResult: (
    id: string,
    name: string,
    content: string,
    images: ImageAttachment[],
    isError: boolean,
  ) => void;
  handleToolApprovalRequired: (id: string, name: string, args: string, reason: string) => void;
  setToolApproval: (approval: ChatState['toolApproval']) => void;
  // P11: ask_user 问题的状态控制
  setPendingQuestion: (q: PendingQuestion | null) => void;
  answerPendingQuestion: (selected: number[], inputs?: string[], custom?: string) => Promise<boolean>;
  handleStreamDone: () => void;
  handleStreamError: (reason: string, message: string) => void;
  finalizeMessage: () => void;
  // P9: 平滑渲染
  feedTextDelta: (delta: string) => void;
  smoothTextDelta: (count: number) => void;
  flushTextBuffer: () => void;
  saveMessage: (message: Message) => Promise<void>;
  loadMessages: () => Promise<void>;
  loadOlderMessages: () => Promise<void>;
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
 * 从字符串头部取指定数量的 Unicode 字符。
 * 只扫描实际消费的部分，避免每次都复制整个流式缓冲区。
 */
function takeUnicodePrefix(
  input: string,
  count: number,
): { prefix: string; rest: string; characterCount: number } {
  const limit = Math.max(0, Math.floor(count));
  if (limit === 0 || input.length === 0) {
    return { prefix: '', rest: input, characterCount: 0 };
  }

  let end = 0;
  let characterCount = 0;
  for (const character of input) {
    if (characterCount >= limit) break;
    end += character.length;
    characterCount += 1;
  }

  return {
    prefix: input.slice(0, end),
    rest: input.slice(end),
    characterCount,
  };
}

/**
 * 把 Rust 已规范化的正文 delta 写入 blocks。
 * `<think>` 标签已在后端转换成 thinking_start/delta/end，前端不再扫描原文。
 */
function appendStreamingText(
  sourceBlocks: ContentBlock[],
  delta: string,
): ContentBlock[] {
  const blocks = [...sourceBlocks];
  const initialLast = blocks[blocks.length - 1];
  if (initialLast?.type === 'thinking' && initialLast.is_open) {
    blocks[blocks.length - 1] = { ...initialLast, is_open: false };
  }

  const last = blocks[blocks.length - 1];
  if (last?.type === 'text') {
    blocks[blocks.length - 1] = { ...last, content: last.content + delta };
  } else {
    blocks.push({ type: 'text', content: delta });
  }
  return blocks;
}

/** text_end 到达时，避免把已经增量解析好的块再次插入一遍。 */
function endsWithBlocks(
  blocks: ContentBlock[],
  suffix: ContentBlock[],
): boolean {
  if (suffix.length === 0 || suffix.length > blocks.length) return false;
  const offset = blocks.length - suffix.length;
  return suffix.every((expected, index) => {
    const actual = blocks[offset + index];
    if (actual.type !== expected.type || actual.content !== expected.content) {
      return false;
    }
    return (
      actual.type !== 'thinking' ||
      expected.type !== 'thinking' ||
      actual.is_open === expected.is_open
    );
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

/**
 * 只有已获得 tool_result 且参数是完整 JSON 对象的调用才能进入后续对话历史。
 * 流式中断时 calling/executing 状态的 arguments 可能只有半截，必须丢弃。
 */
function getPersistableToolCalls(
  activeToolCalls: Record<string, ToolCall>,
): ToolCall[] {
  return Object.values(activeToolCalls).filter((toolCall) => {
    if (toolCall.status !== 'done' && toolCall.status !== 'error') {
      return false;
    }
    if (!toolCall.id.trim() || !toolCall.name.trim()) return false;

    try {
      const parsed = JSON.parse(toolCall.arguments);
      return Boolean(
        parsed &&
          typeof parsed === 'object' &&
          !Array.isArray(parsed),
      );
    } catch {
      return false;
    }
  });
}

const HISTORY_PAGE_SIZE = 10;

export function hydrateHistoryMessages(messages: Message[]): Message[] {
  const toolResults = new Map<string, Message>();
  for (const message of messages) {
    if (message.role === 'tool' && message.tool_call_id) {
      toolResults.set(message.tool_call_id, message);
    }
  }

  return messages.map((msg) => {
    if (msg.role !== 'assistant') return msg;

    // 只在实际需要水化（补 blocks / tool_calls 状态）时才创建新对象；
    // 否则返回原引用。流式期间 smoothTextDelta 每帧都会 set 新 messages 数组，
    // 若这里对所有 assistant 消息都 `{ ...msg }` 拷贝一份，MessageBubble 的
    // memo 会在每帧失效，导致整屏重新渲染。
    let hydrated: Message = msg;
    let changed = false;

    if ((!msg.blocks || msg.blocks.length === 0) && msg.content) {
      hydrated = { ...hydrated, blocks: parseThinkFromText(msg.content) };
      changed = true;
    }
    if (msg.tool_calls?.length) {
      const newCalls: ToolCall[] = msg.tool_calls.map((toolCall): ToolCall => {
        const result = toolResults.get(toolCall.id);
        if (result) {
          const isError = result.is_error === true;
          const status: ToolCallStatus = isError ? 'error' : 'done';
          if (
            toolCall.status === status &&
            toolCall.result === result.content &&
            toolCall.is_error_result === isError &&
            toolCall.images === result.images
          ) {
            return toolCall;
          }
          return {
            ...toolCall,
            status,
            result: result.content,
            is_error_result: isError,
            images: result.images,
          };
        }

        // 后端历史中的 ToolCall 不持久化 UI 状态。找不到对应 tool
        // result 时，说明调用在进程退出或主动停止时被中断。
        if (!toolCall.status) {
          return { ...toolCall, status: 'interrupted' };
        }
        return toolCall;
      });
      if (newCalls.some((call, index) => call !== msg.tool_calls![index])) {
        hydrated = changed ? hydrated : { ...msg };
        hydrated.tool_calls = newCalls;
        changed = true;
      }
    }
    return changed ? hydrated : msg;
  });
}

export const useChatStore = create<ChatState>((set, get) => ({
  // 浏览器模式预填充 mock 消息，方便 UI 调试
  messages: isBrowser ? [...MOCK_MESSAGES] : [],
  historyOffset: 0,
  hasMoreHistory: false,
  isLoadingHistory: false,
  draftInput: '',
  draftImages: [],
  isStreaming: false,
  streamingTokens: 0,
  streamingModelId: null,
  streamingBlocks: [],
  streamingRevealCount: 0,
  streamingRevealRevision: 0,
  error: null,
  toolApproval: null,
  activeToolCalls: {},
  pendingQuestion: null,
  pendingTextBuffer: '',
  pendingTextEnd: null,
  streamDonePending: false,
  /** 设置输入框草稿文本（用于接收外部选中的文本） */
  setDraftInput: (text: string) => {
    set({ draftInput: text });
  },
  addDraftImages: async (images: ImageAttachment[]) => {
    try {
      const storedImages = isBrowser
        ? images
        : await Promise.all(
            images.map(async (image) => {
              const { saveChatImage } = await import('@/api/chat');
              return saveChatImage(image);
            }),
          );
      set((state) => ({
        draftImages: [...(state.draftImages || []), ...storedImages],
      }));
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : String(error),
      });
    }
  },
  removeDraftImage: (id: string) => {
    const target = (get().draftImages || []).find((image) => image.id === id);
    const targetPath = target?.path;
    set((state) => ({
      draftImages: (state.draftImages || []).filter((image) => image.id !== id),
    }));
    // 图片在选入时已被写入磁盘；若未发送就被移除，删除对应文件避免孤儿堆积。
    // （发送后 draftImages 被清空，此时图片随消息持久化，不走此路径。）
    if (targetPath && !isBrowser) {
      import('@/api/chat')
        .then(({ deleteChatImage }) => deleteChatImage(targetPath))
        .catch(() => {});
    }
  },
  clearDraftImages: () => {
    set({ draftImages: [] });
  },

  /**
   * 发送消息到 AI
   * 1. 创建 user 消息和空的 assistant 消息
   * 2. 设置 isStreaming = true 进入流式状态
   * 3. 浏览器模式：使用 setInterval 逐字输出 mock 回复
   * 4. Tauri 模式：调用 Rust 后端 send_message 命令
   */
  sendMessage: async (content: string, modelId: string) => {
    const { messages, draftImages = [] } = get();

    // 构建用户消息
    const userMessage: Message = {
      id: generateId(),
      role: 'user',
      content,
      images: draftImages,
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
      draftImages: [],
      isStreaming: true,
      streamingTokens: 0,
      streamingModelId: modelId,
      streamingBlocks: [],
      streamingRevealCount: 0,
      streamingRevealRevision: 0,
      pendingTextBuffer: '',
      pendingTextEnd: null,
      streamDonePending: false,
      error: null,
      activeToolCalls: {},
      toolApproval: null,
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
      const { sendMessage: sendMsg } = await import('@/api/chat');
      await sendMsg(toSend, modelId);
    } catch (e) {
      // 发送失败（参数校验/图片水化失败等）：移除刚创建的空 assistant 占位气泡，
      // 避免"幽灵气泡"留在对话里、并在下一次发送时被重新发给 API。
      const { messages: current } = get();
      const idx = findLastAssistantIdx(current);
      let updated = [...current];
      if (
        idx >= 0 &&
        idx === updated.length - 1 &&
        !updated[idx].content &&
        !(updated[idx].blocks && updated[idx].blocks.length > 0)
      ) {
        updated.splice(idx, 1);
      }
      set({
        messages: updated,
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
  // (注: 原本的 appendToken 1 行透传已删除 —— 调用方应直接用 appendTextToken)

  /**
   * 追加文本 token 到当前最后一条 assistant 消息
   * 同时更新 content 字符串（向后兼容）和 blocks 数组。
   * 改用 findLastAssistantIdx: handleToolResult 会在 messages 末尾 push role='tool' 的消息,
   * 直接用 length-1 可能命中 tool 消息,把新 token 写到 tool 消息的 content 上,造成污染。
   */
  appendTextToken: (token: string) => {
    const { messages } = get();
    const updated = [...messages];
    const idx = findLastAssistantIdx(updated);
    if (idx < 0) return;
    const lastMsg = { ...updated[idx] } as Message;
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

    updated[idx] = lastMsg;
    set({
      messages: updated,
      streamingTokens: get().streamingTokens + 1,
      streamingRevealCount: Array.from(token).length,
      streamingRevealRevision: get().streamingRevealRevision + 1,
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

  /** 追加 Rust 已规范化的正文 delta */
  handleTextDelta: (_contentIndex: number, delta: string) => {
    const { messages } = get();

    // 改用 findLastAssistantIdx: 同 appendTextToken 注释,避免覆盖 tool 消息
    const updated = [...messages];
    const idx = findLastAssistantIdx(updated);
    if (idx < 0) return;
    const lastMsg = { ...updated[idx] } as Message;
    lastMsg.content = (lastMsg.content || '') + delta;
    updated[idx] = lastMsg;

    const newBlocks = appendStreamingText(get().streamingBlocks, delta);

    set({
      messages: updated,
      streamingBlocks: newBlocks,
      streamingTokens: get().streamingTokens + 1,
    });
  },

  /** 文本块结束 */
  handleTextEnd: (contentIndex: number, content: string) => {
    if (get().pendingTextBuffer.length > 0) {
      set({ pendingTextEnd: { contentIndex, content } });
      return;
    }

    if (contentIndex === 0) {
      // OpenAI 单块模式:每个 turn 都会发 text_end(idx=0, content=本 turn 完整文本)
      // 这里解析 `content` 中的 <think>…</think> 标签,把它拆成 thinking + text 块。
      // (在 commit 788f29c 中错误地只 push 空分隔块,导致 DeepSeek/Qwen 风格响应在
      // 流式期间把 <<think>> 标签作为原始文本显示 —— 用户看 `<think>我先想一下</think>实际…`)。
      // 同时 push 一个空 text 块作为下一轮的起点分隔,避免下轮 smoothTextDelta 把字符
      // 追加到本 turn 的 text 块尾部。缓冲未清空时会在函数入口延后提交。
      const blocks = [...get().streamingBlocks];
      if (content.length > 0) {
        const parsed = parseThinkFromText(content);
        if (endsWithBlocks(blocks, parsed)) {
          blocks.push({ type: 'text', content: '' });
          set({ streamingBlocks: blocks });
          return;
        }
        // 判断传入的 content 是否是「过期快照」:多轮工具循环中 text_end 可能被渲染
        // 缓冲延迟,当缓冲最终排空时,消息里已经累积了后续轮次的文本,本 turn 的
        // content 只是累积文本的严格前缀。若用该过期快照去 splice,会覆盖掉后续轮次
        // 的文本(数据丢失)—— 这是本方法的核心防御。
        const { messages } = get();
        const lastAssistantIdx = findLastAssistantIdx(messages);
        const accumulated =
          lastAssistantIdx >= 0 ? (messages[lastAssistantIdx].content || '') : '';
        const staleSnapshot =
          accumulated.length > content.length && accumulated.startsWith(content);

        // 找到本 turn 对应的 text 块位置:
        // - 若末尾是空 text 块(说明上一轮 text_end 已推入分隔符),则倒数第二块是本 turn 的 text
        // - 否则末尾就是本 turn 的 text
        const lastIdx = blocks.length - 1;
        const isTrailingSeparator =
          lastIdx >= 0 &&
          blocks[lastIdx].type === 'text' &&
          blocks[lastIdx].content === '';
        const targetIdx = isTrailingSeparator ? lastIdx - 1 : lastIdx;
        if (targetIdx >= 0) {
          const targetBlock = blocks[targetIdx];
          if (staleSnapshot && targetBlock.type === 'text') {
            // 过期快照: 以该文本块当前累积的内容重建, 保留后续轮次的文本
            // (thinking 块由 thinking_delta / <think> 标签增量生成, 不在 content 里,
            // 因此只重建文本块, 不整体重parse)。
            blocks.splice(targetIdx, 1, ...parseThinkFromText(targetBlock.content));
          } else {
            // 替换为目标位置上的内容 (1 个或多个 block: 可能是 [thinking, text] 或 [text])
            blocks.splice(targetIdx, 1, ...parsed);
          }
        } else {
          // 极端情况: 没有任何 block —— 退化为追加
          blocks.push(...parsed);
        }
      }
      // 推入下一轮的空 text 分隔块
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
    // text_start 可能先创建一个空占位块；内联 <think> 位于正文开头时移除它，
    // 保持与 Rust 最终持久化的 blocks 结构一致。
    const placeholder = blocks[blocks.length - 1];
    if (placeholder?.type === 'text' && placeholder.content === '') {
      blocks.pop();
    }
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

  /** 流式完成：等逐字动画队列清空，再将 streamingBlocks 附加到消息 */
  handleStreamDone: () => {
    if (get().pendingTextBuffer.length > 0) {
      set({ streamDonePending: true });
      return;
    }

    const pendingEnd = get().pendingTextEnd;
    if (pendingEnd) {
      set({ pendingTextEnd: null });
      get().handleTextEnd(pendingEnd.contentIndex, pendingEnd.content);
    }

    const { messages, streamingBlocks, activeToolCalls } = get();
    const updated = [...messages];
    // 找到最后一条 assistant 消息 —— 它才是 streamingBlocks / tool_calls 的目标。
    // 注意: handleToolResult 会在 messages 末尾 push role='tool' 的消息,
    // 所以 messages 的最后一条可能是 tool 消息而不是 assistant。
    const lastAssistantIdx = findLastAssistantIdx(updated);
    if (lastAssistantIdx >= 0) {
      const lastAssistant = { ...updated[lastAssistantIdx] } as Message;
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
      // 仅保留已完成且参数完整的 tool_calls。
      // 主动停止时，calling/executing 调用仍可能只有半截 JSON，不能进入下一轮。
      const toolCallsList = getPersistableToolCalls(activeToolCalls);
      if (toolCallsList.length > 0) {
        lastAssistant.tool_calls = toolCallsList;
      } else {
        delete lastAssistant.tool_calls;
      }
      updated[lastAssistantIdx] = lastAssistant;
    }

    set({
      messages: updated,
      isStreaming: false,
      streamingTokens: 0,
      streamingModelId: null,
      streamingBlocks: [],
      pendingTextBuffer: '',
      pendingTextEnd: null,
      streamDonePending: false,
      toolApproval: null,
      activeToolCalls: {},
      pendingQuestion: null,
    });
  },

  /** 流式错误：先清缓冲再重置状态 */
  handleStreamError: (_reason: string, message: string) => {
    get().flushTextBuffer();
    // 错误时仅保留此前已经完成的调用，不能把半截调用伪装成 error 后写入历史。
    const { messages, activeToolCalls } = get();
    let updated = [...messages];
    const lastAssistantIdx = findLastAssistantIdx(updated);
    if (lastAssistantIdx >= 0) {
      const lastAssistant = updated[lastAssistantIdx];
      const isEmpty =
        !lastAssistant.content &&
        !(lastAssistant.blocks && lastAssistant.blocks.length > 0);
      // 流式立即报错且没有任何产出（无内容、无工具结果依赖它）时，移除空占位气泡，
      // 避免"幽灵气泡"留在对话里、并在下一次发送时被重新发给 API。
      if (isEmpty && lastAssistantIdx === updated.length - 1) {
        updated.splice(lastAssistantIdx, 1);
      } else {
        const toolCallsList = getPersistableToolCalls(activeToolCalls);
        const updatedAssistant = { ...lastAssistant } as Message;
        if (toolCallsList.length > 0) {
          updatedAssistant.tool_calls = toolCallsList;
        } else {
          delete updatedAssistant.tool_calls;
        }
        updated[lastAssistantIdx] = updatedAssistant;
      }
    }
    set({
      messages: updated,
      isStreaming: false,
      streamingTokens: 0,
      streamingModelId: null,
      streamingBlocks: [],
      pendingTextBuffer: '',
      pendingTextEnd: null,
      streamDonePending: false,
      error: message,
      activeToolCalls: {},
      toolApproval: null,
      pendingQuestion: null,
    });
  },

  // ── P9: 平滑文本渲染 ────────────────────────────

  /** 将 text_delta 入队到缓冲，等待 rAF 循环小批量消费 */
  feedTextDelta: (delta: string) => {
    set((s) => ({ pendingTextBuffer: s.pendingTextBuffer + delta }));
  },

  /** rAF 定时调用：从缓冲区取 count 个字符，追加到 streamingBlocks 和 messages[lastAssistant].content */
  smoothTextDelta: (count: number) => {
    const { pendingTextBuffer, messages, streamingBlocks } = get();
    if (pendingTextBuffer.length === 0) return;

    const {
      prefix: chars,
      rest,
      characterCount,
    } = takeUnicodePrefix(pendingTextBuffer, count);
    if (characterCount === 0) return;

    // 改用 findLastAssistantIdx: handleToolResult 可能把 tool 消息 push 到末尾
    const updated = [...messages];
    const idx = findLastAssistantIdx(updated);
    if (idx < 0) return;
    const lastMsg = { ...updated[idx] } as Message;
    lastMsg.content = (lastMsg.content || '') + chars;
    updated[idx] = lastMsg;

    const blocks = appendStreamingText(streamingBlocks, chars);

    set({
      pendingTextBuffer: rest,
      messages: updated,
      streamingBlocks: blocks,
      streamingTokens: get().streamingTokens + 1,
      streamingRevealCount: characterCount,
      streamingRevealRevision: get().streamingRevealRevision + 1,
    });

    if (rest.length === 0) {
      const pendingEnd = get().pendingTextEnd;
      if (pendingEnd) {
        set({ pendingTextEnd: null });
        get().handleTextEnd(pendingEnd.contentIndex, pendingEnd.content);
      }

      if (
        get().streamDonePending &&
        get().pendingTextBuffer.length === 0 &&
        !get().pendingTextEnd
      ) {
        set({ streamDonePending: false });
        get().handleStreamDone();
      }
    }
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
    // 改用 findLastAssistantIdx: 同 appendTextToken
    const updated = [...messages];
    const idx = findLastAssistantIdx(updated);
    if (idx < 0) {
      set({ isStreaming: false, streamingTokens: 0, streamingModelId: null });
      return;
    }
    const lastMsg = { ...updated[idx] } as Message;
    if (lastMsg.blocks) {
      lastMsg.blocks = lastMsg.blocks.map((b: ContentBlock) =>
        b.type === 'thinking' && b.is_open ? { ...b, is_open: false } : b,
      );
    }
    // 如果没有 blocks 但有 content，从 content 解析 <think> 标签
    if ((!lastMsg.blocks || lastMsg.blocks.length === 0) && lastMsg.content) {
      lastMsg.blocks = parseThinkFromText(lastMsg.content);
    }
    updated[idx] = lastMsg;

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

  /** 从磁盘加载最新一页历史消息，自动补全缺失的 blocks。 */
  loadMessages: async () => {
    if (isBrowser) return;
    try {
      set({ isLoadingHistory: true });
      const { getMessageCount, loadMessages: load } = await import('@/api/storage');
      const total = await getMessageCount();
      const offset = Math.max(0, total - HISTORY_PAGE_SIZE);
      const history = await load(offset, HISTORY_PAGE_SIZE);
      const loadedIds = new Set((history || []).map((message) => message.id));

      set((state) => {
        // 先合并完整消息页，再关联 tool_call 与 tool result，兼容分页边界。
        const merged = [
          ...(history || []),
          ...state.messages.filter((message) => !loadedIds.has(message.id)),
        ];
        return {
          messages: hydrateHistoryMessages(merged),
          historyOffset: offset,
          hasMoreHistory: offset > 0,
          isLoadingHistory: false,
        };
      });
    } catch (e) {
      set({ isLoadingHistory: false });
      console.error('[Buddy] 加载历史消息失败:', e);
    }
  },

  /** 加载当前最早消息之前的一页，并追加到列表开头。 */
  loadOlderMessages: async () => {
    const { historyOffset, hasMoreHistory, isLoadingHistory } = get();
    if (isBrowser || !hasMoreHistory || isLoadingHistory) return;

    try {
      set({ isLoadingHistory: true });
      const nextOffset = Math.max(0, historyOffset - HISTORY_PAGE_SIZE);
      const limit = historyOffset - nextOffset;
      const { loadMessages: load } = await import('@/api/storage');
      const history = await load(nextOffset, limit);

      set((state) => {
        const existingIds = new Set(state.messages.map((message) => message.id));
        const merged = [
          ...(history || []).filter((message) => !existingIds.has(message.id)),
          ...state.messages,
        ];
        return {
          messages: hydrateHistoryMessages(merged),
          historyOffset: nextOffset,
          hasMoreHistory: nextOffset > 0,
          isLoadingHistory: false,
        };
      });
    } catch (e) {
      set({ isLoadingHistory: false });
      console.error('[Buddy] 加载更早历史消息失败:', e);
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
   * 如果还没有任何内容，返回 -1，明确表示工具应放在第一个 block 之前。
   * 后续即使思考块到达，工具仍保持在调用发生时的正确位置。
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

  handleToolResult: (
    id: string,
    name: string,
    content: string,
    images: ImageAttachment[],
    isError: boolean,
  ) => {
    // 用一个 set + 一个本地变量,防止两次 set 的间隙被其他事件插入
    const state = get();
    const active = { ...state.activeToolCalls };
    const prev = active[id];
    if (prev) {
      active[id] = {
        ...prev,
        name,
        status: isError ? 'error' : 'done',
        result: content,
        is_error_result: isError,
        images,
      };
    } else {
      active[id] = {
        id,
        name,
        arguments: '',
        status: isError ? 'error' : 'done',
        result: content,
        is_error_result: isError,
        images,
      };
    }

    const toolMsg: Message = {
      id: 'tool-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 9),
      role: 'tool' as const,
      content,
      images,
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
  },

  handleToolApprovalRequired: (id: string, name: string, args: string, reason: string) => {
    set({ toolApproval: { id, name, arguments: args, reason } });
  },

  setToolApproval: (approval) => {
    set({ toolApproval: approval });
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
    if (!pendingQuestion) return false;
    if (isBrowser) {
      // 浏览器 mock 直接清空
      set({ pendingQuestion: null });
      return true;
    }
    try {
      const { answerToolQuestion } = await import('@/api/chat');
      await answerToolQuestion({
        id: pendingQuestion.id,
        selected,
        inputs: inputs ?? [],
        custom: custom ?? null,
      });
      set({ pendingQuestion: null });
      return true;
    } catch (e) {
      console.error('[Buddy] 回答 ask_user 失败:', e);
      return false;
    }
  },
  clearMessages: () => {
    set({
      messages: [],
      draftInput: '',
      draftImages: [],
      error: null,
      activeToolCalls: {},
      toolApproval: null,
      streamingRevealCount: 0,
      streamingRevealRevision: 0,
      pendingTextBuffer: '',
      pendingTextEnd: null,
      streamDonePending: false,
      historyOffset: 0,
      hasMoreHistory: false,
      isLoadingHistory: false,
    });
  },

  /** 设置完整的消息列表（用于加载历史对话） */
  setMessages: (messages: Message[]) => {
    set({ messages: hydrateHistoryMessages(messages) });
  },

  /** 设置错误信息并停止流式状态 */
  setError: (error: string | null) => {
    set({ error, isStreaming: false });
  },
}));
