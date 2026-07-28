import { describe, expect, it } from 'vitest';

import { calculateBottomAnchoredTargetGeometry } from './windowResize';

describe('calculateBottomAnchoredTargetGeometry', () => {
  it('keeps the compact window bottom and horizontal centre while expanding', () => {
    const result = calculateBottomAnchoredTargetGeometry(
      { x: 500, y: 400 },
      { width: 560, height: 60 },
      { width: 750, height: 500 },
    );

    expect(result).toEqual({
      position: { x: 405, y: -40 },
      size: { width: 750, height: 500 },
    });
  });

  it('keeps the expanded window inside the monitor work area', () => {
    const result = calculateBottomAnchoredTargetGeometry(
      { x: 20, y: 30 },
      { width: 460, height: 78 },
      { width: 750, height: 500 },
      { x: 0, y: 25, width: 1440, height: 875 },
      12,
    );

    expect(result.position).toEqual({ x: 12, y: 37 });
  });
});
