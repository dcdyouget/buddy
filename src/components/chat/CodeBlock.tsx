import { memo, useState } from 'react';
import { Copy, Check } from 'lucide-react';
import { Highlight, type PrismTheme } from 'prism-react-renderer';

/**
 * 使用 CSS 变量承载语法色，主题切换时无需重新解析代码。
 * 浅色模式采用蓝灰底 + 蓝/青/琥珀辅助色，暗色模式由同名变量覆盖。
 */
const buddyCodeTheme: PrismTheme = {
  plain: {
    color: 'var(--code-text)',
    backgroundColor: 'transparent',
  },
  styles: [
    {
      types: ['comment', 'prolog', 'doctype', 'cdata'],
      style: {
        color: 'var(--code-syntax-comment)',
        fontStyle: 'italic',
      },
    },
    {
      types: ['punctuation'],
      style: { color: 'var(--code-syntax-punctuation)' },
    },
    {
      types: ['property', 'tag', 'constant', 'symbol', 'deleted'],
      style: { color: 'var(--code-syntax-property)' },
    },
    {
      types: ['boolean', 'number'],
      style: { color: 'var(--code-syntax-number)' },
    },
    {
      types: [
        'selector',
        'attr-name',
        'string',
        'char',
        'builtin',
        'inserted',
      ],
      style: { color: 'var(--code-syntax-string)' },
    },
    {
      types: ['operator', 'entity', 'url', 'string-variable'],
      style: { color: 'var(--code-syntax-operator)' },
    },
    {
      types: ['atrule', 'attr-value', 'keyword'],
      style: {
        color: 'var(--code-syntax-keyword)',
        fontWeight: '600',
      },
    },
    {
      types: ['function', 'class-name'],
      style: { color: 'var(--code-syntax-function)' },
    },
    {
      types: ['regex', 'important', 'variable'],
      style: { color: 'var(--code-syntax-special)' },
    },
    {
      types: ['important', 'bold'],
      style: { fontWeight: 'bold' },
    },
    {
      types: ['italic'],
      style: { fontStyle: 'italic' },
    },
  ],
};

/**
 * CodeBlock 组件的 Props
 * @param language - 代码语言标识，用于语法高亮和头部标签显示
 * @param source - 代码源码字符串
 */
interface CodeBlockProps {
  language: string;
  source: string;
}

/**
 * 代码块组件
 * 在 AI 消息中渲染带语法高亮的代码块。包含顶部语言标签、复制按钮，
 * 以及使用 prism-react-renderer 和 Buddy 主题变量进行语法高亮。
 * 复制功能优先使用 Clipboard API，并带有降级方案以兼容旧环境。
 *
 * 使用 React.memo 避免在流式输出其他文本时重复进行代码语法高亮解析。
 */
export const CodeBlock = memo(function CodeBlock({ language, source }: CodeBlockProps) {
  // 复制状态：true 时显示"已复制"反馈
  const [copied, setCopied] = useState(false);
  const normalizedLanguage = language.toLowerCase();
  const isPlainText = ['plain', 'plaintext', 'text', 'txt'].includes(
    normalizedLanguage,
  );
  const languageLabel = isPlainText ? '文本结构' : language;

  /**
   * 复制代码到剪贴板
   * 优先使用 modern Clipboard API，失败时回退到 document.execCommand('copy')
   * 复制成功后显示 2 秒的"已复制"反馈
   */
  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(source);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // 降级方案：兼容不支持 Clipboard API 的环境（如旧版浏览器或非 HTTPS）
      const textarea = document.createElement('textarea');
      textarea.value = source;
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand('copy');
      document.body.removeChild(textarea);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div
      className={`markdown-code-block ${isPlainText ? 'is-plain-text' : ''}`}
      style={{
        borderRadius: 'var(--radius-md)',
        overflow: 'hidden',
        margin: 'var(--space-2) 0',
        border: '1px solid var(--border-subtle)',
      }}
    >
      {/* 头部栏：显示语言标签 + 复制按钮 */}
      <div
        className="markdown-code-header"
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: 'var(--space-1) var(--space-3)',
        }}
      >
        <span
          className="t-caption markdown-code-language"
          style={{
            fontFamily: 'var(--font-mono)',
            textTransform: 'lowercase',
          }}
        >
          {languageLabel}
        </span>
        <button
          onClick={handleCopy}
          title={copied ? '已复制' : '复制代码'}
          className={`markdown-code-copy ${copied ? 'is-copied' : ''}`}
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: 'var(--space-1)',
            padding: '2px var(--space-2)',
            borderRadius: 'var(--radius-sm)',
            border: 'none',
            background: 'transparent',
            // 复制成功后变为绿色，提供视觉反馈
            color: copied ? 'var(--state-success)' : 'var(--text-muted)',
            cursor: 'pointer',
            fontSize: 'var(--font-size-sm)',
            transition: `all var(--duration-fast) var(--ease-standard)`,
          }}
        >
          {copied ? (
            <>
              <Check size={12} />
              <span>已复制</span>
            </>
          ) : (
            <>
              <Copy size={12} />
              <span>复制</span>
            </>
          )}
        </button>
      </div>

      {/* 代码内容区域：使用 prism-react-renderer 进行语法高亮 */}
      <div
        style={{
          overflowX: 'auto',
          maxWidth: '100%',
        }}
      >
        <Highlight
          theme={buddyCodeTheme}
          code={source}
          language={normalizedLanguage}
        >
          {({ tokens, getLineProps, getTokenProps }) => (
            <pre
              style={{
                margin: 0,
                padding: 'var(--space-3)',
                fontSize: '13px',
                fontFamily: 'var(--font-mono)',
                lineHeight: 1.5,
                background: 'var(--code-bg)',
                overflowX: 'auto',
              }}
            >
              {tokens.map((line, i) => (
                <div key={i} {...getLineProps({ line })}>
                  {line.map((token, key) => (
                    <span key={key} {...getTokenProps({ token })} />
                  ))}
                </div>
              ))}
            </pre>
          )}
        </Highlight>
      </div>
    </div>
  );
},
(prevProps, nextProps) => {
  return prevProps.language === nextProps.language && prevProps.source === nextProps.source;
});
