import { memo, useCallback, useEffect, useState } from 'react';
import {
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  CircleDashed,
  FilePlus2,
  FileText,
  FilePenLine,
  FileOutput,
  HelpCircle,
  Loader2,
  Wrench,
  XCircle,
} from 'lucide-react';
import type { ToolCall, ToolCallStatus } from '@/types';
import { CodeBlock } from './CodeBlock';

interface ToolSectionProps {
  toolCall: ToolCall;
  isStreaming: boolean;
}

// ── 状态图标 + 文字 ──

function getStatusMeta(status: ToolCallStatus | undefined): {
  label: string;
  Icon: typeof Loader2;
  color: string;
  spin: boolean;
} {
  switch (status) {
    case 'executing':
      return { label: '执行中', Icon: Loader2, color: 'var(--state-info)', spin: true };
    case 'done':
      return { label: '已完成', Icon: CheckCircle2, color: 'var(--state-success)', spin: false };
    case 'error':
      return { label: '失败',   Icon: XCircle,     color: 'var(--state-error)', spin: false };
    case 'calling':
    default:
      return { label: '准备中', Icon: CircleDashed, color: 'var(--text-muted)', spin: false };
  }
}

// ── 工具名 → 图标 + 左边框色 ──

function getToolIcon(name: string): { Icon: typeof FileText; borderColor: string } {
  switch (name) {
    case 'read_file':
      return { Icon: FileText, borderColor: 'var(--state-info)' };
    case 'create_file':
      return { Icon: FilePlus2, borderColor: 'var(--state-success)' };
    case 'overwrite_file':
      return { Icon: FilePenLine, borderColor: 'var(--state-warning)' };
    case 'append_file':
      return { Icon: FileOutput, borderColor: 'var(--state-info)' };
    case 'ask_user':
      return { Icon: HelpCircle, borderColor: 'var(--buddy-primary)' };
    default:
      return { Icon: Wrench, borderColor: 'var(--text-muted)' };
  }
}

// ── 工具调用摘要(折叠时显示) ──

function actionSummary(tc: ToolCall): string {
  try {
    const args = JSON.parse(tc.arguments);
    if (args.path) return args.path as string;
    if (args.question) return (args.question as string).slice(0, 50);
    return '';
  } catch {
    return '';
  }
}

// ── JSON pretty print ──

function prettyArgs(raw: string): string {
  if (!raw) return '(空参数)';
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
}

export const ToolSection = memo(function ToolSection({
  toolCall,
  isStreaming,
}: ToolSectionProps) {
  const { label, Icon, color, spin } = getStatusMeta(toolCall.status);
  const { Icon: ToolIcon, borderColor } = getToolIcon(toolCall.name);

  const initialExpanded =
    isStreaming && (toolCall.status === 'calling' || toolCall.status === 'executing');
  const [expanded, setExpanded] = useState(initialExpanded);
  const [userToggled, setUserToggled] = useState(false);

  useEffect(() => {
    if (initialExpanded) {
      setExpanded(true);
      setUserToggled(false);
    } else if (!userToggled) {
      setExpanded(false);
    }
  }, [initialExpanded, userToggled]);

  const toggle = useCallback(() => {
    setUserToggled(true);
    setExpanded((prev) => !prev);
  }, []);

  const argsText = prettyArgs(toolCall.arguments);
  const hasResult = toolCall.status === 'done' || toolCall.status === 'error';
  const summary = actionSummary(toolCall);
  const isAskUser = toolCall.name === 'ask_user';

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
        borderTop: '1px solid var(--border-default)',
        borderRight: '1px solid var(--border-default)',
        borderBottom: '1px solid var(--border-default)',
        borderLeft: `3px solid ${borderColor}`,
        borderRadius: 'var(--radius-md)',
        borderTopLeftRadius: 0,
        borderBottomLeftRadius: 0,
        overflow: 'hidden',
        margin: 0,
        cursor: 'pointer',
        width: '100%',
        boxSizing: 'border-box',
        background: 'var(--panel-surface)',
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
        <ToolIcon size={14} style={{ color: borderColor, flexShrink: 0 }} />
        <span
          style={{
            fontWeight: 600,
            color: 'var(--text-primary)',
            fontFamily: 'var(--font-mono)',
            fontSize: 'var(--font-size-xs)',
          }}
        >
          {toolCall.name}
        </span>
        {/* 折叠时显示摘要 */}
        {!expanded && summary && (
          <span
            style={{
              flex: 1,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
              fontSize: 'var(--font-size-xs)',
              color: 'var(--text-muted)',
              fontFamily: 'var(--font-mono)',
            }}
          >
            → {summary}
          </span>
        )}
        {/* 状态徽标 */}
        <span
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: 3,
            padding: '1px 6px',
            borderRadius: 'var(--radius-full)',
            background: `color-mix(in srgb, ${color} 10%, transparent)`,
            color,
            fontSize: 10,
            fontWeight: 600,
            lineHeight: 1.4,
            flexShrink: 0,
          }}
        >
          <Icon size={10} className={spin ? 'buddy-spin' : undefined} />
          {label}
        </span>
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
          style={{
            padding: '0 var(--space-2) var(--space-2) var(--space-2)',
            borderTop: '1px solid var(--border-subtle)',
            display: 'flex',
            flexDirection: 'column',
            gap: 'var(--space-2)',
            width: '100%',
            boxSizing: 'border-box',
            background: 'transparent',
          }}
        >
          {/* ── ask_user 特殊渲染 ── */}
          {isAskUser && (
            <AskUserCard toolCall={toolCall} hasResult={hasResult} />
          )}

          {/* ── 普通 tool 参数 ── */}
          {!isAskUser && (
            <div>
              <div style={{
                fontSize: 10, color: 'var(--text-tertiary)', margin: 'var(--space-2) 0 4px',
                textTransform: 'uppercase', letterSpacing: '0.04em', fontWeight: 700,
              }}>
                参数
              </div>
              <CodeBlock language="json" source={argsText} />
            </div>
          )}

          {/* ── 结果 ── */}
          {hasResult && toolCall.result && (
            <div>
              <div style={{
                fontSize: 10,
                color: toolCall.is_error_result ? 'var(--state-error)' : 'var(--text-tertiary)',
                margin: 'var(--space-1) 0 4px',
                textTransform: 'uppercase',
                letterSpacing: '0.04em',
                fontWeight: 700,
              }}>
                {toolCall.is_error_result ? '错误' : '结果'}
              </div>
              <CodeBlock language="text" source={toolCall.result} />
            </div>
          )}
        </div>
      )}
    </div>
  );
},
(prevProps, nextProps) => {
  return (
    prevProps.toolCall === nextProps.toolCall &&
    prevProps.isStreaming === nextProps.isStreaming
  );
});

