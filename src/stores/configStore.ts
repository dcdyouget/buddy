/**
 * configStore.ts — 配置状态管理
 *
 * 管理应用的全局配置，包括：
 * - 主题（亮色/暗色）
 * - 快捷键
 * - 服务提供商（API Key、base_url）
 * - 模型列表（启用/禁用、默认模型）
 *
 * 所有持久化操作通过 Tauri invoke 调用 Rust 后端的 get_config / save_config。
 * 浏览器环境下使用 MOCK_CONFIG 回退，方便前端独立开发调试。
 */

import { create } from 'zustand';
import type { AppConfig, Theme, ProviderConfig, ModelInfo } from '@/types';
import { isBrowser, MOCK_CONFIG } from '@/utils/mock';
import { normalizeModelIds, scopedModelId } from '@/utils/modelIdentity';

/** ConfigStore 状态和操作定义 */
interface ConfigState {
  config: AppConfig | null;  // 当前配置，null 表示尚未加载
  loading: boolean;           // 是否正在加载/保存中
  error: string | null;       // 最近的错误信息

  // ── 操作 ──
  loadConfig: () => Promise<void>;                          // 加载配置
  saveConfig: (config: AppConfig) => Promise<void>;         // 保存完整配置
  updateTheme: (theme: Theme) => Promise<void>;             // 切换主题
  addProvider: (provider: ProviderConfig) => Promise<void>; // 添加/更新提供商
  addModels: (models: ModelInfo[]) => Promise<void>;        // 批量添加模型（去重）
  addProviderWithModels: (
    provider: ProviderConfig,
    models: ModelInfo[],
    defaultModelId?: string,
  ) => Promise<void>; // 添加服务及模型（单次保存）
  toggleModel: (modelId: string) => Promise<void>;          // 切换模型启用状态
  setDefaultModel: (modelId: string) => Promise<void>;      // 设置默认模型
  removeProvider: (providerId: string) => Promise<void>;    // 删除提供商及其模型
  updateModel: (modelId: string, updates: Partial<ModelInfo>) => Promise<void>; // 更新模型字段
  updateHotkey: (hotkey: string) => Promise<void>;          // 更新快捷键
}

