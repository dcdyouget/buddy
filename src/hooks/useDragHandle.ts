/**
 * useDragHandle.ts — Spotlight 风格窗口拖拽
 *
 * 关键：Tauri 的 startDragging() 必须在原生 mousedown 事件中同步调用，
 * React 合成事件（onMouseDown）经过事件委托后会丢失原生事件上下文。
 * 因此这里使用原生 addEventListener，并通过 ref 挂载到容器元素上。
 */

import { useEffect, useRef } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';

/** 点击、输入类控件的整个区域都不能触发窗口拖动。 */
const INTERACTIVE_SELECTOR = [
  'input',
  'textarea',
  'button',
  'select',
  'a',
  'label',
  '[role="button"]',
  '[contenteditable]:not([contenteditable="false"])',
  '[data-no-window-drag]',
].join(',');

const DRAG_WHEN_EMPTY_SELECTOR = '[data-window-drag-when-empty]';

/**
 * 文本元素通常是块级元素，会占满一整行。
 * 只有实际字形所在的范围需要保留文本选择，右侧留白仍应允许拖窗。
 */
const TEXT_CONTAINER_SELECTOR = [
  'p',
  'span',
  'pre',
  'code',
  'blockquote',
  'li',
  'strong',
  'em',
  'del',
  'time',
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

interface PointerPosition {
  clientX: number;
  clientY: number;
}

function pointTouchesRenderedText(
  container: Element,
  { clientX, clientY }: PointerPosition,
): boolean {
  const document = container.ownerDocument;
  const walker = document.createTreeWalker(
    container,
    NodeFilter.SHOW_TEXT,
  );
  const range = document.createRange();

  // jsdom 或旧 WebView 缺少文字矩形 API 时，优先保留文本选择行为。
  if (typeof range.getClientRects !== 'function') return true;

  let textNode = walker.nextNode();
  while (textNode) {
    if (textNode.textContent?.trim()) {
      range.selectNodeContents(textNode);
      const rects = range.getClientRects();

      for (let index = 0; index < rects.length; index += 1) {
        const rect = rects[index];
        const hitSlop = 2;
        if (
          clientX >= rect.left - hitSlop &&
          clientX <= rect.right + hitSlop &&
          clientY >= rect.top - hitSlop &&
          clientY <= rect.bottom + hitSlop
        ) {
          return true;
        }
      }
    }
    textNode = walker.nextNode();
  }

  return false;
}

/**
 * 可交互控件始终阻止拖动；文本元素在没有坐标时采用保守判断，
 * 有坐标时仅实际文字范围阻止拖动，同一块级元素内的留白可用于拖窗。
 */
export function shouldStartWindowDrag(
  target: EventTarget | null,
  position?: PointerPosition,
): boolean {
  if (!(target instanceof Element)) return false;

  const emptyDraggableField = target.closest(DRAG_WHEN_EMPTY_SELECTOR);
  if (
    emptyDraggableField instanceof HTMLTextAreaElement &&
    emptyDraggableField.value.trim().length === 0
  ) {
    return true;
  }

  if (target.closest(INTERACTIVE_SELECTOR)) return false;

  const textContainer = target.closest(TEXT_CONTAINER_SELECTOR);
  if (!textContainer) return true;
  if (!position) return false;

  return !pointTouchesRenderedText(textContainer, position);
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

      if (
        !shouldStartWindowDrag(e.target, {
          clientX: e.clientX,
          clientY: e.clientY,
        })
      ) {
        return;
      }

      if (
        e.target instanceof HTMLTextAreaElement &&
        e.target.matches(DRAG_WHEN_EMPTY_SELECTOR)
      ) {
        e.target.focus({ preventScroll: true });
      }

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
