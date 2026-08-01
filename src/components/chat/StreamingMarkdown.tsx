import {
  Children,
  isValidElement,
  memo,
  useMemo,
  type ReactNode,
} from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { isTauri } from '@tauri-apps/api/core';
import { open as openExternal } from '@tauri-apps/plugin-shell';
import {
  MARKDOWN_EMPHASIS_GUARD,
  normalizeMarkdownEmphasis,
} from '@/utils/markdownNormalizer';
import { CodeBlock } from './CodeBlock';

interface MarkdownAstNode {
  type: string;
  tagName?: string;
  value?: string;
  properties?: Record<string, unknown>;
  children?: MarkdownAstNode[];
}

const STREAM_EFFECT_EXCLUDED_TAGS = new Set([
  'code',
  'pre',
  'script',
  'style',
]);
const STREAM_SETTLE_TRAIL_LENGTH = 9;

/**
 * 标记刚出现的字符，并把四角星插到下一个字符的位置。
 * 代码块不参与效果，避免破坏代码排版和复制体验。
 */
function createStreamingEffectPlugin(
  revealCount: number,
  revealKey: number,
  showStar: boolean,
) {
  return () => (tree: MarkdownAstNode) => {
    let remaining =
      showStar && revealCount > 0 ? STREAM_SETTLE_TRAIL_LENGTH : 0;
    let settleAge = 0;
    const phaseClass =
      revealKey % 2 === 0 ? 'is-phase-a' : 'is-phase-b';

    const decorateFromEnd = (node: MarkdownAstNode) => {
      if (remaining <= 0 || !node.children) return;
      if (
        node.tagName &&
        STREAM_EFFECT_EXCLUDED_TAGS.has(node.tagName)
      ) {
        return;
      }

      for (let index = node.children.length - 1; index >= 0; index -= 1) {
        if (remaining <= 0) break;
        const child = node.children[index];

        if (child.type !== 'text' || typeof child.value !== 'string') {
          decorateFromEnd(child);
          continue;
        }

        const characters = Array.from(child.value);
        const agesByIndex = new Map<number, number>();
        for (
          let characterIndex = characters.length - 1;
          characterIndex >= 0 && remaining > 0;
          characterIndex -= 1
        ) {
          // Markdown 会在块之间生成结构性换行；它们没有可见字形，
          // 不能占用字符轨迹或成为星标锚点。
          if (/^\s$/u.test(characters[characterIndex])) continue;
          agesByIndex.set(characterIndex, settleAge);
          settleAge += 1;
          remaining -= 1;
        }
        if (agesByIndex.size === 0) continue;

        const replacements: MarkdownAstNode[] = [];
        let stableBuffer = '';
        const flushStableBuffer = () => {
          if (!stableBuffer) return;
          replacements.push({ type: 'text', value: stableBuffer });
          stableBuffer = '';
        };

        characters.forEach((character, characterIndex) => {
          const age = agesByIndex.get(characterIndex);
          if (age === undefined) {
            stableBuffer += character;
            return;
          }
          flushStableBuffer();
          replacements.push({
            type: 'element',
            tagName: 'span',
            properties: {
              className: [
                'streaming-char-settle',
                phaseClass,
                `is-age-${age}`,
              ],
            },
            children: [{ type: 'text', value: character }],
          });
        });
        flushStableBuffer();

        node.children.splice(index, 1, ...replacements);
      }
    };

    decorateFromEnd(tree);
    if (!showStar) return;

    const starNode: MarkdownAstNode = {
      type: 'element',
      tagName: 'span',
      properties: {
        className: ['streaming-next-star', phaseClass],
        ariaHidden: 'true',
      },
      children: [],
    };

    const insertStarAtEnd = (node: MarkdownAstNode): boolean => {
      if (!node.children) return false;
      if (
        node.tagName &&
        STREAM_EFFECT_EXCLUDED_TAGS.has(node.tagName)
      ) {
        return false;
      }

      for (let index = node.children.length - 1; index >= 0; index -= 1) {
        const child = node.children[index];
        const childClasses = child.properties?.className;
        if (
          Array.isArray(childClasses) &&
          childClasses.includes('streaming-char-settle')
        ) {
          node.children.splice(index + 1, 0, starNode);
          return true;
        }
        if (
          child.type === 'text' &&
          typeof child.value === 'string' &&
          /\S/u.test(child.value)
        ) {
          node.children.splice(index + 1, 0, starNode);
          return true;
        }
        if (insertStarAtEnd(child)) return true;
      }
      return false;
    };

    insertStarAtEnd(tree);
  };
}

function StreamingNextStar({ revealKey }: { revealKey: number }) {
  const phaseClass =
    revealKey % 2 === 0 ? 'is-phase-a' : 'is-phase-b';
  return (
    <span
      className={`streaming-next-star ${phaseClass}`}
      aria-hidden="true"
    />
  );
}

/**
 * 判断 URL 是否需要拦截（除内部锚点与 mailto 外一律视为外链）。
 * 模型输出中常出现不带 scheme 的裸域名（example.com）、相对路径、协议相对链接
 * （//host），若放行它们会触发 webview 整体导航走——应用界面被替换/白屏。
 * 空 href（如被 urlTransform 中和掉的 javascript: 链接）也要拦截，避免页面刷新。
 */
function isExternalUrl(href: string): boolean {
  if (!href) return true;
  if (href.startsWith('#')) return false; // 内部锚点
  return !/^mailto:/i.test(href);
}

/** 把无 scheme 的链接规范化为 https，便于系统默认浏览器正确打开。 */
function normalizeHref(href: string): string {
  if (!href || href.startsWith('#') || /^[a-z][a-z0-9+.-]*:/i.test(href)) {
    return href;
  }
  if (href.startsWith('//')) return `https:${href}`;
  return `https://${href}`;
}

