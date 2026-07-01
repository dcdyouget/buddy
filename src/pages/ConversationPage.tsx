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
import { useDragHandle } from '@/utils/windowDrag';
import type { ModelInfo } from '@/types';

/**
 * 对话页组件
 *
 * 展示完整的对话历史（用户消息 + AI 回复），并提供输入框继续对话。
 * 用户可在此页面浏览历史消息、发送新消息、切换模型或进入设置。
 * 发送新消息后自动跳转到流式页（StreamingPage）以展示实时生成过程。
 *
 * 无 props —— 所需状态全部来自全局 store（uiStore / chatStore / configStore）。
 */
export function ConversationPage() {
  const dragRef = useDragHandle();
  const { setPage } = useUIStore();
  const { messages, draftInput, setDraftInput, sendMessage, stopGeneration, isStreaming } =
    useChatStore();
  const { config } = useConfigStore();
  const [showDropdown, setShowDropdown] = useState(false);

  // 从配置中查找当前选中的模型信息
  const selectedModel: ModelInfo | null =
    config?.models.find((m) => m.id === config?.selected_model_id) ?? null;

  /** 处理发送消息：校验输入后发起对话，并跳转到流式页展示实时生成 */
  const handleSend = () => {
    // 输入为空或未选择模型时不发送
    if (!draftInput.trim() || !config?.selected_model_id) return;
    sendMessage(draftInput, config.selected_model_id);
    setPage('streaming');
  };

  /** 跳转到设置页面 */
  const goSettings = () => {
    setPage('settings');
  };

  // 消息列表发生变化时自动滚动到底部
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
            onClick={goSettings}
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
          {/* 无消息时显示空态提示 */}
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
          {/* 渲染历史消息列表 */}
          {messages.map((msg, i) => {
            // 找到此 AI 消息对应的用户问题（前一条消息）
            const questionId =
              msg.role === 'assistant' && i > 0 && messages[i - 1].role === 'user'
                ? `msg-${messages[i - 1].id}`
                : undefined;
            return <MessageBubble key={msg.id} message={msg} questionId={questionId} />;
          })}
        </div>

        {/* 底部输入区域 */}
        <InputDock
          isStreaming={isStreaming}
          selectedModel={selectedModel}
          draftInput={draftInput}
          onDraftChange={setDraftInput}
          onSend={handleSend}
          onStop={stopGeneration}
          onModelPickerClick={() => setShowDropdown(!showDropdown)}
        />
      </GlassPanel>

      {/* 模型选择下拉菜单 */}
      <AnimatePresence>
        {showDropdown && (
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
