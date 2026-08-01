import { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { AlertCircle, ChevronDown, X } from 'lucide-react';
import { useUIStore } from '@/stores/uiStore';
import {
  hydrateHistoryMessages,
  useChatStore,
} from '@/stores/chatStore';
import { useShallow } from 'zustand/react/shallow';
import { useConfigStore } from '@/stores/configStore';
import { GlassPanel } from '@/components/shared/GlassPanel';
import { IconButton } from '@/components/shared/IconButton';
import { ApprovalModal } from '@/components/shared/ApprovalModal';
import { InputDock } from '@/components/chat/InputDock';
import { MessageBubble } from '@/components/chat/MessageBubble';
import { ModelDropdown } from '@/components/chat/ModelDropdown';
import { useDragHandle } from '@/hooks/useDragHandle';
import { useSmoothTextRenderer } from '@/hooks/useSmoothTextRenderer';
import { openNativeModelMenu } from '@/utils/modelMenu';
import type { ModelInfo } from '@/types';

/** 用户离开底部超过该距离后，立即停止流式自动跟随。 */
const BOTTOM_FOLLOW_TOLERANCE = 12;

/**
 * 统一聊天页组件
 *
 * 合并了 ConversationPage（对话浏览）和 StreamingPage（流式生成）的功能。
 * 通过 chatStore.isStreaming 判断当前模式：
 * - 非流式：可发送消息、切换模型、打开模型下拉菜单
 * - 流式中：显示实时内容块、token 计数、停止按钮，禁止发送/切换模型
 */
export function ChatPage() {
  const dragRef = useDragHandle();
  const { setPage } = useUIStore();
  const [isHistoryVisible, setIsHistoryVisible] = useState(false);
  // 改用 useShallow + 细粒度选择器,避免每次 set 都触发整棵 ChatPage 树重渲染。
  // 之前 `useChatStore()` 无选择器,流式期间 smoothTextDelta 每 ~16ms set 一次,
  // 整页 + 所有 MessageBubble 子树被强行 re-render 60Hz/秒。
  const {
    messages,
    draftInput,
    draftImages: storedDraftImages,
    setDraftInput,
    addDraftImages,
    removeDraftImage,
    sendMessage,
    stopGeneration,
    isStreaming,
    streamingModelId,
    streamingBlocks,
    streamingRevealCount,
    streamingRevealRevision,
    activeToolCalls,
    pendingQuestionId,
    error,
    setError,
    hasMoreHistory,
    isLoadingHistory,
    loadOlderMessages,
  } = useChatStore(
    useShallow((s) => ({
      messages: s.messages,
      draftInput: s.draftInput,
      draftImages: s.draftImages,
      setDraftInput: s.setDraftInput,
      addDraftImages: s.addDraftImages,
      removeDraftImage: s.removeDraftImage,
      sendMessage: s.sendMessage,
      stopGeneration: s.stopGeneration,
      isStreaming: s.isStreaming,
      streamingModelId: s.streamingModelId,
      streamingBlocks: s.streamingBlocks,
      streamingRevealCount: s.streamingRevealCount,
      streamingRevealRevision: s.streamingRevealRevision,
      activeToolCalls: s.activeToolCalls,
      pendingQuestionId: s.pendingQuestion?.id ?? null,
      error: s.error,
      setError: s.setError,
      hasMoreHistory: s.hasMoreHistory,
      isLoadingHistory: s.isLoadingHistory,
      loadOlderMessages: s.loadOlderMessages,
    })),
  );
  const draftImages = storedDraftImages ?? [];
  const { config } = useConfigStore();
  const [showDropdown, setShowDropdown] = useState(false);

  // 平滑文本渲染器：rAF 循环从缓冲队列逐字消费到 streamingBlocks
  useSmoothTextRenderer();

  // 气泡展开时先让外壳完成 GPU 缩放，再挂载历史 Markdown。
  // 避免大量 Markdown / 代码块的首次解析与展开动画抢占同一帧。
  useEffect(() => {
    const timer = window.setTimeout(() => setIsHistoryVisible(true), 240);
    return () => window.clearTimeout(timer);
  }, []);

  // 当前选中的模型
  const selectedModel: ModelInfo | null =
    config?.models.find((m) => m.id === config?.selected_model_id) ?? null;
  const enabledModels = (config?.models || []).filter((model) =>
    config?.providers.some(
      (provider) =>
        provider.id === model.provider_id &&
        provider.enabled_model_ids.includes(model.id),
    ),
  );
  // 当前正在流式输出的模型
  const streamingModel = config?.models.find((m) => m.id === streamingModelId);

  /** 发送消息：校验后发起对话 */
  const handleSend = () => {
    if ((!draftInput.trim() && draftImages.length === 0) || !config?.selected_model_id) {
      return;
    }
    if (draftImages.length > 0 && !selectedModel?.supports_vision) {
      setError('当前模型不支持图片，请移除图片或切换模型');
      return;
    }
    sendMessage(draftInput, config.selected_model_id);
  };

  /** 停止生成并保持在对话页 */
  const handleStop = () => {
    stopGeneration();
  };

  const handleModelPickerClick = async () => {
    if (!showDropdown && config) {
      const openedNativeMenu = await openNativeModelMenu({
        models: enabledModels,
        selectedId: config.selected_model_id,
        onSelect: (id) => useConfigStore.getState().setDefaultModel(id),
      });
      if (openedNativeMenu) return;
    }
    setShowDropdown((open) => !open);
  };

  // ── 智能滚动：仅当用户在底部时才自动跟随，翻看历史时不强拉 ──
  const scrollRef = useRef<HTMLDivElement>(null);
  const [isAtBottom, setIsAtBottom] = useState(true);
  // 用 ref 跟踪上一次的 isAtBottom 和 showScrollButton，避免重复打 log
  const prevIsAtBottomRef = useRef(true);
  const prevShowButtonRef = useRef(false);
  const isLoadingOlderRef = useRef(false);
  const lastSeenMessageCountRef = useRef(messages.length);

  /** 判断滚动容器是否仍在底部附近。容差保持很小，便于用户立即接管滚动。 */
  const checkAtBottom = useCallback((): boolean => {
    const el = scrollRef.current;
    if (!el) return true;
    return el.scrollHeight - el.scrollTop - el.clientHeight < BOTTOM_FOLLOW_TOLERANCE;
  }, []);

  /** 用户手动滚动时更新 isAtBottom 状态 */
  const loadOlderHistory = useCallback(async () => {
    const el = scrollRef.current;
    if (!el || !hasMoreHistory || isLoadingOlderRef.current) return;

    isLoadingOlderRef.current = true;
    const previousHeight = el.scrollHeight;
    const previousTop = el.scrollTop;
    await loadOlderMessages();

    requestAnimationFrame(() => {
      const heightDelta = el.scrollHeight - previousHeight;
      el.scrollTop = previousTop + heightDelta;
      isLoadingOlderRef.current = false;
    });
  }, [hasMoreHistory, loadOlderMessages]);

  const handleScroll = useCallback(() => {
    const atBottom = checkAtBottom();
    if (atBottom !== prevIsAtBottomRef.current) {
      console.log('[Scroll] isAtBottom:', prevIsAtBottomRef.current, '→', atBottom,
        `(scrollTop=${scrollRef.current?.scrollTop}, scrollHeight=${scrollRef.current?.scrollHeight}, clientHeight=${scrollRef.current?.clientHeight})`);
      prevIsAtBottomRef.current = atBottom;
    }
    setIsAtBottom(atBottom);

    if (scrollRef.current && scrollRef.current.scrollTop <= 56) {
      void loadOlderHistory();
    }
  }, [checkAtBottom, loadOlderHistory]);

  /**
   * 仅在用户处于底部时才自动跟随新内容。
   * 思考过程和流式正文写入 streamingBlocks，而非 messages；若不监听它，
   * 新一轮的首个思考卡片会在消息创建后才插入，停留在输入栏下方。
   */
  useEffect(() => {
    if (scrollRef.current && isAtBottom) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [
    messages,
    streamingBlocks,
    pendingQuestionId,
    isAtBottom,
    isHistoryVisible,
  ]);

  /**
   * ask_user 的交互内容由 ToolSection 自己管理，选择选项或出现补充输入框时
   * 不会改动 messages / streamingBlocks。监听最后一条消息的实际高度，
   * 让处于底部跟随状态的用户始终能看到完整卡片。
   */
  useEffect(() => {
    const scrollElement = scrollRef.current;
    if (!scrollElement || !isHistoryVisible) return;

    const rows = scrollElement.querySelectorAll<HTMLElement>('.message-row');
    const lastRow = rows.item(rows.length - 1);
    if (!lastRow) return;

    let frameId = 0;
    const followLastRow = () => {
      if (!prevIsAtBottomRef.current) return;
      window.cancelAnimationFrame(frameId);
      frameId = window.requestAnimationFrame(() => {
        const current = scrollRef.current;
        if (current && prevIsAtBottomRef.current) {
          current.scrollTop = current.scrollHeight;
        }
      });
    };

    const observer = new ResizeObserver(followLastRow);
    observer.observe(lastRow);
    followLastRow();

    return () => {
      observer.disconnect();
      window.cancelAnimationFrame(frameId);
    };
  }, [messages.length, isHistoryVisible]);

  /** 流式开始时重置为跟随模式 */
  useEffect(() => {
    if (isStreaming) {
      console.log('[Scroll] 流式开始，重置 isAtBottom = true');
      setIsAtBottom(true);
      prevIsAtBottomRef.current = true;
    }
  }, [isStreaming]);

  /** 到达底部即将现有消息标记为已查看。 */
  useEffect(() => {
    if (isAtBottom) {
      lastSeenMessageCountRef.current = messages.length;
    }
  }, [isAtBottom, messages.length]);

  /** 手动滚动到底部 */
  const scrollToBottom = () => {
    console.log('[Scroll] 用户点击"滚动到底部"按钮');
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
      setIsAtBottom(true);
      prevIsAtBottomRef.current = true;
    }
  };

  // 滚动到底按钮：流式结束后、用户翻看历史时显示
  const showScrollButton = !isAtBottom && !isStreaming && messages.length > 0;
  const hasUnseenMessages =
    !isAtBottom && messages.length > lastSeenMessageCountRef.current;

  // 按钮显隐变化时打印日志（避免每帧 render 都打）
  if (showScrollButton !== prevShowButtonRef.current) {
    console.log('[Scroll] showScrollButton:', prevShowButtonRef.current, '→', showScrollButton,
      `(isAtBottom=${isAtBottom}, isStreaming=${isStreaming}, msgCount=${messages.length})`);
    prevShowButtonRef.current = showScrollButton;
  }

  // 把『每帧重算』的两份数据移出 render: 60Hz 期间 messages 引用会变,
  // 但 visible + childByParent 的内容只在 messages 真正变化时才变。
  // 用 useMemo 把它们 memoize 住,避免每帧都重新 filter + 建 Map。
  const visible = useMemo(
    () =>
      hydrateHistoryMessages(messages).filter(
        (message) => message.role !== 'tool',
      ),
    [messages],
  );

  // 每条 assistant 消息向上查找最近一条 user 消息。
  // 原来在 render 里对每条消息做 arr.slice(0,i).reverse().find(...)，是 O(n²)；
  // 改为一次遍历建 Map（O(n)），只在 visible 变化时重算，流式期间不重算。
  const previousUserIds = useMemo(() => {
    const map = new Map<string, string>();
    let lastUserId: string | null = null;
    for (const message of visible) {
      if (message.role === 'user') {
        lastUserId = message.id;
      } else if (message.role === 'assistant' && lastUserId) {
        map.set(message.id, lastUserId);
      }
    }
    return map;
  }, [visible]);

  // liveToolCalls 引用稳定: 仅当 activeToolCalls 真的变化时,Object.values 重新计算。
  // 之前每帧 `Object.values(activeToolCalls)` 分配新数组,让 MessageBubble memo 永远失效。
  const liveToolCallsForLast = useMemo(
    () =>
      isStreaming && Object.keys(activeToolCalls).length > 0
        ? Object.values(activeToolCalls)
        : undefined,
    [isStreaming, activeToolCalls],
  );

  return (
    <div
      ref={dragRef}
      style={{
        width: '100vw',
        height: '100vh',
        display: 'flex',
        flexDirection: 'column',
        background: 'transparent',
        position: 'relative',
      }}
    >
      <GlassPanel
        className="buddy-shell conversation-shell"
        style={{
          flex: 1,
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
          margin: 0,
          borderRadius: 'var(--radius-xl)',
          position: 'relative',
        }}
      >
        {/* 消息列表 */}
        <div
          ref={scrollRef}
          className="no-scrollbar"
          onScroll={handleScroll}
          style={{
            flex: 1,
            minHeight: 0,
            overflowY: 'auto',
            padding: 'var(--space-3) 0 var(--space-2)',
            display: 'flex',
            flexDirection: 'column',
            justifyContent: 'flex-start',
            position: 'relative',
          }}
        >
          {isLoadingHistory && hasMoreHistory && (
            <div
              style={{
                padding: 'var(--space-2) var(--space-4)',
                color: 'var(--text-tertiary)',
                fontSize: 'var(--font-size-xs)',
                textAlign: 'center',
              }}
            >
              正在加载更早消息…
            </div>
          )}

          {/* tool 消息是内部消息，不单独展示。visible / liveToolCallsForLast
              已在 useMemo 中算好，此处只做纯映射。 */}
          {isHistoryVisible && visible.map((msg, i, arr) => {
            // 工具消息不会单独显示，工具循环后的最终 assistant 消息在可见列表中
            // 可能紧跟另一条 assistant。向上按钮需跨过这些续段，定位到本轮原始问题。
            const previousUserId = msg.role === 'assistant' ? previousUserIds.get(msg.id) : undefined;
            const questionId = previousUserId ? `msg-${previousUserId}` : undefined;
            const isLast = isStreaming && msg.role === 'assistant' && i === arr.length - 1;
            const isLatestAssistant =
              msg.role === 'assistant' && i === arr.length - 1;
            // 工具循环会连续产生多个 assistant 消息；视觉上应作为同一条回答紧凑衔接。
            const isContinuation =
              msg.role === 'assistant' && i > 0 && arr[i - 1].role === 'assistant';
            const continuesToNext =
              msg.role === 'assistant' &&
              i < arr.length - 1 &&
              arr[i + 1].role === 'assistant';
            // 流式过程中将 live blocks 注入最后一条 assistant 消息
            const displayMsg =
              isLast && streamingBlocks.length > 0
                ? { ...msg, blocks: streamingBlocks }
                : msg;
            // 流式最后一条:把 chatStore 累积的 live tool_calls 注入显示
            // (liveToolCallsForLast 引用稳定,只有 activeToolCalls 真变时才更新)
            return (
              <MessageBubble
                key={msg.id}
                message={displayMsg}
                isStreaming={isLast}
                questionId={questionId}
                isContinuation={isContinuation}
                continuesToNext={continuesToNext}
                liveToolCalls={isLast ? liveToolCallsForLast : undefined}
                streamingRevealCount={
                  isLatestAssistant ? streamingRevealCount : 0
                }
                streamingRevealRevision={
                  isLatestAssistant ? streamingRevealRevision : 0
                }
              />
            );
          })}

          {/* 滚动到底按钮：轻量浮动圆形按钮，仅在用户翻看历史且不在流式中显示 */}
          <AnimatePresence>
            {showScrollButton && (
              <motion.button
                className={`scroll-to-bottom-button ${hasUnseenMessages ? 'has-new-message' : ''}`}
                initial={{ opacity: 0, scale: 0.8 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.8 }}
                transition={{ duration: 0.15 }}
                onClick={scrollToBottom}
                title="滚动到底部"
                style={{
                  position: 'absolute',
                  bottom: 'var(--space-4)',
                  right: 'var(--space-4)',
                  width: 'var(--space-8)',
                  height: 'var(--space-8)',
                  borderRadius: 'var(--radius-full)',
                  background: 'var(--bg-elevated)',
                  border: '1px solid var(--border-default)',
                  boxShadow: 'var(--shadow-floating-sm)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  cursor: 'pointer',
                  zIndex: 20,
                  color: 'var(--text-muted)',
                }}
              >
                <ChevronDown size={16} />
              </motion.button>
            )}
          </AnimatePresence>
        </div>

        {/* 输入区域 + 弹窗容器：弹窗悬浮在输入框上方，不压缩消息列表 */}
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
          }}
        >
          {error && (
            <div className="chat-error" role="alert">
              <AlertCircle size={15} />
              <span>{error}</span>
              <IconButton
                icon={X}
                onClick={() => setError(null)}
                size={24}
                iconSize={13}
                title="关闭错误提示"
              />
            </div>
          )}

          {/* Approval 弹窗(工具调用审批) */}
          <ApprovalModal />

          <InputDock
            isStreaming={isStreaming}
            streamingModelName={streamingModel?.display_name}
            selectedModel={selectedModel}
            draftInput={draftInput}
            draftImages={draftImages}
            onDraftChange={setDraftInput}
            onAddImages={addDraftImages}
            onRemoveImage={removeDraftImage}
            onAttachmentError={setError}
            onSend={isStreaming ? () => {} : handleSend}
            onStop={handleStop}
            onModelPickerClick={isStreaming ? undefined : handleModelPickerClick}
            onSettingsClick={() => setPage('settings')}
          />
        </div>
      </GlassPanel>

      {/* 模型选择下拉（仅非流式时可用） */}
      <AnimatePresence>
        {showDropdown && !isStreaming && (
          <ModelDropdown
            models={enabledModels}
            selectedId={config?.selected_model_id || ''}
            onSelect={(id) => {
              useConfigStore.getState().setDefaultModel(id);
            }}
            onClose={() => setShowDropdown(false)}
          />
        )}
      </AnimatePresence>
    </div>
  );
}
