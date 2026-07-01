import { HotkeyRecorder } from '@/components/settings/HotkeyRecorder';

interface HotkeySettingProps {
  hotkey: string;
  onHotkeyChange: (hotkey: string) => void;
}

/** 快捷键设置：展示当前快捷键 + 录制新快捷键 */
export function HotkeySetting({ hotkey, onHotkeyChange }: HotkeySettingProps) {
  return (
    <section>
      <h3 className="t-h3" style={{ color: 'var(--text-primary)', marginBottom: 'var(--space-3)' }}>
        快捷键
      </h3>
      <HotkeyRecorder currentHotkey={hotkey} onRecord={onHotkeyChange} />
    </section>
  );
}
