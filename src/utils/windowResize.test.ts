import { describe, expect, it } from 'vitest';

import { calculateCenteredTargetGeometry } from './windowResize';

describe('calculateCenteredTargetGeometry', () => {
  it('keeps the compact window centre while expanding', () => {
    const result = calculateCenteredTargetGeometry(
      { x: 500, y: 400 },
      { width: 460, height: 78 },
      { width: 750, height: 500 },
    );

    expect(result).toEqual({
      position: { x: 355, y: 189 },
      size: { width: 750, height: 500 },
    });
  });

  it('keeps the expanded window inside the monitor work area', () => {
    const result = calculateCenteredTargetGeometry(
      { x: 20, y: 30 },
      { width: 460, height: 78 },
      { width: 750, height: 500 },
      { x: 0, y: 25, width: 1440, height: 875 },
      12,
    );

    expect(result.position).toEqual({ x: 12, y: 37 });
  });
});
