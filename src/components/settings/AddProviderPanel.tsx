import { useState } from 'react';
import { ArrowLeft, Eye, EyeOff, ExternalLink } from 'lucide-react';
import { useConfigStore } from '@/stores/configStore';
import { fetchModels, testLatency } from '@/api/provider';
import { GlassPanel } from '@/components/shared/GlassPanel';
import { FooterActions } from '@/components/shared/FooterActions';
import { ProviderCard } from '@/components/settings/ProviderCard';
import { PROVIDER_PRESETS } from '@/types';
import type { ModelInfo, ProviderConfig, ProviderType } from '@/types';

/** 上下文窗口预设选项（token 数） */
const CTX_PRESETS = [128_000, 256_000, 512_000, 1_000_000];

/** 获取下拉选项列表：预设 + 当前值（如果不在预设中） */
function getCtxOptions(current: number): number[] {
  const opts = [...CTX_PRESETS];
  if (!opts.includes(current)) opts.push(current);
  return opts.sort((a, b) => a - b);
}

/** 格式化 token 数为可读字符串（如 "128K"、"1.0M"） */
function formatContextWindow(tokens: number): string {
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(1)}M`;
  return `${Math.round(tokens / 1000)}K`;
}

interface AddProviderPanelProps {
  onBack: () => void;
}

/**
 * 添加模型子页面组件
 *
 * 在设置页中通过 SlideInPanel 滑入展示，提供完整的 Provider 配置流程：
 * 1. 选择预设 Provider（如 DeepSeek、Anthropic 等）或自定义兼容服务
 * 2. 填写 Base URL 和 API Key
 * 3. 获取模型列表并勾选需要启用的模型
 * 4. 可选地对首个模型进行延迟测速
 * 5. 确认添加后将 provider 和模型写入配置
 */
export function AddProviderPanel({ onBack }: AddProviderPanelProps) {
  const { addProvider, addModels, setDefaultModel } = useConfigStore();
  const [selectedPreset, setSelectedPreset] = useState<string | null>(null);
  const [baseUrl, setBaseUrl] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [fetchedModels, setFetchedModels] = useState<ModelInfo[]>([]);
  const [selectedModelIds, setSelectedModelIds] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [customProviderType, setCustomProviderType] = useState<ProviderType>('openai_compatible');

  /** 获取当前有效的 provider_type */
  const effectiveProviderType: ProviderType = selectedPreset === 'custom'
    ? customProviderType
    : (PROVIDER_PRESETS.find((p) => p.id === selectedPreset)?.provider_type || 'openai_compatible');

  /** 选择预设 Provider，自动填充其默认 Base URL */
  const handleSelectPreset = (presetId: string) => {
    setSelectedPreset(presetId);
    const preset = PROVIDER_PRESETS.find((p) => p.id === presetId);
    if (preset) setBaseUrl(preset.base_url);
  };

  /** 调用 Rust 后端 fetch_models 命令获取可用模型列表 */
  const handleFetchModels = async () => {
    if (!baseUrl || !apiKey) return;
    setLoading(true);
    setError(null);
    try {
      const models = await fetchModels(baseUrl, apiKey, effectiveProviderType);
      if (models.length === 0) {
        setError('未获取到模型列表（该厂商可能不支持 /models 端点）');
        return;
      }
      const modelsWithProvider = models.map((m) => ({
        ...m,
        provider_id: selectedPreset || 'custom',
      }));
      setFetchedModels(modelsWithProvider);
      setSelectedModelIds(new Set(modelsWithProvider.map((m) => m.id)));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  /** 调用 Rust 后端 test_latency 命令对首个模型进行延迟测试 */
  const handleTestLatency = async () => {
    if (!baseUrl || !apiKey || fetchedModels.length === 0) return;
    const firstModel = fetchedModels[0];
    try {
      const latency = await testLatency(baseUrl, apiKey, firstModel.id, effectiveProviderType);
      setFetchedModels((prev) =>
        prev.map((m) => (m.id === firstModel.id ? { ...m, latency_ms: latency } : m)),
      );
    } catch (e) {
      setError(String(e));
    }
  };

  /** 确认添加：写入 provider 配置和选中的模型，自动选中首个模型，然后返回设置页 */
  const handleAdd = async () => {
    if (!selectedPreset || !baseUrl || !apiKey) return;

    if (selectedPreset === 'custom') {
      const customId = `custom-${Date.now()}`;
      const provider: ProviderConfig = {
        id: customId,
        name: '自定义服务',
        base_url: baseUrl,
        api_key: apiKey,
        enabled_model_ids: Array.from(selectedModelIds),
        provider_type: effectiveProviderType,
        compat: undefined,
      };
      const modelsToAdd = fetchedModels
        .filter((m) => selectedModelIds.has(m.id))
        .map((m) => ({ ...m, provider_id: customId }));
      await addProvider(provider);
      if (modelsToAdd.length > 0) await addModels(modelsToAdd);
      // 自动选中第一个模型，避免出现「已配 Key 但仍提示无 Key」的情况
      const firstId = modelsToAdd[0]?.id;
      if (firstId) await setDefaultModel(firstId);
      onBack();
      return;
    }

    const preset = PROVIDER_PRESETS.find((p) => p.id === selectedPreset);
    if (!preset) return;

    const provider: ProviderConfig = {
      id: preset.id,
      name: preset.name,
      base_url: baseUrl,
      api_key: apiKey,
      enabled_model_ids: Array.from(selectedModelIds),
      provider_type: preset.provider_type,
      compat: preset.compat,
    };
    const modelsToAdd = fetchedModels.filter((m) => selectedModelIds.has(m.id));

    await addProvider(provider);
    if (modelsToAdd.length > 0) await addModels(modelsToAdd);
    // 自动选中第一个模型，避免出现「已配 Key 但仍提示无 Key」的情况
    const firstId = modelsToAdd[0]?.id;
    if (firstId) await setDefaultModel(firstId);
    onBack();
  };

  /** 切换单个模型的选中状态 */
  const toggleModel = (modelId: string) => {
    setSelectedModelIds((prev) => {
      const next = new Set(prev);
      next.has(modelId) ? next.delete(modelId) : next.add(modelId);
      return next;
    });
  };

  /** 更新单个模型的上下文窗口大小 */
  const updateContextWindow = (modelId: string, ctx: number) => {
    setFetchedModels((prev) =>
      prev.map((m) => (m.id === modelId ? { ...m, context_window: ctx } : m)),
    );
  };

  /** 用户手动声明模型是否支持图片输入 */
  const updateVisionSupport = (modelId: string, supportsVision: boolean) => {
    setFetchedModels((prev) =>
      prev.map((m) =>
        m.id === modelId ? { ...m, supports_vision: supportsVision } : m,
      ),
    );
  };

  /** 用户手动声明模型是否允许调用图片生成工具 */
  const updateImageGenerationSupport = (
    modelId: string,
    supportsImageGeneration: boolean,
  ) => {
    setFetchedModels((prev) =>
      prev.map((m) =>
        m.id === modelId
          ? {
              ...m,
              supports_image_generation: supportsImageGeneration,
            }
          : m,
      ),
    );
  };

  return (
    <GlassPanel
      className="buddy-shell provider-panel"
      style={{
        width: '100%',
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        borderRadius: 0,
      }}
    >
      {/* Header */}
      <div
        className="provider-header"
        style={{
          display: 'flex',
          alignItems: 'center',
          padding: 'var(--space-3) var(--space-4)',
        }}
      >
        <button
          onClick={onBack}
          title="返回设置"
          style={{
            border: 'none',
            background: 'none',
            color: 'var(--text-muted)',
            cursor: 'pointer',
            padding: '4px',
            display: 'flex',
            alignItems: 'center',
            gap: '4px',
            fontSize: '13px',
            fontFamily: 'var(--font-sans)',
          }}
        >
          <ArrowLeft size={16} />
        </button>
        <div>
          <h2 className="t-title" style={{ color: 'var(--text-primary)' }}>
            添加模型
          </h2>
          <p className="t-caption">连接模型服务并选择可用模型</p>
        </div>
      </div>

      {/* Content：可滚动的配置区域 */}
      <div
        className="no-scrollbar provider-content"
        style={{
          flex: 1,
          overflowY: 'auto',
          padding: 'var(--space-6)',
          display: 'flex',
          flexDirection: 'column',
          gap: 'var(--space-5)',
        }}
      >
        {/* Provider 预设选择：2×2 网格布局 */}
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 'var(--space-3)' }}>
          {PROVIDER_PRESETS.map((preset) => (
            <ProviderCard
              key={preset.id}
              id={preset.id}
              name={preset.name}
              iconLetter={preset.icon_letter}
              active={selectedPreset === preset.id}
              onSelect={() => handleSelectPreset(preset.id)}
              providerType={preset.provider_type}
            />
          ))}
        </div>

        {/* 自定义 OpenAI 兼容服务入口 */}
        <button
          onClick={() => { setSelectedPreset('custom'); setBaseUrl(''); }}
          style={{
            border: 'none',
            background: 'none',
            color: 'var(--buddy-primary)',
            cursor: 'pointer',
            fontSize: '13px',
            fontFamily: 'var(--font-sans)',
            textAlign: 'left',
            padding: 0,
          }}
        >
          + 自定义兼容服务
        </button>

        {/* 自定义 Provider 类型选择（仅自定义时显示） */}
        {selectedPreset === 'custom' && (
          <>
            <div>
              <label className="t-body-sm" style={{ display: 'block', color: 'var(--text-muted)', marginBottom: 'var(--space-1)' }}>
                Provider 类型
              </label>
              <select
                value={effectiveProviderType}
                onChange={(e) => {
                  setCustomProviderType(e.target.value as ProviderType);
                }}
                style={{
                  width: '100%',
                  padding: 'var(--space-2) var(--space-3)',
                  borderRadius: 'var(--radius-md)',
                  border: '1px solid var(--border-default)',
                  background: 'var(--bg-sunken)',
                  color: 'var(--text-primary)',
                  fontFamily: 'var(--font-sans)',
                  fontSize: '13px',
                  outline: 'none',
                }}
              >
                <option value="openai_compatible">OpenAI 兼容 (chat/completions)</option>
                <option value="anthropic">Anthropic (messages API)</option>
              </select>
            </div>
            {/* 自定义 Provider 的 compat 配置 */}
            <details style={{ fontSize: '12px' }}>
              <summary style={{ cursor: 'pointer', color: 'var(--text-muted)', marginBottom: 'var(--space-2)' }}>
                兼容性配置 (Compat)
              </summary>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--space-2)', paddingLeft: 'var(--space-2)' }}>
                {effectiveProviderType === 'openai_compatible' && (
                  <>
                    <label className="t-caption" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                      Thinking 格式
                      <select
                        defaultValue="openai"
                        style={{
                          padding: '2px 6px', borderRadius: 'var(--radius-sm)',
                          border: '1px solid var(--border-default)',
                          background: 'var(--bg-sunken)', color: 'var(--text-primary)',
                          fontSize: '11px', fontFamily: 'var(--font-sans)',
                        }}
                      >
                        <option value="openai">openai (reasoning_effort)</option>
                        <option value="deepseek">deepseek (thinking type)</option>
                        <option value="openrouter">openrouter (reasoning effort)</option>
                        <option value="qwen">qwen (enable_thinking)</option>
                        <option value="together">together (reasoning enabled)</option>
                        <option value="zai">zai (thinking type)</option>
                      </select>
                    </label>
                    <label className="t-caption" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                      Max Tokens 字段
                      <select
                        defaultValue="max_tokens"
                        style={{
                          padding: '2px 6px', borderRadius: 'var(--radius-sm)',
                          border: '1px solid var(--border-default)',
                          background: 'var(--bg-sunken)', color: 'var(--text-primary)',
                          fontSize: '11px', fontFamily: 'var(--font-sans)',
                        }}
                      >
                        <option value="max_tokens">max_tokens</option>
                        <option value="max_completion_tokens">max_completion_tokens</option>
                      </select>
                    </label>
                  </>
                )}
              </div>
            </details>
          </>
        )}

        {/* Base URL 输入 */}
        <div>
          <label className="t-body-sm" style={{ display: 'block', color: 'var(--text-muted)', marginBottom: 'var(--space-1)' }}>
            Base URL
          </label>
          <input
            type="text"
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="https://api.deepseek.com/v1"
            style={{
              width: '100%',
              padding: 'var(--space-2) var(--space-3)',
              borderRadius: 'var(--radius-md)',
              border: '1px solid var(--border-default)',
              background: 'var(--bg-sunken)',
              color: 'var(--text-primary)',
              fontFamily: 'var(--font-mono)',
              fontSize: '13px',
              outline: 'none',
            }}
          />
        </div>

        {/* API Key 输入（支持显示/隐藏切换） */}
        <div>
          <label className="t-body-sm" style={{ display: 'block', color: 'var(--text-muted)', marginBottom: 'var(--space-1)' }}>
            API Key
          </label>
          <div style={{ display: 'flex', gap: 'var(--space-2)' }}>
            <input
              type={showKey ? 'text' : 'password'}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="sk-..."
              style={{
                flex: 1,
                padding: 'var(--space-2) var(--space-3)',
                borderRadius: 'var(--radius-md)',
                border: '1px solid var(--border-default)',
                background: 'var(--bg-sunken)',
                color: 'var(--text-primary)',
                fontFamily: 'var(--font-mono)',
                fontSize: '13px',
                outline: 'none',
              }}
            />
            <button
              onClick={() => setShowKey(!showKey)}
              style={{
                padding: 'var(--space-2)',
                borderRadius: 'var(--radius-md)',
                border: '1px solid var(--border-default)',
                background: 'var(--bg-elevated)',
                color: 'var(--text-muted)',
                cursor: 'pointer',
                display: 'flex',
                alignItems: 'center',
              }}
            >
              {showKey ? <EyeOff size={16} /> : <Eye size={16} />}
            </button>
            <a
              href="#"
              onClick={(e) => e.preventDefault()}
              style={{
                display: 'flex',
                alignItems: 'center',
                color: 'var(--buddy-primary)',
                fontSize: '13px',
                textDecoration: 'none',
                whiteSpace: 'nowrap',
              }}
            >
              获取 Key
              <ExternalLink size={12} style={{ marginLeft: '2px' }} />
            </a>
          </div>
        </div>

        {/* 操作按钮：获取模型列表 + 测速 */}
        <div style={{ display: 'flex', gap: 'var(--space-3)' }}>
          <button
            onClick={handleFetchModels}
            disabled={loading}
            style={{
              flex: 1,
              padding: 'var(--space-2) var(--space-4)',
              borderRadius: 'var(--radius-md)',
              border: 'none',
              background: 'var(--buddy-primary)',
              color: 'var(--text-on-primary)',
              cursor: loading ? 'default' : 'pointer',
              fontFamily: 'var(--font-sans)',
              fontSize: '14px',
              fontWeight: 600,
              opacity: loading ? 0.6 : 1,
            }}
          >
            {loading ? '获取中...' : '获取模型列表'}
          </button>
          <button
            onClick={handleTestLatency}
            disabled={fetchedModels.length === 0}
            style={{
              padding: 'var(--space-2) var(--space-4)',
              borderRadius: 'var(--radius-md)',
              border: '1px solid var(--border-default)',
              background: 'var(--bg-elevated)',
              color: 'var(--text-primary)',
              cursor: fetchedModels.length === 0 ? 'default' : 'pointer',
              fontFamily: 'var(--font-sans)',
              fontSize: '14px',
              fontWeight: 500,
              opacity: fetchedModels.length === 0 ? 0.5 : 1,
            }}
          >
            测速
          </button>
        </div>

        {/* 错误信息展示 */}
        {error && (
          <div className="t-body-sm" style={{ color: 'var(--state-error)' }}>
            {error}
          </div>
        )}

        {/* 获取到的模型列表：支持多选勾选 */}
        {fetchedModels.length > 0 && (
          <div>
            <h4 className="t-h3" style={{ color: 'var(--text-primary)', marginBottom: 'var(--space-3)' }}>
              可用模型 ({fetchedModels.length})
            </h4>
            {fetchedModels.map((model) => (
              <div
                key={model.id}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 'var(--space-2)',
                  padding: 'var(--space-1) 0',
                }}
              >
                <label
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 'var(--space-2)',
                    cursor: 'pointer',
                    flex: 1,
                    minWidth: 0,
                  }}
                >
                  <input
                    type="checkbox"
                    checked={selectedModelIds.has(model.id)}
                    onChange={() => toggleModel(model.id)}
                    style={{ accentColor: 'var(--buddy-primary)', width: 16, height: 16, flexShrink: 0 }}
                  />
                  <span className="t-body" style={{ color: 'var(--text-primary)' }}>
                    {model.display_name}
                  </span>
                </label>
                {model.latency_ms != null && (
                  <span className="t-caption" style={{ color: 'var(--text-muted)', flexShrink: 0 }}>
                    {model.latency_ms}ms
                  </span>
                )}
                <label
                  title="开启后允许在聊天中向此模型发送图片"
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 'var(--space-1)',
                    color: 'var(--text-muted)',
                    fontSize: '11px',
                    whiteSpace: 'nowrap',
                    cursor: 'pointer',
                    flexShrink: 0,
                  }}
                >
                  <input
                    type="checkbox"
                    checked={model.supports_vision}
                    onChange={(event) =>
                      updateVisionSupport(model.id, event.target.checked)
                    }
                    style={{
                      width: 14,
                      height: 14,
                      accentColor: 'var(--buddy-primary)',
                      cursor: 'pointer',
                    }}
                  />
                  支持图片
                </label>
                <label
                  title={
                    effectiveProviderType === 'openai_compatible'
                      ? '开启后允许此模型调用图片生成工具；已适配厂商会自动使用对应原生生图接口'
                      : 'Anthropic 协议暂不支持图片生成'
                  }
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 'var(--space-1)',
                    color: 'var(--text-muted)',
                    fontSize: '11px',
                    whiteSpace: 'nowrap',
                    cursor:
                      effectiveProviderType === 'openai_compatible'
                        ? 'pointer'
                        : 'not-allowed',
                    flexShrink: 0,
                  }}
                >
                  <input
                    type="checkbox"
                    checked={
                      effectiveProviderType === 'openai_compatible' &&
                      model.supports_image_generation
                    }
                    disabled={effectiveProviderType !== 'openai_compatible'}
                    onChange={(event) =>
                      updateImageGenerationSupport(
                        model.id,
                        event.target.checked,
                      )
                    }
                    style={{
                      width: 14,
                      height: 14,
                      accentColor: 'var(--buddy-primary)',
                      cursor:
                        effectiveProviderType === 'openai_compatible'
                          ? 'pointer'
                          : 'not-allowed',
                    }}
                  />
                  支持生图
                </label>
                <select
                  value={model.context_window}
                  onChange={(e) => updateContextWindow(model.id, Number(e.target.value))}
                  style={{
                    padding: '2px 4px',
                    borderRadius: 'var(--radius-sm)',
                    border: '1px solid var(--border-default)',
                    background: 'var(--bg-sunken)',
                    color: 'var(--text-primary)',
                    fontSize: '11px',
                    fontFamily: 'var(--font-sans)',
                    flexShrink: 0,
                    cursor: 'pointer',
                  }}
                >
                  {getCtxOptions(model.context_window).map((opt) => (
                    <option key={opt} value={opt}>{formatContextWindow(opt)}</option>
                  ))}
                </select>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* 底部操作栏：取消 / 确认添加 */}
      <FooterActions
        onCancel={onBack}
        onConfirm={handleAdd}
        confirmLabel="添加"
        confirmDisabled={!selectedPreset || !apiKey || selectedModelIds.size === 0}
      />
    </GlassPanel>
  );
}
