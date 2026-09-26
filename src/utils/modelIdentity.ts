import type { AppConfig, ModelInfo } from '@/types';

export const MODEL_ID_SEPARATOR = '::';

/** 为配置和界面生成 Provider 范围内唯一的模型 ID。只接收 API 原始 ID。 */
export function scopedModelId(providerId: string, rawModelId: string): string {
  return `${providerId}${MODEL_ID_SEPARATOR}${rawModelId}`;
}

/** 把配置中的旧裸模型 ID 迁移成 Provider 范围内唯一的 ID。 */
export function normalizeModelIds(config: AppConfig): AppConfig {
  const previousSelectedId = config.selected_model_id;
  const previousModels = config.models.map((model) => ({
    providerId: model.provider_id,
    id: model.id,
    apiModelId: model.api_model_id ?? model.id,
  }));
  const models = config.models.map((model) => ({
    ...model,
    id: scopedModelId(model.provider_id, model.api_model_id ?? model.id),
    api_model_id: model.api_model_id ?? model.id,
  }));
  const providers = config.providers.map((provider) => ({
    ...provider,
    enabled_model_ids: provider.enabled_model_ids.map((modelId) => {
      const model = previousModels.find(
        (item) => item.providerId === provider.id
          && (item.id === modelId || item.apiModelId === modelId),
      );
      return scopedModelId(provider.id, model?.apiModelId ?? modelId);
    }),
  }));

  const selected = previousModels.find(
    (model) => model.id === previousSelectedId || model.apiModelId === previousSelectedId,
  );
  const selected_model_id = selected
    ? scopedModelId(selected.providerId, selected.apiModelId)
    : previousSelectedId;

  return { ...config, providers, models, selected_model_id };
}

/** 供只拿到模型元数据的调用方展示或请求原始模型名称。 */
export function rawModelId(model: Pick<ModelInfo, 'id' | 'api_model_id'>): string {
  return model.api_model_id ?? model.id;
}
