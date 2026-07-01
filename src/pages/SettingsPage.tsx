import { useState, useEffect, useCallback, useRef } from 'react';
import { ArrowLeft, Eye, EyeOff, ExternalLink } from 'lucide-react';
import { motion } from 'framer-motion';
import { invoke } from '@tauri-apps/api/core';
import { useConfigStore } from '@/stores/configStore';
import { GlassPanel } from '@/components/shared/GlassPanel';
import { FooterActions } from '@/components/shared/FooterActions';
import { ProviderCard } from '@/components/settings/ProviderCard';
import { SlideInPanel } from '@/components/shared/SlideInPanel';
import { useDragHandle } from '@/utils/windowDrag';
import { PROVIDER_PRESETS } from '@/types';

interface SettingsPageProps {
  onBack: () => void;
}
import type { ModelInfo, ProviderConfig } from '@/types';

/* ── SettingsPage ──────────────────────────────────────── */

/**
 * 设置页组件
 *
 * 提供应用的全局配置入口，包括：
 * 1. 主题切换（浅色 / 深色）
 * 2. 全局快捷键录制（按下组合键实时捕获）
 * 3. 模型管理（查看已添加模型、设置默认模型、添加新模型）
 * 4. 通过 SlideInPanel 滑入"添加模型"子面板
 *
 * 无 props —— 所需状态全部来自全局 store（configStore / uiStore）。
 */
