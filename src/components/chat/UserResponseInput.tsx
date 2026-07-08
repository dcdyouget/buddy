import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { CornerDownLeft, MessageSquarePlus, Send } from 'lucide-react';
import { IconButton } from '@/components/shared/IconButton';
import { useChatStore } from '@/stores/chatStore';
import { useConfigStore } from '@/stores/configStore';

/**
 * UserResponseInput — 回应模型问题的输入面板
 *
 * 与 InputDock 的区别：
 * - 上方展示模型刚才提出的问题（带 ↳ 引用样式）
 * - 标题提示 "回应模型" 而不是 "新问题"
 * - 提供"发送"和"提出新问题"两个动作
 * - 提交时携带 parentMessageId,后端按 user 消息处理（带完整上下文）,
 *   前端按 child response 嵌套在父 assistant 消息内渲染
 */
export function UserResponseInput() {
  const waiting = useChatStore((s) => s.waitingForResponse);
  const setWaiting = useChatStore((s) => s.setWaitingForResponse);
  const sendMessage = useChatStore((s) => s.sendMessage);
  const config = useConfigStore((s) => s.config);

  const [text, setText] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // 切到回应模式时自动聚焦
  useEffect(() => {
    if (waiting) {
      requestAnimationFrame(() => textareaRef.current?.focus());
    }
  }, [waiting]);

  if (!waiting) return null;

  const submit = () => {
    const content = text.trim();
    if (!content || !config?.selected_model_id) return;
    sendMessage(content, config.selected_model_id, waiting.parentMessageId);
    setText('');
  };

  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter') {
      if (e.nativeEvent.isComposing || e.nativeEvent.keyCode === 229) {
        return;
      }
      if (e.metaKey || e.ctrlKey) {
        return;
      }
      e.preventDefault();
      if (text.trim()) submit();
    }
  };

  const dismissToNewTurn = () => {
    setWaiting(null);
  };

  // 自动撑高
  useEffect(() => {
    const ta = textareaRef.current;
    if (ta) {
      ta.style.height = 'auto';
      ta.style.height = Math.min(ta.scrollHeight, 120) + 'px';
    }
  }, [text]);

  return (
    <div
      style={{
        padding: 'var(--space-3)',
        borderTop: '1px solid var(--border-subtle)',
        background: 'var(--bg-sunken)',
        width: '100%',
        boxSizing: 'border-box',
      }}
    >
      {/* 模型的问题引用 */}
      <div
        style={{
          display: 'flex',
          alignItems: 'flex-start',
          gap: 'var(--space-2)',
          marginBottom: 'var(--space-2)',
          padding: 'var(--space-2) var(--space-3)',
          background: 'var(--bg-surface)',
          borderLeft: '2px solid var(--buddy-primary)',
          borderRadius: 'var(--radius-sm)',
          fontSize: '12px',
          color: 'var(--text-muted)',
          maxHeight: '72px',
          overflowY: 'auto',
        }}
      >
        <CornerDownLeft
          size={12}
          style={{ color: 'var(--buddy-primary)', flexShrink: 0, marginTop: 2 }}
        />
        <span style={{ overflowWrap: 'break-word' }}>{waiting.question}</span>
      </div>

      {/* 回应输入区 */}
      <div
        style={{
          display: 'flex',
          alignItems: 'flex-end',
          gap: 'var(--space-2)',
          width: '100%',
        }}
      >
        <textarea
          ref={textareaRef}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="回应模型... Enter 发送 · ⌘Enter 换行"
          rows={1}
          style={{
            flex: 1,
            padding: 'var(--space-2) var(--space-3)',
            borderRadius: 'var(--radius-md)',
            border: '1px solid var(--border-subtle)',
            background: 'var(--bg-surface)',
            color: 'var(--text-primary)',
            fontFamily: 'var(--font-sans)',
            fontSize: '14px',
            lineHeight: 1.5,
            resize: 'none',
            outline: 'none',
            maxHeight: '120px',
            overflowY: 'auto',
            overflowWrap: 'break-word',
            wordBreak: 'break-word',
            boxSizing: 'border-box',
          }}
        />
        <IconButton
          icon={MessageSquarePlus}
          onClick={dismissToNewTurn}
          variant="default"
          size={32}
          iconSize={14}
          title="改为提出新问题"
        />
        <IconButton
          icon={Send}
          onClick={submit}
          variant="primary"
          size={32}
          iconSize={14}
          title="发送回应"
        />
      </div>
    </div>
  );
}
