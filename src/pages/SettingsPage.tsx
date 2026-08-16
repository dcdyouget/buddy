import { useState } from 'react';
import { ArrowLeft } from 'lucide-react';
import { motion } from 'framer-motion';
import { useShallow } from 'zustand/react/shallow';
import { useConfigStore } from '@/stores/configStore';
import { GlassPanel } from '@/components/shared/GlassPanel';
import { SlideInPanel } from '@/components/shared/SlideInPanel';
import { ThemeSetting } from '@/components/settings/ThemeSetting';
import { HotkeySetting } from '@/components/settings/HotkeySetting';
import { UpdateSetting } from '@/components/settings/UpdateSetting';
import { ModelList } from '@/components/settings/ModelList';
import { AddProviderPanel } from '@/components/settings/AddProviderPanel';
import { useDragHandle } from '@/hooks/useDragHandle';

interface SettingsPageProps {
  onBack: () => void;
}

/**
 * 设置页组件
 *
 * 作为编排层，组合各设置子组件：
 * - ThemeSetting：主题切换
 * - HotkeySetting：快捷键录制（委托 HotkeyRecorder 组件）
 * - UpdateSetting：手动检查、下载并安装应用更新
 * - ModelList：模型管理（委托 ModelRow 组件）
 * - AddProviderPanel：侧滑添加模型面板
 */
export function SettingsPage({ onBack }: SettingsPageProps) {
  const dragRef = useDragHandle();
  const {
    config,
    updateTheme,
    updateHotkey,
    setDefaultModel,
    toggleModel,
    updateModel,
  } = useConfigStore(
    useShallow((state) => ({
      config: state.config,
      updateTheme: state.updateTheme,
      updateHotkey: state.updateHotkey,
      setDefaultModel: state.setDefaultModel,
      toggleModel: state.toggleModel,
      updateModel: state.updateModel,
    })),
  );
  const [showAddProvider, setShowAddProvider] = useState(false);

  if (!config) return null;

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
        className="buddy-shell"
        style={{
          flex: 1,
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
          margin: 0,
          borderRadius: 'var(--radius-xl)',
        }}
      >
        {/* Header */}
        <div
          className="settings-header"
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 'var(--space-2)',
            padding: 'var(--space-2) var(--space-4)',
            minHeight: '36px',
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
          <div>
            <h2 className="t-title" style={{ color: 'var(--text-primary)' }}>
              设置
            </h2>
          </div>
        </div>

        {/* Content */}
        <div
          className="no-scrollbar settings-content"
          style={{
            flex: 1,
            overflowY: 'auto',
            padding: 'var(--space-4)',
            display: 'flex',
            flexDirection: 'column',
            gap: 'var(--space-3)',
          }}
        >
          <ThemeSetting theme={config.theme} onThemeChange={updateTheme} />
          <HotkeySetting hotkey={config.hotkey} onHotkeyChange={updateHotkey} />
          <UpdateSetting />
          <ModelList
            models={config.models}
            selectedModelId={config.selected_model_id}
            onSetDefault={setDefaultModel}
            onAddClick={() => setShowAddProvider(true)}
            enabledModelIds={config.providers.flatMap((provider) => provider.enabled_model_ids)}
            onToggle={toggleModel}
            onUpdateModel={updateModel}
            providers={config.providers}
          />
        </div>

      </GlassPanel>

      <SlideInPanel from="right" show={showAddProvider}>
        <AddProviderPanel
          onBack={() => setShowAddProvider(false)}
          onAdded={onBack}
        />
      </SlideInPanel>
    </motion.div>
  );
}
