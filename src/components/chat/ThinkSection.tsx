import { memo, useCallback, useEffect, useState } from 'react';
import { Brain, ChevronDown, ChevronRight } from 'lucide-react';
import { StreamingMarkdown } from './StreamingMarkdown';

/**
 * ThinkSection — 可折叠的思考区块
 *
 * 视觉特征：
 * - 左侧蓝色边框,与常规文本明显区分
 * - 流式折叠时展示最新一段思考，完成后展示第一行
 * - 流式写入时默认折叠 + 轻量呼吸/流光动效，可由用户手动展开
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

/** 提取最新思考内容，压成单行并保留末尾 96 个 Unicode 字符。 */
function latestContentPreview(content: string): string {
  const normalized = content.replace(/\s+/g, ' ').trim();
  const characters = Array.from(normalized);
  if (characters.length <= 96) return normalized;
  return `…${characters.slice(-96).join('')}`;
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

  const preview = isStreaming
    ? latestContentPreview(content)
    : firstLinePreview(content);

  return (
    <div
      className={`think-section ${isStreaming ? 'is-streaming' : ''} ${
        expanded ? 'is-expanded' : ''
      }`}
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
        borderTop: '1px solid var(--border-default)',
        borderRight: '1px solid var(--border-default)',
        borderBottom: '1px solid var(--border-default)',
        borderLeft: '2px solid var(--tool-ui-accent)',
        borderRadius: 'var(--radius-md)',
        borderTopLeftRadius: 0,
        borderBottomLeftRadius: 0,
        overflow: 'hidden',
        margin: 0,
        cursor: 'pointer',
        width: '100%',
        boxSizing: 'border-box',
        background: 'var(--panel-surface)',
        position: 'relative',
      }}
    >
      {/* ── Header ── */}
      <div
        className="think-section-header"
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
        <Brain
          className="think-section-icon"
          size={14}
          style={{ color: 'var(--tool-ui-accent)', flexShrink: 0 }}
        />
        <span
          className="think-section-label"
          style={{
            fontWeight: 600,
            fontSize: 'var(--font-size-sm)',
            color: 'var(--text-muted)',
          }}
        >
          {isStreaming ? '正在思考' : '思考过程'}
        </span>
        {isStreaming && (
          <span className="think-section-loader" aria-label="思考中">
            <span />
            <span />
            <span />
          </span>
        )}
        {/* 流式时显示最新思考，完成后显示第一行预览 */}
        {!expanded && preview && (
          <span
            className="think-section-preview"
            style={{
              flex: 1,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
              fontSize: 'var(--font-size-xs)',
              color: 'var(--text-muted)',
              marginLeft: 4,
            }}
          >
            {preview}
          </span>
        )}
        <span style={{ flex: expanded ? 1 : 0 }} />
        {expanded ? (
          <ChevronDown size={13} style={{ color: 'var(--text-muted)', flexShrink: 0 }} />
        ) : (
          <ChevronRight size={13} style={{ color: 'var(--text-muted)', flexShrink: 0 }} />
        )}
      </div>

      {/* ── 展开内容 ── */}
      {expanded && (
        <div
          className="think-section-content"
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
            background: 'transparent',
          }}
        >
          {content ? (
            <StreamingMarkdown content={content} isStreaming={isStreaming} />
          ) : (
            <span style={{ color: 'var(--text-muted)', fontStyle: 'italic' }}>
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
