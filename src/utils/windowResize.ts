/**
 * windowResize.ts — 窗口尺寸与位置管理
 *
 * 从气泡切换到内容页时，以气泡底边中心为锚点调整尺寸和位置，
 * 让窗口沿展开箭头指示的方向向上展开。
 */

import { PhysicalPosition, PhysicalSize } from '@tauri-apps/api/dpi';
import { currentMonitor, getCurrentWindow } from '@tauri-apps/api/window';
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
  empty: { width: 560, height: 60 },
  noapikey: { width: 560, height: 60 },
  conversation: { width: 750, height: 500 },
  streaming: { width: 750, height: 500 },
  settings: { width: 760, height: 640 },
  'add-provider': { width: 760, height: 640 },
};

const WINDOW_MARGIN = 12;

let activeResizeId = 0;

const clamp = (value: number, min: number, max: number) =>
  Math.min(Math.max(value, min), Math.max(min, max));

export function calculateBottomAnchoredTargetGeometry(
  startPosition: WindowPosition,
  startSize: WindowSize,
  targetSize: WindowSize,
  workArea?: WorkArea,
  margin = WINDOW_MARGIN,
): TargetGeometry {
  const anchoredPosition = {
    x: startPosition.x + (startSize.width - targetSize.width) / 2,
    y: startPosition.y + startSize.height - targetSize.height,
  };

  if (!workArea) {
    return {
      position: {
        x: Math.round(anchoredPosition.x),
        y: Math.round(anchoredPosition.y),
      },
      size: targetSize,
    };
  }

  return {
    position: {
      x: Math.round(
        clamp(
          anchoredPosition.x,
          workArea.x + margin,
          workArea.x + workArea.width - targetSize.width - margin,
        ),
      ),
      y: Math.round(
        clamp(
          anchoredPosition.y,
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
 * 一次性设置原生窗口尺寸，视觉过渡交给前端动画完成。
 * 避免每帧跨 IPC 调整窗口导致 macOS 重绘卡顿。
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
    const geometry = calculateBottomAnchoredTargetGeometry(
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
    await setGeometry(geometry.position, geometry.size);
  } catch (error) {
    console.error('[Buddy] resizeWindowToPage error:', error);
  }
}
