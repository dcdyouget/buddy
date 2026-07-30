import {
  memo,
  useCallback,
  useEffect,
  useState,
} from 'react';
import { AnimatePresence, motion, useReducedMotion } from 'framer-motion';
import {
  Braces,
  CheckCircle2,
  ChevronDown,
  CircleDashed,
  FileCheck2,
  FileDiff,
  FileOutput,
  FilePenLine,
  FilePlus2,
  FileText,
  FolderTree,
  HelpCircle,
  Loader2,
  Search,
  Wrench,
  XCircle,
} from 'lucide-react';
import type { ToolCall, ToolCallStatus } from '@/types';
import { useChatStore } from '@/stores/chatStore';
import { CodeBlock } from './CodeBlock';
import { AskUserCard } from './AskUserCard';
import { WebSearchSection } from './WebSearchSection';

interface ToolSectionProps {
  toolCall: ToolCall;
  isStreaming: boolean;
}

interface StatusMeta {
  label: string;
  Icon: typeof Loader2;
  tone: 'neutral' | 'info' | 'success' | 'error';
  spin: boolean;
}

interface ToolMeta {
  label: string;
  Icon: typeof FileText;
}

function getStatusMeta(status: ToolCallStatus | undefined): StatusMeta {
  switch (status) {
    case 'executing':
      return { label: '执行中', Icon: Loader2, tone: 'info', spin: true };
    case 'done':
      return {
        label: '已完成',
        Icon: CheckCircle2,
        tone: 'success',
        spin: false,
      };
    case 'error':
      return { label: '失败', Icon: XCircle, tone: 'error', spin: false };
    case 'interrupted':
      return {
        label: '已中断',
        Icon: XCircle,
        tone: 'neutral',
        spin: false,
      };
    case 'calling':
    default:
      return {
        label: '准备中',
        Icon: CircleDashed,
        tone: 'neutral',
        spin: false,
      };
  }
}

function getToolMeta(name: string): ToolMeta {
  switch (name) {
    case 'read_file':
      return {
        label: '读取文件',
        Icon: FileText,
      };
    case 'list_directory':
      return {
        label: '浏览目录',
        Icon: FolderTree,
      };
    case 'search_files':
      return {
        label: '搜索文件',
        Icon: Search,
      };
    case 'create_file':
      return {
        label: '创建文件',
        Icon: FilePlus2,
      };
    case 'overwrite_file':
      return {
        label: '覆盖文件',
        Icon: FilePenLine,
      };
    case 'append_file':
      return {
        label: '追加文件',
        Icon: FileOutput,
      };
    case 'edit_file':
      return {
        label: '编辑文件',
        Icon: FileDiff,
      };
    case 'ask_user':
      return {
        label: '询问用户',
        Icon: HelpCircle,
      };
    default:
      return {
        label: '调用工具',
        Icon: Wrench,
      };
  }
}

function actionSummary(toolCall: ToolCall): string {
  try {
    const args = JSON.parse(toolCall.arguments);
    if (toolCall.name === 'ask_user') {
      return args.question || '等待用户回答';
    }

    const summaryKeys = ['path', 'command', 'query', 'url', 'name'];
    for (const key of summaryKeys) {
      if (typeof args[key] === 'string' && args[key].trim()) {
        return args[key];
      }
    }
    return '';
  } catch {
    return '';
  }
}

function prettyArgs(raw: string): string {
  if (!raw) return '(空参数)';
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
}

interface DetailBlockProps {
  label: string;
  Icon: typeof Braces;
  language: string;
  source: string;
  isError?: boolean;
}

function DetailBlock({
  label,
  Icon,
  language,
  source,
  isError = false,
}: DetailBlockProps) {
  return (
    <section className={`tool-detail-block ${isError ? 'is-error' : ''}`}>
      <div className="tool-detail-label">
        <Icon size={12} aria-hidden="true" />
        <span>{label}</span>
      </div>
      <CodeBlock language={language} source={source} />
    </section>
  );
}

