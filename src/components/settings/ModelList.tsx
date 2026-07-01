import { ModelRow } from '@/components/settings/ModelRow';
import type { ModelInfo } from '@/types';

interface ModelListProps {
  models: ModelInfo[];
  selectedModelId: string;
  onSetDefault: (modelId: string) => void;
  onAddClick: () => void;
}

/** 模型范围设置：展示已添加的模型列表 + 添加新模型入口 */
export function ModelList({ models, selectedModelId, onSetDefault, onAddClick }: ModelListProps) {
  return (
    <section>
      <h3 className="t-h3" style={{ color: 'var(--text-primary)', marginBottom: 'var(--space-3)' }}>
        模型范围
      </h3>
      {models.length === 0 ? (
        <div className="t-body" style={{ color: 'var(--text-tertiary)', padding: 'var(--space-4) 0' }}>
          暂无模型，请点击下方按钮添加
        </div>
      ) : (
        models.map((model) => {
          const isDefault = model.id === selectedModelId;
          return (
            <ModelRow
              key={model.id}
              model={model}
              enabled={true}
              isDefault={isDefault}
              onToggle={() => {}}
              onSetDefault={() => onSetDefault(model.id)}
            />
          );
        })
      )}
      <button
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
        + 添加模型
      </button>
    </section>
  );
}
