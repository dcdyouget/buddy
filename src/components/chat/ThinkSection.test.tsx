// @vitest-environment jsdom

import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { ThinkSection } from './ThinkSection';

afterEach(cleanup);

describe('ThinkSection', () => {
  it('shows the latest thinking content while streaming and collapsed', () => {
    const beginning = '这是较早的思考内容。'.repeat(12);
    const latest = '当前正在核对最后一种实现方案。';
    const { container } = render(
      <ThinkSection
        content={`${beginning}\n\n${latest}`}
        isStreaming
        defaultExpanded={false}
      />,
    );

    const preview = container.querySelector('.think-section-preview');
    expect(preview?.textContent).toContain(latest);
    expect(preview?.textContent?.startsWith('…')).toBe(true);
    expect(Array.from(preview?.textContent || '')).toHaveLength(97);
  });

  it('keeps the first line as the completed thinking preview', () => {
    const { container } = render(
      <ThinkSection
        content={'第一行总结\n后续详细思考'}
        isStreaming={false}
        defaultExpanded={false}
      />,
    );

    expect(
      container.querySelector('.think-section-preview')?.textContent,
    ).toBe('第一行总结');
  });
});