// ── ask_user 卡片(展开时显示) ──

function AskUserCard({ toolCall, hasResult }: { toolCall: ToolCall; hasResult: boolean }) {
  let parsed: {
    question?: string;
    header?: string;
    options?: { label: string; description?: string; requires_input?: boolean; input_placeholder?: string }[];
    multi_select?: boolean;
  } = {};
  try { parsed = JSON.parse(toolCall.arguments); } catch { /* */ }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--space-2)' }}>
      {/* header chip */}
      {parsed.header && (
        <div style={{
          display: 'inline-flex', alignItems: 'center', gap: 5, alignSelf: 'flex-start',
          marginTop: 'var(--space-2)',
          padding: '3px 10px', borderRadius: 'var(--radius-full)',
          background: 'var(--primary-tint-soft)', color: 'var(--buddy-primary)', fontSize: 11, fontWeight: 700,
        }}>
          <HelpCircle size={12} />
          {parsed.header}
          {parsed.multi_select && <span style={{ color: 'var(--text-tertiary)', fontWeight: 400 }}>· 多选</span>}
        </div>
      )}
      {/* question */}
      {parsed.question && (
        <div style={{ fontSize: 'var(--font-size-base)', color: 'var(--text-primary)', fontWeight: 500, lineHeight: 1.5 }}>
          {parsed.question}
        </div>
      )}
      {/* options — 横向排列,与 QuestionModal 保持一致 */}
      {parsed.options && parsed.options.length > 0 && (
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
          {parsed.options.map((opt, i) => (
            <span key={i} style={{
              display: 'inline-flex', alignItems: 'center', gap: 5,
              padding: '4px 10px', border: '1px solid var(--border-subtle)',
              borderRadius: 'var(--radius-md)', background: 'var(--bg-surface)',
              fontSize: 'var(--font-size-xs)', fontWeight: 500,
              color: 'var(--text-primary)', whiteSpace: 'nowrap',
            }}>
              {opt.label}
              {opt.requires_input && (
                <span style={{ fontSize: 9, padding: '0px 4px', borderRadius: 'var(--radius-full)', background: 'var(--bg-sunken)', color: 'var(--text-tertiary)', fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.03em' }}>
                  input
                </span>
              )}
              {opt.description && (
                <span style={{ color: 'var(--text-muted)', fontWeight: 400 }} title={opt.description}>
                  — {opt.description.length > 30 ? opt.description.slice(0, 30) + '…' : opt.description}
                </span>
              )}
            </span>
          ))}
        </div>
      )}
      {/* answer */}
      {hasResult && toolCall.result && (
        <div style={{
          fontSize: 'var(--font-size-sm)', color: 'var(--text-muted)',
          padding: '6px 10px', background: 'var(--bg-surface)', borderRadius: 'var(--radius-sm)', whiteSpace: 'pre-wrap',
        }}>
          <span style={{ color: 'var(--text-tertiary)' }}>用户回应: </span>
          <span style={{ color: 'var(--text-primary)', fontWeight: 500 }}>{toolCall.result}</span>
        </div>
      )}
    </div>
  );
}
