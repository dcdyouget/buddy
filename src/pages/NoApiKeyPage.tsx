import { useUIStore } from '@/stores/uiStore';
import { GlassPanel } from '@/components/shared/GlassPanel';
import { handleDragStart } from '@/utils/windowDrag';

/**
 * 无密钥页组件
 *
 * 当用户尚未配置 API Key 时展示此页面。
 * 点击面板可跳转到设置页进行密钥配置。
 *
 * 无 props —— 仅依赖全局 store 判断是否需要跳转。
 */
export function NoApiKeyPage() {
  const { setPage } = useUIStore();

  /** 跳转到设置页，并同步调整窗口尺寸以匹配设置页面的布局 */
  const goSettings = () => {
    setPage('settings');
  };

  return (
    <div
      onMouseDown={handleDragStart}
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
          width: 520,
          minHeight: 60,
          padding: 'var(--space-3) var(--space-4)',
          display: 'flex',
          alignItems: 'center',
          gap: 'var(--space-3)',
          cursor: 'pointer',
        }}
      >
        {/* B logo */}
        <div
          style={{
            width: 24,
            height: 24,
            borderRadius: 'var(--radius-sm)',
            background: 'var(--buddy-primary)',
            color: 'var(--text-on-primary)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontSize: '12px',
            fontWeight: 800,
            flexShrink: 0,
          }}
        >
          B
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
          设置 →
        </span>
      </GlassPanel>
    </div>
  );
}
