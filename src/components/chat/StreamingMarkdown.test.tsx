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
});