/**
 * 调用系统默认应用打开外链：
 * - Tauri 环境下用 plugin-shell open()（用系统默认浏览器）
 * - 浏览器环境降级到 window.open
 */
async function openInDefaultApp(url: string) {
  try {
    if (isTauri()) {
      await openExternal(url);
    } else {
      window.open(url, '_blank', 'noopener,noreferrer');
    }
  } catch (err) {
    console.error('[StreamingMarkdown] failed to open external url', url, err);
  }
}

/**
 * 共享的 markdown 组件映射，避免每次渲染都重新创建
 */
const COMPONENTS = {
  pre({ children }: { children?: ReactNode }) {
    const childNodes = Children.toArray(children);
    const codeChild = childNodes.length === 1 ? childNodes[0] : null;

    if (
      isValidElement<{
        className?: string;
        children?: ReactNode;
      }>(codeChild)
    ) {
      const className = codeChild.props.className || '';
      const languageMatch = /language-([\w-]+)/.exec(className);
      const source = String(codeChild.props.children ?? '').replace(/\n$/, '');

      return (
        <CodeBlock
          language={languageMatch?.[1] || 'text'}
          source={source}
        />
      );
    }

    return <pre>{children}</pre>;
  },
  code({ className, children, ...props }: any) {
    return (
      <code
        className={`markdown-inline-code ${className || ''}`.trim()}
        {...props}
      >
        {children}
      </code>
    );
  },
  p({ children }: any) {
    return <p style={{ margin: '0 0 var(--space-2) 0' }}>{children}</p>;
  },
  strong({ children }: any) {
    return (
      <strong>
        {Children.map(children, (child) =>
          typeof child === 'string'
            ? child.replaceAll(MARKDOWN_EMPHASIS_GUARD, '')
            : child,
        )}
      </strong>
    );
  },
  // 拦截外链：阻止 webview 内部跳转，强制走系统默认浏览器
  a({ href, children, ...props }: any) {
    const external = isExternalUrl(href);
    const resolvedHref = normalizeHref(href);
    return (
      <a
        className="markdown-link"
        href={resolvedHref}
        rel="noopener noreferrer"
        // 除内部锚点外一律拦截，强制走系统默认应用（Tauri shell.open / 浏览器 window.open），
        // 防止无 scheme / 相对 / 协议相对链接把整个 webview 导航走。
        onClick={(e) => {
          if (!external) return; // 内部锚点放行
          e.preventDefault();
          openInDefaultApp(resolvedHref);
        }}
        {...props}
      >
        {children}
      </a>
    );
  },
  table({ children }: any) {
    return (
      <div className="markdown-table-wrap">
        <table>{children}</table>
      </div>
    );
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
  /** 本次平滑渲染新增的字符数，仅用于正文入场光效。 */
  revealCount?: number;
  /** 每次字符批次递增，用于强制连续光效重新播放。 */
  revealKey?: number;
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
export function StreamingMarkdown({
  content,
  isStreaming,
  revealCount = 0,
  revealKey = 0,
}: StreamingMarkdownProps) {
  const normalizedContent = useMemo(
    () => normalizeMarkdownEmphasis(content),
    [content],
  );

  // 将内容切分为稳定块和不稳定尾部
  const { stablePart, unstablePart } = useMemo(() => {
    // 未在流式输出中，或内容为空 → 全部当作稳定内容
    if (!isStreaming || !normalizedContent) {
      return { stablePart: normalizedContent, unstablePart: '' };
    }

    // 查找最后一个段落分隔符 \n\n
    const lastDoubleNewline = normalizedContent.lastIndexOf('\n\n');
    if (lastDoubleNewline === -1) {
      // 没有段落分隔 → 全部内容都还不稳定
      return { stablePart: '', unstablePart: normalizedContent };
    }

    // 检查是否在代码围栏内部（围栏中的空行不应作为段落边界）
    const stableCandidate = normalizedContent.substring(
      0,
      lastDoubleNewline + 2,
    );
    const fenceCount = (stableCandidate.match(/```/g) || []).length;
    if (fenceCount % 2 !== 0) {
      // 处于未闭合的代码围栏中 → 回退到围栏开始位置
      const openingFence = stableCandidate.lastIndexOf('```');
      if (openingFence > 0) {
        return {
          stablePart: normalizedContent.substring(0, openingFence),
          unstablePart: normalizedContent.substring(openingFence),
        };
      }
      // 开围栏在开头 → 全部不稳定
      return { stablePart: '', unstablePart: content };
    }

    return {
      stablePart: stableCandidate,
      unstablePart: normalizedContent.substring(lastDoubleNewline + 2),
    };
  }, [normalizedContent, isStreaming]);

  const effectPlugin = useMemo(
    () =>
      createStreamingEffectPlugin(
        revealCount,
        revealKey,
        isStreaming && Boolean(unstablePart),
      ),
    [isStreaming, revealCount, revealKey, unstablePart],
  );
  const decorateUnstablePart =
    isStreaming && Boolean(unstablePart);

  return (
    <div className="ai-message-content">
      {/* 稳定部分：已写完整的段落，memo 后不会随 token 重解析 */}
      {stablePart && <StableMarkdown content={stablePart} />}

      {/* 不稳定部分：当前正在写的段落/标题/列表，跟随 token 更新 */}
      {unstablePart && (
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          rehypePlugins={
            decorateUnstablePart ? [effectPlugin] : undefined
          }
          components={COMPONENTS}
        >
          {unstablePart}
        </ReactMarkdown>
      )}
      {isStreaming && !unstablePart && (
        <StreamingNextStar revealKey={revealKey} />
      )}
    </div>
  );
}
