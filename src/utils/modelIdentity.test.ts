import { describe, expect, it } from 'vitest';

import type { AppConfig } from '@/types';
import { normalizeModelIds, rawModelId, scopedModelId } from './modelIdentity';

const legacyConfig: AppConfig = {
  theme: 'light',
  hotkey: 'CmdOrCtrl+J',
  providers: [
    {
      id: 'openai', name: 'OpenAI', base_url: 'https://api.openai.com/v1', api_key: 'one',
      enabled_model_ids: ['gpt-4o'], provider_type: 'openai_compatible',
    },
    {
      id: 'relay', name: 'Relay', base_url: 'https://relay.example.com/v1', api_key: 'two',
      enabled_model_ids: ['gpt-4o'], provider_type: 'openai_compatible',
    },
  ],
  models: [
    {
      id: 'gpt-4o', provider_id: 'openai', display_name: 'GPT-4o', context_window: 128_000,
      latency_ms: null, supports_vision: true, supports_image_generation: false,
    },
    {
      id: 'gpt-4o', provider_id: 'relay', display_name: 'Relay GPT-4o', context_window: 64_000,
      latency_ms: null, supports_vision: false, supports_image_generation: false,
    },
  ],
  selected_model_id: 'gpt-4o',
  auto_start: false,
  allowed_paths: [],
  mcp_servers: [],
};

describe('model identity', () => {
  it('keeps same-named models from different providers independently routable', () => {
    const config = normalizeModelIds(legacyConfig);

    expect(config.models.map((model) => model.id)).toEqual([
      'openai::gpt-4o',
      'relay::gpt-4o',
    ]);
    expect(config.providers.map((provider) => provider.enabled_model_ids)).toEqual([
      ['openai::gpt-4o'],
      ['relay::gpt-4o'],
    ]);
    expect(config.selected_model_id).toBe('openai::gpt-4o');
    expect(rawModelId(config.models[1])).toBe('gpt-4o');
  });

  it('is idempotent and does not re-prefix scoped ids', () => {
    const once = normalizeModelIds(legacyConfig);
    expect(normalizeModelIds(once)).toEqual(once);
    expect(scopedModelId('openai', 'openai::gpt-4o')).toBe('openai::openai::gpt-4o');
  });

  it('round-trips a raw API id that starts with its provider prefix', () => {
    const rawId = 'openai::special';
    const config = normalizeModelIds({
      ...legacyConfig,
      providers: [{ ...legacyConfig.providers[0], enabled_model_ids: [rawId] }],
      models: [{ ...legacyConfig.models[0], id: rawId }],
      selected_model_id: rawId,
    });

    expect(config.models[0].id).toBe('openai::openai::special');
    expect(config.models[0].api_model_id).toBe(rawId);
    expect(rawModelId(config.models[0])).toBe(rawId);
    expect(config.providers[0].enabled_model_ids).toEqual(['openai::openai::special']);
    expect(config.selected_model_id).toBe('openai::openai::special');
    expect(normalizeModelIds(config)).toEqual(config);
  });
});
