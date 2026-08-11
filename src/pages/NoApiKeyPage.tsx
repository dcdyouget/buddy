import { ChevronRight, KeyRound } from 'lucide-react';
import { useUIStore } from '@/stores/uiStore';
import { GlassPanel } from '@/components/shared/GlassPanel';
import { useDragHandle } from '@/hooks/useDragHandle';

/**
 * 无密钥页组件
 *
 * 当用户尚未配置 API Key 时展示此页面。
 * 点击面板可跳转到设置页进行密钥配置。
 * 如果配置已有效，自动跳回对话页。
 *
 * 无 props —— 仅依赖全局 store 判断是否需要跳转。
 */
export function NoApiKeyPage() {
  const dragRef = useDragHandle();
  const { setPage } = useUIStore();

  /** 跳转到设置页，并同步调整窗口尺寸以匹配设置页面的布局 */
  const goSettings = () => {
    setPage('settings');
  };

  return (
    <div
      ref={dragRef}
      style={{
        width: '100vw',
        height: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'transparent',
      }}
    >
      <GlassPanel
        onClick={goSettings}
        style={{
          width: '100%',
          minHeight: 60,
          padding: 'var(--space-3) var(--space-4)',
          display: 'flex',
          alignItems: 'center',
          gap: 'var(--space-3)',
          cursor: 'pointer',
        }}
      >
        <div className="brand-mark">
          <KeyRound size={14} />
        </div>

        <span
          style={{
            color: 'var(--state-error)',
            fontSize: '14px',
            fontWeight: 500,
            flex: 1,
          }}
        >
          请先设置 API Key
        </span>

        <span
          style={{
            color: 'var(--state-error)',
            fontSize: '14px',
            fontWeight: 600,
            flexShrink: 0,
          }}
        >
          设置
        </span>
        <ChevronRight size={16} style={{ color: 'var(--state-error)' }} />
      </GlassPanel>
    </div>
  );
}
