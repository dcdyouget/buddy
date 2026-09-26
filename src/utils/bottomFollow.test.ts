import { describe, expect, it } from 'vitest';
import { resolveBottomFollowState } from './bottomFollow';

describe('resolveBottomFollowState', () => {
  it('小幅向上滚动仍在底部容差内时保持脱离', () => {
    expect(
      resolveBottomFollowState({
        geometricallyAtBottom: true,
        isDetachedFromBottom: true,
        previousScrollTop: 800,
        currentScrollTop: 798.88,
      }),
    ).toEqual({ isAtBottom: false, isDetachedFromBottom: true });
  });

  it('向下回到底部容差时恢复自动跟随', () => {
    expect(
      resolveBottomFollowState({
        geometricallyAtBottom: true,
        isDetachedFromBottom: true,
        previousScrollTop: 790,
        currentScrollTop: 792,
      }),
    ).toEqual({ isAtBottom: true, isDetachedFromBottom: false });
  });

  it('离开底部容差后锁定为脱离状态', () => {
    expect(
      resolveBottomFollowState({
        geometricallyAtBottom: false,
        isDetachedFromBottom: false,
        previousScrollTop: 800,
        currentScrollTop: 780,
      }),
    ).toEqual({ isAtBottom: false, isDetachedFromBottom: true });
  });
});
