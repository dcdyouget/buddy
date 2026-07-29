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
 *
 * 在 ChatPage 中挂载一次即可；只在 isStreaming 期间活跃。
 */

import { useEffect, useRef } from 'react';
import { useChatStore } from '@/stores/chatStore';

const STREAMING_CHARACTERS_PER_SECOND = 50;
const STREAMING_CHARACTER_INTERVAL =
  1000 / STREAMING_CHARACTERS_PER_SECOND;

export function useSmoothTextRenderer() {
  const rafRef = useRef<number>(0);
  const nextRevealAtRef = useRef(0);

  useEffect(() => {
    const tick = (timestamp: number) => {
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
    return () => cancelAnimationFrame(rafRef.current);
  }, []);
}
