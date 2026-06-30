import { memo } from 'react';
import type { Message } from '@/types';
import { StreamingMarkdown } from './StreamingMarkdown';

/**
 * MessageBubble 组件的 Props
 * @param message - 要渲染的消息对象，包含角色和内容
 * @param isStreaming - 是否正在流式输出中，用于显示光标动画
 */
interface MessageBubbleProps {
  message: Message;
  isStreaming?: boolean;
}

/**
 * 消息气泡组件
 * 根据消息角色（user/assistant）渲染不同样式的聊天气泡。
 * 用户消息右对齐，带品牌色背景；AI 消息左对齐，使用流式增量 Markdown 渲染。
 * AI 流式输出时会在末尾显示闪烁光标。
 *
 * 使用 React.memo + 自定义比较函数防止不必要的重渲染：
 * - 流式输出期间，只有最后一条消息（内容在变化）需要重渲染
 * - 其余消息完全不变，跳过渲染以保持主线程流畅
 * - 流式消息内部通过 StreamingMarkdown 做增量 Markdown 解析，避免每 token 全量 AST 重解析
 */
export const MessageBubble = memo(function MessageBubble({
  message,
  isStreaming = false,
}: MessageBubbleProps) {
  // 判断是否为用户消息，决定对齐和样式方向
  const isUser = message.role === 'user';

  return (
    <div
      style={{
        display: 'flex',
        // 用户消息靠右，AI 消息靠左
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
                // 右下角为小圆角，模拟对话气泡的尾巴
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
                padding: 'var(--space-2) 0',
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
          // 用户消息：纯文本展示
          <span>{message.content}</span>
        ) : (
          // AI 消息：流式增量 Markdown 渲染，支持代码高亮
          <StreamingMarkdown
            content={message.content}
            isStreaming={isStreaming}
          />
        )}
      </div>
    </div>
  );
},
// 自定义比较函数：仅在 message 内容或 isStreaming 状态变化时才重渲染
// 流式过程中，只有最后一条消息的内容在持续变化，其余消息保持不变
(prevProps, nextProps) => {
  return (
    prevProps.message.id === nextProps.message.id &&
    prevProps.message.content === nextProps.message.content &&
    prevProps.isStreaming === nextProps.isStreaming
  );
});
