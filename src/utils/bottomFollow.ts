interface ResolveBottomFollowStateOptions {
  geometricallyAtBottom: boolean;
  isDetachedFromBottom: boolean;
  previousScrollTop: number;
  currentScrollTop: number;
}

interface BottomFollowState {
  isAtBottom: boolean;
  isDetachedFromBottom: boolean;
}

/**
 * 接近底部不等于允许自动跟随。
 * 用户向上滚动后保持脱离状态，直到滚动方向重新朝下并进入底部容差。
 */
export function resolveBottomFollowState({
  geometricallyAtBottom,
  isDetachedFromBottom,
  previousScrollTop,
  currentScrollTop,
}: ResolveBottomFollowStateOptions): BottomFollowState {
  if (!geometricallyAtBottom) {
    return { isAtBottom: false, isDetachedFromBottom: true };
  }

  if (isDetachedFromBottom && currentScrollTop <= previousScrollTop) {
    return { isAtBottom: false, isDetachedFromBottom: true };
  }

  return { isAtBottom: true, isDetachedFromBottom: false };
}
