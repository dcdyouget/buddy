// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import { shouldStartWindowDrag } from './useDragHandle';

afterEach(() => {
  vi.restoreAllMocks();
});

describe('shouldStartWindowDrag', () => {
  it('允许从空白容器拖动窗口', () => {
    const blank = document.createElement('div');
    expect(shouldStartWindowDrag(blank)).toBe(true);
  });

  it('仅消息中的文字保留选择，气泡空白处可以拖动', () => {
    const bubble = document.createElement('div');
    bubble.className = 'message-bubble';
    const paragraph = document.createElement('p');
    bubble.appendChild(paragraph);

    expect(shouldStartWindowDrag(paragraph)).toBe(false);
    expect(shouldStartWindowDrag(bubble)).toBe(true);
  });

  it('点击按钮内部图标时不会触发拖动', () => {
    const button = document.createElement('button');
    const icon = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    button.appendChild(icon);

    expect(shouldStartWindowDrag(icon)).toBe(false);
  });

  it('ARIA button 容器不会触发窗口拖动', () => {
    const button = document.createElement('div');
    button.setAttribute('role', 'button');
    const icon = document.createElement('span');
    button.appendChild(icon);

    expect(shouldStartWindowDrag(icon)).toBe(false);
  });

  it('保留输入框和普通文本元素的原生交互', () => {
    expect(shouldStartWindowDrag(document.createElement('textarea'))).toBe(false);
    expect(shouldStartWindowDrag(document.createElement('span'))).toBe(false);
  });

  it('气泡输入框仅在没有有效文字时允许拖动', () => {
    const textarea = document.createElement('textarea');
    textarea.setAttribute('data-window-drag-when-empty', '');

    expect(shouldStartWindowDrag(textarea)).toBe(true);

    textarea.value = '准备发送的内容';
    expect(shouldStartWindowDrag(textarea)).toBe(false);
  });

  it('标题文字右侧的块级留白可以拖动', () => {
    const heading = document.createElement('h2');
    heading.textContent = '六、加官制度';
    document.body.appendChild(heading);

    vi.spyOn(document, 'createRange').mockReturnValue({
      selectNodeContents: vi.fn(),
      getClientRects: () =>
        [
          {
            left: 32,
            right: 180,
            top: 40,
            bottom: 68,
          },
        ] as unknown as DOMRectList,
    } as unknown as Range);

    expect(
      shouldStartWindowDrag(heading, { clientX: 100, clientY: 52 }),
    ).toBe(false);
    expect(
      shouldStartWindowDrag(heading, { clientX: 420, clientY: 52 }),
    ).toBe(true);
  });
});
