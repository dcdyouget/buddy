/** 状态类型 */
type StatusKind = 'success' | 'warning' | 'error';

interface StatusDotProps {
  /** 状态种类，决定圆点颜色 */
  kind: StatusKind;
  /** 圆点直径，默认 8px */
  size?: number;
}

// 状态 → 颜色映射，使用设计系统 CSS 变量确保一致
const statusColors: Record<StatusKind, string> = {
  success: 'var(--state-success)',
  warning: 'var(--state-warning)',
  error: 'var(--state-error)',
};

/**
 * StatusDot - 状态指示灯
 *
 * 一个小巧的纯色圆点，用于指示连接状态、服务可用性等。
 * 支持 success（绿）、warning（黄）、error（红）三种状态。
 *
 * @param props - 见 StatusDotProps
 */
export function StatusDot({ kind, size = 8 }: StatusDotProps) {
  return (
    <span
      style={{
        display: 'inline-block',
        width: size,
        height: size,
        minWidth: size,
        minHeight: size,
        borderRadius: 'var(--radius-full)', // 正圆形
        background: statusColors[kind], // 根据 kind 选取对应颜色
      }}
    />
  );
}
