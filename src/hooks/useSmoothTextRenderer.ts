/**
 * useSmoothTextRenderer.ts —— 平滑文本渲染 Hook
 *
 * 后端 SS E 推送的 text_delta 先入队到 pendingTextBuffer，
 * 本 hook 通过 requestAnimationFrame 轮询，小批量消费到 streamingBlocks，
 * 实现类似 Claude Code 的平滑打字机效果。
 *
 * 速度策略：
 * - 基础速度约 50 个 Unicode 字符/秒，积压较多时自动追赶
 * - 每秒最多更新 Store 25 次，减少 Markdown 解析和 React 重渲染
 * - 后端暂时没有新内容时不消费，星标留在原位播放呼吸动画
 * - 窗口隐藏/失焦时立即消费缓冲，避免 requestAnimationFrame 暂停后积压
 * - 窗口重新显示后短暂保持追赶模式，清理 WebView 恢复时补发的事件
 *
 * 在 ChatPage 中挂载一次即可；只在 isStreaming 期间活跃。
 */

import { useEffect, useRef } from 'react';
import { useChatStore } from '@/stores/chatStore';
import { isBrowser } from '@/utils/mock';
import { WINDOW_WILL_HIDE_EVENT } from '@/utils/windowEvents';

const STREAMING_CHARACTERS_PER_SECOND = 50;
const STREAMING_UPDATES_PER_SECOND = 25;
const STREAMING_UPDATE_INTERVAL = 1000 / STREAMING_UPDATES_PER_SECOND;
const BASE_CHARACTERS_PER_UPDATE = Math.ceil(
  STREAMING_CHARACTERS_PER_SECOND / STREAMING_UPDATES_PER_SECOND,
);
const MAX_CHARACTERS_PER_UPDATE = 16;
const RESUME_CATCH_UP_DURATION = 600;

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

    const stopLoop = () => {
      if (rafRef.current !== 0) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = 0;
      }
    };

    const enterBackgroundMode = () => {
      window.clearTimeout(resumeTimer);
      // WebKit 可能在窗口隐藏时直接丢弃已登记的 rAF 回调。主动取消并清零，
      // 避免恢复后把一个永远不会执行的旧句柄误认为“循环仍在运行”。
      stopLoop();
      renderImmediately = true;
      nextRevealAtRef.current = 0;
      flushPendingText();
    };

    const leaveBackgroundMode = () => {
      // 先清已有缓冲，再给 Tauri/WebView 一小段时间派发隐藏期间积压的事件。
      stopLoop();
      renderImmediately = true;
      flushPendingText();
      queueMicrotask(flushPendingText);
      window.clearTimeout(resumeTimer);
      resumeTimer = window.setTimeout(() => {
        flushPendingText();
        renderImmediately = false;
        nextRevealAtRef.current = 0;
        startLoop();
      }, RESUME_CATCH_UP_DURATION);
    };

    const handleVisibilityChange = () => {
      if (document.visibilityState === 'hidden') {
        enterBackgroundMode();
      } else {
        leaveBackgroundMode();
      }
    };

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
      // 当前回调已经开始执行，对应句柄不再处于 pending 状态。
      // 先清零，允许本帧内到达的新 delta 安排下一帧。
      rafRef.current = 0;
      if (disposed) return;

      if (renderImmediately) {
        flushPendingText();
        nextRevealAtRef.current = 0;
        return;
      }

      const { pendingTextBuffer, isStreaming } = useChatStore.getState();
      // 按固定时间间隔小批量消费，减少 Store 更新和 Markdown 重算。
      if (isStreaming && pendingTextBuffer.length > 0) {
        if (nextRevealAtRef.current === 0) {
          nextRevealAtRef.current = timestamp;
        }
        if (timestamp >= nextRevealAtRef.current) {
          const charactersToReveal = Math.min(
            MAX_CHARACTERS_PER_UPDATE,
            Math.max(
              BASE_CHARACTERS_PER_UPDATE,
              Math.ceil(pendingTextBuffer.length / STREAMING_UPDATES_PER_SECOND),
            ),
          );
          useChatStore.getState().smoothTextDelta(charactersToReveal);
          const scheduledNext =
            nextRevealAtRef.current + STREAMING_UPDATE_INTERVAL;
          nextRevealAtRef.current =
            timestamp - scheduledNext > STREAMING_UPDATE_INTERVAL
              ? timestamp + STREAMING_UPDATE_INTERVAL
              : scheduledNext;
        }
      } else {
        nextRevealAtRef.current = 0;
      }
      // smoothTextDelta 会同步更新 store，必须按最新缓冲决定是否继续，
      // 不能使用本帧开始时的 pendingTextBuffer 快照。
      startLoop();
    };

    const startLoop = () => {
      if (disposed || renderImmediately || rafRef.current !== 0) return;
      const { pendingTextBuffer, isStreaming } = useChatStore.getState();
      if (!isStreaming || pendingTextBuffer.length === 0) return;
      rafRef.current = requestAnimationFrame(tick);
    };

    const unsubscribeStore = useChatStore.subscribe((state, previous) => {
      // 有新内容入队：确保 rAF 循环在运行（空闲时它已停止）
      if (
        state.pendingTextBuffer &&
        state.pendingTextBuffer !== previous.pendingTextBuffer
      ) {
        if (renderImmediately) {
          flushPendingText();
        } else {
          startLoop();
        }
      }
    });

    if (renderImmediately) {
      flushPendingText();
    } else {
      startLoop();
    }
    return () => {
      disposed = true;
      window.clearTimeout(resumeTimer);
      stopLoop();
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
