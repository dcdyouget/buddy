// @vitest-environment jsdom

import { act, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  WINDOW_WILL_HIDE_EVENT,
  WINDOW_WILL_SHOW_EVENT,
} from '@/utils/windowEvents';
import { WindowEntrance } from './WindowEntrance';

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
  vi.restoreAllMocks();
});

describe('WindowEntrance', () => {
  it('每次呼出随机切换炫光轨迹且不连续重复', () => {
    vi.spyOn(Math, 'random').mockReturnValue(0);
    render(
      <WindowEntrance>
        <button type="button">随机炫光</button>
      </WindowEntrance>,
    );
    const root = screen.getByRole('button', { name: '随机炫光' }).parentElement
      ?.parentElement;
    const firstVariant = root?.dataset.glowVariant;

    act(() => {
      window.dispatchEvent(new Event(WINDOW_WILL_SHOW_EVENT));
    });

    expect(root?.dataset.glowVariant).not.toBe(firstVariant);
  });

  it('随机轨迹包含右上到左下和中心扩散', () => {
    const random = vi.spyOn(Math, 'random').mockReturnValue(0.45);
    const diagonal = render(
      <WindowEntrance>
        <button type="button">对角线炫光</button>
      </WindowEntrance>,
    );
    const diagonalRoot = screen.getByRole('button', { name: '对角线炫光' })
      .parentElement?.parentElement;
    expect(diagonalRoot?.dataset.glowVariant).toBe(
      'top-right-to-bottom-left',
    );
    diagonal.unmount();

    random.mockReturnValue(0.99);
    render(
      <WindowEntrance>
        <button type="button">中心炫光</button>
      </WindowEntrance>,
    );
    const centerRoot = screen.getByRole('button', { name: '中心炫光' })
      .parentElement?.parentElement;
    expect(centerRoot?.dataset.glowVariant).toBe('center-out');
  });

  it('每次窗口显示都重新播放形变动画', () => {
    render(
      <WindowEntrance>
        <button type="button">内容</button>
      </WindowEntrance>,
    );
    const root = screen.getByRole('button', { name: '内容' }).parentElement
      ?.parentElement;

    expect(root?.dataset.entrancePhase).toBe('entering');
    expect(root?.style.getPropertyValue('--window-entrance-scale-x')).toBe(
      '0.78',
    );
    expect(root?.style.getPropertyValue('--window-entrance-scale-y')).toBe(
      '0.58',
    );
    expect(root?.style.getPropertyValue('--window-entrance-radius')).toBe(
      'var(--radius-xl)',
    );

    act(() => {
      vi.advanceTimersByTime(600);
    });
    expect(root?.dataset.entrancePhase).toBe('settled');

    act(() => {
      window.dispatchEvent(new Event(WINDOW_WILL_HIDE_EVENT));
    });
    expect(root?.dataset.entrancePhase).toBe('hidden');

    act(() => {
      window.dispatchEvent(new Event(WINDOW_WILL_SHOW_EVENT));
    });
    expect(root?.dataset.entrancePhase).toBe('entering');
  });

  it('闲置呼出时先恢复气泡页，再播放形变动画', async () => {
    let completeCompactSwitch!: () => void;
    const onCompactRequested = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          completeCompactSwitch = resolve;
        }),
    );
    render(
      <WindowEntrance onCompactRequested={onCompactRequested}>
        <button type="button">气泡内容</button>
      </WindowEntrance>,
    );
    const root = screen.getByRole('button', { name: '气泡内容' }).parentElement
      ?.parentElement;

    act(() => {
      vi.advanceTimersByTime(600);
      window.dispatchEvent(new Event(WINDOW_WILL_HIDE_EVENT));
      window.dispatchEvent(
        new CustomEvent(WINDOW_WILL_SHOW_EVENT, {
          detail: { open_compact: true },
        }),
      );
    });

    expect(onCompactRequested).toHaveBeenCalledTimes(1);
    expect(root?.dataset.entrancePhase).toBe('hidden');

    await act(async () => {
      completeCompactSwitch();
      await Promise.resolve();
    });

    expect(root?.dataset.entrancePhase).toBe('entering');
  });
});
