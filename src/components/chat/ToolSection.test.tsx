// @vitest-environment jsdom

import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { ToolCall } from '@/types';
import { useChatStore } from '@/stores/chatStore';
import { ToolSection } from './ToolSection';

afterEach(() => {
  cleanup();
  useChatStore.setState({ pendingQuestion: null });
});

describe('ToolSection', () => {
  it.each([
    ['list_directory', '浏览目录'],
    ['search_files', '搜索文件'],
    ['edit_file', '编辑文件'],
  ])('shows the localized label for %s', (name, label) => {
    const toolCall: ToolCall = {
      id: `call-${name}`,
      name,
      arguments: JSON.stringify({ path: '/tmp/project' }),
      status: 'done',
      result: 'ok',
      is_error_result: false,
    };

    render(<ToolSection toolCall={toolCall} isStreaming={false} />);

    expect(
      screen.getByRole('button', { name: `${label}：${name}` }),
    ).toBeTruthy();
  });

  it('shows a compact summary and expands into parameters and result', () => {
    const toolCall: ToolCall = {
      id: 'call-read',
      name: 'read_file',
      arguments: JSON.stringify({ path: '/tmp/example.md' }),
      status: 'done',
      result: '# example',
      is_error_result: false,
    };

    render(<ToolSection toolCall={toolCall} isStreaming={false} />);

    const trigger = screen.getByRole('button', {
      name: '读取文件：read_file',
    });
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    expect(screen.getByText('/tmp/example.md')).toBeTruthy();
    expect(screen.getByText('已完成')).toBeTruthy();

    fireEvent.click(trigger);

    expect(trigger.getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByText('调用参数')).toBeTruthy();
    expect(screen.getByText('执行结果')).toBeTruthy();
    expect(screen.getByText('# example')).toBeTruthy();
  });

  it('answers ask_user directly inside the tool card', async () => {
    const toolCall: ToolCall = {
      id: 'call-question',
      name: 'ask_user',
      arguments: JSON.stringify({
        header: '选择方向',
        question: '接下来做什么？',
        options: [
          { label: '继续优化' },
          { label: '先运行测试' },
        ],
      }),
      status: 'calling',
    };
    useChatStore.setState({
      pendingQuestion: {
        id: toolCall.id,
        header: '选择方向',
        question: '接下来做什么？',
        options: [
          { label: '继续优化' },
          { label: '先运行测试' },
        ],
        multiSelect: false,
      },
    });

    const { container } = render(
      <ToolSection toolCall={toolCall} isStreaming />,
    );

    expect(container.querySelector('.is-awaiting-user')).toBeTruthy();
    expect(container.querySelector('.question-prompt')).toBeTruthy();
    expect(screen.getByText('等待回答')).toBeTruthy();
    expect(screen.getByText('模型正在等待你的回答')).toBeTruthy();

    fireEvent.click(
      screen.getByRole('button', { name: '先运行测试' }),
    );
    fireEvent.click(screen.getByRole('button', { name: '确认' }));

    await waitFor(() => {
      expect(useChatStore.getState().pendingQuestion).toBeNull();
    });
    expect(screen.queryByRole('button', { name: '确认' })).toBeNull();
  });
});
