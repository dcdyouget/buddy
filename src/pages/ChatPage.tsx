import { useState, useEffect, useRef, useMemo } from 'react';
import { AnimatePresence } from 'framer-motion';
import { Settings } from 'lucide-react';
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
    })),
  );
  const { config } = useConfigStore();
  const [showDropdown, setShowDropdown] = useState(false);

  // 平滑文本渲染器：rAF 循环从缓冲队列逐字消费到 streamingBlocks
  useSmoothTextRenderer();

  // 当前选中的模型
  const selectedModel: ModelInfo | null =
    config?.models.find((m) => m.id === config?.selected_model_id) ?? null;
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

  // 消息列表变化时自动滚动到底部
  const scrollRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

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
        {/* 设置按钮 */}
        <div
          style={{
            position: 'absolute',
            top: 0,
            left: 0,
            right: 0,
            height: '24px',
            display: 'flex',
            justifyContent: 'flex-end',
            alignItems: 'center',
            padding: '0 var(--space-2)',
            zIndex: 10,
          }}
        >
          <IconButton
            icon={Settings}
            onClick={() => setPage('settings')}
            size={24}
            iconSize={14}
            title="设置"
          />
        </div>

        {/* 消息列表 */}
        <div
          ref={scrollRef}
          className="no-scrollbar"
          style={{
            flex: 1,
            overflowY: 'auto',
            padding: 'var(--space-6) 0 var(--space-4) 0',
            display: 'flex',
            flexDirection: 'column',
            justifyContent: messages.length === 0 ? 'center' : 'flex-start',
          }}
        >
          {messages.length === 0 && (
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                color: 'var(--text-tertiary)',
                fontSize: '14px',
                paddingBottom: 'var(--space-3)',
              }}
            >
              开始新对话
            </div>
          )}
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
                liveToolCalls={isLast ? liveToolCallsForLast : undefined}
                childResponses={childResponses}
              />
            );
          })}
        </div>

        {/* 输入区域 + 弹窗容器:弹窗在输入框正上方,作为正常流元素撑起消息 */}
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            padding: messages.length === 0 ? '0 var(--space-3) var(--space-4)' : undefined,
          }}
        >
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
              onModelPickerClick={
                isStreaming ? () => {} : () => setShowDropdown(!showDropdown)
              }
            />
          )}
        </div>
      </GlassPanel>

      {/* 模型选择下拉（仅非流式时可用） */}
      <AnimatePresence>
        {showDropdown && !isStreaming && (
          <ModelDropdown
            models={config?.models || []}
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
