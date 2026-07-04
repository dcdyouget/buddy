import { memo, useState } from 'react';
import { Check, Copy, CornerUpLeft } from 'lucide-react';
import type { Message, ContentBlock } from '@/types';
import { parseThinkBlocks, type TextBlock } from '@/utils/thinkParser';
import { StreamingMarkdown } from './StreamingMarkdown';
import { ThinkSection } from './ThinkSection';

/**
 * 把秒级 Unix 时间戳格式化为本地时区的 YYYY-MM-DD HH:MM 字符串。
 * 例如 1752135600 → "2025-07-10 10:30"（按系统时区显示）
 */
function formatMessageTime(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  const pad = (n: number) => n.toString().padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/**
 * 提取消息里"对用户可见"的回答正文，不含思考过程。
 * - v2.0 blocks：按顺序拼接所有 type === 'text' 的块
 * - v1.0 旧格式：用 thinkParser 剥掉 思考 段
 */
function getAnswerText(message: Message): string {
  if (message.blocks && message.blocks.length > 0) {
    return message.blocks
      .filter((b): b is Extract<ContentBlock, { type: 'text' }> => b.type === 'text')
      .map((b) => b.content)
      .join('\n\n')
      .trim();
  }
  return parseThinkBlocks(message.content)
    .filter((s): s is TextBlock => s.type === 'text')
    .map((s) => s.content)
    .join('\n\n')
    .trim();
}

/**
 * MessageBubble 组件的 Props
 */
interface MessageBubbleProps {
  message: Message;
  isStreaming?: boolean;
  questionId?: string;
}

/**
 * 渲染 AI 助手消息的内容区域
 *
 * 优先级：
 * 1. 如果 message.blocks 存在（v2.0 结构化格式），直接用 blocks 渲染
 * 2. 否则回退到旧的 <think> 标签解析（v1.0 兼容）
 */
function AssistantContent({ message, isStreaming }: { message: Message; isStreaming: boolean }) {
  // ── v2.0: 结构化 blocks 渲染 ──
  if (message.blocks && message.blocks.length > 0) {
    return (
      <>
        {message.blocks.map((block: ContentBlock, i: number) => {
          const isLast = i === message.blocks!.length - 1;
          if (block.type === 'thinking') {
            const thinkStreaming = isLast && block.is_open && isStreaming;
            return (
              <ThinkSection
                key={`block-think-${i}`}
                content={block.content}
                isStreaming={thinkStreaming}
                defaultExpanded={thinkStreaming}
              />
            );
          }
          // text block
          return (
            <StreamingMarkdown
              key={`block-text-${i}`}
              content={block.content}
              isStreaming={isLast && isStreaming}
            />
          );
        })}
        {isStreaming && <span className="buddy-cursor" />}
      </>
    );
  }

  // ── v1.0: 旧格式 <think> 标签解析（向后兼容）──
  const segments = parseThinkBlocks(message.content);
  if (segments.length === 0 || (segments.length === 1 && segments[0].type === 'text')) {
    return (
      <>
        <StreamingMarkdown content={message.content} isStreaming={isStreaming} />
        {isStreaming && <span className="buddy-cursor" />}
      </>
    );
  }

  return (
    <>
      {segments.map((seg, i) => {
        const isLast = i === segments.length - 1;
        if (seg.type === 'think') {
          const thinkStreaming = isLast && seg.isOpen && isStreaming;
          return (
            <ThinkSection
              key={`think-${i}`}
              content={seg.content}
              isStreaming={thinkStreaming}
              defaultExpanded={thinkStreaming}
            />
          );
        }
        return (
          <StreamingMarkdown
            key={`text-${i}`}
            content={seg.content}
            isStreaming={isLast && isStreaming}
          />
        );
      })}
      {isStreaming && <span className="buddy-cursor" />}
    </>
  );
}

/**
 * 消息气泡组件
 */
export const MessageBubble = memo(function MessageBubble({
  message,
  isStreaming = false,
  questionId,
}: MessageBubbleProps) {
  const isUser = message.role === 'user';
  const [copied, setCopied] = useState(false);

  const handleBackToQuestion = () => {
    if (!questionId) return;
    document.getElementById(questionId)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  };

  const handleCopy = async () => {
    const text = getAnswerText(message);
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch (err) {
      console.error('复制失败', err);
    }
  };

  return (
    <div
      id={isUser ? `msg-${message.id}` : undefined}
      style={{
        display: 'flex',
        justifyContent: isUser ? 'flex-end' : 'flex-start',
        padding: 'var(--space-2) var(--space-4)',
      }}
    >
      <div
        style={
          isUser
            ? {
                maxWidth: '80%',
                padding: 'var(--space-2) var(--space-3)',
                borderRadius: '8px 8px 8px 4px',
                background: 'var(--primary-tint-soft)',
                border: '1px solid var(--primary-tint-strong)',
                color: 'var(--text-primary)',
                fontSize: '14px',
                lineHeight: 1.5,
                overflowWrap: 'break-word',
                wordBreak: 'break-word',
                minWidth: 0,
              }
            : {
                width: '100%',
                maxWidth: '85%',
                padding: 'var(--space-2) var(--space-2)',
                color: 'var(--text-primary)',
                fontSize: '14px',
                lineHeight: 1.6,
                overflowWrap: 'break-word',
                wordBreak: 'break-word',
                minWidth: 0,
              }
        }
      >
        {isUser ? (
          <span>{message.content}</span>
        ) : (
          <>
            <AssistantContent message={message} isStreaming={isStreaming} />
            {!isStreaming && questionId && message.content && (
              <div
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: 'var(--space-2)',
                  marginTop: 'var(--space-2)',
                }}
              >
                <span
                  title={new Date(message.created_at * 1000).toLocaleString()}
                  style={{
                    fontSize: '12px',
                    color: 'var(--text-tertiary)',
                    fontVariantNumeric: 'tabular-nums',
                    userSelect: 'none',
                  }}
                >
                  {formatMessageTime(message.created_at)}
                </span>
                <button
                  onClick={handleCopy}
                  title={copied ? '已复制' : '复制回答'}
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: '4px',
                    padding: '2px 8px',
                    border: 'none',
                    borderRadius: 'var(--radius-sm)',
                    background: copied ? 'var(--primary-tint-soft)' : 'transparent',
                    color: copied ? 'var(--primary)' : 'var(--text-tertiary)',
                    fontSize: '12px',
                    cursor: 'pointer',
                    transition: 'color 0.15s, background 0.15s',
                  }}
                  onMouseEnter={(e) => {
                    if (copied) return;
                    e.currentTarget.style.color = 'var(--text-muted)';
                    e.currentTarget.style.background = 'var(--bg-sunken)';
                  }}
                  onMouseLeave={(e) => {
                    if (copied) return;
                    e.currentTarget.style.color = 'var(--text-tertiary)';
                    e.currentTarget.style.background = 'transparent';
                  }}
                >
                  {copied ? <Check size={13} /> : <Copy size={13} />}
                  {copied ? '已复制' : '复制'}
                </button>
                <button
                  onClick={handleBackToQuestion}
                  title="回到问题"
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: '4px',
                    padding: '2px 8px',
                    border: 'none',
                    borderRadius: 'var(--radius-sm)',
                    background: 'transparent',
                    color: 'var(--text-tertiary)',
                    fontSize: '12px',
                    cursor: 'pointer',
                    transition: 'color 0.15s, background 0.15s',
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.color = 'var(--text-muted)';
                    e.currentTarget.style.background = 'var(--bg-sunken)';
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.color = 'var(--text-tertiary)';
                    e.currentTarget.style.background = 'transparent';
                  }}
                >
                  <CornerUpLeft size={13} />
                  回到问题
                </button>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
},
(prevProps, nextProps) => {
  return (
    prevProps.message.id === nextProps.message.id &&
    prevProps.message.content === nextProps.message.content &&
    prevProps.message.blocks === nextProps.message.blocks &&
    prevProps.isStreaming === nextProps.isStreaming &&
    prevProps.questionId === nextProps.questionId
  );
});
