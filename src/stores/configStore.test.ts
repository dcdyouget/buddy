import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { AppConfig, ModelInfo, ProviderConfig } from '@/types';

const api = vi.hoisted(() => ({
  getConfig: vi.fn(),
  saveConfig: vi.fn(),
}));

vi.mock('@/api/config', () => api);

function createConfig(): AppConfig {
  return {
    theme: 'light',
    hotkey: 'CmdOrCtrl+J',
    providers: [
      {
        id: 'alpha',
        name: 'Alpha',
        base_url: 'https://alpha.example.com/v1',
        api_key: 'alpha-key',
        enabled_model_ids: ['alpha::chat', 'alpha::reasoner'],
        provider_type: 'openai_compatible',
      },
    ],
    models: [
      model('alpha', 'alpha::chat'),
      model('alpha', 'alpha::reasoner'),
    ],
    selected_model_id: 'alpha::chat',
    auto_start: false,
    allowed_paths: [],
    mcp_servers: [],
  };
}

function model(providerId: string, id: string): ModelInfo {
  const prefix = `${providerId}::`;
  return {
    id,
    provider_id: providerId,
    api_model_id: id.startsWith(prefix) ? id.slice(prefix.length) : undefined,
    display_name: id,
    context_window: 128_000,
    latency_ms: null,
    supports_vision: false,
    supports_image_generation: false,
  };
}

function provider(id: string, enabledModelIds: string[]): ProviderConfig {
  return {
    id,
    name: id,
    base_url: `https://${id}.example.com/v1`,
    api_key: `${id}-key`,
    enabled_model_ids: enabledModelIds,
    provider_type: 'openai_compatible',
  };
}

async function createStore() {
  Object.defineProperty(window, '__TAURI_INTERNALS__', {
    configurable: true,
    value: {},
  });
  vi.resetModules();
  const { useConfigStore } = await import('./configStore');
  useConfigStore.setState({
    config: createConfig(),
    loading: false,
    error: null,
  });
  return useConfigStore;
}

beforeEach(() => {
  api.getConfig.mockReset();
  api.saveConfig.mockReset().mockResolvedValue(undefined);
});

afterEach(() => {
  delete (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  vi.resetModules();
});

describe('configStore', () => {
  it('serializes rapid updates against the latest saved config', async () => {
    const store = await createStore();

    await Promise.all([
      store.getState().toggleModel('alpha::chat'),
      store.getState().toggleModel('alpha::reasoner'),
    ]);

    expect(api.saveConfig).toHaveBeenCalledTimes(2);
    expect(api.saveConfig.mock.calls[0][0].providers[0].enabled_model_ids).toEqual([
      'alpha::reasoner',
    ]);
    expect(api.saveConfig.mock.calls[1][0].providers[0].enabled_model_ids).toEqual([]);
    expect(store.getState().config?.providers[0].enabled_model_ids).toEqual([]);
  });

  it('keeps same-named models from different providers as separate entries', async () => {
    const store = await createStore();

    await store.getState().addProviderWithModels(
      provider('beta', ['chat']),
      [model('beta', 'chat')],
      'chat',
    );

    const config = store.getState().config!;
    expect(config.models.map((item) => item.id)).toEqual([
      'alpha::chat',
      'alpha::reasoner',
      'beta::chat',
    ]);
    expect(config.providers.find((item) => item.id === 'beta')?.enabled_model_ids).toEqual([
      'beta::chat',
    ]);
    expect(config.selected_model_id).toBe('beta::chat');
  });

  it('keeps the add-provider panel transaction pending on save failure', async () => {
    const store = await createStore();
    api.saveConfig.mockRejectedValueOnce(new Error('保存配置失败'));

    await expect(
      store.getState().addProviderWithModels(
        provider('beta', ['chat']),
        [model('beta', 'chat')],
        'chat',
      ),
    ).rejects.toThrow('保存配置失败');

    expect(store.getState().config?.providers.map((item) => item.id)).toEqual(['alpha']);
    expect(store.getState().error).toContain('保存配置失败');

    await store.getState().updateTheme('dark');
    expect(store.getState().config?.theme).toBe('dark');
  });
});
