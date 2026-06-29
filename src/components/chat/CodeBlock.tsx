import { useState } from 'react';
import { Copy, Check } from 'lucide-react';
import { Highlight, themes } from 'prism-react-renderer';

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
 * 以及使用 prism-react-renderer 进行 VS Code Dark 主题的语法高亮。
 * 复制功能优先使用 Clipboard API，并带有降级方案以兼容旧环境。
 */
export function CodeBlock({ language, source }: CodeBlockProps) {
  // 复制状态：true 时显示"已复制"反馈
  const [copied, setCopied] = useState(false);

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
      style={{
        borderRadius: 'var(--radius-md)',
        overflow: 'hidden',
        margin: 'var(--space-2) 0',
        border: '1px solid var(--border-subtle)',
      }}
    >
      {/* 头部栏：显示语言标签 + 复制按钮 */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: 'var(--space-1) var(--space-3)',
          background: 'var(--bg-sunken)',
          borderBottom: '1px solid var(--border-subtle)',
        }}
      >
        <span
          className="t-caption"
          style={{
            color: 'var(--text-muted)',
            fontFamily: 'var(--font-mono)',
            fontSize: '11px',
            textTransform: 'lowercase',
          }}
        >
          {language}
        </span>
        <button
          onClick={handleCopy}
          title={copied ? '已复制' : '复制代码'}
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: '4px',
            padding: '2px 6px',
            borderRadius: 'var(--radius-sm)',
            border: 'none',
            background: 'transparent',
            // 复制成功后变为绿色，提供视觉反馈
            color: copied ? 'var(--state-success)' : 'var(--text-muted)',
            cursor: 'pointer',
            fontSize: '12px',
            transition: `all var(--duration-fast) var(--ease-standard)`,
          }}
        >
          {copied ? (
            <>
              <Check size={12} />
              <span>Copied!</span>
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
        <Highlight theme={themes.vsDark} code={source} language={language}>
          {({ tokens, getLineProps, getTokenProps }) => (
            <pre
              style={{
                margin: 0,
                padding: 'var(--space-3)',
                fontSize: '13px',
                fontFamily: 'var(--font-mono)',
                lineHeight: 1.5,
                // VS Code Dark 主题的背景色
                background: '#1E1E1E',
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
}
