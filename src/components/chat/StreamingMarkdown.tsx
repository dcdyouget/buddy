import { memo, useMemo } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { CodeBlock } from './CodeBlock';

/**
 * 共享的 markdown 组件映射，避免每次渲染都重新创建
 */
const COMPONENTS = {
  code({ className, children, ...props }: any) {
    const match = /language-(\w+)/.exec(className || '');
    const codeStr = String(children).replace(/\n$/, '');
    if (match) {
      return <CodeBlock language={match[1]} source={codeStr} />;
    }
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
  p({ children }: any) {
    return <p style={{ margin: '0 0 var(--space-2) 0' }}>{children}</p>;
  },
};

/**
 * 稳定 Markdown 渲染器（memo 保护）
 *
 * 仅当 content 字符串实际变化时才重新解析渲染。
 * 这确保已完成的段落不会随 token 到来而重新做 AST 解析。
 */
const StableMarkdown = memo(
  ({ content }: { content: string }) => (
    <ReactMarkdown remarkPlugins={[remarkGfm]} components={COMPONENTS}>
      {content}
    </ReactMarkdown>
  ),
  (prev, next) => prev.content === next.content,
);

interface StreamingMarkdownProps {
  content: string;
  isStreaming: boolean;
}

/**
 * 流式 Markdown 增量渲染组件
 *
 * 策略：将内容按最后一个 \n\n 切分为「稳定」与「不稳定」两个区域。
 *
 * ┌─────────────────────────────┐
 * │ 完整段落 A                   │  ← 稳定部分：已写完，通过 StableMarkdown
 * │                             │     渲染一次后 memo，不再重解析
 * │\n\n                        │
 * │ 完整段落 B                   │
 * │                             │
 * │\n\n                        │
 * │ 正在输入的当前段落...         │  ← 不稳定部分：跟随 token 更新，
 * └─────────────────────────────┘     每次只解析少量文字，代价极小
 *
 * 特殊处理：代码围栏（```...```）内部可能包含 \n\n，
 * 通过计数围栏符号的奇偶性判断是否处于未闭合的围栏内部，
 * 若是则回退到围栏起始位置作为稳定边界。
 *
 * 效果：500 token 的回复，稳定部分约在每 ~50-100 token（段落边界）
 * 处更新一次，不稳定部分始终保持很短。相比每个 token 全量解析，
 * 复杂度从 O(n²) 降低到 O(n)。
 */
export function StreamingMarkdown({ content, isStreaming }: StreamingMarkdownProps) {
  // 将内容切分为稳定块和不稳定尾部
  const { stablePart, unstablePart } = useMemo(() => {
    // 未在流式输出中，或内容为空 → 全部当作稳定内容
    if (!isStreaming || !content) {
      return { stablePart: content, unstablePart: '' };
    }

    // 查找最后一个段落分隔符 \n\n
    const lastDoubleNewline = content.lastIndexOf('\n\n');
    if (lastDoubleNewline === -1) {
      // 没有段落分隔 → 全部内容都还不稳定
      return { stablePart: '', unstablePart: content };
    }

    // 检查是否在代码围栏内部（围栏中的空行不应作为段落边界）
    const stableCandidate = content.substring(0, lastDoubleNewline + 2);
    const fenceCount = (stableCandidate.match(/```/g) || []).length;
    if (fenceCount % 2 !== 0) {
      // 处于未闭合的代码围栏中 → 回退到围栏开始位置
      const openingFence = stableCandidate.lastIndexOf('```');
      if (openingFence > 0) {
        return {
          stablePart: content.substring(0, openingFence),
          unstablePart: content.substring(openingFence),
        };
      }
      // 开围栏在开头 → 全部不稳定
      return { stablePart: '', unstablePart: content };
    }

    return {
      stablePart: stableCandidate,
      unstablePart: content.substring(lastDoubleNewline + 2),
    };
  }, [content, isStreaming]);

  return (
    <div className="ai-message-content">
      {/* 稳定部分：已写完整的段落，memo 后不会随 token 重解析 */}
      {stablePart && <StableMarkdown content={stablePart} />}

      {/* 不稳定部分：当前正在写的段落/标题/列表，跟随 token 更新 */}
      {unstablePart && (
        <ReactMarkdown remarkPlugins={[remarkGfm]} components={COMPONENTS}>
          {unstablePart}
        </ReactMarkdown>
      )}

      {/* 流式输出中的闪烁光标已移至 MessageBubble 层统一控制 */}
    </div>
  );
}
