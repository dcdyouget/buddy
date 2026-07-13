/**
 * windowResize.ts — 窗口尺寸与位置管理
 *
 * 从气泡切换到内容页时，以气泡中心为锚点同步调整尺寸和位置，
 * 避免无边框窗口默认从左上角生硬展开或跑出当前屏幕。
 */

import type { PageState } from '@/types';

interface WindowSize {
  width: number;
  height: number;
}

interface WindowPosition {
  x: number;
  y: number;
}

interface WorkArea extends WindowPosition, WindowSize {}

interface TargetGeometry {
  position: WindowPosition;
  size: WindowSize;
}

const COMPACT_PAGES: PageState[] = ['empty', 'noapikey'];

const PAGE_SIZES: Record<PageState, WindowSize> = {
  empty: { width: 460, height: 78 },
  noapikey: { width: 460, height: 78 },
  conversation: { width: 750, height: 500 },
  streaming: { width: 750, height: 500 },
  settings: { width: 760, height: 640 },
  'add-provider': { width: 760, height: 640 },
};

const RESIZE_DURATION_MS = 240;
const WINDOW_MARGIN = 12;

let activeResizeId = 0;

const clamp = (value: number, min: number, max: number) =>
  Math.min(Math.max(value, min), Math.max(min, max));

const interpolate = (from: number, to: number, progress: number) =>
  Math.round(from + (to - from) * progress);

const easeOutQuart = (progress: number) =>
  1 - Math.pow(1 - progress, 4);

const waitForNextFrame = () =>
  new Promise<void>((resolve) => {
    if (typeof requestAnimationFrame === 'function') {
      requestAnimationFrame(() => resolve());
      return;
    }

    setTimeout(resolve, 16);
  });

export function calculateCenteredTargetGeometry(
  startPosition: WindowPosition,
  startSize: WindowSize,
  targetSize: WindowSize,
  workArea?: WorkArea,
  margin = WINDOW_MARGIN,
): TargetGeometry {
  const centeredPosition = {
    x: startPosition.x + (startSize.width - targetSize.width) / 2,
    y: startPosition.y + (startSize.height - targetSize.height) / 2,
  };

  if (!workArea) {
    return {
      position: {
        x: Math.round(centeredPosition.x),
        y: Math.round(centeredPosition.y),
      },
      size: targetSize,
    };
  }

  return {
    position: {
      x: Math.round(
        clamp(
          centeredPosition.x,
          workArea.x + margin,
          workArea.x + workArea.width - targetSize.width - margin,
        ),
      ),
      y: Math.round(
        clamp(
          centeredPosition.y,
          workArea.y + margin,
          workArea.y + workArea.height - targetSize.height - margin,
        ),
      ),
    },
    size: targetSize,
  };
}

/**
 * 仅在离开紧凑页面时自动展开。普通内容页切换仍保留用户尺寸。
 */
export async function resizeWindowForPage(
  fromPage: PageState,
  toPage: PageState,
): Promise<void> {
  if (!COMPACT_PAGES.includes(fromPage)) return;
  if (COMPACT_PAGES.includes(toPage)) return;
  await resizeWindowToPage(toPage);
}

/**
 * 将窗口平滑过渡到指定页面的预设尺寸，并保持原窗口中心点。
 */
export async function resizeWindowToPage(page: PageState): Promise<void> {
  if (
    typeof window === 'undefined' ||
    !(window as typeof window & { __TAURI_INTERNALS__?: unknown })
      .__TAURI_INTERNALS__
  ) {
    return;
  }

  const size = PAGE_SIZES[page];
  if (!size) return;

  const resizeId = ++activeResizeId;

  try {
    const [
      { currentMonitor, getCurrentWindow },
      { PhysicalPosition, PhysicalSize },
    ] = await Promise.all([
      import('@tauri-apps/api/window'),
      import('@tauri-apps/api/dpi'),
    ]);
    const appWindow = getCurrentWindow();
    const [startPosition, startSize, scaleFactor, monitor] =
      await Promise.all([
        appWindow.outerPosition(),
        appWindow.outerSize(),
        appWindow.scaleFactor(),
        currentMonitor(),
      ]);

    if (resizeId !== activeResizeId) return;

    const targetSize = {
      width: Math.round(size.width * scaleFactor),
      height: Math.round(size.height * scaleFactor),
    };
    const workArea = monitor
      ? {
          x: monitor.workArea.position.x,
          y: monitor.workArea.position.y,
          width: monitor.workArea.size.width,
          height: monitor.workArea.size.height,
        }
      : undefined;
    const geometry = calculateCenteredTargetGeometry(
      startPosition,
      startSize,
      targetSize,
      workArea,
      Math.round(WINDOW_MARGIN * scaleFactor),
    );

    const setGeometry = async (
      position: WindowPosition,
      nextSize: WindowSize,
    ) => {
      await Promise.all([
        appWindow.setPosition(
          new PhysicalPosition(position.x, position.y),
        ),
        appWindow.setSize(
          new PhysicalSize(nextSize.width, nextSize.height),
        ),
      ]);
    };
    const hasMeaningfulChange =
      Math.abs(startPosition.x - geometry.position.x) > 1 ||
      Math.abs(startPosition.y - geometry.position.y) > 1 ||
      Math.abs(startSize.width - geometry.size.width) > 1 ||
      Math.abs(startSize.height - geometry.size.height) > 1;
    const prefersReducedMotion =
      window.matchMedia('(prefers-reduced-motion: reduce)').matches;

    if (!hasMeaningfulChange || prefersReducedMotion) {
      await setGeometry(geometry.position, geometry.size);
      return;
    }

    const startedAt = performance.now();

    while (resizeId === activeResizeId) {
      const elapsed = performance.now() - startedAt;
      const progress = Math.min(elapsed / RESIZE_DURATION_MS, 1);
      const easedProgress = easeOutQuart(progress);

      await setGeometry(
        {
          x: interpolate(
            startPosition.x,
            geometry.position.x,
            easedProgress,
          ),
          y: interpolate(
            startPosition.y,
            geometry.position.y,
            easedProgress,
          ),
        },
        {
          width: interpolate(
            startSize.width,
            geometry.size.width,
            easedProgress,
          ),
          height: interpolate(
            startSize.height,
            geometry.size.height,
            easedProgress,
          ),
        },
      );

      if (progress >= 1) return;
      await waitForNextFrame();
    }
  } catch (error) {
    console.error('[Buddy] resizeWindowToPage error:', error);
  }
}
