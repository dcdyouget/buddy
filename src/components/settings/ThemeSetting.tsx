import type { Theme } from '@/types';
import { Moon, Sun } from 'lucide-react';

interface ThemeSettingProps {
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
}

/** 主题设置：浅色 / 深色切换 */
export function ThemeSetting({ theme, onThemeChange }: ThemeSettingProps) {
  return (
    <section className="settings-section">
      <div className="settings-row">
        <div className="settings-copy">
          <h3>外观</h3>
          <p>选择窗口的显示模式</p>
        </div>
        <div className="segmented-control">
        {(['light', 'dark'] as const).map((t) => (
          <button
            key={t}
            className={theme === t ? 'is-active' : ''}
            onClick={() => onThemeChange(t)}
            title={t === 'light' ? '浅色' : '深色'}
          >
            {t === 'light' ? <Sun size={14} /> : <Moon size={14} />}
            {t === 'light' ? '浅色' : '深色'}
          </button>
        ))}
        </div>
      </div>
    </section>
  );
}
