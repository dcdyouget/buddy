import { useEffect, useRef } from 'react';
import { Settings } from 'lucide-react';
import { useUIStore } from '@/stores/uiStore';
import { useChatStore } from '@/stores/chatStore';
import { useConfigStore } from '@/stores/configStore';
import { GlassPanel } from '@/components/shared/GlassPanel';
import { IconButton } from '@/components/shared/IconButton';
import { InputDock } from '@/components/chat/InputDock';
import { MessageBubble } from '@/components/chat/MessageBubble';
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
        }}
      >
        {/* 顶部工具栏：设置按钮 + 拖拽区域 */}
        <div
          style={{
            display: 'flex',
            justifyContent: 'flex-end',
            padding: 'var(--space-2) var(--space-3)',
            borderBottom: '1px solid var(--border-subtle)',
          }}
          data-tauri-drag-region
        >
          <IconButton
            icon={Settings}
            onClick={() => { setPage('settings'); }}
            size={28}
            iconSize={16}
            title="设置"
          />
        </div>

        {/* 消息列表区域，支持滚动 */}
        <div
          ref={scrollRef}
          style={{
            flex: 1,
            overflowY: 'auto',
            padding: 'var(--space-4) 0',
          }}
        >
          {messages.map((msg, i) => (
            <MessageBubble
              key={msg.id}
              message={msg}
              isStreaming={
                // 仅当消息为 assistant 角色、是最后一条、且正在流式输出时标记为流式中
                isStreaming &&
                msg.role === 'assistant' &&
                i === messages.length - 1
              }
            />
          ))}
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
