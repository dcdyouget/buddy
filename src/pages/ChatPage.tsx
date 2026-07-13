import { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import { AlertCircle, ChevronDown, X } from 'lucide-react';
import { useUIStore } from '@/stores/uiStore';
import { useChatStore } from '@/stores/chatStore';
import { useShallow } from 'zustand/react/shallow';
import { useConfigStore } from '@/stores/configStore';
import { GlassPanel } from '@/components/shared/GlassPanel';
import { IconButton } from '@/components/shared/IconButton';
import { ApprovalModal } from '@/components/shared/ApprovalModal';
import { QuestionModal } from '@/components/shared/QuestionModal';
import { InputDock } from '@/components/chat/InputDock';
import { UserResponseInput } from '@/components/chat/UserResponseInput';
import { MessageBubble } from '@/components/chat/MessageBubble';
import { ModelDropdown } from '@/components/chat/ModelDropdown';
import { useDragHandle } from '@/hooks/useDragHandle';
import { useSmoothTextRenderer } from '@/hooks/useSmoothTextRenderer';
import { openNativeModelMenu } from '@/utils/modelMenu';
import type { Message, ModelInfo } from '@/types';

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
  // 改用 useShallow + 细粒度选择器,避免每次 set 都触发整棵 ChatPage 树重渲染。
  // 之前 `useChatStore()` 无选择器,流式期间 smoothTextDelta 每 ~16ms set 一次,
  // 整页 + 所有 MessageBubble 子树被强行 re-render 60Hz/秒。
  const {
    messages,
    draftInput,
    setDraftInput,
    sendMessage,
    stopGeneration,
    isStreaming,
    streamingTokens,
    streamingModelId,
    streamingBlocks,
    activeToolCalls,
    waitingForResponse,
    error,
    setError,
  } = useChatStore(
    useShallow((s) => ({
      messages: s.messages,
      draftInput: s.draftInput,
      setDraftInput: s.setDraftInput,
      sendMessage: s.sendMessage,
      stopGeneration: s.stopGeneration,
      isStreaming: s.isStreaming,
      streamingTokens: s.streamingTokens,
      streamingModelId: s.streamingModelId,
      streamingBlocks: s.streamingBlocks,
      activeToolCalls: s.activeToolCalls,
      waitingForResponse: s.waitingForResponse,
      error: s.error,
      setError: s.setError,
    })),
  );
  const { config } = useConfigStore();
  const [showDropdown, setShowDropdown] = useState(false);

  // 平滑文本渲染器：rAF 循环从缓冲队列逐字消费到 streamingBlocks
  useSmoothTextRenderer();

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
    if (!draftInput.trim() || !config?.selected_model_id) return;
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

  /** 判断滚动容器是否在底部（50px 容差） */
  const checkAtBottom = useCallback((): boolean => {
    const el = scrollRef.current;
    if (!el) return true;
    return el.scrollHeight - el.scrollTop - el.clientHeight < 50;
  }, []);

  /** 用户手动滚动时更新 isAtBottom 状态 */
  const handleScroll = useCallback(() => {
    const atBottom = checkAtBottom();
    if (atBottom !== prevIsAtBottomRef.current) {
      console.log('[Scroll] isAtBottom:', prevIsAtBottomRef.current, '→', atBottom,
        `(scrollTop=${scrollRef.current?.scrollTop}, scrollHeight=${scrollRef.current?.scrollHeight}, clientHeight=${scrollRef.current?.clientHeight})`);
      prevIsAtBottomRef.current = atBottom;
    }
    setIsAtBottom(atBottom);
  }, [checkAtBottom]);

  /** 仅在用户处于底部时才自动跟随新消息 */
  useEffect(() => {
    if (scrollRef.current && isAtBottom) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages, isAtBottom]);

  /** 流式开始时重置为跟随模式 */
  useEffect(() => {
    if (isStreaming) {
      console.log('[Scroll] 流式开始，重置 isAtBottom = true');
      setIsAtBottom(true);
      prevIsAtBottomRef.current = true;
    }
  }, [isStreaming]);

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

  // 按钮显隐变化时打印日志（避免每帧 render 都打）
  if (showScrollButton !== prevShowButtonRef.current) {
    console.log('[Scroll] showScrollButton:', prevShowButtonRef.current, '→', showScrollButton,
      `(isAtBottom=${isAtBottom}, isStreaming=${isStreaming}, msgCount=${messages.length})`);
    prevShowButtonRef.current = showScrollButton;
  }

  // 把『每帧重算』的两份数据移出 render: 60Hz 期间 messages 引用会变,
  // 但 visible + childByParent 的内容只在 messages 真正变化时才变。
  // 用 useMemo 把它们 memoize 住,避免每帧都重新 filter + 建 Map。
  const { visible, childByParent } = useMemo(() => {
    const v: Message[] = messages.filter(
      (m) => m.role !== 'tool' && !m.parent_message_id,
    );
    const cbp = new Map<string, Message[]>();
    for (const m of messages) {
      if (m.parent_message_id) {
        const arr = cbp.get(m.parent_message_id) || [];
        arr.push(m);
        cbp.set(m.parent_message_id, arr);
      }
    }
    return { visible: v, childByParent: cbp };
  }, [messages]);

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
            overflowY: 'auto',
            padding: 'var(--space-3) 0 var(--space-2)',
            display: 'flex',
            flexDirection: 'column',
            justifyContent: 'flex-start',
            position: 'relative',
          }}
        >
          {/* tool 消息是内部消息，不展示给用户;parent_message_id 非空的用户回应
              也不在主列表渲染，而是嵌套到对应的 assistant 消息内。
              visible / childByParent / liveToolCallsForLast 已在 useMemo 中算好,
              此处只做纯映射,不再 filter / Object.values。 */}
          {visible.map((msg, i, arr) => {
            const questionId =
              msg.role === 'assistant' && i > 0 && arr[i - 1].role === 'user'
                ? `msg-${arr[i - 1].id}`
                : undefined;
            const isLast = isStreaming && msg.role === 'assistant' && i === arr.length - 1;
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
            const childResponses =
              msg.role === 'assistant' ? childByParent.get(msg.id) : undefined;
            return (
              <MessageBubble
                key={msg.id}
                message={displayMsg}
                isStreaming={isLast}
                questionId={questionId}
                isContinuation={isContinuation}
                continuesToNext={continuesToNext}
                liveToolCalls={isLast ? liveToolCallsForLast : undefined}
                childResponses={childResponses}
              />
            );
          })}

          {/* 滚动到底按钮：轻量浮动圆形按钮，仅在用户翻看历史且不在流式中显示 */}
          <AnimatePresence>
            {showScrollButton && (
              <motion.button
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
                  width: '32px',
                  height: '32px',
                  borderRadius: '50%',
                  background: 'var(--bg-elevated)',
                  border: '1px solid var(--border-default)',
                  boxShadow: 'var(--shadow-floating-sm)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  cursor: 'pointer',
                  zIndex: 20,
                  color: 'var(--text-muted)',
                  transition: 'color 0.15s, background 0.15s',
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.color = 'var(--text-primary)';
                  e.currentTarget.style.background = 'var(--bg-surface)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.color = 'var(--text-muted)';
                  e.currentTarget.style.background = 'var(--bg-elevated)';
                }}
              >
                <ChevronDown size={16} />
              </motion.button>
            )}
          </AnimatePresence>
        </div>

        {/* 输入区域 + 弹窗容器:弹窗在输入框正上方,作为正常流元素撑起消息 */}
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
          {/* ask_user 弹窗(模型问题时) */}
          <QuestionModal />

          {waitingForResponse ? (
            <UserResponseInput />
          ) : (
            <InputDock
              isStreaming={isStreaming}
              streamingModelName={streamingModel?.display_name}
              streamingTokens={streamingTokens}
              selectedModel={selectedModel}
              draftInput={draftInput}
              onDraftChange={setDraftInput}
              onSend={isStreaming ? () => {} : handleSend}
              onStop={handleStop}
              onModelPickerClick={isStreaming ? undefined : handleModelPickerClick}
              onSettingsClick={() => setPage('settings')}
            />
          )}
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
              setShowDropdown(false);
            }}
            onClose={() => setShowDropdown(false)}
          />
        )}
      </AnimatePresence>
    </div>
  );
}
