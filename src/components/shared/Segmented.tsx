import type { LucideIcon } from 'lucide-react';

/** 分段控制器的单个选项 */
interface SegmentedOption {
  /** 选项显示文本 */
  label: string;
  /** 可选的图标组件（lucide-react 图标） */
  icon?: LucideIcon;
  /** 选项对应的值，用于标识选中状态 */
  value: string;
}

/** Segmented 分段控制器组件的 Props */
interface SegmentedProps {
  /** 可选项列表 */
  options: SegmentedOption[];
  /** 当前选中的值 */
  value: string;
  /** 选中值变化时的回调 */
  onChange: (value: string) => void;
}

/**
 * Segmented — 分段控制器组件
 *
 * 类似 iOS 分段控件，在一组互斥选项中切换选择。
 * 选中项有高亮背景和阴影，支持可选的图标。
 *
 * @param props.options - 选项列表，每项包含 label、value 和可选的 icon
 * @param props.value - 当前选中项的值
 * @param props.onChange - 切换选项时的回调，传入新选中的 value
 */
export function Segmented({ options, value, onChange }: SegmentedProps) {
  return (
    <div
      style={{
        display: 'inline-flex',
        background: 'var(--bg-sunken)',
        borderRadius: 'var(--radius-md)',
        padding: '2px',
        gap: '2px',
      }}
    >
      {options.map((option) => {
        // 判断当前选项是否被选中
        const isSelected = option.value === value;
        const Icon = option.icon;
        return (
          <button
            key={option.value}
            onClick={() => onChange(option.value)}
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              gap: 'var(--space-1)',
              padding: 'var(--space-1) var(--space-3)',
              borderRadius: 'var(--radius-sm)',
              border: 'none',
              // 选中时使用高亮背景和主色文字，未选中时半透明
              background: isSelected ? 'var(--bg-elevated)' : 'transparent',
              color: isSelected ? 'var(--text-primary)' : 'var(--text-muted)',
              cursor: 'pointer',
              fontFamily: 'var(--font-sans)',
              fontSize: '13px',
              // 选中时加粗字体并添加阴影，增强视觉区分
              fontWeight: isSelected ? 600 : 400,
              boxShadow: isSelected ? 'var(--shadow-static)' : 'none',
              transition: `all var(--duration-fast) var(--ease-standard)`,
            }}
          >
            {/* 如果有图标，渲染在文字前面 */}
            {Icon && <Icon size={14} />}
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
