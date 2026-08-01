// @vitest-environment jsdom

import { cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import type { Message, ToolCall } from '@/types';
import { MessageBubble } from './MessageBubble';

afterEach(cleanup);

function assistantMessage(blocks: Message['blocks']): Message {
  return {
    id: 'assistant-tool-order',
    role: 'assistant',
    content: '',
    blocks,
    model_id: 'test-model',
    created_at: 0,
  };
}

const liveToolCalls: ToolCall[] = [
  {
    id: 'search-company',
    name: 'websearch',
    arguments: '{"query":"Google Alphabet 最新业务"}',
    status: 'done',
    result: '{"status":"ok","results":[]}',
    insertAfterBlockIndex: -1,
  },
  {
    id: 'search-stock',
    name: 'websearch',
    arguments: '{"query":"Alphabet GOOGL 股价"}',
    status: 'executing',
    insertAfterBlockIndex: -1,
  },
];

describe('MessageBubble 工具调用位置', () => {
  it('流式无内容时每个工具只渲染一次', () => {
    const { container } = render(
      <MessageBubble
        message={assistantMessage([])}
        isStreaming
        liveToolCalls={liveToolCalls}
      />,
    );

    expect(container.querySelectorAll('.websearch-section')).toHaveLength(2);
    expect(
      Array.from(
        container.querySelectorAll('.websearch-section-query'),
        (element) => element.textContent,
      ),
    ).toEqual(['Google Alphabet 最新业务', 'Alphabet GOOGL 股价']);
  });

  it('思考内容后到达时仍把工具保持在思考过程之前', () => {
    const { container } = render(
      <MessageBubble
        message={assistantMessage([
          {
            type: 'thinking',
            content: '根据搜索结果整理公司信息',
            is_open: true,
          },
        ])}
        isStreaming
        liveToolCalls={liveToolCalls}
      />,
    );

    const flow = container.querySelector('.assistant-content-flow');
    expect(flow?.children).toHaveLength(3);
    expect(flow?.children[0].classList.contains('websearch-section')).toBe(true);
    expect(flow?.children[1].classList.contains('websearch-section')).toBe(true);
    expect(flow?.children[2].classList.contains('think-section')).toBe(true);
    expect(
      flow?.children[2].classList.contains('websearch-section'),
    ).toBe(false);
  });
});

describe('MessageBubble 图片消息', () => {
  it('展示用户图片附件及文本', () => {
    const message: Message = {
      id: 'user-image',
      role: 'user',
      content: '请描述这张图片',
      images: [
        {
          id: 'image-1',
          name: 'sample.png',
          media_type: 'image/png',
          data_url: 'data:image/png;base64,aGVsbG8=',
        },
      ],
      model_id: null,
      created_at: 0,
    };

    const { getByAltText, getByText } = render(
      <MessageBubble message={message} />,
    );

    expect(getByAltText('sample.png').getAttribute('src')).toBe(
      'data:image/png;base64,aGVsbG8=',
    );
    expect(getByText('请描述这张图片')).toBeTruthy();
  });

  it('本地图片路径失效时显示已删除状态和原路径', () => {
    const message: Message = {
      id: 'user-missing-image',
      role: 'user',
      content: '',
      images: [
        {
          id: 'image-missing',
          name: 'missing.png',
          media_type: 'image/png',
          path: '/Users/test/Buddy/attachments/missing.png',
        },
      ],
      model_id: null,
      created_at: 0,
    };

    const { getByAltText, getByText } = render(
      <MessageBubble message={message} />,
    );
    fireEvent.error(getByAltText('missing.png'));

    // 有来源但加载失败：显示可重试的失败态（而不是误导性的"图片已删除"）
    expect(getByText('图片加载失败')).toBeTruthy();
    expect(getByText('重试')).toBeTruthy();
    expect(
      getByText('/Users/test/Buddy/attachments/missing.png'),
    ).toBeTruthy();
  });
});
