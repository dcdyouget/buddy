// @vitest-environment jsdom

import { cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { ModelRow } from './ModelRow';

afterEach(cleanup);

describe('ModelRow 图片能力配置', () => {
  it('展示图片输入与生图开关并保存用户选择', () => {
    const onUpdateVision = vi.fn();
    const onUpdateImageGeneration = vi.fn();
    const { getByText } = render(
      <ModelRow
        model={{
          id: 'vision-model',
          provider_id: 'test-provider',
          display_name: 'Vision Model',
          context_window: 128_000,
          latency_ms: null,
          supports_vision: false,
          supports_image_generation: false,
        }}
        enabled
        isDefault
        onToggle={() => {}}
        onSetDefault={() => {}}
        onUpdateVision={onUpdateVision}
        onUpdateImageGeneration={onUpdateImageGeneration}
        canGenerateImages
      />,
    );

    const checkbox = getByText('支持图片')
      .closest('label')
      ?.querySelector('input[type="checkbox"]');
    expect(checkbox).toBeTruthy();
    fireEvent.click(checkbox!);
    expect(onUpdateVision).toHaveBeenCalledWith(true);

    const generationCheckbox = getByText('支持生图')
      .closest('label')
      ?.querySelector('input[type="checkbox"]');
    expect(generationCheckbox).toBeTruthy();
    fireEvent.click(generationCheckbox!);
    expect(onUpdateImageGeneration).toHaveBeenCalledWith(true);
  });

  it('Anthropic 模型禁用生图开关', () => {
    const { getByText } = render(
      <ModelRow
        model={{
          id: 'claude-model',
          provider_id: 'anthropic',
          display_name: 'Claude',
          context_window: 200_000,
          latency_ms: null,
          supports_vision: true,
          supports_image_generation: true,
        }}
        enabled
        isDefault
        onToggle={() => {}}
        onSetDefault={() => {}}
        onUpdateVision={() => {}}
        onUpdateImageGeneration={() => {}}
        canGenerateImages={false}
      />,
    );

    const checkbox = getByText('支持生图')
      .closest('label')
      ?.querySelector<HTMLInputElement>('input[type="checkbox"]');
    expect(checkbox?.disabled).toBe(true);
    expect(checkbox?.checked).toBe(false);
  });
});
