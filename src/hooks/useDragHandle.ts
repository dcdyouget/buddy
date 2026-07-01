/**
 * useDragHandle.ts — Spotlight 风格窗口拖拽
 *
 * 关键：Tauri 的 startDragging() 必须在原生 mousedown 事件中同步调用，
 * React 合成事件（onMouseDown）经过事件委托后会丢失原生事件上下文。
 * 因此这里使用原生 addEventListener，并通过 ref 挂载到容器元素上。
 */

import { useEffect, useRef } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';

/** 仅排除这些必须保持原生交互的元素 */
const INTERACTIVE_TAGS = ['INPUT', 'TEXTAREA', 'BUTTON', 'SELECT'];

/**
 * Spotlight 风格拖拽 Hook
 *
 * 返回一个 ref，挂到页面最外层容器上即可让整个窗口任意位置（排除输入框/按钮）
 * 都可拖拽，包括消息列表、文字内容、GlassPanel 边缘等。
 *
 * 用法：<div ref={dragRef}> ... </div>
 */
export function useDragHandle() {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const onMouseDown = (e: MouseEvent) => {
      if (e.button !== 0) return;

      const target = e.target as HTMLElement;
      if (INTERACTIVE_TAGS.includes(target.tagName)) return;
      if (target.isContentEditable) return;

      try {
        getCurrentWindow().startDragging();
      } catch {
        // 浏览器环境下静默失败
      }
    };

    el.addEventListener('mousedown', onMouseDown);
    return () => el.removeEventListener('mousedown', onMouseDown);
  }, []);

  return ref;
}
