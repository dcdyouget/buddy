// @vitest-environment jsdom

import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { SlideInPanel } from './SlideInPanel';

describe('SlideInPanel', () => {
  it('退出动画开始时立即释放鼠标事件', () => {
    const { rerender } = render(
      <SlideInPanel show>
        <button type="button">面板按钮</button>
      </SlideInPanel>,
    );
    const layer = screen.getByRole('button', { name: '面板按钮' }).parentElement;

    expect(layer?.style.pointerEvents).toBe('auto');

    rerender(
      <SlideInPanel show={false}>
        <button type="button">面板按钮</button>
      </SlideInPanel>,
    );

    expect(layer?.style.pointerEvents).toBe('none');
  });
});
