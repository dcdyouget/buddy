import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { Message } from '@/types';
import { CodeBlock } from './CodeBlock';

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
 * 用户消息右对齐，带品牌色背景；AI 消息左对齐，支持 Markdown 渲染和代码高亮。
 * AI 流式输出时会在末尾显示闪烁光标。
 */
export function MessageBubble({ message, isStreaming = false }: MessageBubbleProps) {
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
              }
            : {
                maxWidth: '85%',
                padding: 'var(--space-2) 0',
                color: 'var(--text-primary)',
                fontSize: '14px',
                lineHeight: 1.6,
              }
        }
      >
        {isUser ? (
          // 用户消息：纯文本展示
          <span>{message.content}</span>
        ) : (
          // AI 消息：Markdown 渲染，支持代码高亮
          <div className="ai-message-content">
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              components={{
                // 自定义代码块渲染：区分行内代码和围栏代码块
                code({ className, children, ...props }) {
                  // 匹配 language-xxx 格式的 className，判断是否为围栏代码块
                  const match = /language-(\w+)/.exec(className || '');
                  const codeStr = String(children).replace(/\n$/, '');
                  if (match) {
                    // 围栏代码块：使用 CodeBlock 组件进行语法高亮
                    return (
                      <CodeBlock language={match[1]} source={codeStr} />
                    );
                  }
                  // 行内代码：简单样式
                  return (
                    <code
                      className={className}
                      style={{
                        fontFamily: 'var(--font-mono)',
                        fontSize: '13px',
                        background: 'var(--bg-sunken)',
                        padding: '2px 4px',
                        borderRadius: 'var(--radius-sm)',
                      }}
                      {...props}
                    >
                      {children}
                    </code>
                  );
                },
                // 自定义段落渲染，控制段落间距
                p({ children }) {
                  return (
                    <p style={{ margin: '0 0 var(--space-2) 0' }}>
                      {children}
                    </p>
                  );
                },
              }}
            >
              {/* 流式输出时空内容也保留一个空格，防止 ReactMarkdown 因空内容报错 */}
              {message.content || (isStreaming ? ' ' : '')}
            </ReactMarkdown>
            {/* 流式输出时显示闪烁光标，模拟打字效果 */}
            {isStreaming && <span className="buddy-cursor" />}
          </div>
        )}
      </div>
    </div>
  );
}
