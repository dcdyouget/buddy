import { memo, useCallback, useEffect, useState } from 'react';
import { Brain, ChevronDown, ChevronRight } from 'lucide-react';
import { StreamingMarkdown } from './StreamingMarkdown';

/**
 * ThinkSection — 可折叠的思考区块
 *
 * 视觉特征：
 * - 左侧紫/amber 渐变边框,与常规文本明显区分
 * - 折叠时展示第一行思考内容作为预览
 * - 流式写入时自动展开 + pulsing dot
 * - 完成后默认折叠,click 展开
 */

interface ThinkSectionProps {
  content: string;
  isStreaming: boolean;
  defaultExpanded: boolean;
}

/** 提取 content 第一行(最多 80 字)作为折叠态预览 */
function firstLinePreview(content: string): string {
  const line = content.split('\n')[0] || '';
  return line.length > 80 ? line.slice(0, 80) + '…' : line;
}

export const ThinkSection = memo(function ThinkSection({
  content,
  isStreaming,
  defaultExpanded,
}: ThinkSectionProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  const [userToggled, setUserToggled] = useState(false);

  useEffect(() => {
    if (defaultExpanded) {
      setExpanded(true);
      setUserToggled(false);
    } else if (!userToggled) {
      setExpanded(false);
    }
  }, [defaultExpanded, userToggled]);

  const toggle = useCallback(() => {
    setUserToggled(true);
    setExpanded((prev) => !prev);
  }, []);

  const preview = firstLinePreview(content);

  return (
    <div
      onClick={toggle}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          toggle();
        }
      }}
      style={{
        border: '1px solid var(--border-subtle)',
        borderLeft: '3px solid #b8a0d0', // 淡紫色左边框 — 思考标识
        borderRadius: 'var(--radius-md)',
        borderTopLeftRadius: 0,
        borderBottomLeftRadius: 0,
        overflow: 'hidden',
        margin: 'var(--space-2) 0',
        cursor: 'pointer',
        width: '100%',
        boxSizing: 'border-box',
        background: 'var(--bg-sunken)',
      }}
    >
      {/* ── Header ── */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 'var(--space-2)',
          width: '100%',
          padding: 'var(--space-1) var(--space-2)',
          background: 'transparent',
          border: 'none',
          fontSize: 'var(--font-size-sm)',
          color: 'var(--text-muted)',
          boxSizing: 'border-box',
          minHeight: 28,
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.background = 'var(--bg-sunken)';
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.background = 'transparent';
        }}
      >
        <Brain size={14} style={{ color: '#9b7ec4', flexShrink: 0 }} />
        <span
          style={{
            fontWeight: 600,
            fontSize: 'var(--font-size-sm)',
            color: 'var(--text-muted)',
          }}
        >
          {isStreaming ? '正在思考' : '思考过程'}
        </span>
        {isStreaming && (
          <span
            style={{
              width: 5,
              height: 5,
              borderRadius: 'var(--radius-full)',
              background: '#9b7ec4',
              opacity: 0.7,
              flexShrink: 0,
            }}
          />
        )}
        {/* 折叠时显示第一行预览 */}
        {!expanded && preview && (
          <span
            style={{
              flex: 1,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
              fontSize: 'var(--font-size-xs)',
              color: 'var(--text-tertiary)',
              marginLeft: 4,
            }}
          >
            {preview}
          </span>
        )}
        <span style={{ flex: expanded ? 1 : 0 }} />
        {expanded ? (
          <ChevronDown size={13} style={{ color: 'var(--text-tertiary)', flexShrink: 0 }} />
        ) : (
          <ChevronRight size={13} style={{ color: 'var(--text-tertiary)', flexShrink: 0 }} />
        )}
      </div>

      {/* ── 展开内容 ── */}
      {expanded && (
        <div
          style={{
            padding: '0 var(--space-2) var(--space-2) var(--space-2)',
            borderTop: '1px solid var(--border-subtle)',
            fontSize: 'var(--font-size-base)',
            color: 'var(--text-muted)',
            width: '100%',
            boxSizing: 'border-box',
            overflowWrap: 'break-word',
            wordBreak: 'break-word',
            overflow: 'hidden',
            background: 'var(--bg-sunken)',
          }}
        >
          {content ? (
            <StreamingMarkdown content={content} isStreaming={isStreaming} />
          ) : (
            <span style={{ color: 'var(--text-tertiary)', fontStyle: 'italic' }}>
              等待思考内容...
            </span>
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
