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
  toggleModel: (modelId: string) => Promise<void>;          // 切换模型启用状态
  setDefaultModel: (modelId: string) => Promise<void>;      // 设置默认模型
  removeProvider: (providerId: string) => Promise<void>;    // 删除提供商及其模型
  updateModel: (modelId: string, updates: Partial<ModelInfo>) => Promise<void>; // 更新模型字段
  updateHotkey: (hotkey: string) => Promise<void>;          // 更新快捷键
}

export const useConfigStore = create<ConfigState>((set, get) => ({
  config: null,
  loading: false,
  error: null,

  /** 加载配置：优先从 Rust 后端读取，首次启动时自动填充 mock 预设 */
  loadConfig: async () => {
    set({ loading: true, error: null });
    try {
      if (isBrowser) {
        // 浏览器模式：直接使用 mock 配置
        set({ config: { ...MOCK_CONFIG }, loading: false });
        return;
      }
      const { getConfig: getCfg } = await import('@/api/config');
      const config = await getCfg();

      set({ config, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  /** 保存完整配置到 Rust 后端 */
  saveConfig: async (config: AppConfig) => {
    set({ loading: true, error: null });
    try {
      const { saveConfig: saveCfg } = await import('@/api/config');
      await saveCfg(config);
      set({ config, loading: false });
    } catch (e) {
      // 保存失败：记录错误但不向调用方抛异常。
      // 内部各 action（updateTheme/addProvider/updateHotkey…）都不捕获，
      // 抛出去只会产生 unhandled rejection。配置保持为旧值，避免误显示已保存。
      set({ error: String(e), loading: false });
    }
  },

  /** 切换主题并立即持久化 */
  updateTheme: async (theme: Theme) => {
    const { config } = get();
    if (!config) return;
    const updated = { ...config, theme };
    await get().saveConfig(updated);
  },

  /**
   * 添加或更新服务提供商
   * 如果已存在同 id 的 provider，则先删除旧的再添加新的（实现编辑覆盖）
   */
  addProvider: async (provider: ProviderConfig) => {
    const { config } = get();
    if (!config) return;
    // 过滤掉同 id 的旧 provider，实现 upsert
    const providers = config.providers.filter((p) => p.id !== provider.id);
    providers.push(provider);
    const updated = { ...config, providers };
    await get().saveConfig(updated);
  },

  /**
   * 批量添加模型信息
   * 自动去重：已存在的模型 ID 不会重复添加
   */
  addModels: async (models: ModelInfo[]) => {
    const { config } = get();
    if (!config) return;
    // 使用 Set 快速去重
    const existingIds = new Set(config.models.map((m) => m.id));
    const newModels = models.filter((m) => !existingIds.has(m.id));
    const updated = { ...config, models: [...config.models, ...newModels] };
    await get().saveConfig(updated);
  },

  /** 切换指定模型的启用/禁用状态（通过 ProviderConfig.enabled_model_ids 管理） */
  toggleModel: async (modelId: string) => {
    const { config } = get();
    if (!config) return;

    // 1. 找到该模型所属的 provider
    const model = config.models.find((m) => m.id === modelId);
    if (!model) return;

    // 2. 找到对应的 ProviderConfig
    const providers = config.providers.map((p) => {
      if (p.id !== model.provider_id) return p;
      const enabled = p.enabled_model_ids.includes(modelId);
      const enabled_model_ids = enabled
        ? p.enabled_model_ids.filter((id) => id !== modelId)   // 已在列表中 → 移除（禁用）
        : [...p.enabled_model_ids, modelId];                    // 不在列表中 → 添加（启用）
      return { ...p, enabled_model_ids };
    });

    const enabledModelIds = new Set(
      providers.flatMap((provider) => provider.enabled_model_ids),
    );
    const selected_model_id = enabledModelIds.has(config.selected_model_id)
      ? config.selected_model_id
      : config.models.find((item) => enabledModelIds.has(item.id))?.id ?? '';

    await get().saveConfig({ ...config, providers, selected_model_id });
  },

  /** 设置当前选中的默认模型 */
  setDefaultModel: async (modelId: string) => {
    const { config } = get();
    if (!config) return;
    await get().saveConfig({ ...config, selected_model_id: modelId });
  },

  /**
   * 删除指定提供商及其所有关联模型
   * 如果当前选中的模型属于被删除的提供商，则清除选中状态
   */
  removeProvider: async (providerId: string) => {
    const { config } = get();
    if (!config) return;
    // 过滤掉该 provider 及其模型
    const providers = config.providers.filter((p) => p.id !== providerId);
    const models = config.models.filter((m) => m.provider_id !== providerId);
    // 如果当前选中的模型仍存在则保留，否则清空
    const selected_model_id =
      config.selected_model_id &&
      models.find((m) => m.id === config.selected_model_id)
        ? config.selected_model_id
        : '';
    await get().saveConfig({ ...config, providers, models, selected_model_id });
  },

  /** 更新指定模型的字段（如 context_window） */
  updateModel: async (modelId: string, updates: Partial<ModelInfo>) => {
    const { config } = get();
    if (!config) return;
    const models = config.models.map((m) =>
      m.id === modelId ? { ...m, ...updates } : m,
    );
    await get().saveConfig({ ...config, models });
  },

  /** 更新全局快捷键 */
  updateHotkey: async (hotkey: string) => {
    const { config } = get();
    if (!config) return;
    await get().saveConfig({ ...config, hotkey });
  },
}));
