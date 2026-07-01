import { useRef, useEffect, type KeyboardEvent } from 'react';
import { Send, Square } from 'lucide-react';
import { IconButton } from '@/components/shared/IconButton';
import { ClearButton } from './ClearButton';
import type { ModelInfo } from '@/types';

/**
 * InputDock 组件的 Props
 * @param isStreaming - 是否正在流式生成中，控制输入栏的状态切换
 * @param streamingModelName - 流式生成时显示的模型名称
 * @param streamingTokens - 流式生成时显示的 token 计数
 * @param selectedModel - 当前选中的模型信息（预留，尚未启用 ModelPicker）
 * @param draftInput - 输入框中的当前草稿文本
 * @param onDraftChange - 输入文本变化时的回调
 * @param onSend - 发送消息的回调
 * @param onStop - 停止生成的回调
 * @param onModelPickerClick - 模型选择器点击回调（预留）
 */
interface InputDockProps {
  isStreaming: boolean;
  streamingModelName?: string;
  streamingTokens?: number;
  selectedModel: ModelInfo | null;
  draftInput: string;
  onDraftChange: (text: string) => void;
  onSend: () => void;
  onStop: () => void;
  onModelPickerClick?: () => void;
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
  streamingTokens,
  draftInput,
  onDraftChange,
  onSend,
  onStop,
}: InputDockProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  // 保持最新的 isStreaming 值供事件回调使用（避免闭包过期）
  const isStreamingRef = useRef(isStreaming);
  isStreamingRef.current = isStreaming;

  // 输入内容变化时，自动调整 textarea 高度（最高 120px）
  useEffect(() => {
    const ta = textareaRef.current;
    if (ta) {
      ta.style.height = 'auto';
      ta.style.height = Math.min(ta.scrollHeight, 120) + 'px';
    }
  }, [draftInput]);

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
   * - 普通 Enter：阻止默认换行行为，触发发送
   * - Cmd/Ctrl + Enter：允许默认换行行为
   * - 流式生成中或无有效输入时不发送
   */
  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter') {
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

  // 是否有有效输入内容（去除空白后）
  const hasContent = draftInput.trim().length > 0;

  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'flex-end',
        gap: 'var(--space-2)',
        padding: 'var(--space-3) var(--space-4)',
        borderTop: '1px solid var(--border-subtle)',
        width: '100%',
      }}
    >
      {/* B logo placeholder */}
      {/* <div
        style={{
          width: 24,
          height: 24,
          borderRadius: 'var(--radius-sm)',
          background: 'var(--buddy-primary)',
          color: 'var(--text-on-primary)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          fontSize: '12px',
          fontWeight: 800,
          flexShrink: 0,
          alignSelf: 'center',
        }}
      >
        B
      </div> */}

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
            {streamingModelName || 'AI'} · 生成中...{' '}
            {streamingTokens ? `${streamingTokens} tokens` : ''}
          </div>
          <IconButton
            icon={Square}
            onClick={onStop}
            variant="danger"
            size={32}
            iconSize={14}
            title="停止生成"
          />
        </>
      ) : (
        /* 正常输入状态：textarea + 清除按钮 + 发送按钮 */
        <>
          <textarea
            ref={textareaRef}
            value={draftInput}
            onChange={(e) => onDraftChange(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="输入消息... Enter 发送 · ⌘Enter 换行"
            rows={1}
            style={{
              flex: 1,
              padding: 'var(--space-2) var(--space-3)',
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

          {/* 清除按钮：有输入内容时才显示 */}
          <ClearButton visible={hasContent} onClear={() => onDraftChange('')} />


          <IconButton
            icon={Send}
            onClick={onSend}
            // 有内容时显示主色，无内容时显示默认色
            variant={hasContent ? 'primary' : 'default'}
            disabled={!hasContent}
            size={32}
            iconSize={16}
            title="发送"
          />
        </>
      )}
    </div>
  );
}