export const useConfigStore = create<ConfigState>((set, get) => {
  // 配置更新必须串行：每个操作开始时读取上一个操作保存后的最新状态，
  // 防止快速切换两个模型时第二次保存覆盖第一次的结果。
  let configUpdateQueue: Promise<void> = Promise.resolve();

  const enqueueConfigUpdate = (
    update: (config: AppConfig) => AppConfig,
    throwOnError = false,
  ): Promise<void> => {
    const operation = configUpdateQueue.then(async () => {
      const currentConfig = get().config;
      if (!currentConfig) {
        const error = new Error('配置尚未加载完成');
        set({ error: String(error) });
        if (throwOnError) throw error;
        return;
      }

      const nextConfig = normalizeModelIds(update(currentConfig));
      set({ loading: true, error: null });
      try {
        const { saveConfig: saveCfg } = await import('@/api/config');
        await saveCfg(nextConfig);
        set({ config: nextConfig, loading: false });
      } catch (e) {
        set({ error: String(e), loading: false });
        if (throwOnError) throw e;
      }
    });

    // 当前操作失败时，后续用户操作仍可继续保存。
    configUpdateQueue = operation.catch(() => undefined);
    return operation;
  };

  return {
  config: null,
  loading: false,
  error: null,

  /** 加载配置：优先从 Rust 后端读取，首次启动时自动填充 mock 预设 */
  loadConfig: async () => {
    set({ loading: true, error: null });
    try {
      if (isBrowser) {
        // 浏览器模式：直接使用 mock 配置
        set({ config: normalizeModelIds(MOCK_CONFIG), loading: false });
        return;
      }
      const { getConfig: getCfg } = await import('@/api/config');
      const config = await getCfg();

      set({ config: normalizeModelIds(config), loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  /** 保存完整配置到 Rust 后端 */
  saveConfig: async (config: AppConfig) => {
    await enqueueConfigUpdate(() => config);
  },

  /** 切换主题并立即持久化 */
  updateTheme: async (theme: Theme) => {
    await enqueueConfigUpdate((config) => ({ ...config, theme }));
  },

  /**
   * 添加或更新服务提供商
   * 如果已存在同 id 的 provider，则先删除旧的再添加新的（实现编辑覆盖）
   */
  addProvider: async (provider: ProviderConfig) => {
    await enqueueConfigUpdate((config) => {
      const normalizedProvider = {
        ...provider,
        enabled_model_ids: provider.enabled_model_ids.map((modelId) =>
          scopedModelId(provider.id, modelId),
        ),
      };
      const providers = config.providers.filter((item) => item.id !== provider.id);
      providers.push(normalizedProvider);
      return { ...config, providers };
    });
  },

  /**
   * 批量添加模型信息
   * 自动去重：已存在的模型 ID 不会重复添加
   */
  addModels: async (models: ModelInfo[]) => {
    await enqueueConfigUpdate((config) => {
      const existingIds = new Set(config.models.map((model) => model.id));
      const newModels = models
        .map((model) => {
          const api_model_id = model.api_model_id ?? model.id;
          return {
            ...model,
            id: scopedModelId(model.provider_id, api_model_id),
            api_model_id,
          };
        })
        .filter((model) => {
          if (existingIds.has(model.id)) return false;
          existingIds.add(model.id);
          return true;
        });
      return { ...config, models: [...config.models, ...newModels] };
    });
  },

  /** 添加服务与模型只写入一次，避免中间状态覆盖或半配置成功。 */
  addProviderWithModels: async (
    provider: ProviderConfig,
    models: ModelInfo[],
    defaultModelId?: string,
  ) => {
    await enqueueConfigUpdate((config) => {
      const normalizedProvider = {
        ...provider,
        enabled_model_ids: provider.enabled_model_ids.map((modelId) =>
          scopedModelId(provider.id, modelId),
        ),
      };
      const providers = config.providers.filter((item) => item.id !== provider.id);
      providers.push(normalizedProvider);

      const existingIds = new Set(config.models.map((model) => model.id));
      const newModels = models
        .map((model) => {
          const api_model_id = model.api_model_id ?? model.id;
          return {
            ...model,
            provider_id: provider.id,
            id: scopedModelId(provider.id, api_model_id),
            api_model_id,
          };
        })
        .filter((model) => {
          if (existingIds.has(model.id)) return false;
          existingIds.add(model.id);
          return true;
        });
      const selected_model_id = defaultModelId
        ? scopedModelId(provider.id, defaultModelId)
        : config.selected_model_id;

      return {
        ...config,
        providers,
        models: [...config.models, ...newModels],
        selected_model_id,
      };
    }, true);
  },

  /** 切换指定模型的启用/禁用状态（通过 ProviderConfig.enabled_model_ids 管理） */
  toggleModel: async (modelId: string) => {
    await enqueueConfigUpdate((config) => {
      const model = config.models.find((item) => item.id === modelId);
      if (!model) return config;

      const providers = config.providers.map((provider) => {
        if (provider.id !== model.provider_id) return provider;
        const enabled = provider.enabled_model_ids.includes(modelId);
        const enabled_model_ids = enabled
          ? provider.enabled_model_ids.filter((id) => id !== modelId)
          : [...provider.enabled_model_ids, modelId];
        return { ...provider, enabled_model_ids };
      });
      const enabledModelIds = new Set(
        providers.flatMap((provider) => provider.enabled_model_ids),
      );
      const selected_model_id = enabledModelIds.has(config.selected_model_id)
        ? config.selected_model_id
        : config.models.find((item) => enabledModelIds.has(item.id))?.id ?? '';

      return { ...config, providers, selected_model_id };
    });
  },

  /** 设置当前选中的默认模型 */
  setDefaultModel: async (modelId: string) => {
    await enqueueConfigUpdate((config) => ({ ...config, selected_model_id: modelId }));
  },

  /**
   * 删除指定提供商及其所有关联模型
   * 如果当前选中的模型属于被删除的提供商，则清除选中状态
   */
  removeProvider: async (providerId: string) => {
    await enqueueConfigUpdate((config) => {
      const providers = config.providers.filter((provider) => provider.id !== providerId);
      const models = config.models.filter((model) => model.provider_id !== providerId);
      const selected_model_id = models.some((model) => model.id === config.selected_model_id)
        ? config.selected_model_id
        : '';
      return { ...config, providers, models, selected_model_id };
    });
  },

  /** 更新指定模型的字段（如 context_window） */
  updateModel: async (modelId: string, updates: Partial<ModelInfo>) => {
    await enqueueConfigUpdate((config) => {
      const { id: _ignoredId, provider_id: _ignoredProviderId, ...modelUpdates } = updates;
      const models = config.models.map((model) =>
        model.id === modelId ? { ...model, ...modelUpdates } : model,
      );
      return { ...config, models };
    });
  },

  /** 更新全局快捷键 */
  updateHotkey: async (hotkey: string) => {
    await enqueueConfigUpdate((config) => ({ ...config, hotkey }));
  },
  };
});
