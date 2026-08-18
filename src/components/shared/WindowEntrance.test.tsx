// @vitest-environment jsdom

import { act, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  WINDOW_WILL_HIDE_EVENT,
  WINDOW_WILL_SHOW_EVENT,
} from '@/utils/windowEvents';
import { WindowEntrance } from './WindowEntrance';

function emitWillShow(detail?: Record<string, unknown>) {
  window.dispatchEvent(
    new CustomEvent(WINDOW_WILL_SHOW_EVENT, { detail }),
  );
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal(
    'requestAnimationFrame',
    vi.fn((callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    }),
  );
  vi.stubGlobal('cancelAnimationFrame', vi.fn());
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('WindowEntrance', () => {
  it('对话框使用完整尺寸，真实内容不缩放', () => {
    render(
      <WindowEntrance mode="expanded">
        <button type="button">完整尺寸</button>
      </WindowEntrance>,
    );
    const root = screen.getByRole('button', { name: '完整尺寸' }).parentElement
      ?.parentElement;

    expect(root?.classList.contains('is-expanded')).toBe(true);
    expect(root?.style.getPropertyValue('--window-entrance-scale-x')).toBe('1');
    expect(root?.style.getPropertyValue('--window-entrance-scale-y')).toBe('1');
  });

  it('紧凑气泡只保留装饰外壳形变', () => {
    render(
      <WindowEntrance mode="compact">
        <button type="button">紧凑气泡</button>
      </WindowEntrance>,
    );
    const root = screen.getByRole('button', { name: '紧凑气泡' }).parentElement
      ?.parentElement;

    expect(root?.classList.contains('is-compact')).toBe(true);
    expect(root?.style.getPropertyValue('--window-entrance-scale-x')).toBe('0.94');
    expect(root?.style.getPropertyValue('--window-entrance-scale-y')).toBe('0.88');
  });

  it('不渲染炫光图层', () => {
    const { container } = render(
      <WindowEntrance mode="expanded">
        <button type="button">无炫光</button>
      </WindowEntrance>,
    );

    expect(container.querySelector('.window-entrance-glow')).toBeNull();
  });

  it('隐藏后重新显示时只播放紧凑外壳过渡', () => {
    render(
      <WindowEntrance mode="compact">
        <button type="button">再次呼出</button>
      </WindowEntrance>,
    );
    const root = screen.getByRole('button', { name: '再次呼出' }).parentElement
      ?.parentElement;

    act(() => window.dispatchEvent(new Event(WINDOW_WILL_HIDE_EVENT)));
    expect(root?.dataset.entrancePhase).toBe('hidden');

    act(() => emitWillShow());
    expect(root?.dataset.entrancePhase).toBe('entering');

    act(() => vi.advanceTimersByTime(260));
    expect(root?.dataset.entrancePhase).toBe('settled');
  });

  it('闲置呼出时先恢复气泡页，再播放外壳过渡', async () => {
    let completeCompactSwitch!: () => void;
    const onCompactRequested = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          completeCompactSwitch = resolve;
        }),
    );
    render(
      <WindowEntrance mode="expanded" onCompactRequested={onCompactRequested}>
        <button type="button">气泡内容</button>
      </WindowEntrance>,
    );
    const root = screen.getByRole('button', { name: '气泡内容' }).parentElement
      ?.parentElement;

    act(() => emitWillShow({ open_compact: true }));
    expect(onCompactRequested).toHaveBeenCalledTimes(1);
    expect(root?.dataset.entrancePhase).toBe('hidden');

    await act(async () => {
      completeCompactSwitch();
      await Promise.resolve();
    });
    expect(root?.dataset.entrancePhase).toBe('entering');
  });
});
