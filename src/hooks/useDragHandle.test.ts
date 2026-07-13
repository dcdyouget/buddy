// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';
import { shouldStartWindowDrag } from './useDragHandle';

describe('shouldStartWindowDrag', () => {
  it('允许从空白容器拖动窗口', () => {
    const blank = document.createElement('div');
    expect(shouldStartWindowDrag(blank)).toBe(true);
  });

  it('保留消息文本的选择行为', () => {
    const bubble = document.createElement('div');
    bubble.className = 'message-bubble';
    const paragraph = document.createElement('p');
    bubble.appendChild(paragraph);

    expect(shouldStartWindowDrag(paragraph)).toBe(false);
    expect(shouldStartWindowDrag(bubble)).toBe(false);
  });

  it('点击按钮内部图标时不会触发拖动', () => {
    const button = document.createElement('button');
    const icon = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    button.appendChild(icon);

    expect(shouldStartWindowDrag(icon)).toBe(false);
  });

  it('保留输入框和普通文本元素的原生交互', () => {
    expect(shouldStartWindowDrag(document.createElement('textarea'))).toBe(false);
    expect(shouldStartWindowDrag(document.createElement('span'))).toBe(false);
  });
});
