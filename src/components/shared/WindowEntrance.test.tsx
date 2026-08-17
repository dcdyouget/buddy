// @vitest-environment jsdom

import { act, render, screen } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  WINDOW_WILL_HIDE_EVENT,
  WINDOW_WILL_SHOW_EVENT,
} from '@/utils/windowEvents';
import { WindowEntrance } from './WindowEntrance';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

beforeEach(() => {
  vi.mocked(invoke).mockClear();
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

  it('将边框炫光与真实内容拆成独立图层', () => {
    const { container } = render(
      <WindowEntrance>
        <button type="button">炫光层级</button>
      </WindowEntrance>,
    );
    const root = container.querySelector('.window-entrance');
    const content = root?.querySelector('.window-entrance-content');
    const glow = root?.querySelector('.window-entrance-glow');

    expect(glow?.parentElement).toBe(root);
    expect(content?.nextElementSibling).toBe(glow);
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
      '0.97',
    );
    expect(root?.style.getPropertyValue('--window-entrance-scale-y')).toBe(
      '0.96',
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

  it('已预置隐藏帧时不等待 RAF 就进入动画', () => {
    render(
      <WindowEntrance>
        <button type="button">立即呼出</button>
      </WindowEntrance>,
    );
    const root = screen.getByRole('button', { name: '立即呼出' })
      .parentElement?.parentElement;

    act(() => {
      vi.advanceTimersByTime(600);
      window.dispatchEvent(new Event(WINDOW_WILL_HIDE_EVENT));
    });
    expect(root?.dataset.entrancePhase).toBe('hidden');

    vi.stubGlobal('requestAnimationFrame', vi.fn(() => 9));
    act(() => {
      window.dispatchEvent(new Event(WINDOW_WILL_SHOW_EVENT));
    });

    expect(root?.dataset.entrancePhase).toBe('entering');
  });

  it('将事件接收和首个 RAF 写入同一个诊断 trace', () => {
    render(
      <WindowEntrance>
        <button type="button">诊断内容</button>
      </WindowEntrance>,
    );

    act(() => {
      window.dispatchEvent(
        new CustomEvent(WINDOW_WILL_SHOW_EVENT, {
          detail: {
            open_compact: false,
            trace_id: 42,
            emitted_at_ms: 1_700_000_000_000,
          },
        }),
      );
    });

    expect(invoke).toHaveBeenCalledWith(
      'log_window_frontend_diagnostic',
      expect.objectContaining({ traceId: 42, stage: 'event-received' }),
    );
    expect(invoke).toHaveBeenCalledWith(
      'log_window_frontend_diagnostic',
      expect.objectContaining({ traceId: 42, stage: 'raf-restart' }),
    );
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
