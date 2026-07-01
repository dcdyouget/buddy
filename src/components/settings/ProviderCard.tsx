/** 提供商卡片的 Props */
interface ProviderCardProps {
  /** 提供商标识符 */
  id: string;
  /** 提供商显示名称 */
  name: string;
  /** 图标中显示的字母（通常取名称首字母） */
  iconLetter: string;
  /** 当前提供商是否处于激活（选中）状态 */
  active: boolean;
  /** 点击选中该提供商的回调 */
  onSelect: () => void;
  /** 提供商类型标签（可选） */
  providerType?: string;
}

/**
 * 提供商卡片
 * 在设置页的提供商列表中渲染单个提供商的卡片按钮。
 * 显示名称、图标和提供商类型标签。
 */
export function ProviderCard({
  id: _id,
  name,
  iconLetter,
  active,
  onSelect,
  providerType,
}: ProviderCardProps) {
  return (
    <button
      onClick={onSelect}
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        gap: 'var(--space-2)',
        padding: 'var(--space-4)',
        borderRadius: 'var(--radius-lg)',
        border: active
          ? '1px solid var(--buddy-primary)'
          : '1px solid var(--border-subtle)',
        background: active
          ? 'var(--primary-tint-soft)'
          : 'var(--bg-elevated)',
        cursor: 'pointer',
        fontFamily: 'var(--font-sans)',
        position: 'relative',
        transition: `all var(--duration-fast) var(--ease-standard)`,
      }}
    >
      {/* 提供商类型标签（如 "Anthropic"） */}
      {providerType && providerType !== 'openai_compatible' && (
        <span
          style={{
            position: 'absolute',
            top: '6px',
            right: '6px',
            padding: '1px 6px',
            borderRadius: 'var(--radius-full)',
            background: active ? 'var(--buddy-primary)' : 'var(--bg-sunken)',
            color: active ? 'var(--text-on-primary)' : 'var(--text-muted)',
            fontSize: '9px',
            fontWeight: 600,
            lineHeight: '16px',
          }}
        >
          {providerType === 'anthropic' ? 'A' : providerType.slice(0, 3).toUpperCase()}
        </span>
      )}
      <div
        style={{
          width: 40,
          height: 40,
          borderRadius: 'var(--radius-md)',
          background: active ? 'var(--buddy-primary)' : 'var(--bg-sunken)',
          color: active ? 'var(--text-on-primary)' : 'var(--text-muted)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          fontSize: '18px',
          fontWeight: 700,
        }}
      >
        {iconLetter}
      </div>
      <span
        className="t-body-sm"
        style={{
          color: active ? 'var(--buddy-primary)' : 'var(--text-primary)',
          fontWeight: active ? 600 : 400,
        }}
      >
        {name}
      </span>
    </button>
  );
}
