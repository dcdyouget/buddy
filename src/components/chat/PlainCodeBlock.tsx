interface PlainCodeBlockProps {
  language: string;
  source: string;
  isStreaming?: boolean;
}

/** 代码高亮模块加载前及围栏未闭合时使用的轻量纯文本代码块。 */
export function PlainCodeBlock({
  language,
  source,
  isStreaming = false,
}: PlainCodeBlockProps) {
  const normalizedLanguage = language.toLowerCase();
  const isPlainText = ['plain', 'plaintext', 'text', 'txt'].includes(
    normalizedLanguage,
  );
  return (
    <div
      className={`markdown-code-block ${isPlainText ? 'is-plain-text' : ''} ${
        isStreaming ? 'is-streaming-code' : ''
      }`.trim()}
      style={{
        borderRadius: 'var(--radius-md)',
        overflow: 'hidden',
        margin: 'var(--space-2) 0',
        border: '1px solid var(--border-subtle)',
      }}
    >
      {!isPlainText && (
        <div
          className="markdown-code-header"
          style={{ padding: 'var(--space-1) var(--space-3)' }}
        >
          <span className="t-caption markdown-code-language">{language}</span>
        </div>
      )}
      <pre
        style={{
          margin: 0,
          padding: 'var(--space-3)',
          fontSize: 'var(--font-size-sm)',
          fontFamily: 'var(--font-mono)',
          lineHeight: 1.5,
          background: 'var(--code-bg)',
          overflowX: 'auto',
        }}
      >
        <code>{source}</code>
      </pre>
    </div>
  );
}