export function SettingsPage({ onBack }: SettingsPageProps) {
  const dragRef = useDragHandle();
  const { config, updateTheme, updateHotkey, setDefaultModel } = useConfigStore();
  const [showAddProvider, setShowAddProvider] = useState(false);
  const [recording, setRecording] = useState(false);
  const [recordedKeys, setRecordedKeys] = useState<string[]>([]);
  // 用 ref 跟踪是否已按下主键（非修饰键），避免在释放修饰键时误触发热键保存
  const hasMainKey = useRef(false);

  // 配置未加载时渲染空（等待 config 初始化）
  if (!config) return null;

  /**
   * keydown 事件处理：录制模式下逐键累积修饰键和主键，实时展示当前按键组合
   * 使用 capture 阶段以在浏览器默认行为之前拦截按键
   */
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();

    const keys: string[] = [];
    // 修饰键统一转换为 Tauri 全局快捷键格式
    if (e.metaKey || e.ctrlKey) keys.push('CmdOrCtrl');
    if (e.shiftKey) keys.push('Shift');
    if (e.altKey) keys.push('Alt');

    const keyName = e.key;
    // 过滤纯修饰键，只将实际按键（字母/数字/符号等）加入组合
    if (!['Meta', 'Control', 'Shift', 'Alt'].includes(keyName)) {
      // 单字母转大写以统一显示格式
      keys.push(keyName.length === 1 ? keyName.toUpperCase() : keyName);
      hasMainKey.current = true;
    }

    if (keys.length > 0) {
      setRecordedKeys(keys);
    }
  }, [recording]);

  /**
   * keyup 事件处理：当用户释放非修饰键时，将累积的按键组合保存为热键
   * 如果所有修饰键都已释放且未按下主键，则清空显示
   */
  const handleKeyUp = useCallback((e: KeyboardEvent) => {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();

    const keyName = e.key;
    // 释放的是主键（非修饰键）且此前已记录过主键时，完成快捷键录制
    if (!['Meta', 'Control', 'Shift', 'Alt'].includes(keyName) && hasMainKey.current) {
      const keys: string[] = [];
      // 重新捕获当前仍按住的修饰键
      if (e.metaKey || e.ctrlKey) keys.push('CmdOrCtrl');
      if (e.shiftKey) keys.push('Shift');
      if (e.altKey) keys.push('Alt');

      const displayKey = keyName.length === 1 ? keyName.toUpperCase() : keyName;
      keys.push(displayKey);

      if (keys.length > 0) {
        updateHotkey(keys.join('+')); // 保存快捷键到配置
      }
      // 重置录制状态
      setRecording(false);
      setRecordedKeys([]);
      hasMainKey.current = false;
    }

    // 如果释放的是修饰键且未按下过主键，当所有修饰键都释放时清空显示
    if (['Meta', 'Control', 'Shift', 'Alt'].includes(keyName) && !hasMainKey.current) {
      if (!e.metaKey && !e.ctrlKey && !e.shiftKey && !e.altKey) {
        setRecordedKeys([]);
      }
    }
  }, [recording, updateHotkey]);

  // 录制模式下挂载全局键盘事件监听（capture 阶段）
  useEffect(() => {
    if (!recording) return;
    window.addEventListener('keydown', handleKeyDown, true);
    window.addEventListener('keyup', handleKeyUp, true);
    return () => {
      window.removeEventListener('keydown', handleKeyDown, true);
      window.removeEventListener('keyup', handleKeyUp, true);
    };
  }, [recording, handleKeyDown, handleKeyUp]);

  // 录制中展示实时按键；非录制时展示已保存的快捷键
  const displayKeys = (recording && recordedKeys.length > 0) ? recordedKeys : config.hotkey.split('+');

  /** 开始录制快捷键 */
  const startRecording = () => {
    setRecording(true);
    setRecordedKeys([]);
    hasMainKey.current = false;
  };

  return (
    <motion.div
      initial={{ x: '100%' }}
      animate={{ x: 0 }}
      exit={{ x: '100%' }}
      transition={{ type: 'tween', duration: 0.25, ease: [0.2, 0, 0, 1] }}
      ref={dragRef}
      style={{
        position: 'absolute',
        top: 0,
        left: 0,
        width: '100%',
        height: '100%',
        zIndex: 100,
        display: 'flex',
        background: 'transparent',
      }}
    >
      <GlassPanel
        style={{
          flex: 1,
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
          margin: 0,
          borderRadius: 'var(--radius-xl)',
        }}
      >
        {/* Header：返回按钮 + 标题 */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 'var(--space-2)',
            padding: 'var(--space-2) var(--space-4)',
            minHeight: '36px',
            borderBottom: '1px solid var(--border-subtle)',
          }}
        >
          <button
            onClick={onBack}
            style={{
              border: 'none',
              background: 'none',
              color: 'var(--text-muted)',
              cursor: 'pointer',
              padding: '4px',
              borderRadius: 'var(--radius-sm)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
            title="返回"
          >
            <ArrowLeft size={18} />
          </button>
          <h2 className="t-title" style={{ color: 'var(--text-primary)' }}>
            设置
          </h2>
        </div>

        {/* Content：可滚动的设置区域 */}
        <div
          className="no-scrollbar"
          style={{
            flex: 1,
            overflowY: 'auto',
            padding: 'var(--space-6)',
            display: 'flex',
            flexDirection: 'column',
            gap: 'var(--space-6)',
          }}
        >
          {/* ── 主题设置 ── */}
          <section>
            <h3 className="t-h3" style={{ color: 'var(--text-primary)', marginBottom: 'var(--space-3)' }}>
              主题
            </h3>
            <div style={{ display: 'flex', gap: 'var(--space-3)' }}>
              {(['light', 'dark'] as const).map((theme) => (
                <button
                  key={theme}
                  onClick={() => updateTheme(theme)}
                  style={{
                    padding: 'var(--space-2) var(--space-4)',
                    borderRadius: 'var(--radius-md)',
                    border:
                      config.theme === theme
                        ? '1px solid var(--buddy-primary)'
                        : '1px solid var(--border-default)',
                    background:
                      config.theme === theme
                        ? 'var(--primary-tint-soft)'
                        : 'var(--bg-elevated)',
                    color:
                      config.theme === theme
                        ? 'var(--buddy-primary)'
                        : 'var(--text-primary)',
                    cursor: 'pointer',
                    fontFamily: 'var(--font-sans)',
                    fontSize: '14px',
                    fontWeight: config.theme === theme ? 600 : 400,
                  }}
                >
                  {theme === 'light' ? '浅色' : '深色'}
                </button>
              ))}
            </div>
          </section>

          {/* ── 快捷键设置 ── */}
          <section>
            <h3 className="t-h3" style={{ color: 'var(--text-primary)', marginBottom: 'var(--space-3)' }}>
              快捷键
            </h3>
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 'var(--space-3)',
                outline: 'none',
              }}
            >
              {/* 快捷键展示：将按键数组渲染为 kbd 标签 */}
              <span style={{ display: 'inline-flex', gap: 'var(--space-1)', alignItems: 'center' }}>
                {displayKeys.map((key, i) => (
                  <kbd
                    key={i}
                    style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      minWidth: '24px',
                      height: '22px',
                      padding: '0 var(--space-1)',
                      borderRadius: 'var(--radius-sm)',
                      background: 'var(--bg-sunken)',
                      border: '1px solid var(--border-default)',
                      color: 'var(--text-primary)',
                      fontFamily: 'var(--font-sans)',
                      fontSize: '12px',
                      fontWeight: 500,
                    }}
                  >
                    {key}
                  </kbd>
                ))}
              </span>
              {/* 录制 / 重新录制按钮 */}
              <button
                onClick={startRecording}
                disabled={recording}
                style={{
                  padding: 'var(--space-1) var(--space-3)',
                  borderRadius: 'var(--radius-sm)',
                  border: recording
                    ? '1px solid var(--buddy-primary)'
                    : '1px solid var(--border-default)',
                  background: recording
                    ? 'var(--primary-tint-soft)'
                    : 'var(--bg-elevated)',
                  color: recording ? 'var(--buddy-primary)' : 'var(--text-primary)',
                  cursor: recording ? 'default' : 'pointer',
                  fontSize: '13px',
                  fontFamily: 'var(--font-sans)',
                  fontWeight: 500,
                }}
              >
                {recording ? '按下新快捷键...' : '重新录制'}
              </button>
            </div>
          </section>

          {/* ── 模型范围设置 ── */}
          <section>
            <h3 className="t-h3" style={{ color: 'var(--text-primary)', marginBottom: 'var(--space-3)' }}>
              模型范围
            </h3>
            {config.models.length === 0 ? (
              // 无模型时的空态提示
              <div className="t-body" style={{ color: 'var(--text-tertiary)', padding: 'var(--space-4) 0' }}>
                暂无模型，请点击下方按钮添加
              </div>
            ) : (
              // 模型列表：展示每个模型的名称、默认标记、设为默认按钮
              config.models.map((model) => {
                const isDefault = model.id === config.selected_model_id;
                return (
                  <div
                    key={model.id}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: 'var(--space-3)',
                      padding: 'var(--space-2) 0',
                    }}
                  >
                    {/* Provider 首字母图标 */}
                    <div
                      style={{
                        width: 24,
                        height: 24,
                        borderRadius: 'var(--radius-sm)',
                        background: 'var(--buddy-primary)',
                        color: 'var(--text-on-primary)',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        fontSize: '11px',
                        fontWeight: 700,
                        flexShrink: 0,
                      }}
                    >
                      {model.provider_id.charAt(0).toUpperCase()}
                    </div>
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                        <span className="t-body" style={{ fontWeight: 500, color: 'var(--text-primary)' }}>
                          {model.display_name}
                        </span>
                        {/* 默认模型标记 */}
                        {isDefault && (
                          <span
                            style={{
                              padding: '0 6px',
                              borderRadius: 'var(--radius-full)',
                              background: 'var(--buddy-primary)',
                              color: 'var(--text-on-primary)',
                              fontSize: '10px',
                              fontWeight: 600,
                            }}
                          >
                            默认
                          </span>
                        )}
                      </div>
                    </div>
                    {/* 非默认模型显示"设为默认"按钮 */}
                    {!isDefault && (
                      <button
                        onClick={() => setDefaultModel(model.id)}
                        style={{
                          padding: '2px 10px',
                          borderRadius: 'var(--radius-sm)',
                          border: '1px solid var(--border-default)',
                          background: 'var(--bg-elevated)',
                          color: 'var(--text-primary)',
                          cursor: 'pointer',
                          fontSize: '12px',
                          fontFamily: 'var(--font-sans)',
                          whiteSpace: 'nowrap',
                          flexShrink: 0,
                        }}
                      >
                        设为默认
                      </button>
                    )}
                  </div>
                );
              })
            )}
            {/* 添加模型按钮 */}
            <button
              onClick={() => setShowAddProvider(true)}
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
        </div>

        {/* 底部操作栏：取消/确定 */}
        <FooterActions
          onCancel={onBack}
          onConfirm={onBack}
          confirmLabel="确定"
        />
      </GlassPanel>

      {/* 添加 Provider 的侧滑面板 */}
      <SlideInPanel from="right" show={showAddProvider}>
        <AddProviderPageContent onBack={() => setShowAddProvider(false)} />
      </SlideInPanel>
    </motion.div>
  );
}

