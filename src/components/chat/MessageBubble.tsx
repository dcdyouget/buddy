import { Fragment, memo, useEffect, useRef } from 'react';
import type { Message, ContentBlock, ToolCall } from '@/types';
import { parseThinkBlocks } from '@/utils/thinkParser';
import { MessageActions } from './MessageActions';
import { StreamingMarkdown } from './StreamingMarkdown';
import { ThinkSection } from './ThinkSection';
import { ToolSection } from './ToolSection';
import { AttachmentImage } from './AttachmentImage';

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
  streamingRevealCount?: number;
  streamingRevealRevision?: number;
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
  streamingRevealCount,
  streamingRevealRevision,
}: {
  message: Message;
  isStreaming: boolean;
  liveToolCalls?: ToolCall[];
  streamingRevealCount: number;
  streamingRevealRevision: number;
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

  // 把 tool_calls 按 insertAfterBlockIndex 分桶：
  // - -1：第一个 block 之前
  // - 0..n-1：对应 block 之后
  // - n：全部 block 之后
  // - 有 insertAfterBlockIndex 的: 按索引插入
  // - 没有的(旧消息/兜底): 全部放在最后一个 block 之后
  const toolCallsByIndex = new Map<number, ToolCall[]>();
  const hasAnyInsertIndex = toolCallsToRender.some(
    (tc) => typeof tc.insertAfterBlockIndex === 'number',
  );
  const fallbackIndex = blocks.length > 0 ? blocks.length - 1 : -1;
  for (const tc of toolCallsToRender) {
    const requestedIndex =
      typeof tc.insertAfterBlockIndex === 'number'
        ? tc.insertAfterBlockIndex
        : hasAnyInsertIndex
        ? blocks.length // 跳过 block,放到末尾
        : fallbackIndex;
    const idx = Math.max(-1, Math.min(requestedIndex, blocks.length));
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
          defaultExpanded={false}
        />
      );
    }
    return (
      <StreamingMarkdown
        key={`block-text-${i}`}
        content={block.content}
        isStreaming={isLast && isStreaming}
        revealCount={isLast ? streamingRevealCount : 0}
        revealKey={streamingRevealRevision}
      />
    );
  };

  // ── 主体:遍历 blocks,每个 block 之后插入属于该索引的 tool_calls ──
  return (
    <div className="assistant-content-flow">
      {/* 工具在任何内容出现前被调用时，固定渲染在第一个 block 前。 */}
      {(toolCallsByIndex.get(-1) || []).map((tc) => (
        <ToolSection key={tc.id} toolCall={tc} isStreaming={isStreaming} />
      ))}
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
      {/* 所有 block 之后的兜底工具调用。 */}
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
  streamingRevealCount = 0,
  streamingRevealRevision = 0,
}: MessageBubbleProps) {
  const isUser = message.role === 'user';
  const hasStreamedRef = useRef(isStreaming);
  useEffect(() => {
    if (isStreaming) hasStreamedRef.current = true;
  }, [isStreaming]);

  const hasAnswerText = Boolean(
    message.content.trim() ||
    message.blocks?.some(
      (block) => block.type === 'text' && block.content.trim(),
    ),
  );
  const showMessageMeta =
    !isStreaming &&
    !continuesToNext &&
    hasAnswerText;

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
          <div
            style={{
              display: 'flex',
              flexDirection: 'column',
              gap: 'var(--space-2)',
            }}
          >
            {message.images && message.images.length > 0 && (
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns:
                    message.images.length === 1
                      ? 'minmax(0, 1fr)'
                      : 'repeat(2, minmax(0, 1fr))',
                  gap: 'var(--space-1)',
                }}
              >
                {message.images.map((image) => (
                  <AttachmentImage
                    key={image.id}
                    image={image}
                    alt={image.name}
                    style={{
                      width: '100%',
                      maxWidth: 260,
                      maxHeight: 240,
                      objectFit: 'contain',
                      borderRadius: 'var(--radius-sm)',
                      background: 'var(--bg-sunken)',
                    }}
                  />
                ))}
              </div>
            )}
            {message.content && (
              <span style={{ whiteSpace: 'pre-wrap' }}>{message.content}</span>
            )}
          </div>
        ) : (
          <>
            <AssistantContent
              message={message}
              isStreaming={isStreaming}
              liveToolCalls={liveToolCalls}
              streamingRevealCount={streamingRevealCount}
              streamingRevealRevision={streamingRevealRevision}
            />
            {showMessageMeta && (
              <MessageActions
                message={message}
                questionId={questionId}
                animateIn={hasStreamedRef.current}
              />
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
    prevProps.message.images === nextProps.message.images &&
    prevProps.message.blocks === nextProps.message.blocks &&
    prevProps.message.tool_calls === nextProps.message.tool_calls &&
    prevProps.isStreaming === nextProps.isStreaming &&
    prevProps.questionId === nextProps.questionId &&
    prevProps.isContinuation === nextProps.isContinuation &&
    prevProps.continuesToNext === nextProps.continuesToNext &&
    prevProps.liveToolCalls === nextProps.liveToolCalls &&
    prevProps.streamingRevealCount === nextProps.streamingRevealCount &&
    prevProps.streamingRevealRevision === nextProps.streamingRevealRevision
  );
});
