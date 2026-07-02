import type { ModelInfo } from '@/types';
import { StatusDot } from '@/components/shared/StatusDot';

/** 上下文窗口预设选项（token 数） */
const CTX_PRESETS = [128_000, 256_000, 512_000, 1_000_000];

/** 获取下拉选项列表：预设 + 当前值（如果不在预设中） */
function getCtxOptions(current: number): number[] {
  const opts = [...CTX_PRESETS];
  if (!opts.includes(current)) opts.push(current);
  return opts.sort((a, b) => a - b);
}

/** 格式化 token 数为可读字符串 */
function formatContextWindow(tokens: number): string {
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(1)}M`;
  return `${Math.round(tokens / 1000)}K`;
}

/** 模型行组件的 Props */
interface ModelRowProps {
  /** 模型数据 */
  model: ModelInfo;
  /** 该模型是否已启用 */
  enabled: boolean;
  /** 是否为当前默认模型 */
  isDefault: boolean;
  /** 切换启用/禁用的回调 */
  onToggle: () => void;
  /** 设为默认模型的回调 */
  onSetDefault: () => void;
  /** 更新上下文窗口的回调 */
  onUpdateContextWindow?: (ctx: number) => void;
}

/**
 * 模型行
 * 在设置页的提供商卡片内渲染单条模型记录，包含：
 * - 启用/禁用 checkbox
 * - 提供商首字母图标
 * - 模型名称、默认标签、上下文窗口
 * - 延迟状态（绿/黄/红状态点 + 毫秒数）
 * - "设为默认"按钮（非默认且已启用时可见）
 * 未启用的模型行半透明显示。
 */
export function ModelRow({
  model,
  enabled,
  isDefault,
  onToggle,
  onSetDefault,
  onUpdateContextWindow,
}: ModelRowProps) {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 'var(--space-3)',
        padding: 'var(--space-2) var(--space-4)',
        // 未启用时降低透明度，视觉上标记为不可用
        opacity: enabled ? 1 : 0.5,
        transition: `opacity var(--duration-fast) var(--ease-standard)`,
      }}
    >
      {/* 启用/禁用 checkbox */}
      <label
        style={{
          display: 'flex',
          alignItems: 'center',
          cursor: 'pointer',
          flexShrink: 0,
        }}
      >
        <input
          type="checkbox"
          checked={enabled}
          onChange={onToggle}
          style={{
            width: 16,
            height: 16,
            accentColor: 'var(--buddy-primary)',
            cursor: 'pointer',
          }}
        />
      </label>

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
        <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--space-1)' }}>
          <span className="t-body" style={{ fontWeight: 500, color: 'var(--text-primary)' }}>
            {model.display_name}
          </span>
          {/* 默认模型显示"默认"徽章 */}
          {isDefault && (
            <span
              style={{
                padding: '0 var(--space-2)',
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
        {/* 上下文窗口大小：可下拉选择或只读展示 */}
        {onUpdateContextWindow ? (
          <select
            value={model.context_window}
            onChange={(e) => onUpdateContextWindow(Number(e.target.value))}
            style={{
              marginTop: 2,
              padding: '1px 4px',
              borderRadius: 'var(--radius-sm)',
              border: '1px solid var(--border-default)',
              background: 'var(--bg-sunken)',
              color: 'var(--text-muted)',
              fontSize: '11px',
              fontFamily: 'var(--font-sans)',
              cursor: 'pointer',
            }}
          >
            {getCtxOptions(model.context_window).map((opt) => (
              <option key={opt} value={opt}>{formatContextWindow(opt)}</option>
            ))}
          </select>
        ) : (
          <div className="t-caption" style={{ color: 'var(--text-muted)' }}>
            {model.context_window
              ? `${(model.context_window / 1000).toFixed(0)}K 上下文`
              : ''}
          </div>
        )}
      </div>

      {/* 延迟状态：根据 latency_ms 分级显示状态点 */}
      {model.latency_ms != null && (
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 'var(--space-1)',
            flexShrink: 0,
          }}
        >
          <StatusDot
            kind={
              // < 500ms → 绿色，< 1500ms → 黄色，>= 1500ms → 红色
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

      {/* "设为默认"按钮：仅在非默认且已启用时显示 */}
      {!isDefault && enabled && (
        <button
          onClick={onSetDefault}
          style={{
            padding: '2px 10px',
            borderRadius: 'var(--radius-sm)',
            border: '1px solid var(--border-default)',
            background: 'var(--bg-elevated)',
            color: 'var(--text-primary)',
            cursor: 'pointer',
            fontSize: '12px',
            fontFamily: 'var(--font-sans)',
            whiteSpace: 'nowrap',
            flexShrink: 0,
            transition: `all var(--duration-fast) var(--ease-standard)`,
          }}
        >
          设为默认
        </button>
      )}
    </div>
  );
}
