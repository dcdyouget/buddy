import { useEffect, useRef } from 'react';
import { Settings } from 'lucide-react';
import { useUIStore } from '@/stores/uiStore';
import { useChatStore } from '@/stores/chatStore';
import { useConfigStore } from '@/stores/configStore';
import { GlassPanel } from '@/components/shared/GlassPanel';
import { IconButton } from '@/components/shared/IconButton';
import { InputDock } from '@/components/chat/InputDock';
import { MessageBubble } from '@/components/chat/MessageBubble';
import { useDragHandle } from '@/utils/windowDrag';
import type { ModelInfo } from '@/types';

/**
 * 流式页组件
 *
 * 在 AI 生成回复时展示，实时显示逐 token 输出的消息内容。
 * 与 ConversationPage 结构相似，但输入区域会展示流式状态指示器（模型名称、token 计数）。
 * 用户可点击停止按钮中断生成，之后自动回到对话页。
 *
 * 无 props —— 所需状态全部来自全局 store。
 */
export function StreamingPage() {
  const dragRef = useDragHandle();
  const { setPage } = useUIStore();
  const {
    messages,
    draftInput,
    setDraftInput,
    stopGeneration,
    isStreaming,
    streamingTokens,
    streamingModelId,
  } = useChatStore();
  const { config } = useConfigStore();

  // 从配置中查找当前选中的模型信息（用于输入区域默认显示）
  const selectedModel: ModelInfo | null =
    config?.models.find((m) => m.id === config?.selected_model_id) ?? null;
  // 查找当前正在流式输出的模型信息（用于展示流式状态）
  const streamingModel = config?.models.find((m) => m.id === streamingModelId);

  // 消息列表发生变化时自动滚动到底部，确保最新 token 始终可见
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
        {/* 顶部：设置按钮（浮动，不占布局空间） */}
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
            onClick={() => { setPage('settings'); }}
            size={24}
            iconSize={14}
            title="设置"
          />
        </div>

        {/* 消息列表区域，支持滚动 */}
        <div
          ref={scrollRef}
          className="no-scrollbar"
          style={{
            flex: 1,
            overflowY: 'auto',
            padding: 'var(--space-6) 0 var(--space-4) 0',
          }}
        >
          {messages.map((msg, i) => {
            const questionId =
              msg.role === 'assistant' && i > 0 && messages[i - 1].role === 'user'
                ? `msg-${messages[i - 1].id}`
                : undefined;
            return (
              <MessageBubble
                key={msg.id}
                message={msg}
                isStreaming={
                  isStreaming &&
                  msg.role === 'assistant' &&
                  i === messages.length - 1
                }
                questionId={questionId}
              />
            );
          })}
        </div>

        {/* 底部输入区域：流式进行中，展示模型名称和 token 计数 */}
        <InputDock
          isStreaming={isStreaming}
          streamingModelName={streamingModel?.display_name}
          streamingTokens={streamingTokens}
          selectedModel={selectedModel}
          draftInput={draftInput}
          onDraftChange={setDraftInput}
          onSend={() => {}} // 流式进行中不允许发送新消息
          onStop={() => {
            // 停止生成后回到对话页
            stopGeneration();
            setPage('conversation');
          }}
          onModelPickerClick={() => {}} // 流式进行中不允许切换模型
        />
      </GlassPanel>
    </div>
  );
}
