import { memo } from 'react';
import { CornerUpLeft } from 'lucide-react';
import type { Message, ContentBlock } from '@/types';
import { parseThinkBlocks } from '@/utils/thinkParser';
import { StreamingMarkdown } from './StreamingMarkdown';
import { ThinkSection } from './ThinkSection';

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

  const handleBackToQuestion = () => {
    if (!questionId) return;
    document.getElementById(questionId)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
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
              <button
                onClick={handleBackToQuestion}
                title="回到问题"
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '4px',
                  marginTop: 'var(--space-2)',
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
