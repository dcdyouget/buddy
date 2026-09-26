// @vitest-environment jsdom

import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useUIStore } from '@/stores/uiStore';
import { PageRenderer } from './App';

vi.mock('@/pages/EmptyPage', () => ({
  EmptyPage: () => <div>紧凑输入页</div>,
}));
vi.mock('@/pages/NoApiKeyPage', () => ({
  NoApiKeyPage: () => <div>配置提示页</div>,
}));
vi.mock('@/pages/ChatPage', () => ({
  ChatPage: () => <div>对话内容</div>,
}));

beforeEach(() => {
  useUIStore.setState({
    currentPage: 'empty',
    previousPage: null,
  });
});

afterEach(cleanup);

describe('PageRenderer', () => {
  it('气泡发送切换到流式页时立即挂载对话内容', () => {
    useUIStore.setState({
      currentPage: 'streaming',
      previousPage: 'empty',
    });

    render(<PageRenderer />);

    expect(screen.getByText('对话内容')).toBeTruthy();
  });
});
