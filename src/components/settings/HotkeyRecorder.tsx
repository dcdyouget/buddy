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

  // 键盘按下事件：累积修饰键 + 主键，实时更新显示
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!recording) return;
      e.preventDefault();
      e.stopPropagation();

      const keys: string[] = [];
      // 收集当前按下的修饰键
      if (e.metaKey || e.ctrlKey) keys.push('CmdOrCtrl');
      if (e.shiftKey) keys.push('Shift');
      if (e.altKey) keys.push('Alt');

      const keyName = e.key;
      // 排除纯修饰键本身，仅将实际按键（字母/数字/符号等）作为主键
      if (!['Meta', 'Control', 'Shift', 'Alt'].includes(keyName)) {
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
    [recording],
  );

  // 键盘松开事件：在非修饰键松开时完成录制
  const handleKeyUp = useCallback(
    (e: KeyboardEvent) => {
      if (!recording) return;
      e.preventDefault();
      e.stopPropagation();

      const keyName = e.key;
      // 仅当松开的是非修饰键且已捕获到主键时才完成录制
      if (!['Meta', 'Control', 'Shift', 'Alt'].includes(keyName) && hasMainKey.current) {
        // 重新读取当前修饰键状态，构建最终组合
        const keys: string[] = [];
        if (e.metaKey || e.ctrlKey) keys.push('CmdOrCtrl');
        if (e.shiftKey) keys.push('Shift');
        if (e.altKey) keys.push('Alt');

        const displayKey = keyName.length === 1 ? keyName.toUpperCase() : keyName;
        keys.push(displayKey);

        if (keys.length > 0) {
          // 用 "+" 连接各键生成快捷键字符串
          const hotkey = keys.join('+');
          onRecord(hotkey);
        }
        setRecording(false);
        hasMainKey.current = false;
      }

      // 处理用户仅按修饰键后松开的情况（改变主意/误操作）
      if (['Meta', 'Control', 'Shift', 'Alt'].includes(keyName) && !hasMainKey.current) {
        // 检查所有修饰键是否都已松开
        if (!e.metaKey && !e.ctrlKey && !e.shiftKey && !e.altKey) {
          // 所有修饰键已松开且未按下主键，清空显示继续录制
          setRecordedKeys([]);
        }
      }
    },
    [recording, onRecord],
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
    setRecordedKeys([]);
    hasMainKey.current = false;
  };

  // 将当前快捷键字符串拆分为数组以显示 Kbd 行
  const currentKeys = currentHotkey.split('+');

  return (
    <div
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