export const ToolSection = memo(
  function ToolSection({ toolCall, isStreaming }: ToolSectionProps) {
    const shouldReduceMotion = useReducedMotion();
    const isAwaitingAnswer = useChatStore(
      (state) => state.pendingQuestion?.id === toolCall.id,
    );
    const statusMeta: StatusMeta = isAwaitingAnswer
      ? {
          label: '等待回答',
          Icon: HelpCircle,
          tone: 'info',
          spin: false,
        }
      : getStatusMeta(toolCall.status);
    const toolMeta = getToolMeta(toolCall.name);
    const initialExpanded =
      isAwaitingAnswer ||
      (isStreaming &&
        (toolCall.status === 'calling' || toolCall.status === 'executing'));
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
      setExpanded((previous) => !previous);
    }, []);

    const hasResult =
      toolCall.status === 'done' || toolCall.status === 'error';
    const isInterrupted = toolCall.status === 'interrupted';
    const summary = actionSummary(toolCall);
    const isAskUser = toolCall.name === 'ask_user';
    const StatusIcon = statusMeta.Icon;
    const ToolIcon = toolMeta.Icon;

    if (toolCall.name === 'websearch') {
      return <WebSearchSection toolCall={toolCall} />;
    }

    return (
      <div
        className={`tool-section is-${toolCall.status || 'calling'} ${
          expanded ? 'is-expanded' : ''
        } ${isAskUser ? 'is-ask-user' : ''} ${
          isAwaitingAnswer ? 'is-awaiting-user' : ''
        }`}
      >
        <button
          className="tool-section-trigger"
          type="button"
          onClick={toggle}
          aria-expanded={expanded}
          aria-label={`${toolMeta.label}：${toolCall.name}`}
        >
          <span className="tool-section-icon">
            <ToolIcon size={14} aria-hidden="true" />
          </span>

          <span className="tool-section-main">
            <span className="tool-section-title">
              <span>{toolMeta.label}</span>
              <code>{toolCall.name}</code>
            </span>
            {!expanded && summary && (
              <span className="tool-section-summary" title={summary}>
                {summary}
              </span>
            )}
          </span>

          <span className={`tool-status-badge is-${statusMeta.tone}`}>
            <StatusIcon
              size={11}
              className={statusMeta.spin ? 'buddy-spin' : undefined}
              aria-hidden="true"
            />
            {statusMeta.label}
          </span>

          <ChevronDown
            className="tool-section-chevron"
            size={14}
            aria-hidden="true"
          />
        </button>

        <AnimatePresence initial={false}>
          {expanded && (
            <motion.div
              className="tool-section-expand"
              initial={
                shouldReduceMotion
                  ? { opacity: 1 }
                  : { height: 0, opacity: 0, y: -2 }
              }
              animate={{ height: 'auto', opacity: 1, y: 0 }}
              exit={
                shouldReduceMotion
                  ? { opacity: 0 }
                  : { height: 0, opacity: 0, y: -2 }
              }
              transition={{
                duration: shouldReduceMotion ? 0 : 0.16,
                ease: [0.2, 0, 0, 1],
              }}
            >
              <div className="tool-section-body">
                {isAskUser ? (
                  <AskUserCard
                    toolCall={toolCall}
                    hasResult={hasResult}
                    isInterrupted={isInterrupted}
                  />
                ) : (
                  <DetailBlock
                    label="调用参数"
                    Icon={Braces}
                    language="json"
                    source={prettyArgs(toolCall.arguments)}
                  />
                )}

                {hasResult && !isAskUser && (
                  <DetailBlock
                    label={toolCall.is_error_result ? '执行错误' : '执行结果'}
                    Icon={FileCheck2}
                    language="text"
                    source={toolCall.result || '(无返回内容)'}
                    isError={toolCall.is_error_result}
                  />
                )}
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    );
  },
  (previous, next) =>
    previous.toolCall === next.toolCall &&
    previous.isStreaming === next.isStreaming,
);
