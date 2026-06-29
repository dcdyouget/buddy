import { AlertTriangle } from 'lucide-react';

/** WarningPill 警告条组件的 Props */
interface WarningPillProps {
  /** 警告标题/主要信息 */
  headline: string;
  /** 操作按钮文字（Call to Action） */
  cta: string;
  /** 点击回调 */
  onClick: () => void;
}

/**
 * WarningPill — 警告条组件
 *
 * 用于在页面中展示警告信息并提供操作入口。
 * 整个组件是一个可点击的按钮，左侧为警告图标，中间为警告文案，
 * 右侧为操作引导文字，整体使用错误态红色。
 *
 * @param props.headline - 警告标题文字
 * @param props.cta - 操作按钮文字，如"去设置"、"立即修复"
 * @param props.onClick - 点击整条警告时的回调
 */
export function WarningPill({ headline, cta, onClick }: WarningPillProps) {
  return (
    <button
      onClick={onClick}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 'var(--space-3)',
        width: '100%',
        padding: 'var(--space-3) var(--space-4)',
        border: 'none',
        background: 'transparent',
        cursor: 'pointer',
        fontFamily: 'var(--font-sans)',
      }}
    >
      {/* 左侧警告图标，固定尺寸不缩放 */}
      <AlertTriangle
        size={18}
        style={{ color: 'var(--state-error)', flexShrink: 0 }}
      />
      {/* 中间警告文案，自动撑满剩余空间 */}
      <span
        style={{
          color: 'var(--state-error)',
          fontSize: '14px',
          fontWeight: 500,
          flex: 1,
          textAlign: 'left',
        }}
      >
        {headline}
      </span>
      {/* 右侧操作引导文字，固定不缩放 */}
      <span
        style={{
          color: 'var(--state-error)',
          fontSize: '14px',
          fontWeight: 600,
          flexShrink: 0,
        }}
      >
        {cta}
      </span>
    </button>
  );
}
