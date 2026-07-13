import { memo, useState } from 'react';
import { Fragment } from 'react';
import { ArrowUp, Check, Copy, CornerDownLeft } from 'lucide-react';
import type { Message, ContentBlock, ToolCall } from '@/types';
import { parseThinkBlocks, type TextBlock } from '@/utils/thinkParser';
import { StreamingMarkdown } from './StreamingMarkdown';
import { ThinkSection } from './ThinkSection';
import { ToolSection } from './ToolSection';

/**
 * 把秒级 Unix 时间戳格式化为本地时区的 YYYY-MM-DD HH:MM 字符串。
 * 例如 1752135600 → "2025-07-10 10:30"（按系统时区显示）
 */
function formatMessageTime(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  const pad = (n: number) => n.toString().padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(
    d.getHours(),
  )}:${pad(d.getMinutes())}`;
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
 * 渲染一个嵌套在父 assistant 消息内部的"用户回应"小气泡。
 * 视觉上用 ↳ 前缀 + 缩进,与主消息气泡明显区分。
 */
function ChildResponseBubble({ response }: { response: Message }) {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'flex-start',
        gap: 'var(--space-2)',
        marginTop: 'var(--space-2)',
        marginLeft: 'var(--space-4)',
        paddingLeft: 'var(--space-3)',
        borderLeft: '2px solid var(--border-default)',
      }}
    >
      <CornerDownLeft
        size={13}
        style={{ color: 'var(--text-tertiary)', flexShrink: 0, marginTop: 3 }}
      />
      <div
        style={{
          flex: 1,
          padding: 'var(--space-2) var(--space-3)',
          borderRadius: 'var(--radius-md) var(--radius-md) var(--radius-sm) var(--radius-md)',
          background: 'var(--primary-tint-soft)',
          color: 'var(--text-primary)',
          fontSize: '13px',
          lineHeight: 1.5,
          overflowWrap: 'break-word',
          wordBreak: 'break-word',
          minWidth: 0,
        }}
      >
        <span style={{ whiteSpace: 'pre-wrap' }}>{response.content}</span>
      </div>
    </div>
  );
}

/**
 * MessageBubble 组件的 Props
 */
interface MessageBubbleProps {
  message: Message;
  isStreaming?: boolean;
  questionId?: string;
  /** 当前消息是上一条 assistant 消息的工具循环续段。 */
  isContinuation?: boolean;
  /** 下一条消息仍是当前 assistant 工具循环的续段。 */
  continuesToNext?: boolean;
  /**
   * 流式过程中 chatStore 累积的实时 tool_call 列表。
   * 当传入时,会与 message.tool_calls 合并(去重:以 id 为键,live 优先)。
   * 非流式消息可省略 — 直接读 message.tool_calls。
   */
  liveToolCalls?: ToolCall[];
  /**
   * 父 assistant 消息的子回应(用户对模型问题的回答)。
   * 由 ChatPage 预先按 parent_message_id 筛选后传入,
   * 在 assistant 消息块的下方以小气泡形式渲染。
   */
  childResponses?: Message[];
}

/**
 * 渲染 AI 助手消息的内容区域
 *
 * 优先级：
 * 1. 如果 message.blocks 存在（v2.0 结构化格式），直接用 blocks 渲染
 * 2. 否则回退到旧的 <think> 标签解析（v1.0 兼容）
 * 3. 在内容区之后追加 tool_calls 区块（live 优先 + message.tool_calls）
 */
function AssistantContent({
  message,
  isStreaming,
  liveToolCalls,
}: {
  message: Message;
  isStreaming: boolean;
  liveToolCalls?: ToolCall[];
}) {
  // 合并 live 与已持久化的 tool_calls:以 id 去重,live 优先
  const persisted = message.tool_calls || [];
  const live = liveToolCalls || [];
  const liveIds = new Set(live.map((tc) => tc.id));
  const toolCallsToRender = [
    ...live,
    ...persisted.filter((tc) => !liveIds.has(tc.id)),
  ];

  // 决定渲染哪些 block(走 v2 路径还是 v1 解析回退)
  let blocks: ContentBlock[];
  if (message.blocks && message.blocks.length > 0) {
    blocks = message.blocks;
  } else {
    const segments = parseThinkBlocks(message.content);
    blocks = segments.map((s) =>
      s.type === 'think'
        ? { type: 'thinking' as const, content: s.content, is_open: s.isOpen }
        : { type: 'text' as const, content: s.content },
    );
  }

  // 把 tool_calls 按 insertAfterBlockIndex 分桶,
  // 渲染时在 block[i] 之后插入属于 i 的 tool_call,实现"在模型调用 tool 的位置显示"
  // - 有 insertAfterBlockIndex 的: 按索引插入
  // - 没有的(旧消息/兜底): 全部放在最后一个 block 之后
  const toolCallsByIndex = new Map<number, ToolCall[]>();
  const hasAnyInsertIndex = toolCallsToRender.some(
    (tc) => typeof tc.insertAfterBlockIndex === 'number',
  );
  const fallbackIndex = blocks.length > 0 ? blocks.length - 1 : 0;
  for (const tc of toolCallsToRender) {
    const idx =
      typeof tc.insertAfterBlockIndex === 'number'
        ? tc.insertAfterBlockIndex
        : hasAnyInsertIndex
        ? blocks.length // 跳过 block,放到末尾
        : fallbackIndex;
    if (!toolCallsByIndex.has(idx)) toolCallsByIndex.set(idx, []);
    toolCallsByIndex.get(idx)!.push(tc);
  }

  /**
   * 渲染单个 block(根据 type 分发到 ThinkSection / StreamingMarkdown)
   */
  const renderBlock = (block: ContentBlock, i: number, isLast: boolean) => {
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
    return (
      <StreamingMarkdown
        key={`block-text-${i}`}
        content={block.content}
        isStreaming={isLast && isStreaming}
      />
    );
  };

  // ── 主体:遍历 blocks,每个 block 之后插入属于该索引的 tool_calls ──
  return (
    <div className="assistant-content-flow">
      {blocks.map((block, i) => {
        const isLast = i === blocks.length - 1;
        const tcs = toolCallsByIndex.get(i) || [];
        return (
          <Fragment key={`frag-${i}`}>
            {renderBlock(block, i, isLast)}
            {tcs.map((tc) => (
              <ToolSection
                key={tc.id}
                toolCall={tc}
                isStreaming={isStreaming}
              />
            ))}
          </Fragment>
        );
      })}
      {/* 没有 block 但有 tool_call:把兜底 tool_call 放在最前 */}
      {blocks.length === 0 &&
        toolCallsToRender.map((tc) => (
          <ToolSection
            key={tc.id}
            toolCall={tc}
            isStreaming={isStreaming}
          />
        ))}
      {/* block 之后兜底放(insertAfterBlockIndex >= blocks.length 的) */}
      {(toolCallsByIndex.get(blocks.length) || []).map((tc) => (
        <ToolSection key={tc.id} toolCall={tc} isStreaming={isStreaming} />
      ))}
    </div>
  );
}

/**
 * 消息气泡组件
 */
export const MessageBubble = memo(function MessageBubble({
  message,
  isStreaming = false,
  questionId,
  isContinuation = false,
  continuesToNext = false,
  liveToolCalls,
  childResponses,
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
      className={[
        'message-row',
        isUser ? 'is-user' : 'is-assistant',
        isContinuation ? 'is-continuation' : '',
        continuesToNext ? 'has-continuation' : '',
      ].filter(Boolean).join(' ')}
      id={isUser ? `msg-${message.id}` : undefined}
      style={{
        display: 'flex',
        justifyContent: isUser ? 'flex-end' : 'flex-start',
        padding: isUser
          ? 'var(--space-2) var(--space-4)'
          : `${isContinuation ? 'var(--space-1)' : 'var(--space-2)'} var(--space-4) ${
              continuesToNext ? '0' : 'var(--space-2)'
            }`,
      }}
    >
      <div
        className="message-bubble"
        style={
          isUser
            ? {
                maxWidth: '80%',
                padding: 'var(--space-2) var(--space-3)',
                borderRadius: 'var(--radius-md) var(--radius-md) var(--radius-md) var(--radius-sm)',
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
                maxWidth: '100%',
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
          <span style={{ whiteSpace: 'pre-wrap' }}>{message.content}</span>
        ) : (
          <>
            <AssistantContent
              message={message}
              isStreaming={isStreaming}
              liveToolCalls={liveToolCalls}
            />
            {/* 嵌套渲染:用户对模型问题的回应,以小气泡形式挂在父消息内 */}
            {childResponses && childResponses.length > 0 && (
              <div data-testid="child-responses">
                {childResponses.map((r) => (
                  <ChildResponseBubble key={r.id} response={r} />
                ))}
              </div>
            )}
            {!isStreaming && questionId && message.content && (
              <div
                className="message-actions"
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: 'var(--space-2)',
                  marginTop: 'var(--space-2)',
                }}
              >
                <span
                  className="message-time"
                  title={new Date(message.created_at * 1000).toLocaleString()}
                  style={{
                    fontSize: '12px',
                    fontVariantNumeric: 'tabular-nums',
                    userSelect: 'none',
                  }}
                >
                  {formatMessageTime(message.created_at)}
                </span>
                <button
                  className={`message-action-button ${copied ? 'is-copied' : ''}`}
                  onClick={handleCopy}
                  title={copied ? '已复制' : '复制回答'}
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: '4px',
                    padding: '2px 8px',
                    border: 'none',
                    borderRadius: 'var(--radius-sm)',
                    fontSize: '12px',
                    cursor: 'pointer',
                    transition: 'color 0.15s, background 0.15s',
                  }}
                >
                  {copied ? <Check size={13} /> : <Copy size={13} />}
                </button>
                <button
                  className="message-action-button"
                  onClick={handleBackToQuestion}
                  title="回到问题"
                  aria-label="回到问题"
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: '4px',
                    padding: '2px 8px',
                    border: 'none',
                    borderRadius: 'var(--radius-sm)',
                    fontSize: '12px',
                    cursor: 'pointer',
                    transition: 'color 0.15s, background 0.15s',
                  }}
                >
                  <ArrowUp size={13} />
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
    prevProps.message.tool_calls === nextProps.message.tool_calls &&
    prevProps.isStreaming === nextProps.isStreaming &&
    prevProps.questionId === nextProps.questionId &&
    prevProps.isContinuation === nextProps.isContinuation &&
    prevProps.continuesToNext === nextProps.continuesToNext &&
    prevProps.liveToolCalls === nextProps.liveToolCalls &&
    prevProps.childResponses === nextProps.childResponses
  );
});
