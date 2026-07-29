import { useRef, useEffect, type KeyboardEvent, type PointerEvent } from 'react';
import { Bot, Send, Settings, Square } from 'lucide-react';
import { IconButton } from '@/components/shared/IconButton';
import { ClearButton } from './ClearButton';
import type { ModelInfo } from '@/types';

/**
 * InputDock 组件的 Props
 * @param isStreaming - 是否正在流式生成中，控制输入栏的状态切换
 * @param streamingModelName - 流式生成时显示的模型名称
 * @param selectedModel - 当前选中的模型信息
 * @param draftInput - 输入框中的当前草稿文本
 * @param onDraftChange - 输入文本变化时的回调
 * @param onSend - 发送消息的回调
 * @param onStop - 停止生成的回调
 * @param onModelPickerClick - 模型选择器点击回调
 */
interface InputDockProps {
  isStreaming: boolean;
  streamingModelName?: string;
  selectedModel: ModelInfo | null;
  draftInput: string;
  onDraftChange: (text: string) => void;
  onSend: () => void;
  onStop: () => void;
  onModelPickerClick?: () => void;
  onSettingsClick?: () => void;
  /** 禁用 textarea 自动撑高，改为固定高度 + 滚动条（用于紧凑窗口） */
  disableAutoResize?: boolean;
  /** 隐藏顶部分隔线（空态气泡中不需要分隔消息列表） */
  hideBorder?: boolean;
}

/**
 * 输入栏组件
 * 聊天窗口底部的输入区域，包含两种状态：
 * 1. 正常状态：多行输入框（自动撑高，最高 120px）+ 清除按钮 + 发送按钮
 * 2. 流式状态：显示生成进度文本 + 停止按钮
 *
 * 键盘交互：
 * - Enter 直接发送消息
 * - Cmd/Ctrl + Enter 换行
 */
