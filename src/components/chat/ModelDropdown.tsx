import { useEffect, useRef } from 'react';
import { motion } from 'framer-motion';
import { StatusDot } from '@/components/shared/StatusDot';
import type { ModelInfo } from '@/types';

/** 模型下拉菜单的 Props */
interface ModelDropdownProps {
  /** 所有可选的模型列表 */
  models: ModelInfo[];
  /** 当前默认模型的 ID */
  selectedId: string;
  /** 选择模型后的回调，传入模型 ID */
  onSelect: (modelId: string) => void;
  /** 关闭下拉菜单的回调（Esc 或点击外部） */
  onClose: () => void;
}

/**
 * 模型下拉菜单
 * 以浮动面板形式展示所有可用模型，支持选择默认模型。
 * - 按 Esc 或点击面板外部自动关闭
 * - 过滤 enabled 为 false 的模型，仅显示已启用的
 * - 每条模型行包含：提供商图标、名称、默认标签、上下文窗口大小、延迟状态
 * - 带缩放 + 淡入淡出的进场/退场动画
 */
export function ModelDropdown({
  models,
  selectedId,
  onSelect,
  onClose,
}: ModelDropdownProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // 按 Esc 键关闭下拉菜单
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };
    // 点击面板外部区域关闭下拉菜单
    const handleClickOutside = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [onClose]);

  // 仅展示 enabled 不为 false 的模型
  const enabledModels = models.filter((m) => (m as any).enabled !== false);

  return (
      <motion.div
        ref={ref}
        // 动画：从 95% 缩放 + 透明淡入，关闭时反向
        initial={{ scale: 0.95, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        exit={{ scale: 0.95, opacity: 0 }}
        transition={{ duration: 0.15, ease: [0.2, 0.0, 0, 1] }}
        style={{
          position: 'absolute',
          bottom: '100%',
          right: 'var(--space-4)',
          marginBottom: 'var(--space-2)',
          width: 320,
          maxHeight: 400,
          overflowY: 'auto',
          borderRadius: 'var(--radius-lg)',
          background: 'var(--bg-elevated)',
          border: '1px solid var(--border-subtle)',
          boxShadow: 'var(--shadow-floating-md)',
          backdropFilter: 'blur(20px) saturate(160%)',
          WebkitBackdropFilter: 'blur(20px) saturate(160%)',
          zIndex: 200,
        }}
      >
        {/* 顶部标题栏：显示"切换默认模型"及已启用模型数量 */}
        <div
          style={{
            padding: 'var(--space-3) var(--space-4)',
            borderBottom: '1px solid var(--border-subtle)',
          }}
        >
          <div
            className="t-h3"
            style={{ color: 'var(--text-primary)' }}
          >
            切换默认模型
          </div>
          <div
            className="t-caption"
            style={{ color: 'var(--text-muted)', marginTop: '2px' }}
          >
            共 {enabledModels.length} 个已启用
          </div>
        </div>

        {/* 模型列表行 */}
        {models.map((model) => {
          const isDefault = model.id === selectedId;

          return (
            <button
              key={model.id}
              onClick={() => {
                onSelect(model.id);
                onClose();
              }}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 'var(--space-3)',
                width: '100%',
                padding: 'var(--space-2) var(--space-4)',
                border: 'none',
                // 当前默认模型高亮背景
                background: isDefault
                  ? 'var(--primary-tint-soft)'
                  : 'transparent',
                cursor: 'pointer',
                textAlign: 'left',
                fontFamily: 'var(--font-sans)',
                transition: `background var(--duration-fast) var(--ease-standard)`,
              }}
            >
              {/* 提供商首字母图标 */}
              <div
                style={{
                  width: 28,
                  height: 28,
                  borderRadius: 'var(--radius-sm)',
                  background: 'var(--buddy-primary)',
                  color: 'var(--text-on-primary)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  fontSize: '12px',
                  fontWeight: 700,
                  flexShrink: 0,
                }}
              >
                {model.provider_id.charAt(0).toUpperCase()}
              </div>

              {/* 模型信息：名称 + 默认标签 + 上下文窗口 */}
              <div style={{ flex: 1, minWidth: 0 }}>
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 'var(--space-1)',
                  }}
                >
                  <span
                    className="t-body"
                    style={{
                      color: 'var(--text-primary)',
                      fontWeight: 500,
                    }}
                  >
                    {model.display_name}
                  </span>
                  {/* 当前默认模型显示"默认"徽章 */}
                  {isDefault && (
                    <span
                      style={{
                        padding: '0 6px',
                        borderRadius: 'var(--radius-full)',
                        background: 'var(--buddy-primary)',
                        color: 'var(--text-on-primary)',
                        fontSize: '10px',
                        fontWeight: 600,
                      }}
                    >
                      默认
                    </span>
                  )}
                </div>
                {/* 上下文窗口大小，单位转换为 K */}
                <div
                  className="t-caption"
                  style={{
                    color: 'var(--text-muted)',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                  }}
                >
                  {model.context_window
                    ? `${(model.context_window / 1000).toFixed(0)}K 上下文`
                    : ''}
                </div>
              </div>

              {/* 延迟状态：根据 latency_ms 分级显示绿/黄/红状态点 */}
              {model.latency_ms != null && (
                <div
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '4px',
                    flexShrink: 0,
                  }}
                >
                  <StatusDot
                    kind={
                      // < 500ms → 绿色(success)，< 1500ms → 黄色(warning)，>= 1500ms → 红色(error)
                      model.latency_ms < 500
                        ? 'success'
                        : model.latency_ms < 1500
                          ? 'warning'
                          : 'error'
                    }
                    size={6}
                  />
                  <span className="t-caption" style={{ color: 'var(--text-muted)' }}>
                    {model.latency_ms}ms
                  </span>
                </div>
              )}
            </button>
          );
        })}
      </motion.div>
  );
}
