import { cleanup, fireEvent, render, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { ModelInfo } from '@/types';
import { AddProviderPanel } from './AddProviderPanel';

const store = vi.hoisted(() => ({
  addProviderWithModels: vi.fn(),
}));
const providerApi = vi.hoisted(() => ({
  fetchModels: vi.fn(),
  testLatency: vi.fn(),
}));

vi.mock('@/stores/configStore', () => ({
  useConfigStore: <T,>(selector: (state: typeof store) => T) => selector(store),
}));

vi.mock('@/api/provider', () => providerApi);

const fetchedModel: ModelInfo = {
  id: 'gpt-4o',
  provider_id: '',
  display_name: 'GPT-4o',
  context_window: 128_000,
  latency_ms: null,
  supports_vision: false,
  supports_image_generation: false,
};

beforeEach(() => {
  store.addProviderWithModels.mockReset();
  providerApi.fetchModels.mockReset().mockResolvedValue([fetchedModel]);
  providerApi.testLatency.mockReset();
});

afterEach(cleanup);

describe('AddProviderPanel', () => {
  it('shows a save error and does not leave settings when the atomic add fails', async () => {
    store.addProviderWithModels.mockRejectedValueOnce(new Error('保存配置失败'));
    const onAdded = vi.fn();
    const { container, getByRole, getByText } = render(
      <AddProviderPanel onBack={() => {}} onAdded={onAdded} />,
    );

    fireEvent.click(getByRole('button', { name: /DeepSeek/ }));
    fireEvent.change(container.querySelector('input[placeholder="sk-..."]')!, {
      target: { value: 'test-key' },
    });
    fireEvent.click(getByRole('button', { name: '获取模型列表' }));
    await waitFor(() => expect(getByText('GPT-4o')).toBeTruthy());

    fireEvent.click(getByRole('button', { name: '添加' }));

    await waitFor(() => {
      expect(getByText('Error: 保存配置失败')).toBeTruthy();
    });
    expect(onAdded).not.toHaveBeenCalled();
    expect(store.addProviderWithModels).toHaveBeenCalledTimes(1);
  });
});
