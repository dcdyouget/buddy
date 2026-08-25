import { useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useChatStore } from '@/stores/chatStore';
import type { Message } from '@/types';
import { MessageBubble } from './MessageBubble';

interface LiveMessageBubbleProps {
  message: Message;
  questionId?: string;
  isContinuation: boolean;
  continuesToNext: boolean;
}

/**
 * 仅让最后一条实时消息订阅高频流式状态，避免 ChatPage 和全部历史气泡
 * 随每批字符重新协调。
 */
export function LiveMessageBubble({
  message,
  questionId,
  isContinuation,
  continuesToNext,
}: LiveMessageBubbleProps) {
  const {
    streamingBlocks,
    streamingRevealCount,
    streamingRevealRevision,
    activeToolCalls,
  } = useChatStore(
    useShallow((state) => ({
      streamingBlocks: state.streamingBlocks,
      streamingRevealCount: state.streamingRevealCount,
      streamingRevealRevision: state.streamingRevealRevision,
      activeToolCalls: state.activeToolCalls,
    })),
  );

  const displayMessage = useMemo(
    () =>
      streamingBlocks.length > 0
        ? { ...message, blocks: streamingBlocks }
        : message,
    [message, streamingBlocks],
  );
  const liveToolCalls = useMemo(
    () =>
      Object.keys(activeToolCalls).length > 0
        ? Object.values(activeToolCalls)
        : undefined,
    [activeToolCalls],
  );

  return (
    <MessageBubble
      message={displayMessage}
      isStreaming
      questionId={questionId}
      isContinuation={isContinuation}
      continuesToNext={continuesToNext}
      liveToolCalls={liveToolCalls}
      streamingRevealCount={streamingRevealCount}
      streamingRevealRevision={streamingRevealRevision}
    />
  );
}
