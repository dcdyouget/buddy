/**
 * useSmoothTextRenderer.ts —— 平滑文本渲染 Hook
 *
 * 后端 SS E 推送的 text_delta 先入队到 pendingTextBuffer，
 * 本 hook 通过 requestAnimationFrame 轮询，逐字消费到 streamingBlocks，
 * 实现类似 Claude Code 的平滑打字机效果。
 *
 * 速度策略：
 * - 最多显示 50 个 Unicode 字符/秒
 * - 后端暂时没有新内容时不消费，星标留在原位播放呼吸动画
 * - 窗口隐藏/失焦时立即消费缓冲，避免 requestAnimationFrame 暂停后积压
 * - 窗口重新显示后短暂保持追赶模式，清理 WebView 恢复时补发的事件
 *
 * 在 ChatPage 中挂载一次即可；只在 isStreaming 期间活跃。
 */

import { useEffect, useRef } from 'react';
import { useChatStore } from '@/stores/chatStore';
import { isBrowser } from '@/utils/mock';

const STREAMING_CHARACTERS_PER_SECOND = 50;
const STREAMING_CHARACTER_INTERVAL =
  1000 / STREAMING_CHARACTERS_PER_SECOND;
const RESUME_CATCH_UP_DURATION = 600;

/** Esc 隐藏前由 App 主动发出，避免等待原生失焦事件。 */
export const WINDOW_WILL_HIDE_EVENT = 'buddy:window-will-hide';

export function useSmoothTextRenderer() {
  const rafRef = useRef<number>(0);
  const nextRevealAtRef = useRef(0);

  useEffect(() => {
    let disposed = false;
    let resumeTimer = 0;
    let unlistenFocus: (() => void) | undefined;
    let renderImmediately =
      typeof document !== 'undefined' && document.visibilityState === 'hidden';

    const flushPendingText = () => {
      const state = useChatStore.getState();
      if (state.pendingTextBuffer.length > 0) {
        state.flushTextBuffer();
      }
    };

    const enterBackgroundMode = () => {
      window.clearTimeout(resumeTimer);
      renderImmediately = true;
      nextRevealAtRef.current = 0;
      flushPendingText();
    };

    const leaveBackgroundMode = () => {
      // 先清已有缓冲，再给 Tauri/WebView 一小段时间派发隐藏期间积压的事件。
      renderImmediately = true;
      flushPendingText();
      queueMicrotask(flushPendingText);
      window.clearTimeout(resumeTimer);
      resumeTimer = window.setTimeout(() => {
        flushPendingText();
        renderImmediately = false;
        nextRevealAtRef.current = 0;
      }, RESUME_CATCH_UP_DURATION);
    };

    const handleVisibilityChange = () => {
      if (document.visibilityState === 'hidden') {
        enterBackgroundMode();
      } else {
        leaveBackgroundMode();
      }
    };

    const unsubscribeStore = useChatStore.subscribe((state, previous) => {
      if (
        renderImmediately &&
        state.pendingTextBuffer &&
        state.pendingTextBuffer !== previous.pendingTextBuffer
      ) {
        flushPendingText();
      }
    });

    window.addEventListener(WINDOW_WILL_HIDE_EVENT, enterBackgroundMode);
    document.addEventListener('visibilitychange', handleVisibilityChange);

    // 全局快捷键直接由 Rust 隐藏窗口，不会经过 App 的 Esc 处理；
    // 原生焦点事件为这条路径提供同样的后台消费行为。
    if (!isBrowser) {
      import('@tauri-apps/api/window')
        .then(async ({ getCurrentWindow }) => {
          const unlisten = await getCurrentWindow().onFocusChanged(
            ({ payload: focused }) => {
              if (focused) {
                leaveBackgroundMode();
              } else {
                enterBackgroundMode();
              }
            },
          );
          if (disposed) {
            unlisten();
          } else {
            unlistenFocus = unlisten;
          }
        })
        .catch(() => {});
    }

    const tick = (timestamp: number) => {
      if (renderImmediately) {
        flushPendingText();
        nextRevealAtRef.current = 0;
        rafRef.current = requestAnimationFrame(tick);
        return;
      }

      const { pendingTextBuffer, isStreaming } = useChatStore.getState();
      // 按固定时间间隔逐字消费，避免刷新率变化影响输出速度。
      if (isStreaming && pendingTextBuffer.length > 0) {
        if (nextRevealAtRef.current === 0) {
          nextRevealAtRef.current = timestamp;
        }
        if (timestamp >= nextRevealAtRef.current) {
          useChatStore.getState().smoothTextDelta(1);
          const scheduledNext =
            nextRevealAtRef.current + STREAMING_CHARACTER_INTERVAL;
          nextRevealAtRef.current =
            timestamp - scheduledNext > STREAMING_CHARACTER_INTERVAL
              ? timestamp + STREAMING_CHARACTER_INTERVAL
              : scheduledNext;
        }
      } else {
        nextRevealAtRef.current = 0;
      }
      rafRef.current = requestAnimationFrame(tick);
    };

    rafRef.current = requestAnimationFrame(tick);
    return () => {
      disposed = true;
      window.clearTimeout(resumeTimer);
      cancelAnimationFrame(rafRef.current);
      unsubscribeStore();
      unlistenFocus?.();
      window.removeEventListener(
        WINDOW_WILL_HIDE_EVENT,
        enterBackgroundMode,
      );
      document.removeEventListener(
        'visibilitychange',
        handleVisibilityChange,
      );
    };
  }, []);
}
