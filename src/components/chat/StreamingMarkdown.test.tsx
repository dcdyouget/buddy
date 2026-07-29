// @vitest-environment jsdom

import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { StreamingMarkdown } from './StreamingMarkdown';

afterEach(cleanup);

describe('StreamingMarkdown', () => {
  it('renders adjacent Chinese strong text without showing delimiters', () => {
    const source =
      '这些都是**信息检索（IR）**和**RAG（检索增强生成）**领域的核心概念';
    const { container } = render(
      <StreamingMarkdown content={source} isStreaming={false} />,
    );

    expect(
      Array.from(container.querySelectorAll('strong')).map(
        (element) => element.textContent,
      ),
    ).toEqual(['信息检索（IR）', 'RAG（检索增强生成）']);
    expect(container.textContent).toBe(
      '这些都是信息检索（IR）和RAG（检索增强生成）领域的核心概念',
    );
  });

  it('keeps regular English strong syntax working', () => {
    const { container } = render(
      <StreamingMarkdown
        content="Use **hybrid search** here."
        isStreaming={false}
      />,
    );

    expect(container.querySelector('strong')?.textContent).toBe(
      'hybrid search',
    );
  });

  it('does not normalize markers inside inline or fenced code', () => {
    const source = [
      '行内：`const value = "**（值）**"`',
      '',
      '```md',
      '**（代码）**',
      '```',
    ].join('\n');
    const { container } = render(
      <StreamingMarkdown content={source} isStreaming={false} />,
    );

    expect(container.querySelector('code')?.textContent).toBe(
      'const value = "**（值）**"',
    );
    const codeBlocks = Array.from(container.querySelectorAll('pre'));
    expect(codeBlocks[codeBlocks.length - 1]?.textContent).toBe(
      '**（代码）**',
    );
    expect(container.querySelector('strong')).toBeNull();
  });

  it('renders a language-less fenced block as one plain-text panel', () => {
    const source = [
      '```',
      '三公级 ──── 御史大夫',
      '              ↓',
      '顾问/显职 ── 光禄大夫',
      '```',
    ].join('\n');
    const { container } = render(
      <StreamingMarkdown content={source} isStreaming={false} />,
    );

    const block = container.querySelector(
      '.markdown-code-block.is-plain-text',
    );
    expect(block).not.toBeNull();
    expect(block?.querySelector('.markdown-code-language')?.textContent).toBe(
      '文本结构',
    );
    expect(block?.querySelector('pre')?.textContent).toContain(
      '顾问/显职 ── 光禄大夫',
    );
    expect(block?.querySelector('.markdown-inline-code')).toBeNull();
    expect(container.querySelector('pre > .markdown-code-block')).toBeNull();
  });

  it('keeps regular inline code separate from fenced blocks', () => {
    const { container } = render(
      <StreamingMarkdown content="使用 `npm test` 验证" isStreaming={false} />,
    );

    expect(container.querySelector('.markdown-inline-code')?.textContent).toBe(
      'npm test',
    );
    expect(container.querySelector('.markdown-code-block')).toBeNull();
  });

  it('settles the latest characters and keeps a star at the next position', () => {
    const { container, rerender } = render(
      <StreamingMarkdown
        content="前面的文字新字"
        isStreaming
        revealCount={2}
        revealKey={1}
      />,
    );

    const settlingCharacters = Array.from(
      container.querySelectorAll('.streaming-char-settle'),
    );
    expect(
      settlingCharacters.slice(-2).map((element) => element.textContent),
    ).toEqual(['新', '字']);
    expect(
      settlingCharacters[
        settlingCharacters.length - 1
      ]?.classList.contains('is-age-0'),
    ).toBe(true);
    expect(
      settlingCharacters[
        settlingCharacters.length - 2
      ]?.classList.contains('is-age-1'),
    ).toBe(true);
    expect(
      container.querySelector('.streaming-next-star.is-phase-b'),
    ).not.toBeNull();

    rerender(
      <StreamingMarkdown
        content="前面的文字新字。"
        isStreaming
        revealCount={1}
        revealKey={2}
      />,
    );

    expect(
      container.querySelector('.streaming-char-settle.is-age-0')
        ?.textContent,
    ).toBe('。');
    expect(
      container.querySelector('.streaming-next-star.is-phase-a'),
    ).not.toBeNull();
  });

  it('anchors the star to the final visible list character', () => {
    const { container } = render(
      <StreamingMarkdown
        content={'- 第一项\n- 最后一项\n'}
        isStreaming
        revealCount={1}
        revealKey={1}
      />,
    );

    const star = container.querySelector('.streaming-next-star');
    expect(star?.closest('li')?.textContent).toBe('最后一项');
    expect(star?.previousElementSibling?.textContent).toBe('项');
  });

  it('shows a breathing star before the first character arrives', () => {
    const { container } = render(
      <StreamingMarkdown
        content=""
        isStreaming
        revealCount={0}
        revealKey={0}
      />,
    );

    expect(
      container.querySelectorAll('.streaming-next-star'),
    ).toHaveLength(1);
    expect(container.querySelector('.streaming-char-settle')).toBeNull();
  });

  it('does not add the character transition to code content', () => {
    const { container } = render(
      <StreamingMarkdown
        content={'正文\n\n```ts\nconst value = 1;\n```'}
        isStreaming
        revealCount={20}
        revealKey={1}
      />,
    );

    expect(
      container.querySelector('pre .streaming-char-settle'),
    ).toBeNull();
  });
});
