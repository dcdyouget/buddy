import { memo } from 'react';
import { CornerUpLeft } from 'lucide-react';
import type { Message } from '@/types';
import { parseThinkBlocks } from '@/utils/thinkParser';
import { StreamingMarkdown } from './StreamingMarkdown';
import { ThinkSection } from './ThinkSection';

/**
 * MessageBubble 组件的 Props
 * @param message - 要渲染的消息对象，包含角色和内容
 * @param isStreaming - 是否正在流式输出中，用于显示光标动画
 * @param questionId - 此回答对应的用户问题的 DOM id，用于"回到问题"按钮
 */
interface MessageBubbleProps {
  message: Message;
  isStreaming?: boolean;
  questionId?: string;
}

/**
 * 消息气泡组件
 * 根据消息角色（user/assistant）渲染不同样式的聊天气泡。
 * 用户消息右对齐，带品牌色背景；AI 消息左对齐，使用流式增量 Markdown 渲染。
 * AI 流式输出时会在末尾显示闪烁光标。
 * 非流式 AI 消息末尾显示「回到问题」按钮，点击跳转到对应的问题位置。
 *
 * 使用 React.memo + 自定义比较函数防止不必要的重渲染。
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
            {(() => {
              const segments = parseThinkBlocks(message.content);
              // 无 think 标签 → 沿用原有渲染路径
              if (segments.length === 0 || (segments.length === 1 && segments[0].type === 'text')) {
                return (
                  <StreamingMarkdown
                    content={message.content}
                    isStreaming={isStreaming}
                  />
                );
              }
              // 有 think 标签 → 分段渲染
              return segments.map((seg, i) => {
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
              });
            })()}
            {/* 流式输出中的闪烁光标 — 始终在所有内容末尾 */}
            {isStreaming && <span className="buddy-cursor" />}
            {/* 回答完成且关联了问题：显示"回到问题"按钮 */}
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
    prevProps.isStreaming === nextProps.isStreaming &&
    prevProps.questionId === nextProps.questionId
  );
});
