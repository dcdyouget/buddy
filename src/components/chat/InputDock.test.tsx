// @vitest-environment jsdom

import { cleanup, fireEvent, render, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { ModelInfo } from '@/types';
import { InputDock } from './InputDock';

afterEach(cleanup);

const visionModel: ModelInfo = {
  id: 'vision-model',
  provider_id: 'test-provider',
  display_name: 'Vision Model',
  context_window: 128_000,
  latency_ms: null,
  supports_vision: true,
  supports_image_generation: false,
};

describe('InputDock 图片附件', () => {
  it('多模态模型显示图片入口并读取图片为 Data URL', async () => {
    const onAddImages = vi.fn();
    const { getByTitle, container } = render(
      <InputDock
        isStreaming={false}
        selectedModel={visionModel}
        draftInput=""
        draftImages={[]}
        onDraftChange={() => {}}
        onAddImages={onAddImages}
        onRemoveImage={() => {}}
        onSend={() => {}}
        onStop={() => {}}
      />,
    );

    expect(getByTitle('添加图片')).toBeTruthy();
    const input = container.querySelector('input[type="file"]');
    expect(input).toBeTruthy();

    fireEvent.change(input!, {
      target: {
        files: [new File(['image'], 'sample.png', { type: 'image/png' })],
      },
    });

    await waitFor(() => expect(onAddImages).toHaveBeenCalledTimes(1));
    expect(onAddImages.mock.calls[0][0][0]).toEqual(
      expect.objectContaining({
        name: 'sample.png',
        media_type: 'image/png',
      }),
    );
    expect(onAddImages.mock.calls[0][0][0].data_url).toMatch(
      /^data:image\/png;base64,/,
    );
  });

  it('纯文本模型不显示图片入口', () => {
    const { queryByTitle } = render(
      <InputDock
        isStreaming={false}
        selectedModel={{ ...visionModel, supports_vision: false }}
        draftInput=""
        draftImages={[]}
        onDraftChange={() => {}}
        onAddImages={() => {}}
        onRemoveImage={() => {}}
        onSend={() => {}}
        onStop={() => {}}
      />,
    );

    expect(queryByTitle('添加图片')).toBeNull();
  });
});
