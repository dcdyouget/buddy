import type { LucideIcon } from 'lucide-react';

interface IconButtonProps {
  /** 要渲染的 Lucide 图标组件 */
  icon: LucideIcon;
  /** 点击回调 */
  onClick?: () => void;
  /** 按钮整体尺寸（宽高相同），默认 28px */
  size?: number;
  /** 图标尺寸，默认 16px */
  iconSize?: number;
  /** 额外的 CSS 类名 */
  className?: string;
  /** 是否禁用 */
  disabled?: boolean;
  /** 鼠标悬停提示（HTML title 属性） */
  title?: string;
  /** 视觉变体：default(默认透明) | primary(品牌色) | danger(危险红) */
  variant?: 'default' | 'primary' | 'danger';
}

/**
 * IconButton - 圆形图标按钮
 *
 * 纯图标操作的圆形按钮组件，支持三种视觉变体和禁用态。
 * 使用 CSS 变量引用设计系统中的颜色和间距 token，确保风格统一。
 *
 * @param props - 见 IconButtonProps
 */
export function IconButton({
  icon: Icon,
  onClick,
  size = 28,
  iconSize = 16,
  className = '',
  disabled = false,
  title,
  variant = 'default',
}: IconButtonProps) {
  // 变体样式映射表 —— key 对应 variant prop，value 为对应的前景色和背景色 CSS 变量
  const variantStyles: Record<string, React.CSSProperties> = {
    default: {
      color: 'var(--text-muted)',
      background: 'transparent',
    },
    primary: {
      color: 'var(--text-on-primary)',
      background: 'var(--buddy-primary)',
    },
    danger: {
      color: 'var(--text-on-primary)',
      background: 'var(--state-error)',
    },
  };

  return (
    <button
      onClick={onClick}
      disabled={disabled}
      title={title}
      className={`icon-button ${className}`}
      style={{
        width: size,
        height: size,
        minWidth: size,
        minHeight: size,
        borderRadius: 'var(--radius-full)', // 正圆形
        border: 'none',
        cursor: disabled ? 'default' : 'pointer', // 禁用时不显示手型光标
        display: 'inline-flex',
        alignItems: 'center',
        justifyContent: 'center',
        opacity: disabled ? 0.4 : 1, // 禁用时降低透明度
        transition: `all var(--duration-fast) var(--ease-standard)`, // 统一的过渡动画
        ...variantStyles[variant], // 展开对应变体的颜色样式
      }}
    >
      <Icon size={iconSize} />
    </button>
  );
}
