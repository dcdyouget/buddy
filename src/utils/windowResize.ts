/**
 * windowResize.ts — 原生窗口尺寸切换入口
 *
 * 前端只判断何时需要调整，并通过一次 IPC 告知目标页面；尺寸映射、
 * 显示器工作区裁剪和底边锚定计算统一由 Rust 完成。
 */

import { invoke, isTauri } from '@tauri-apps/api/core';
import type { PageState } from '@/types';

const COMPACT_PAGES: PageState[] = ['empty', 'noapikey'];

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
  if (!isTauri()) return;

  try {
    await invoke('resize_window_to_page', { page });
  } catch (error) {
    console.error('[Buddy] resizeWindowToPage error:', error);
  }
}
