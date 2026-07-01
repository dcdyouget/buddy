import type { Theme } from '@/types';

interface ThemeSettingProps {
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
}

/** 主题设置：浅色 / 深色切换 */
export function ThemeSetting({ theme, onThemeChange }: ThemeSettingProps) {
  return (
    <section>
      <h3 className="t-h3" style={{ color: 'var(--text-primary)', marginBottom: 'var(--space-3)' }}>
        主题
      </h3>
      <div style={{ display: 'flex', gap: 'var(--space-3)' }}>
        {(['light', 'dark'] as const).map((t) => (
          <button
            key={t}
            onClick={() => onThemeChange(t)}
            style={{
              padding: 'var(--space-2) var(--space-4)',
              borderRadius: 'var(--radius-md)',
              border:
                theme === t
                  ? '1px solid var(--buddy-primary)'
                  : '1px solid var(--border-default)',
              background:
                theme === t
                  ? 'var(--primary-tint-soft)'
                  : 'var(--bg-elevated)',
              color:
                theme === t
                  ? 'var(--buddy-primary)'
                  : 'var(--text-primary)',
              cursor: 'pointer',
              fontFamily: 'var(--font-sans)',
              fontSize: '14px',
              fontWeight: theme === t ? 600 : 400,
            }}
          >
            {t === 'light' ? '浅色' : '深色'}
          </button>
        ))}
      </div>
    </section>
  );
}