/* ── AddProviderPage ───────────────────────────────────── */

/** AddProviderPageContent 组件的 props */
interface AddProviderPageProps {
  /** 返回上一级（设置页）的回调 */
  onBack: () => void;
}

/**
 * 添加模型子页面内容组件
 *
 * 在设置页中通过 SlideInPanel 滑入展示，提供完整的 Provider 配置流程：
 * 1. 选择预设 Provider（如 DeepSeek、OpenAI 等）或自定义兼容服务
 * 2. 填写 Base URL 和 API Key
 * 3. 获取模型列表并勾选需要启用的模型
 * 4. 可选地对首个模型进行延迟测速
 * 5. 确认添加后将 provider 和模型写入配置
 */
function AddProviderPageContent({ onBack }: AddProviderPageProps) {
  const { addProvider, addModels } = useConfigStore();
  const [selectedPreset, setSelectedPreset] = useState<string | null>(null);
  const [baseUrl, setBaseUrl] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [showKey, setShowKey] = useState(false);
  const [fetchedModels, setFetchedModels] = useState<ModelInfo[]>([]);
  const [selectedModelIds, setSelectedModelIds] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
      const models = await invoke<ModelInfo[]>('fetch_models', { baseUrl, apiKey });
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
      const latency = await invoke<number>('test_latency', {
        baseUrl, apiKey, modelId: firstModel.id,
      });
      // 将测速结果写入对应模型的 latency_ms 字段
      setFetchedModels((prev) =>
        prev.map((m) => (m.id === firstModel.id ? { ...m, latency_ms: latency } : m)),
      );
    } catch (e) {
      setError(String(e));
    }
  };

  /** 确认添加：写入 provider 配置和选中的模型，然后返回设置页 */
  const handleAdd = async () => {
    if (!selectedPreset || !baseUrl || !apiKey) return;

    // "custom" is not in PROVIDER_PRESETS — handle it separately
    if (selectedPreset === 'custom') {
      const customId = `custom-${Date.now()}`;
      const provider: ProviderConfig = {
        id: customId,
        name: '自定义服务',
        base_url: baseUrl,
        api_key: apiKey,
        enabled_model_ids: Array.from(selectedModelIds),
      };
      const modelsToAdd = fetchedModels
        .filter((m) => selectedModelIds.has(m.id))
        .map((m) => ({ ...m, provider_id: customId }));
      await addProvider(provider);
      if (modelsToAdd.length > 0) await addModels(modelsToAdd);
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
    };
    const modelsToAdd = fetchedModels.filter((m) => selectedModelIds.has(m.id));

    await addProvider(provider);
    if (modelsToAdd.length > 0) await addModels(modelsToAdd);
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

  return (
    <GlassPanel
      style={{
        width: '100%',
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        borderRadius: 0,
      }}
    >
      {/* Header：返回按钮 + 关闭按钮 */}
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          padding: 'var(--space-4) var(--space-6)',
          borderBottom: '1px solid var(--border-subtle)',
        }}
      >
        <button
          onClick={onBack}
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
          返回设置
        </button>
        <button
          onClick={onBack}
          style={{
            border: 'none',
            background: 'none',
            color: 'var(--text-muted)',
            cursor: 'pointer',
            padding: '4px',
          }}
        >
          <ArrowLeft size={18} />
        </button>
      </div>

      {/* Content：可滚动的配置区域 */}
      <div
        className="no-scrollbar"
        style={{
          flex: 1,
          overflowY: 'auto',
          padding: 'var(--space-6)',
          display: 'flex',
          flexDirection: 'column',
          gap: 'var(--space-5)',
        }}
      >
        <h2 className="t-title" style={{ color: 'var(--text-primary)' }}>
          添加模型
        </h2>
        <p className="t-body" style={{ color: 'var(--text-muted)' }}>
          配置 Provider · 获取可用模型
        </p>

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
          + 自定义 OpenAI 兼容服务
        </button>

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
            {/* 切换 API Key 可见性 */}
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
            {/* 获取 Key 链接（当前为空链接，后续可跳转到提供商页面） */}
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
              <label
                key={model.id}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 'var(--space-2)',
                  padding: 'var(--space-1) 0',
                  cursor: 'pointer',
                }}
              >
                <input
                  type="checkbox"
                  checked={selectedModelIds.has(model.id)}
                  onChange={() => toggleModel(model.id)}
                  style={{ accentColor: 'var(--buddy-primary)', width: 16, height: 16 }}
                />
                <span className="t-body" style={{ color: 'var(--text-primary)' }}>
                  {model.display_name}
                </span>
                {/* 显示延迟数据（如有） */}
                {model.latency_ms != null && (
                  <span className="t-caption" style={{ color: 'var(--text-muted)' }}>
                    {model.latency_ms}ms
                  </span>
                )}
              </label>
            ))}
          </div>
        )}
      </div>

      {/* 底部操作栏：取消 / 确认添加 */}
      <FooterActions
        onCancel={onBack}
        onConfirm={handleAdd}
        confirmLabel="✓ 添加"
        confirmDisabled={!selectedPreset || !apiKey || selectedModelIds.size === 0}
      />
    </GlassPanel>
  );
}
