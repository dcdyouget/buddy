import { useEffect, type RefObject } from 'react';

const DOM_DELTA_LINE = 1;
const DOM_DELTA_PAGE = 2;
const LINE_HEIGHT_PX = 20;
const PAGE_SCROLL_RATIO = 0.82;
const SCROLL_EASING = 0.28;
const SETTLE_DISTANCE_PX = 0.5;

export function normalizeWheelDelta(
  deltaY: number,
  deltaMode: number,
  viewportHeight: number,
): number {
  if (deltaMode === DOM_DELTA_LINE) return deltaY * LINE_HEIGHT_PX;
  if (deltaMode === DOM_DELTA_PAGE) {
    return deltaY * viewportHeight * PAGE_SCROLL_RATIO;
  }
  return deltaY;
}

function canNestedScrollerConsume(
  target: EventTarget | null,
  boundary: HTMLElement,
  deltaY: number,
): boolean {
  let element = target instanceof Element ? target : null;
  while (element && element !== boundary) {
    if (element instanceof HTMLElement) {
      const overflowY = window.getComputedStyle(element).overflowY;
      const isScrollable =
        (overflowY === 'auto' || overflowY === 'scroll') &&
        element.scrollHeight > element.clientHeight;
      if (isScrollable) {
        const maxScrollTop = element.scrollHeight - element.clientHeight;
        if (
          (deltaY < 0 && element.scrollTop > 0) ||
          (deltaY > 0 && element.scrollTop < maxScrollTop)
        ) {
          return true;
        }
      }
    }
    element = element.parentElement;
  }
  return false;
}

/**
 * 把对话列表中的所有垂直滚轮输入统一转换为逐帧缓动。
 * 连续事件会累加目标位置，兼顾普通鼠标的离散步进和触控板的连续输入。
 */
export function useSmoothWheelScroll(ref: RefObject<HTMLElement>) {
  useEffect(() => {
    const element = ref.current;
    if (!element) return;

    let animationFrame = 0;
    let targetScrollTop = element.scrollTop;

    const cancelAnimation = () => {
      window.cancelAnimationFrame(animationFrame);
      animationFrame = 0;
      targetScrollTop = element.scrollTop;
    };

    const animate = () => {
      const maxScrollTop = Math.max(
        0,
        element.scrollHeight - element.clientHeight,
      );
      targetScrollTop = Math.min(maxScrollTop, Math.max(0, targetScrollTop));
      const distance = targetScrollTop - element.scrollTop;

      if (Math.abs(distance) <= SETTLE_DISTANCE_PX) {
        element.scrollTop = targetScrollTop;
        animationFrame = 0;
        return;
      }

      element.scrollTop += distance * SCROLL_EASING;
      animationFrame = window.requestAnimationFrame(animate);
    };

    const handleWheel = (event: WheelEvent) => {
      if (
        event.ctrlKey ||
        event.metaKey ||
        event.shiftKey ||
        Math.abs(event.deltaX) > Math.abs(event.deltaY)
      ) {
        return;
      }

      const delta = normalizeWheelDelta(
        event.deltaY,
        event.deltaMode,
        element.clientHeight,
      );
      if (!delta || canNestedScrollerConsume(event.target, element, delta)) {
        return;
      }

      const maxScrollTop = Math.max(
        0,
        element.scrollHeight - element.clientHeight,
      );
      if (
        (delta < 0 && element.scrollTop <= 0) ||
        (delta > 0 && element.scrollTop >= maxScrollTop)
      ) {
        return;
      }

      event.preventDefault();
      if (!animationFrame) targetScrollTop = element.scrollTop;
      targetScrollTop = Math.min(
        maxScrollTop,
        Math.max(0, targetScrollTop + delta),
      );
      if (!animationFrame) {
        animationFrame = window.requestAnimationFrame(animate);
      }
    };

    element.addEventListener('wheel', handleWheel, { passive: false });
    return () => {
      cancelAnimation();
      element.removeEventListener('wheel', handleWheel);
    };
  }, [ref]);
}
