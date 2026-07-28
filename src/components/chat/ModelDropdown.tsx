import { useEffect, useRef } from 'react';
import { motion } from 'framer-motion';
import { Check } from 'lucide-react';
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
 * - 每条模型行仅保留名称、上下文信息和当前选中态
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
        role="listbox"
        aria-label="选择模型"
        style={{
          position: 'absolute',
          bottom: '100%',
          right: 'var(--space-4)',
          marginBottom: 'var(--space-2)',
          overflowY: 'auto',
          borderRadius: 'var(--radius-lg)',
          background: 'var(--bg-elevated)',
          border: '1px solid var(--border-subtle)',
          boxShadow: 'var(--shadow-floating-md)',
          zIndex: 200,
        }}
        className="no-scrollbar model-dropdown"
      >
        {/* 模型列表行 */}
        {enabledModels.length === 0 && (
          <div
            className="t-body-sm"
            style={{
              padding: 'var(--space-5) var(--space-4)',
              color: 'var(--text-muted)',
              textAlign: 'center',
            }}
          >
            暂无已启用模型，请前往设置添加
          </div>
        )}
        {enabledModels.map((model) => {
          const isDefault = model.id === selectedId;

          return (
            <button
              className="model-dropdown-row"
              key={model.id}
              role="option"
              aria-selected={isDefault}
              onClick={() => {
                onSelect(model.id);
                onClose();
              }}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 'var(--space-2)',
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
              {/* 模型信息：名称 + 上下文窗口 / 延迟 */}
              <div style={{ flex: 1, minWidth: 0 }}>
                <span
                  className="model-dropdown-name"
                  style={{
                    color: 'var(--text-primary)',
                    fontWeight: 500,
                  }}
                >
                  {model.display_name}
                </span>
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
                  {model.context_window && model.latency_ms != null ? ' · ' : ''}
                  {model.latency_ms != null ? `${model.latency_ms}ms` : ''}
                </div>
              </div>

              {isDefault && (
                <Check
                  className="model-dropdown-check"
                  size={14}
                  strokeWidth={2}
                  aria-hidden="true"
                />
              )}
            </button>
          );
        })}
      </motion.div>
  );
}
