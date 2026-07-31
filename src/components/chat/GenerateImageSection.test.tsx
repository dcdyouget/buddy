// @vitest-environment jsdom

import { invoke } from '@tauri-apps/api/core';
import { cleanup, fireEvent, render, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GenerateImageSection } from './GenerateImageSection';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('GenerateImageSection', () => {
  it('显示生成结果图片与提示词详情', () => {
    const { getByAltText, getByRole, getByText } = render(
      <GenerateImageSection
        toolCall={{
          id: 'image-call',
          name: 'generate_image',
          arguments: JSON.stringify({ prompt: '一只戴围巾的猫' }),
          status: 'done',
          result: JSON.stringify({
            status: 'ok',
            model: 'gpt-image-2',
            prompt: '一只戴围巾的猫',
            image_count: 1,
          }),
          images: [
            {
              id: 'generated-1',
              name: 'generated-1.png',
              media_type: 'image/png',
              data_url: 'data:image/png;base64,aGVsbG8=',
            },
          ],
        }}
      />,
    );

    expect(getByText('图片生成完成')).toBeTruthy();
    expect(getByAltText('一只戴围巾的猫').getAttribute('src')).toBe(
      'data:image/png;base64,aGVsbG8=',
    );
    expect(getByRole('button', { name: '下载图片 1' })).toBeTruthy();

    fireEvent.click(getByText('图片生成完成'));
    expect(getByText('生成模型')).toBeTruthy();
    expect(getByText('gpt-image-2')).toBeTruthy();
  });

  it('点击下载后保存到系统下载目录并显示成功状态', async () => {
    vi.mocked(invoke).mockResolvedValue(
      '/Users/test/Downloads/Buddy-生成图片.png',
    );
    const { getByRole, getByText } = render(
      <GenerateImageSection
        toolCall={{
          id: 'image-call',
          name: 'generate_image',
          arguments: JSON.stringify({ prompt: '测试图片' }),
          status: 'done',
          images: [
            {
              id: 'generated-1',
              name: 'generated-1.png',
              media_type: 'image/png',
              data_url: 'data:image/png;base64,aGVsbG8=',
            },
          ],
        }}
      />,
    );

    fireEvent.click(getByRole('button', { name: '下载图片 1' }));

    await waitFor(() => expect(getByText('已下载')).toBeTruthy());
    expect(invoke).toHaveBeenCalledWith('download_generated_image', {
      image: {
        id: 'generated-1',
        name: 'generated-1.png',
        media_type: 'image/png',
        data_url: 'data:image/png;base64,aGVsbG8=',
      },
    });
  });

  it('可以复制生成提示词并显示反馈', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });
    const { getByRole, getByText } = render(
      <GenerateImageSection
        toolCall={{
          id: 'image-call',
          name: 'generate_image',
          arguments: JSON.stringify({ prompt: '蓝色星空下的机器人' }),
          status: 'done',
        }}
      />,
    );

    fireEvent.click(getByText('图片生成完成'));
    fireEvent.click(getByRole('button', { name: '复制生成提示词' }));

    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith('蓝色星空下的机器人');
      expect(
        getByRole('button', { name: '生成提示词已复制' }),
      ).toBeTruthy();
    });
  });
});
