/**
 * windowResize.ts — 智能窗口尺寸管理
 *
 * 策略：
 * - 从紧凑页面（empty / noapikey）切换到内容页面时，自动展开窗口以容纳内容
 * - 内容页面之间切换时，保持用户设置的窗口尺寸（遵循硬约束 #6）
 * - 用户手动调整窗口大小后，后续页面切换不再自动调整尺寸
 */

import type { PageState } from '@/types';

/** 紧凑状态页面：窗口初始尺寸较小，切换到内容页面时需要展开 */
const COMPACT_PAGES: PageState[] = ['empty', 'noapikey'];

/** 内容状态页面：需要较大窗口容纳聊天内容（设置已改为覆层，不自动 resize） */
const CONTENT_PAGES: PageState[] = ['conversation', 'streaming'];

/** 各页面对应的窗口预设尺寸（逻辑像素） */
const PAGE_SIZES: Record<PageState, { width: number; height: number }> = {
  empty: { width: 520, height: 78 },
  noapikey: { width: 520, height: 78 },
  conversation: { width: 750, height: 500 },
  streaming: { width: 750, height: 500 },
  settings: { width: 760, height: 640 },
  'add-provider': { width: 760, height: 640 },
};

/**
 * 根据页面切换自动调整窗口尺寸
 *
 * 规则：
 * 1. 仅当来源页面是紧凑页面（empty/noapikey）且目标页面是内容页面时，才自动调整窗口尺寸
 * 2. 内容页面之间的切换（如 conversation ↔ settings）不调整窗口尺寸，遵循硬约束 #6
 * 3. 切换回紧凑页面时也不调整尺寸，保持用户偏好
 *
 * @param fromPage 来源页面
 * @param toPage 目标页面
 */
export async function resizeWindowForPage(fromPage: PageState, toPage: PageState): Promise<void> {
  // 仅允许「紧凑页面 → 内容页面」的初始展开
  if (!COMPACT_PAGES.includes(fromPage)) return; // 来源不是紧凑页面，保持用户尺寸
  if (!CONTENT_PAGES.includes(toPage)) return;   // 目标不是内容页面，不需要展开

  const size = PAGE_SIZES[toPage];
  if (!size) return;

  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    const { LogicalSize } = await import('@tauri-apps/api/dpi');
    console.log('[Buddy] resizeWindow:', fromPage, '→', toPage, 'size:', size.width, 'x', size.height);
    await getCurrentWindow().setSize(new LogicalSize(size.width, size.height));
    console.log('[Buddy] resizeWindow done');
  } catch (e) {
    console.error('[Buddy] resizeWindow error:', e);
  }
}
