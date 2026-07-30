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

  it('shows websearch as a compact searching block that can be expanded', () => {
    const toolCall: ToolCall = {
      id: 'call-websearch',
      name: 'websearch',
      arguments: JSON.stringify({ query: 'Tauri 2 文档' }),
      status: 'executing',
    };

    render(<ToolSection toolCall={toolCall} isStreaming />);

    expect(screen.getByRole('status')).toBeTruthy();
    const trigger = screen.getByRole('button', {
      name: '网络搜索：Tauri 2 文档',
    });
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    expect(screen.getByText('正在搜索网络')).toBeTruthy();
    expect(screen.getByText('Tauri 2 文档')).toBeTruthy();
    expect(screen.getByLabelText('搜索中')).toBeTruthy();
    expect(screen.queryByText('调用参数')).toBeNull();

    fireEvent.click(trigger);

    expect(trigger.getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByText('搜索内容')).toBeTruthy();
    expect(screen.getByText('搜索引擎')).toBeTruthy();
    expect(screen.getByText('DuckDuckGo')).toBeTruthy();
    expect(screen.getByText('正在等待搜索结果…')).toBeTruthy();
  });

  it('shows structured websearch results after expansion', () => {
    const toolCall: ToolCall = {
      id: 'call-websearch-results',
      name: 'websearch',
      arguments: JSON.stringify({ query: 'Tauri 2 文档' }),
      status: 'done',
      result: JSON.stringify({
        status: 'partial',
        query: 'Tauri 2 官方文档',
        provider: 'duckduckgo',
        note: '已获得 2 条搜索结果。',
        results: [
          {
            rank: 1,
            title: 'Tauri 2 Documentation',
            url: 'https://v2.tauri.app/start/',
            snippet: 'Tauri 2 的官方入门文档。',
            content: '正文内容',
          },
          {
            rank: 2,
            title: 'Tauri Releases',
            url: 'https://github.com/tauri-apps/tauri/releases',
            snippet: 'Tauri 的版本发布信息。',
          },
        ],
      }),
      is_error_result: false,
    };

    render(<ToolSection toolCall={toolCall} isStreaming={false} />);

    const trigger = screen.getByRole('button', {
      name: '网络搜索：Tauri 2 官方文档',
    });
    fireEvent.click(trigger);

    expect(screen.getByText('搜索结果（2）')).toBeTruthy();
    expect(screen.getByText('Tauri 2 Documentation')).toBeTruthy();
    expect(screen.getByText('Tauri Releases')).toBeTruthy();
    expect(screen.getByText('Tauri 2 的官方入门文档。')).toBeTruthy();
    expect(screen.getByText('已读取网页正文')).toBeTruthy();
    expect(screen.getByText('使用搜索摘要')).toBeTruthy();
    const sourceLink = screen
      .getByText('Tauri 2 Documentation')
      .closest('a');
    expect(sourceLink?.getAttribute('href')).toBe(
      'https://v2.tauri.app/start/',
    );
    expect(sourceLink?.textContent).not.toContain(
      'https://v2.tauri.app/start/',
    );
    expect(
      screen.queryByText('v2.tauri.app/start', { exact: false }),
    ).toBeNull();
  });

  it('shows a non-blocking message when websearch is unavailable', () => {
    const toolCall: ToolCall = {
      id: 'call-websearch-unavailable',
      name: 'websearch',
      arguments: JSON.stringify({ query: '最新资料' }),
      status: 'done',
      result: JSON.stringify({
        status: 'unavailable',
        results: [],
      }),
      is_error_result: false,
    };

    render(<ToolSection toolCall={toolCall} isStreaming={false} />);

    expect(screen.getByText('网络搜索不可用，已继续回答')).toBeTruthy();
    expect(screen.queryByLabelText('搜索中')).toBeNull();
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
