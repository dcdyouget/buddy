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
            onClick={goSettings}
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
          {messages.map((msg) => (
            <MessageBubble key={msg.id} message={msg} />
          ))}
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
