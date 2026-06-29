interface FooterActionsProps {
  /** 取消按钮的点击回调 */
  onCancel: () => void;
  /** 确认按钮的点击回调 */
  onConfirm: () => void;
  /** 确认按钮文案，默认 "确定" */
  confirmLabel?: string;
  /** 取消按钮文案，默认 "取消" */
  cancelLabel?: string;
  /** 是否禁用确认按钮 */
  confirmDisabled?: boolean;
}

/**
 * FooterActions - 底部双按钮操作栏
 *
 * 常用于弹窗、面板底部的操作区，提供"取消 + 确认"的标准双按钮布局。
 * 确认按钮使用品牌主色，取消按钮使用弱化样式，
 * 支持禁用确认按钮（如用于表单校验未通过时）。
 *
 * @param props - 见 FooterActionsProps
 */
export function FooterActions({
  onCancel,
  onConfirm,
  confirmLabel = '确定',
  cancelLabel = '取消',
  confirmDisabled = false,
}: FooterActionsProps) {
  return (
    <div
      style={{
        display: 'flex',
        justifyContent: 'flex-end', // 按钮右对齐
        gap: 'var(--space-3)',
        padding: 'var(--space-4)',
        borderTop: '1px solid var(--border-subtle)', // 顶部分割线，与内容区隔开
      }}
    >
      {/* 取消按钮 —— 次要样式，带边框 */}
      <button
        onClick={onCancel}
        style={{
          padding: 'var(--space-2) var(--space-4)',
          borderRadius: 'var(--radius-md)',
          border: '1px solid var(--border-default)',
          background: 'var(--bg-elevated)',
          color: 'var(--text-primary)',
          cursor: 'pointer',
          fontFamily: 'var(--font-sans)',
          fontSize: '14px',
          fontWeight: 500,
          transition: `all var(--duration-fast) var(--ease-standard)`,
        }}
      >
        {cancelLabel}
      </button>
      {/* 确认按钮 —— 品牌主色填充 */}
      <button
        onClick={onConfirm}
        disabled={confirmDisabled}
        style={{
          padding: 'var(--space-2) var(--space-4)',
          borderRadius: 'var(--radius-md)',
          border: 'none',
          background: confirmDisabled
            ? 'var(--border-default)' // 禁用时使用灰色背景
            : 'var(--buddy-primary)', // 正常态使用品牌主色
          color: 'var(--text-on-primary)',
          cursor: confirmDisabled ? 'default' : 'pointer', // 禁用时不显示手型光标
          fontFamily: 'var(--font-sans)',
          fontSize: '14px',
          fontWeight: 600, // 确认按钮字重稍大，突出主操作
          opacity: confirmDisabled ? 0.5 : 1, // 禁用时降低整体透明度
          transition: `all var(--duration-fast) var(--ease-standard)`,
        }}
      >
        {confirmLabel}
      </button>
    </div>
  );
}
