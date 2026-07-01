/**
 * useStreaming.ts — 流式事件监听 Hook
 *
 * 在应用根组件中调用一次，注册对 Rust 后端流式事件的监听。
 *
 * 事件协议（v2.0 统一格式）：
 * - stream-event：统一事件，携带 JSON 化的 StreamEvent
 *   - start → 流式开始
 *   - text_start/delta/end → 文本块生命周期
 *   - thinking_start/delta/end → 思考块生命周期
 *   - done → 流式正常完成 → chatStore.handleStreamDone()
 *   - error → 流式错误/取消 → chatStore.handleStreamError()
 *
 * 防竞态机制：
 * 使用 epochRef 计数器确保组件重新挂载时，旧的监听器不会被注册。
 */

import { useEffect, useRef } from 'react';
import { useChatStore } from '@/stores/chatStore';
import { useUIStore } from '@/stores/uiStore';
import type { Message, StreamEvent } from '@/types';
import { isBrowser } from '@/utils/mock';

export function useStreaming() {
  const handleTextStart = useChatStore((s) => s.handleTextStart);
  const handleTextDelta = useChatStore((s) => s.handleTextDelta);
  const handleTextEnd = useChatStore((s) => s.handleTextEnd);
  const handleThinkingStart = useChatStore((s) => s.handleThinkingStart);
  const handleThinkingDelta = useChatStore((s) => s.handleThinkingDelta);
  const handleThinkingEnd = useChatStore((s) => s.handleThinkingEnd);
  const handleStreamDone = useChatStore((s) => s.handleStreamDone);
  const handleStreamError = useChatStore((s) => s.handleStreamError);
  const setPage = useUIStore((s) => s.setPage);

  const epochRef = useRef(0);

  useEffect(() => {
    if (isBrowser) return;

    const epoch = ++epochRef.current;
    const unlisteners: Array<() => void> = [];

    import('@tauri-apps/api/event').then(async ({ listen }) => {
      if (epoch !== epochRef.current) return;

      // ── v2.0 统一事件协议 ────────────────────────────
      unlisteners.push(
        await listen<StreamEvent>('stream-event', (event) => {
          const e = event.payload;
          switch (e.event) {
            case 'start':
              break;

            case 'text_start':
              handleTextStart(e.content_index);
              break;

            case 'text_delta':
              handleTextDelta(e.content_index, e.delta);
              break;

            case 'text_end':
              handleTextEnd(e.content_index, e.content);
              break;

            case 'thinking_start':
              handleThinkingStart(e.content_index);
              break;

            case 'thinking_delta':
              handleThinkingDelta(e.content_index, e.delta);
              break;

            case 'thinking_end':
              handleThinkingEnd(e.content_index, e.content);
              break;

            case 'done':
              handleStreamDone();
              setPage('conversation');
              break;

            case 'error': {
              // 用户主动取消：正常收尾
              if (e.reason === 'aborted') {
                handleStreamDone();
                setPage('conversation');
                break;
              }

              handleStreamError(e.reason, e.message);
              useUIStore.getState().setError(e.message);

              // 根据错误类型进行不同处理
              if (e.message.includes('401') || e.message.includes('unauthorized')) {
                setPage('noapikey');
              } else if (e.message.includes('429') || e.message.includes('quota')) {
                const chatState = useChatStore.getState();
                const warningMsg: Message = {
                  id: 'warn-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 9),
                  role: 'assistant',
                  content: 'API 配额已用尽，请稍后再试或检查您的账户限额。',
                  model_id: null,
                  created_at: Math.floor(Date.now() / 1000),
                };
                chatState.setMessages([...chatState.messages, warningMsg]);
                setPage('conversation');
              } else if (e.message.includes('server') || e.message.includes('500')) {
                const chatState = useChatStore.getState();
                const retryMsg: Message = {
                  id: 'err-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 9),
                  role: 'assistant',
                  content: '请求失败，请重试',
                  model_id: null,
                  created_at: Math.floor(Date.now() / 1000),
                };
                chatState.setMessages([...chatState.messages, retryMsg]);
                setPage('conversation');
              } else if (e.message.includes('network') || e.message.includes('timeout')) {
                const chatState = useChatStore.getState();
                const retryMsg: Message = {
                  id: 'err-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 9),
                  role: 'assistant',
                  content: '网络错误，请重试',
                  model_id: null,
                  created_at: Math.floor(Date.now() / 1000),
                };
                chatState.setMessages([...chatState.messages, retryMsg]);
                setPage('conversation');
              } else {
                setPage('conversation');
              }
              break;
            }
          }
        }),
      );
    });

    return () => {
      epochRef.current++;
      unlisteners.forEach((fn) => fn());
    };
  }, []);
}
