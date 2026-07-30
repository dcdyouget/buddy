import { ModelRow } from '@/components/settings/ModelRow';
import { Plus } from 'lucide-react';
import type { ModelInfo, ProviderConfig } from '@/types';

interface ModelListProps {
  models: ModelInfo[];
  selectedModelId: string;
  onSetDefault: (modelId: string) => void;
  onAddClick: () => void;
  enabledModelIds: string[];
  onToggle: (modelId: string) => void;
  onUpdateModel?: (modelId: string, updates: Partial<ModelInfo>) => void;
  providers: ProviderConfig[];
}

/** 模型范围设置：展示已添加的模型列表 + 添加新模型入口 */
export function ModelList({
  models,
  selectedModelId,
  onSetDefault,
  onAddClick,
  enabledModelIds,
  onToggle,
  onUpdateModel,
  providers,
}: ModelListProps) {
  return (
    <section className="settings-section">
      <div className="settings-section-header">
        <div className="settings-copy">
          <h3>模型</h3>
          <p>选择可用模型与默认模型</p>
        </div>
      </div>
      <div className="model-list">
      {models.length === 0 ? (
        <div className="t-body" style={{ color: 'var(--text-tertiary)', padding: 'var(--space-4) 0' }}>
          暂无模型，请点击下方按钮添加
        </div>
      ) : (
        models.map((model) => {
          const isDefault = model.id === selectedModelId;
          const provider = providers.find(
            (candidate) => candidate.id === model.provider_id,
          );
          const canGenerateImages =
            provider?.provider_type === 'openai_compatible';
          return (
            <ModelRow
              key={model.id}
              model={model}
              enabled={enabledModelIds.includes(model.id)}
              isDefault={isDefault}
              onToggle={() => onToggle(model.id)}
              onSetDefault={() => onSetDefault(model.id)}
              onUpdateContextWindow={
                onUpdateModel
                  ? (ctx: number) => onUpdateModel(model.id, { context_window: ctx })
                  : undefined
              }
              onUpdateVision={
                (supportsVision: boolean) =>
                  onUpdateModel?.(model.id, { supports_vision: supportsVision })
              }
              onUpdateImageGeneration={
                (supportsImageGeneration: boolean) =>
                  onUpdateModel?.(model.id, {
                    supports_image_generation: supportsImageGeneration,
                  })
              }
              canGenerateImages={canGenerateImages}
            />
          );
        })
      )}
      </div>
      <button
        className="add-model-button"
        onClick={onAddClick}
        style={{
          width: '100%',
          padding: 'var(--space-3)',
          marginTop: 'var(--space-3)',
          borderRadius: 'var(--radius-md)',
          border: '1px dashed var(--border-default)',
          background: 'transparent',
          color: 'var(--text-muted)',
          cursor: 'pointer',
          fontFamily: 'var(--font-sans)',
          fontSize: '14px',
        }}
      >
        <Plus size={14} />
        添加模型
      </button>
    </section>
  );
}
