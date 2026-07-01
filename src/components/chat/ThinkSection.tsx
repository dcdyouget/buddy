import { memo, useCallback, useEffect, useState } from 'react';
import { Brain, ChevronDown, ChevronRight } from 'lucide-react';
import { StreamingMarkdown } from './StreamingMarkdown';

/**
 * ThinkSection 组件的 Props
 * @param content - think 标签内部的原始内容（markdown 格式）
 * @param isStreaming - 此 think 块是否正在流式写入中
 * @param defaultExpanded - 初始展开状态（流式中 think 未闭合时为 true）
 */
interface ThinkSectionProps {
  content: string;
  isStreaming: boolean;
  defaultExpanded: boolean;
}

/**
 * 思考区块组件
 *
 * 渲染一个可折叠的 "思考中..." 区块：
 * - 折叠态：Brain 图标 + "思考中..." + ChevronRight（可点击展开）
 * - 展开态：同上但 ChevronDown，下方渲染 markdown 内容
 * - 流式写入时自动展开，完成后默认折叠
 * - 使用 React.memo 防止无关 token 更新导致重渲染
 */
export const ThinkSection = memo(function ThinkSection({
  content,
  isStreaming,
  defaultExpanded,
}: ThinkSectionProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  // 跟踪用户是否手动操作过（用于决定 think 闭合后是否自动折叠）
  const [userToggled, setUserToggled] = useState(false);

  // defaultExpanded 变化时的自动展开/折叠逻辑
  useEffect(() => {
    if (defaultExpanded) {
      // think 开始流入（<think> 到达，</think> 未到）→ 自动展开
      setExpanded(true);
      setUserToggled(false); // 新的 think 块重置手动标记
    } else if (!userToggled) {
      // think 闭合（</think> 到达）且用户未手动操作 → 自动折叠
      setExpanded(false);
    }
  }, [defaultExpanded, userToggled]);

  const toggle = useCallback(() => {
    setUserToggled(true);
    setExpanded((prev) => !prev);
  }, []);

  return (
    <div
      style={{
        border: '1px solid var(--border-subtle)',
        borderRadius: 'var(--radius-md)',
        overflow: 'hidden',
        margin: 'var(--space-2) 0',
      }}
    >
      {/* 可点击的折叠 header */}
      <button
        onClick={toggle}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 'var(--space-2)',
          width: '100%',
          padding: 'var(--space-2) var(--space-2)',
          background: 'transparent',
          border: 'none',
          cursor: 'pointer',
          fontSize: '12px',
          color: 'var(--text-muted)',
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.background = 'var(--bg-sunken)';
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.background = 'transparent';
        }}
      >
        <Brain size={14} style={{ color: 'var(--text-muted)', flexShrink: 0 }} />
        <span style={{ fontWeight: 500 }}>思考中...</span>
        {/* 流式指示灯：一个小的 pulsing dot */}
        {isStreaming && (
          <span
            style={{
              width: 6,
              height: 6,
              borderRadius: '50%',
              background: 'var(--buddy-primary)',
              opacity: 0.6,
            }}
          />
        )}
        <span style={{ flex: 1 }} />
        {expanded ? (
          <ChevronDown size={14} style={{ color: 'var(--text-tertiary)', flexShrink: 0 }} />
        ) : (
          <ChevronRight size={14} style={{ color: 'var(--text-tertiary)', flexShrink: 0 }} />
        )}
      </button>

      {/* 展开时渲染思考内容 */}
      {expanded && (
        <div
          style={{
            padding: '0 var(--space-2) var(--space-2) var(--space-2)',
            borderTop: '1px solid var(--border-subtle)',
            fontSize: '13px',
            color: 'var(--text-muted)',
          }}
        >
          {content ? (
            <StreamingMarkdown content={content} isStreaming={isStreaming} />
          ) : (
            <span style={{ color: 'var(--text-tertiary)', fontStyle: 'italic' }}>等待思考内容...</span>
          )}
        </div>
      )}
    </div>
  );
},
(prevProps, nextProps) => {
  return (
    prevProps.content === nextProps.content &&
    prevProps.isStreaming === nextProps.isStreaming &&
    prevProps.defaultExpanded === nextProps.defaultExpanded
  );
});
