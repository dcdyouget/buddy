// @vitest-environment jsdom

import { createElement, useEffect, useRef } from 'react';
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest';
import {
  normalizeWheelDelta,
  useSmoothWheelScroll,
} from './useSmoothWheelScroll';

interface ScrollHarnessProps {
  onUserScrollIntent?: (deltaY: number) => void;
  onCancelReady?: (cancel: () => void) => void;
}

function ScrollHarness({
  onUserScrollIntent,
  onCancelReady,
}: ScrollHarnessProps) {
  const ref = useRef<HTMLDivElement>(null);
  const cancel = useSmoothWheelScroll(ref, onUserScrollIntent);

  useEffect(() => {
    onCancelReady?.(cancel);
  }, [cancel, onCancelReady]);

  return createElement(
    'div',
    { ref, 'data-testid': 'scroller' },
    createElement('div', { 'data-testid': 'nested' }),
  );
}

let nextFrameId = 1;
let pendingFrames = new Map<number, FrameRequestCallback>();

function configureScrollBox(
  element: HTMLElement,
  { scrollTop, scrollHeight, clientHeight }: {
    scrollTop: number;
    scrollHeight: number;
    clientHeight: number;
  },
) {
  Object.defineProperties(element, {
    scrollTop: { configurable: true, writable: true, value: scrollTop },
    scrollHeight: { configurable: true, value: scrollHeight },
    clientHeight: { configurable: true, value: clientHeight },
  });
}

function runNextFrame() {
  const entry = pendingFrames.entries().next().value as
    | [number, FrameRequestCallback]
    | undefined;
  expect(entry).toBeDefined();
  if (!entry) return;
  pendingFrames.delete(entry[0]);
  entry[1](0);
}

beforeEach(() => {
  nextFrameId = 1;
  pendingFrames = new Map();
  vi.stubGlobal(
    'requestAnimationFrame',
    vi.fn((callback: FrameRequestCallback) => {
      const id = nextFrameId++;
      pendingFrames.set(id, callback);
      return id;
    }),
  );
  vi.stubGlobal(
    'cancelAnimationFrame',
    vi.fn((id: number) => pendingFrames.delete(id)),
  );
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe('useSmoothWheelScroll', () => {
  it('把滚轮的行和页步进换算成像素距离', () => {
    expect(normalizeWheelDelta(3, 1, 500)).toBe(60);
    expect(normalizeWheelDelta(1, 2, 500)).toBe(410);
    expect(normalizeWheelDelta(96, 0, 500)).toBe(96);
  });

  it('向上滚动时立即报告用户意图，不等待首个动画帧', () => {
    const onUserScrollIntent = vi.fn();
    const { getByTestId } = render(
      createElement(ScrollHarness, { onUserScrollIntent }),
    );
    const scroller = getByTestId('scroller');
    configureScrollBox(scroller, {
      scrollTop: 600,
      scrollHeight: 1000,
      clientHeight: 200,
    });

    fireEvent.wheel(scroller, { deltaY: -120 });

    expect(onUserScrollIntent).toHaveBeenCalledWith(-120);
    expect(scroller.scrollTop).toBe(600);
    expect(pendingFrames.size).toBe(1);
  });

  it('程序化修改滚动位置后废弃旧的滚轮目标', () => {
    const { getByTestId } = render(createElement(ScrollHarness));
    const scroller = getByTestId('scroller');
    configureScrollBox(scroller, {
      scrollTop: 600,
      scrollHeight: 1000,
      clientHeight: 200,
    });

    fireEvent.wheel(scroller, { deltaY: -120 });
    act(runNextFrame);
    expect(scroller.scrollTop).toBeCloseTo(566.4);

    // 模拟历史分页补位或工具卡片 ResizeObserver 自动跟随。
    scroller.scrollTop = 300;
    act(runNextFrame);

    expect(scroller.scrollTop).toBe(300);
    expect(pendingFrames.size).toBe(0);
  });

  it('滚轮进入工具结果的内层滚动区时停止外层惯性动画', () => {
    const { getByTestId } = render(createElement(ScrollHarness));
    const scroller = getByTestId('scroller');
    const nested = getByTestId('nested');
    configureScrollBox(scroller, {
      scrollTop: 600,
      scrollHeight: 1000,
      clientHeight: 200,
    });
    configureScrollBox(nested, {
      scrollTop: 100,
      scrollHeight: 400,
      clientHeight: 100,
    });
    nested.style.overflowY = 'auto';

    fireEvent.wheel(scroller, { deltaY: -120 });
    expect(pendingFrames.size).toBe(1);

    fireEvent.wheel(nested, { deltaY: -30 });

    expect(pendingFrames.size).toBe(0);
    expect(scroller.scrollTop).toBe(600);
  });

  it('外部控制器可以在历史分页补位前取消动画', () => {
    let cancel = () => {};
    const { getByTestId } = render(
      createElement(ScrollHarness, {
        onCancelReady: (nextCancel) => {
          cancel = nextCancel;
        },
      }),
    );
    const scroller = getByTestId('scroller');
    configureScrollBox(scroller, {
      scrollTop: 600,
      scrollHeight: 1000,
      clientHeight: 200,
    });

    fireEvent.wheel(scroller, { deltaY: -120 });
    expect(pendingFrames.size).toBe(1);

    act(cancel);

    expect(pendingFrames.size).toBe(0);
  });
});
