// @vitest-environment jsdom

import { act, cleanup, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Message } from '@/types';
import { useChatStore } from '@/stores/chatStore';
import {
  useSmoothTextRenderer,
  WINDOW_WILL_HIDE_EVENT,
} from './useSmoothTextRenderer';

function assistantMessage(): Message {
  return {
    id: 'assistant-background-test',
    role: 'assistant',
    content: '',
    blocks: [],
    model_id: 'test-model',
    created_at: 0,
  };
}

function RendererHarness() {
  useSmoothTextRenderer();
  return null;
}

let visibilityState: DocumentVisibilityState = 'visible';
let frameCallback: FrameRequestCallback | undefined;

beforeEach(() => {
  vi.useFakeTimers();
  visibilityState = 'visible';
  frameCallback = undefined;
  Object.defineProperty(document, 'visibilityState', {
    configurable: true,
    get: () => visibilityState,
  });
  vi.stubGlobal(
    'requestAnimationFrame',
    vi.fn((callback: FrameRequestCallback) => {
      frameCallback = callback;
      return 1;
    }),
  );
  vi.stubGlobal('cancelAnimationFrame', vi.fn());
  useChatStore.setState({
    messages: [assistantMessage()],
    isStreaming: true,
    streamingTokens: 0,
    streamingBlocks: [],
    pendingTextBuffer: '',
    pendingTextEnd: null,
    streamDonePending: false,
    activeToolCalls: {},
    pendingQuestion: null,
    toolApproval: null,
    error: null,
  });
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('useSmoothTextRenderer 后台输出', () => {
  it('窗口可见时仍按动画帧逐字消费', () => {
    render(<RendererHarness />);

    act(() => {
      useChatStore.getState().feedTextDelta('正常');
    });
    expect(useChatStore.getState().pendingTextBuffer).toBe('正常');

    act(() => {
      frameCallback?.(0);
    });
    expect(useChatStore.getState().messages[0].content).toBe('正');
    expect(useChatStore.getState().pendingTextBuffer).toBe('常');
  });

  it('Esc 隐藏后立即消费后续缓冲，不依赖动画帧', () => {
    render(<RendererHarness />);

    act(() => {
      window.dispatchEvent(new Event(WINDOW_WILL_HIDE_EVENT));
      useChatStore.getState().feedTextDelta('后台继续😀');
    });

    expect(useChatStore.getState().pendingTextBuffer).toBe('');
    expect(useChatStore.getState().messages[0].content).toBe('后台继续😀');
  });

  it('重新显示时清理补发事件，随后恢复逐字速度', async () => {
    render(<RendererHarness />);

    act(() => {
      visibilityState = 'hidden';
      document.dispatchEvent(new Event('visibilitychange'));
      visibilityState = 'visible';
      document.dispatchEvent(new Event('visibilitychange'));
      useChatStore.getState().feedTextDelta('积压');
    });

    expect(useChatStore.getState().pendingTextBuffer).toBe('');
    expect(useChatStore.getState().messages[0].content).toBe('积压');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });
    act(() => {
      useChatStore.getState().feedTextDelta('新字');
    });

    expect(useChatStore.getState().pendingTextBuffer).toBe('新字');
  });

  it('隐藏窗口丢失旧动画帧后，恢复时仍能启动正文渲染', async () => {
    visibilityState = 'hidden';
    render(<RendererHarness />);

    // 模拟 WebKit 在隐藏窗口中丢弃已登记的 rAF 回调。
    frameCallback = undefined;
    act(() => {
      visibilityState = 'visible';
      document.dispatchEvent(new Event('visibilitychange'));
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });

    act(() => {
      useChatStore.getState().handleThinkingDelta(0, '已经思考完成');
      useChatStore.getState().feedTextDelta('正文');
    });

    // 正文到达时必须登记一个新的帧，不能被失效的旧句柄挡住。
    expect(frameCallback).toBeDefined();
    act(() => {
      frameCallback?.(0);
    });
    expect(useChatStore.getState().messages[0].content).toBe('正');
    expect(useChatStore.getState().streamingBlocks).toEqual([
      { type: 'thinking', content: '已经思考完成', is_open: false },
      { type: 'text', content: '正' },
    ]);
  });
});
