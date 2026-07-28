/**
 * useDragHandle.ts — Spotlight 风格窗口拖拽
 *
 * 关键：Tauri 的 startDragging() 必须在原生 mousedown 事件中同步调用，
 * React 合成事件（onMouseDown）经过事件委托后会丢失原生事件上下文。
 * 因此这里使用原生 addEventListener，并通过 ref 挂载到容器元素上。
 */

import { useEffect, useRef } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';

/**
 * 这些元素需要保留点击、输入或文本选择行为。
 * 使用 closest 而不是只判断 target.tagName，避免点击按钮内的 SVG 时误触发拖动。
 */
const NO_DRAG_SELECTOR = [
  'input',
  'textarea',
  'button',
  'select',
  'a',
  'label',
  '[contenteditable]:not([contenteditable="false"])',
  '[data-no-window-drag]',
  'p',
  'span',
  'pre',
  'code',
  'blockquote',
  'li',
  'h1',
  'h2',
  'h3',
  'h4',
  'h5',
  'h6',
  'td',
  'th',
  'dt',
  'dd',
].join(',');

/**
 * 仅文字本身与可交互控件阻止拖动。
 * 消息气泡占满整行，不能把整个气泡都列为禁拖区域；否则文字四周的空白
 * 也无法拖窗。实际文本节点由下列语义元素匹配，气泡的空白区域则可拖动。
 */
export function shouldStartWindowDrag(target: EventTarget | null): boolean {
  return target instanceof Element && !target.closest(NO_DRAG_SELECTOR);
}

/**
 * Spotlight 风格拖拽 Hook
 *
 * 返回一个 ref，挂到页面最外层容器上即可从空白区域和玻璃边缘拖动窗口。
 * 文本内容、输入控件和按钮保留原生交互。
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

      if (!shouldStartWindowDrag(e.target)) return;

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
