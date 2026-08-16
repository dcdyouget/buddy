import { useCallback, useEffect, useRef, type RefObject } from 'react';

const DOM_DELTA_LINE = 1;
const DOM_DELTA_PAGE = 2;
const LINE_HEIGHT_PX = 20;
const PAGE_SCROLL_RATIO = 0.82;
const SCROLL_EASING = 0.28;
const SETTLE_DISTANCE_PX = 0.5;
const EXTERNAL_SCROLL_TOLERANCE_PX = 1;

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
export function useSmoothWheelScroll(
  ref: RefObject<HTMLElement>,
  onUserScrollIntent?: (deltaY: number) => void,
) {
  const cancelAnimationRef = useRef<() => void>(() => {});
  const onUserScrollIntentRef = useRef(onUserScrollIntent);
  onUserScrollIntentRef.current = onUserScrollIntent;

  const cancelSmoothScroll = useCallback(() => {
    cancelAnimationRef.current();
  }, []);

  useEffect(() => {
    const element = ref.current;
    if (!element) return;

    let animationFrame = 0;
    let targetScrollTop = element.scrollTop;
    let lastAnimatedScrollTop: number | null = null;

    const cancelAnimation = () => {
      if (animationFrame) {
        window.cancelAnimationFrame(animationFrame);
      }
      animationFrame = 0;
      targetScrollTop = element.scrollTop;
      lastAnimatedScrollTop = null;
    };
    cancelAnimationRef.current = cancelAnimation;

    const animate = () => {
      if (
        lastAnimatedScrollTop !== null &&
        Math.abs(element.scrollTop - lastAnimatedScrollTop) >
          EXTERNAL_SCROLL_TOLERANCE_PX
      ) {
        // 历史分页补位、工具卡片变高后的自动跟随等外部写入，
        // 优先级高于旧滚轮目标；否则旧动画会继续把列表拉回原位置。
        cancelAnimation();
        return;
      }

      const maxScrollTop = Math.max(
        0,
        element.scrollHeight - element.clientHeight,
      );
      targetScrollTop = Math.min(maxScrollTop, Math.max(0, targetScrollTop));
      const distance = targetScrollTop - element.scrollTop;

      if (Math.abs(distance) <= SETTLE_DISTANCE_PX) {
        element.scrollTop = targetScrollTop;
        animationFrame = 0;
        lastAnimatedScrollTop = null;
        return;
      }

      element.scrollTop += distance * SCROLL_EASING;
      lastAnimatedScrollTop = element.scrollTop;
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
      if (!delta) {
        return;
      }
      onUserScrollIntentRef.current?.(delta);

      if (canNestedScrollerConsume(event.target, element, delta)) {
        // 用户已把滚轮交给内层结果区，外层不能继续执行旧的惯性动画。
        cancelAnimation();
        return;
      }

      if (
        animationFrame &&
        lastAnimatedScrollTop !== null &&
        Math.abs(element.scrollTop - lastAnimatedScrollTop) >
          EXTERNAL_SCROLL_TOLERANCE_PX
      ) {
        cancelAnimation();
      }

      const maxScrollTop = Math.max(
        0,
        element.scrollHeight - element.clientHeight,
      );
      if (
        (delta < 0 && element.scrollTop <= 0) ||
        (delta > 0 && element.scrollTop >= maxScrollTop)
      ) {
        cancelAnimation();
        return;
      }

      event.preventDefault();
      if (!animationFrame) targetScrollTop = element.scrollTop;
      targetScrollTop = Math.min(
        maxScrollTop,
        Math.max(0, targetScrollTop + delta),
      );
      if (!animationFrame) {
        lastAnimatedScrollTop = element.scrollTop;
        animationFrame = window.requestAnimationFrame(animate);
      }
    };

    element.addEventListener('wheel', handleWheel, { passive: false });
    return () => {
      cancelAnimation();
      cancelAnimationRef.current = () => {};
      element.removeEventListener('wheel', handleWheel);
    };
  }, [ref]);

  return cancelSmoothScroll;
}
