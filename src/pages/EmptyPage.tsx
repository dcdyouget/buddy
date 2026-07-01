import { useState } from 'react';
import { AnimatePresence } from 'framer-motion';
import { useUIStore } from '@/stores/uiStore';
import { useChatStore } from '@/stores/chatStore';
import { useConfigStore } from '@/stores/configStore';
import { GlassPanel } from '@/components/shared/GlassPanel';
import { InputDock } from '@/components/chat/InputDock';
import { ModelDropdown } from '@/components/chat/ModelDropdown';
import { useDragHandle } from '@/utils/windowDrag';
import type { ModelInfo } from '@/types';

/**
 * 空态页组件
 *
 * 应用启动后、用户尚未开始任何对话时显示的首屏页面。
 * 居中展示一个输入框，用户可直接输入消息发起对话。
 * 如果未配置 API Key 或模型，则会自动跳转到无密钥页面。
 *
 * 无 props —— 所需状态全部来自全局 store。
 */
export function EmptyPage() {
  const dragRef = useDragHandle();
  const { setPage } = useUIStore();
  const { sendMessage, draftInput, setDraftInput } = useChatStore();
  const { config } = useConfigStore();
  const [showDropdown, setShowDropdown] = useState(false);

  // 从配置中查找当前选中的模型信息
  const selectedModel: ModelInfo | null =
    config?.models.find((m) => m.id === config?.selected_model_id) ?? null;

  /** 处理发送消息：校验输入、检查配置完整性，然后发起对话并跳转到流式页 */
  const handleSend = () => {
    // 空白输入不发送
    if (!draftInput.trim()) return;
    // 未配置 provider 或未选择模型时，跳转到 API Key 设置页
    if (!config || config.providers.length === 0 || !config.selected_model_id) {
      setPage('noapikey');
      return;
    }
    // 发送消息并跳转到流式页
    sendMessage(draftInput, config.selected_model_id);
    setPage('streaming');
  };

  return (
    <div
      ref={dragRef}
      style={{
        width: '100vw',
        height: '100vh',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'transparent',
        gap: 'var(--space-4)',
      }}
    >
      <GlassPanel
        style={{
          width: 520,
          minHeight: 60,
          padding: 'var(--space-2) var(--space-4)',
          display: 'flex',
          alignItems: 'center',
        }}
      >
        <InputDock
          isStreaming={false}
          selectedModel={selectedModel}
          draftInput={draftInput}
          onDraftChange={setDraftInput}
          onSend={handleSend}
          onStop={() => {}} // 空态页无流式进行中，stop 为空操作
        />
      </GlassPanel>

      {/* 模型选择下拉菜单，仅在点击模型切换按钮时显示 */}
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
