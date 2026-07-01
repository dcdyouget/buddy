import { useState, useEffect, useRef } from 'react';
import { AnimatePresence } from 'framer-motion';
import { Settings } from 'lucide-react';
import { useUIStore } from '@/stores/uiStore';
import { useChatStore } from '@/stores/chatStore';
import { useConfigStore } from '@/stores/configStore';
import { GlassPanel } from '@/components/shared/GlassPanel';
import { IconButton } from '@/components/shared/IconButton';
import { InputDock } from '@/components/chat/InputDock';
import { MessageBubble } from '@/components/chat/MessageBubble';
import { ModelDropdown } from '@/components/chat/ModelDropdown';
import { useDragHandle } from '@/hooks/useDragHandle';
import type { ModelInfo } from '@/types';

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
  } = useChatStore();
  const { config } = useConfigStore();
  const [showDropdown, setShowDropdown] = useState(false);

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
          }}
        >
          {messages.length === 0 && (
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                height: '100%',
                color: 'var(--text-tertiary)',
                fontSize: '14px',
              }}
            >
              开始新对话
            </div>
          )}
          {messages.map((msg, i) => {
            const questionId =
              msg.role === 'assistant' && i > 0 && messages[i - 1].role === 'user'
                ? `msg-${messages[i - 1].id}`
                : undefined;
            const isLast = isStreaming && msg.role === 'assistant' && i === messages.length - 1;
            // 流式过程中将 live blocks 注入最后一条 assistant 消息
            const displayMsg =
              isLast && streamingBlocks.length > 0
                ? { ...msg, blocks: streamingBlocks }
                : msg;
            return (
              <MessageBubble
                key={msg.id}
                message={displayMsg}
                isStreaming={isLast}
                questionId={questionId}
              />
            );
          })}
        </div>

        {/* 输入区域 */}
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
