import { HotkeyRecorder } from '@/components/settings/HotkeyRecorder';

interface HotkeySettingProps {
  hotkey: string;
  onHotkeyChange: (hotkey: string) => void;
}

/** 快捷键设置：展示当前快捷键 + 录制新快捷键 */
export function HotkeySetting({ hotkey, onHotkeyChange }: HotkeySettingProps) {
  return (
    <section className="settings-section">
      <div className="settings-row">
        <div className="settings-copy">
          <h3>呼出快捷键</h3>
          <p>在任意应用中快速打开 Buddy</p>
        </div>
        <HotkeyRecorder currentHotkey={hotkey} onRecord={onHotkeyChange} />
      </div>
    </section>
  );
}
