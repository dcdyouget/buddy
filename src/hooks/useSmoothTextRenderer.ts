/**
 * useSmoothTextRenderer.ts —— 平滑文本渲染 Hook
 *
 * 后端 SS E 推送的 text_delta 先入队到 pendingTextBuffer，
 * 本 hook 通过 requestAnimationFrame 轮询，逐字消费到 streamingBlocks，
 * 实现类似 Claude Code 的平滑打字机效果。
 *
 * 速度策略（自适应）：
 * - 基础：2 字/帧 ≈ 120 字/秒（保证流畅的最低速率）
 * - 缓冲积压时加速：每帧取 buffer 长度的 10%，上限 50 字/帧
 * - 缓冲 < 10 字时：1 字/帧，让短句也有打字节奏感
 *
 * 在 ChatPage 中挂载一次即可；只在 isStreaming 期间活跃。
 */

import { useEffect, useRef } from 'react';
import { useChatStore } from '@/stores/chatStore';

/** 自适应计算每帧应渲染的字符数 */
function calcCharsPerFrame(bufferLen: number): number {
  if (bufferLen === 0) return 0;
  if (bufferLen < 10) return 1; // 短缓冲时慢速，打字节奏感
  // 基础 2 字 + 10% 缓冲长度，上限 50 字/帧
  return Math.min(50, 2 + Math.floor(bufferLen * 0.1));
}

export function useSmoothTextRenderer() {
  const rafRef = useRef<number>(0);

  useEffect(() => {
    const tick = () => {
      const { pendingTextBuffer, isStreaming } = useChatStore.getState();
      // 只在流式期间且缓冲区有内容时消费
      if (isStreaming && pendingTextBuffer.length > 0) {
        const count = calcCharsPerFrame(pendingTextBuffer.length);
        useChatStore.getState().smoothTextDelta(count);
      }
      rafRef.current = requestAnimationFrame(tick);
    };

    rafRef.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafRef.current);
  }, []);
}
