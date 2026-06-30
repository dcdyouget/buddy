/**
 * windowDrag.ts — 可靠的窗口拖拽工具
 *
 * 使用 Tauri 的 startDragging() API 而非 data-tauri-drag-region 属性，
 * 因为后者在 macOS 透明窗口上不可靠。
 *
 * 关键：startDragging() 必须在 mousedown 事件的同一 tick 同步调用，
 * 因此需要在模块顶层静态导入 getCurrentWindow，不能依赖动态 import()。
 *
 * 用法：在页面最外层容器上绑定 onMouseDown={handleDragStart}
 */

import { getCurrentWindow } from '@tauri-apps/api/window';

/** 交互元素标签：这些元素上的 mousedown 不触发窗口拖拽 */
const INTERACTIVE_TAGS = ['INPUT', 'TEXTAREA', 'BUTTON', 'SELECT', 'A', 'SUMMARY', 'DETAILS'];

/**
 * 智能拖拽处理器
 *
 * 仅当鼠标左键点击且在非交互元素上时才启动窗口拖拽。
 * 这意味着整个页面背景都可以拖，但按钮、输入框、文本域等保持正常交互。
 */
export function handleDragStart(e: React.MouseEvent) {
  // 仅响应鼠标左键
  if (e.button !== 0) return;

  const target = e.target as HTMLElement;

  // 跳过交互元素
  if (INTERACTIVE_TAGS.includes(target.tagName)) return;
  if (target.isContentEditable) return;
  if (target.closest('[role="button"],[role="textbox"],[role="combobox"],[role="slider"]')) return;

  // 同步调用系统拖拽 API
  try {
    getCurrentWindow().startDragging();
  } catch {
    // 浏览器环境下静默失败
  }
}
