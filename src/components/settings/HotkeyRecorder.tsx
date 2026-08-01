import { useState, useEffect, useCallback, useRef } from 'react';
import { KbdRow } from '@/components/shared/KbdRow';

/** 快捷键录制组件的 Props */
interface HotkeyRecorderProps {
  /** 当前已设置的快捷键字符串，如 "CmdOrCtrl+Shift+A" */
  currentHotkey: string;
  /** 录制完成后的回调，传入新快捷键字符串 */
  onRecord: (hotkey: string) => void;
}

/**
 * 快捷键录制器
 * 在设置页中用于录制全局快捷键组合。
 * 工作流程：
 * 1. 用户点击"重新录制"进入录制状态
 * 2. 实时显示当前按下的修饰键（CmdOrCtrl/Shift/Alt）
 * 3. 当用户按下非修饰键（如字母）时，记录主键
 * 4. 松开该非修饰键时，组合所有按键生成快捷键字符串并回调
 * 5. 如果用户仅按下又松开修饰键（改变主意），则清空显示继续等待
 */
export function HotkeyRecorder({ currentHotkey, onRecord }: HotkeyRecorderProps) {
  const [recording, setRecording] = useState(false);
  const [recordedKeys, setRecordedKeys] = useState<string[]>([]);
  // 跟踪当前录制回合是否已捕获到非修饰键（主键）
  const hasMainKey = useRef(false);
  // 最近一次 keydown 时的修饰键（keyup 完成时使用），避免先松开修饰键导致组合丢失
  const lastModifiers = useRef<string[]>([]);

  const isModifierKey = (keyName: string) =>
    ['Meta', 'Control', 'Shift', 'Alt'].includes(keyName);

  const collectModifiers = (e: KeyboardEvent): string[] => {
    const keys: string[] = [];
    if (e.metaKey || e.ctrlKey) keys.push('CmdOrCtrl');
    if (e.shiftKey) keys.push('Shift');
    if (e.altKey) keys.push('Alt');
    return keys;
  };

  const resetRecording = useCallback(() => {
    setRecordedKeys([]);
    hasMainKey.current = false;
    lastModifiers.current = [];
  }, []);

  // 键盘按下事件：累积修饰键 + 主键，实时更新显示
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!recording) return;
      e.preventDefault();
      e.stopPropagation();

      // Esc 取消录制（而不是被录成 "Escape" 组合）
      if (e.key === 'Escape') {
        setRecording(false);
        resetRecording();
        return;
      }

      const keys = collectModifiers(e);
      lastModifiers.current = keys;

      const keyName = e.key;
      // 排除纯修饰键本身，仅将实际按键（字母/数字/符号等）作为主键
      if (!isModifierKey(keyName)) {
        // 单个字符转为大写以保持显示一致
        const displayKey = keyName.length === 1 ? keyName.toUpperCase() : keyName;
        keys.push(displayKey);
        hasMainKey.current = true;
      }

      // 实时更新显示当前按键组合
      if (keys.length > 0) {
        setRecordedKeys(keys);
      }
    },
    [recording, resetRecording],
  );

  // 键盘松开事件：在非修饰键松开时完成录制
  const handleKeyUp = useCallback(
    (e: KeyboardEvent) => {
      if (!recording) return;
      e.preventDefault();
      e.stopPropagation();

      const keyName = e.key;
      // 仅当松开的是非修饰键且已捕获到主键时才完成录制
      if (!isModifierKey(keyName) && hasMainKey.current) {
        // 用 keydown 时捕获的修饰键构建最终组合，避免先松开修饰键导致修饰键丢失
        const modifiers = lastModifiers.current;
        // 至少需要一个修饰键（Windows RegisterHotKey 也拒绝无修饰键的组合）
        if (modifiers.length === 0) {
          resetRecording();
          return;
        }
        const keys = [...modifiers];
        const displayKey = keyName.length === 1 ? keyName.toUpperCase() : keyName;
        keys.push(displayKey);

        // 用 "+" 连接各键生成快捷键字符串
        onRecord(keys.join('+'));
        setRecording(false);
        resetRecording();
      }

      // 处理用户仅按修饰键后松开的情况（改变主意/误操作）
      if (isModifierKey(keyName) && !hasMainKey.current) {
        // 检查所有修饰键是否都已松开
        if (!e.metaKey && !e.ctrlKey && !e.shiftKey && !e.altKey) {
          // 所有修饰键已松开且未按下主键，清空显示继续录制
          resetRecording();
        }
      }
    },
    [recording, onRecord, resetRecording],
  );

  // 注册全局键盘事件监听（capture 阶段，确保快捷键不触发浏览器默认行为）
  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown, true);
    window.addEventListener('keyup', handleKeyUp, true);
    return () => {
      window.removeEventListener('keydown', handleKeyDown, true);
      window.removeEventListener('keyup', handleKeyUp, true);
    };
  }, [handleKeyDown, handleKeyUp]);

  // 开始录制：重置状态，清除之前捕获的按键
  const startRecording = () => {
    setRecording(true);
    resetRecording();
  };

  // 将当前快捷键字符串拆分为数组以显示 Kbd 行
  const currentKeys = currentHotkey.split('+');

  return (
    <div
      className="hotkey-recorder"
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 'var(--space-3)',
      }}
    >
      {/* 快捷键可视化展示：录制中显示实时按键，否则显示当前快捷键 */}
      <KbdRow keys={recording && recordedKeys.length > 0 ? recordedKeys : currentKeys} />
      <button
        onClick={startRecording}
        disabled={recording}
        style={{
          padding: 'var(--space-1) var(--space-3)',
          borderRadius: 'var(--radius-sm)',
          border: recording
            ? '1px solid var(--buddy-primary)'
            : '1px solid var(--border-default)',
          background: recording
            ? 'var(--primary-tint-soft)'
            : 'var(--bg-elevated)',
          color: recording ? 'var(--buddy-primary)' : 'var(--text-primary)',
          cursor: recording ? 'default' : 'pointer',
          fontSize: '13px',
          fontFamily: 'var(--font-sans)',
          fontWeight: 500,
          transition: `all var(--duration-fast) var(--ease-standard)`,
        }}
      >
        {/* 录制中显示提示文字，否则显示操作按钮文字 */}
        {recording ? '按下新快捷键...' : '重新录制'}
      </button>
    </div>
  );
}
