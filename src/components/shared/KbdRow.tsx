/** 快捷键展示组件的 Props */
interface KbdRowProps {
  /** 快捷键组合的键名数组，例如 ['CmdOrCtrl', 'K'] */
  keys: string[];
}

/**
 * 根据当前操作系统平台，将键名转换为对应的显示符号
 * - macOS 上显示 ⌘、⌃、⇧、⌥ 等符号
 * - Windows/Linux 上显示 Ctrl、Shift、Alt 等文字
 */
function platformModifier(key: string): string {
  // 通过 navigator.platform 检测是否为 macOS 平台
  const isMac =
    typeof navigator !== 'undefined' && navigator.platform.toUpperCase().includes('MAC');

  // 根据平台选择对应的键名映射表
  const replacements: Record<string, string> = isMac
    ? {
        CmdOrCtrl: '⌘',
        Cmd: '⌘',
        Ctrl: '⌃',
        Shift: '⇧',
        Alt: '⌥',
        Option: '⌥',
        Space: '␣',
        Enter: '↵',
        Return: '↵',
        Escape: 'Esc',
        Tab: '⇥',
        Backspace: '⌫',
        Plus: '+',
      }
    : {
        CmdOrCtrl: 'Ctrl',
        Cmd: 'Win',
        Ctrl: 'Ctrl',
        Shift: 'Shift',
        Alt: 'Alt',
        Option: 'Alt',
        Space: 'Space',
        Enter: 'Enter',
        Return: 'Enter',
        Escape: 'Esc',
        Tab: 'Tab',
        Backspace: 'Bksp',
        Plus: '+',
      };

  // 如果映射表中没有对应的键名，直接返回原始键名
  return replacements[key] || key;
}

/**
 * KbdRow — 快捷键组合展示组件
 *
 * 将一组快捷键键名渲染为 macOS / Windows 风格的小键盘标签，
 * 键之间用 + 号分隔，自动适配操作系统显示风格。
 *
 * @param props.keys - 快捷键键名数组，支持 CmdOrCtrl/Cmd/Ctrl/Shift/Alt 等虚拟键名
 */
export function KbdRow({ keys }: KbdRowProps) {
  // 将所有键名转换为当前平台的显示文本
  const displayKeys = keys.map(platformModifier);

  return (
    <span
      style={{
        display: 'inline-flex',
        gap: 'var(--space-1)',
        alignItems: 'center',
      }}
    >
      {displayKeys.map((key, i) => (
        <span key={i}>
          <kbd
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              justifyContent: 'center',
              minWidth: '24px',
              height: '22px',
              padding: '0 var(--space-1)',
              borderRadius: 'var(--radius-sm)',
              background: 'var(--bg-sunken)',
              border: '1px solid var(--border-default)',
              color: 'var(--text-primary)',
              fontFamily: 'var(--font-sans)',
              fontSize: '12px',
              fontWeight: 500,
              lineHeight: 1,
            }}
          >
            {key}
          </kbd>
          {/* 键与键之间渲染 + 分隔符，最后一个键后面不加 */}
          {i < displayKeys.length - 1 && (
            <span
              style={{
                color: 'var(--text-tertiary)',
                fontSize: '12px',
                margin: '0 1px',
              }}
            >
              +
            </span>
          )}
        </span>
      ))}
    </span>
  );
}
