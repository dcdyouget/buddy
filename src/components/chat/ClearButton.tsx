import { X } from 'lucide-react';

/**
 * ClearButton 组件的 Props
 * @param visible - 是否显示清除按钮（仅当输入框有内容时为 true）
 * @param onClear - 点击清除按钮时的回调，通常用于清空输入框
 */
interface ClearButtonProps {
  visible: boolean;
  onClear: () => void;
}

/**
 * 清除按钮组件
 * 用于清空输入框内容。仅在 visible 为 true 时渲染，
 * 渲染为一个圆形的小按钮，内含 X 图标。
 */
export function ClearButton({ visible, onClear }: ClearButtonProps) {
  // 不可见时不渲染任何内容
  if (!visible) return null;

  return (
    <button
      onClick={onClear}
      title="清除输入"
      style={{
        width: 20,
        height: 20,
        borderRadius: 'var(--radius-full)',
        border: 'none',
        background: 'var(--bg-sunken)',
        color: 'var(--text-muted)',
        cursor: 'pointer',
        display: 'inline-flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: 0,
        transition: `all var(--duration-fast) var(--ease-standard)`,
        flexShrink: 0,
      }}
    >
      <X size={12} />
    </button>
  );
}
