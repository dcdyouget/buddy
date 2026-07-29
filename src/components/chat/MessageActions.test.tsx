// @vitest-environment jsdom

import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Message } from '@/types';
import { MessageActions } from './MessageActions';

const message: Message = {
  id: 'assistant-1',
  role: 'assistant',
  content: '包含思考和最终回答',
  blocks: [
    { type: 'thinking', content: '不应复制', is_open: false },
    { type: 'text', content: '只复制最终回答' },
  ],
  model_id: 'test-model',
  created_at: 1_752_135_600,
};

const writeText = vi.fn().mockResolvedValue(undefined);

beforeEach(() => {
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText },
  });
});

afterEach(() => {
  cleanup();
  document.getElementById('question-1')?.remove();
  writeText.mockClear();
});

describe('MessageActions', () => {
  it('copies only the visible answer and shows success feedback', async () => {
    render(<MessageActions message={message} />);

    fireEvent.click(screen.getByRole('button', { name: '复制回答' }));

    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith('只复制最终回答');
    });
    expect(
      screen.getByRole('button', { name: '回答已复制' }),
    ).toBeTruthy();
    expect(screen.getByText('已复制')).toBeTruthy();
  });

  it('scrolls back to the question that produced the answer', () => {
    const question = document.createElement('div');
    question.id = 'question-1';
    question.scrollIntoView = vi.fn();
    document.body.appendChild(question);

    render(
      <MessageActions message={message} questionId="question-1" />,
    );

    fireEvent.click(
      screen.getByRole('button', { name: '回到本轮问题' }),
    );

    expect(question.scrollIntoView).toHaveBeenCalledWith({
      behavior: 'smooth',
      block: 'start',
    });
  });
});