export function InputDock({
  isStreaming,
  streamingModelName,
  selectedModel,
  draftInput,
  onDraftChange,
  onSend,
  onStop,
  onModelPickerClick,
  onSettingsClick,
  disableAutoResize = false,
  hideBorder = false,
}: InputDockProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  // 保持最新的 isStreaming 值供事件回调使用（避免闭包过期）
  const isStreamingRef = useRef(isStreaming);
  isStreamingRef.current = isStreaming;

  // 输入内容变化时，自动调整 textarea 高度（最高 120px）
  // 紧凑窗口（如 EmptyPage）禁用自动撑高，使用固定高度 + 滚动条
  useEffect(() => {
    if (disableAutoResize) return;
    const ta = textareaRef.current;
    if (ta) {
      ta.style.height = 'auto';
      ta.style.height = Math.min(ta.scrollHeight, 120) + 'px';
    }
  }, [draftInput, disableAutoResize]);

  // 自动聚焦：组件挂载、流式结束、窗口呼出时聚焦输入框
  useEffect(() => {
    const ta = textareaRef.current;
    if (!ta || isStreaming) return;

    // 立即聚焦（组件挂载或 isStreaming 变为 false 时）
    requestAnimationFrame(() => ta.focus());

    // 监听窗口获得焦点事件（快捷键呼出 / 点击托盘图标时触发）
    let unlisten: (() => void) | undefined;
    import('@tauri-apps/api/window').then(({ getCurrentWindow }) => {
      getCurrentWindow().onFocusChanged((focused) => {
        if (focused && !isStreamingRef.current) {
          requestAnimationFrame(() => textareaRef.current?.focus());
        }
      }).then((fn) => { unlisten = fn; });
    }).catch(() => {});

    return () => {
      unlisten?.();
    };
  }, [isStreaming]);

  /**
   * 键盘事件处理
   * - 输入法组合中（中文输入法确认英文等）：不拦截 Enter
   * - Cmd/Ctrl + Enter：允许默认换行行为
   * - 普通 Enter：阻止默认换行行为，触发发送
   * - 流式生成中或无有效输入时不发送
   */
  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter') {
      // IME 输入法处理中：keyCode 229 表示按键正被输入法拦截处理
      // （中文输入法确认英文时 compositionend 在 keydown 之前触发，
      //   此时 isComposing 已为 false，但 keyCode 仍为 229）
      if (e.nativeEvent.isComposing || e.nativeEvent.keyCode === 229) {
        return;
      }
      // Cmd/Ctrl+Enter → 换行，不拦截
      if (e.metaKey || e.ctrlKey) {
        return; // Let default behavior insert newline
      }
      // Plain Enter → 发送消息
      e.preventDefault();
      if (!isStreaming && draftInput.trim()) {
        onSend();
      }
    }
  };

  const handlePointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const bounds = e.currentTarget.getBoundingClientRect();
    e.currentTarget.style.setProperty(
      '--composer-pointer-x',
      `${e.clientX - bounds.left}px`,
    );
    e.currentTarget.style.setProperty(
      '--composer-pointer-y',
      `${e.clientY - bounds.top}px`,
    );
  };

  const handlePointerLeave = (e: PointerEvent<HTMLDivElement>) => {
    e.currentTarget.style.setProperty('--composer-pointer-x', '50%');
    e.currentTarget.style.setProperty('--composer-pointer-y', '0px');
  };

  // 是否有有效输入内容（去除空白后）
  const hasContent = draftInput.trim().length > 0;

  return (
    <div
      className={`input-dock ${hideBorder ? 'is-standalone' : ''}`}
      onPointerMove={handlePointerMove}
      onPointerLeave={handlePointerLeave}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 'var(--space-2)',
        padding: 'var(--space-3)',
        borderTop: 'none',
        width: '100%',
      }}
    >
      {isStreaming ? (
        /* 流式生成状态：显示模型名称和生成进度 + 停止按钮 */
        <>
          <div
            style={{
              flex: 1,
              padding: 'var(--space-2) var(--space-3)',
              fontSize: '13px',
              color: 'var(--text-muted)',
            }}
          >
            {streamingModelName || 'AI'} · 生成中...
          </div>
          <IconButton
            icon={Square}
            onClick={onStop}
            variant="danger"
            size={28}
            iconSize={14}
            title="停止生成"
          />
        </>
      ) : (
        /* 正常输入状态：textarea（内含清除按钮）+ 设置 + 模型选择 + 发送 */
        <>
          <div style={{ position: 'relative', flex: 1, display: 'flex' }}>
            <textarea
              className="composer-textarea"
              ref={textareaRef}
              value={draftInput}
              onChange={(e) => onDraftChange(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="问点什么…"
              rows={1}
              style={{
                flex: 1,
                padding: 'var(--space-2) var(--space-3)',
                paddingRight: hasContent ? '28px' : 'var(--space-3)',
                borderRadius: 'var(--radius-md)',
                border: '1px solid var(--border-subtle)',
                background: 'var(--bg-sunken)',
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
              }}
            />

            {/* 清除按钮：定位在输入框内部右侧 */}
            <div
              style={{
                position: 'absolute',
                right: '6px',
                top: '50%',
                transform: 'translateY(-50%)',
                display: 'flex',
                pointerEvents: hasContent ? 'auto' : 'none',
              }}
            >
              <ClearButton visible={hasContent} onClear={() => onDraftChange('')} />
            </div>
          </div>

          {onSettingsClick && (
            <IconButton
              icon={Settings}
              onClick={onSettingsClick}
              className="settings-motion-button"
              size={24}
              iconSize={13}
              title="设置"
            />
          )}

          {onModelPickerClick && (
            <button
              className="model-picker-trigger"
              onClick={onModelPickerClick}
              title={`切换模型：${selectedModel?.display_name || '未选择'}`}
              aria-label={`切换模型，当前为${selectedModel?.display_name || '未选择'}`}
              aria-haspopup="menu"
              type="button"
            >
              <span
                key={selectedModel?.id || 'no-model'}
                className="model-picker-icon"
                aria-hidden="true"
              >
                <Bot size={14} strokeWidth={1.8} />
              </span>
            </button>
          )}

          <IconButton
            icon={Send}
            onClick={onSend}
            className={`send-motion-button ${hasContent ? 'is-ready' : ''}`}
            // 有内容时显示主色，无内容时显示默认色
            variant={hasContent ? 'primary' : 'default'}
            disabled={!hasContent}
            size={28}
            iconSize={14}
            title="发送"
          />
        </>
      )}
    </div>
  );
}
