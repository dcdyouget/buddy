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
}

/**
 * 提供商卡片
 * 在设置页的提供商列表中渲染单个提供商的卡片按钮。
 * - 激活状态：蓝色边框 + 浅蓝背景 + 实心图标 + 加粗文字
 * - 非激活状态：细边框 + 白色背景 + 灰色图标 + 常规文字
 * - 图标区显示首字母，使用品牌色作为背景色
 */
export function ProviderCard({
  id: _id,
  name,
  iconLetter,
  active,
  onSelect,
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
        // 激活状态：品牌色边框 + 浅色背景
        border: active
          ? '1px solid var(--buddy-primary)'
          : '1px solid var(--border-subtle)',
        background: active
          ? 'var(--primary-tint-soft)'
          : 'var(--bg-elevated)',
        cursor: 'pointer',
        fontFamily: 'var(--font-sans)',
        transition: `all var(--duration-fast) var(--ease-standard)`,
      }}
    >
      {/* 图标容器：激活时品牌色实心背景，否则灰色低调背景 */}
      <div
        style={{
          width: 40,
          height: 40,
          borderRadius: 'var(--radius-md)',
          background: active
            ? 'var(--buddy-primary)'
            : 'var(--bg-sunken)',
          color: active
            ? 'var(--text-on-primary)'
            : 'var(--text-muted)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          fontSize: '18px',
          fontWeight: 700,
        }}
      >
        {iconLetter}
      </div>
      {/* 提供商名称：激活时品牌色 + 加粗，否则常规 */}
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
