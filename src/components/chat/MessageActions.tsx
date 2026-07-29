import { useEffect, useRef, useState } from 'react';
import { AnimatePresence, motion, useReducedMotion } from 'framer-motion';
import { ArrowUp, Check, Copy } from 'lucide-react';
import type { ContentBlock, Message } from '@/types';
import { parseThinkBlocks, type TextBlock } from '@/utils/thinkParser';

interface MessageActionsProps {
  message: Message;
  questionId?: string;
  animateIn?: boolean;
}

function formatMessageTime(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000);
  const pad = (value: number) => value.toString().padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(
    date.getDate(),
  )} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function getAnswerText(message: Message): string {
  if (message.blocks && message.blocks.length > 0) {
    return message.blocks
      .filter(
        (block): block is Extract<ContentBlock, { type: 'text' }> =>
          block.type === 'text',
      )
      .map((block) => block.content)
      .join('\n\n')
      .trim();
  }

  return parseThinkBlocks(message.content)
    .filter((segment): segment is TextBlock => segment.type === 'text')
    .map((segment) => segment.content)
    .join('\n\n')
    .trim();
}

export function MessageActions({
  message,
  questionId,
  animateIn = false,
}: MessageActionsProps) {
  const [copied, setCopied] = useState(false);
  const resetTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const shouldReduceMotion = useReducedMotion();

  useEffect(() => {
    return () => {
      if (resetTimerRef.current) clearTimeout(resetTimerRef.current);
    };
  }, []);

  const handleCopy = async () => {
    const text = getAnswerText(message);
    if (!text) return;

    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      if (resetTimerRef.current) clearTimeout(resetTimerRef.current);
      resetTimerRef.current = setTimeout(() => setCopied(false), 1600);
    } catch (error) {
      console.error('复制失败', error);
    }
  };

  const handleBackToQuestion = () => {
    if (!questionId) return;
    document.getElementById(questionId)?.scrollIntoView({
      behavior: shouldReduceMotion ? 'auto' : 'smooth',
      block: 'start',
    });
  };

  const buttonMotion = shouldReduceMotion
    ? {}
    : {
        whileHover: { y: -1 },
        whileTap: { scale: 0.96, y: 0 },
      };

  return (
    <motion.div
      className="message-actions"
      role="group"
      aria-label="回答操作"
      initial={
        animateIn && !shouldReduceMotion
          ? { opacity: 0, y: 5, scale: 0.98 }
          : false
      }
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{
        duration: shouldReduceMotion ? 0 : 0.2,
        ease: [0.2, 0, 0, 1],
      }}
    >
      <motion.button
        {...buttonMotion}
        className={`message-action-button is-copy ${copied ? 'is-copied' : ''}`}
        onClick={handleCopy}
        title={copied ? '已复制' : '复制回答'}
        aria-label={copied ? '回答已复制' : '复制回答'}
        type="button"
      >
        <span className="message-action-icon" aria-hidden="true">
          <AnimatePresence initial={false} mode="wait">
            <motion.span
              key={copied ? 'copied' : 'copy'}
              initial={
                shouldReduceMotion
                  ? false
                  : { opacity: 0, scale: 0.7, rotate: -8 }
              }
              animate={{ opacity: 1, scale: 1, rotate: 0 }}
              exit={
                shouldReduceMotion
                  ? { opacity: 0 }
                  : { opacity: 0, scale: 0.7, rotate: 8 }
              }
              transition={{ duration: shouldReduceMotion ? 0 : 0.12 }}
            >
              {copied ? <Check size={13} /> : <Copy size={13} />}
            </motion.span>
          </AnimatePresence>
        </span>
        <span className="message-action-label" aria-live="polite">
          {copied ? '已复制' : '复制'}
        </span>
      </motion.button>

      {questionId && (
        <motion.button
          {...buttonMotion}
          className="message-action-button is-back"
          onClick={handleBackToQuestion}
          title="回到本轮问题"
          aria-label="回到本轮问题"
          type="button"
        >
          <span className="message-action-icon" aria-hidden="true">
            <ArrowUp size={13} />
          </span>
          <span className="message-action-label">回到问题</span>
        </motion.button>
      )}

      <span className="message-action-divider" aria-hidden="true" />
      <time
        className="message-time"
        dateTime={new Date(message.created_at * 1000).toISOString()}
        title={new Date(message.created_at * 1000).toLocaleString()}
      >
        {formatMessageTime(message.created_at)}
      </time>
    </motion.div>
  );
}
