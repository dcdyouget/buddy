// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';
import { normalizeWheelDelta } from './useSmoothWheelScroll';

describe('useSmoothWheelScroll', () => {
  it('把滚轮的行和页步进换算成像素距离', () => {
    expect(normalizeWheelDelta(3, 1, 500)).toBe(60);
    expect(normalizeWheelDelta(1, 2, 500)).toBe(410);
    expect(normalizeWheelDelta(96, 0, 500)).toBe(96);
  });
});
