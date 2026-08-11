// @vitest-environment jsdom

import { cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { SettingsPage } from './SettingsPage';

vi.mock('framer-motion', () => ({
  motion: {
    div: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  },
}));

vi.mock('@/hooks/useDragHandle', () => ({
  useDragHandle: () => ({ current: null }),
}));

vi.mock('@/stores/configStore', () => ({
  useConfigStore: () => ({
    config: {
      theme: 'light',
      hotkey: 'CmdOrCtrl+J',
      providers: [],
      models: [],
      selected_model_id: '',
      auto_start: false,
      allowed_paths: [],
      mcp_servers: [],
    },
    updateTheme: vi.fn(),
    updateHotkey: vi.fn(),
    setDefaultModel: vi.fn(),
    toggleModel: vi.fn(),
    updateModel: vi.fn(),
  }),
}));

vi.mock('@/components/shared/GlassPanel', () => ({
  GlassPanel: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));

vi.mock('@/components/shared/SlideInPanel', () => ({
  SlideInPanel: ({
    children,
    show,
  }: {
    children: React.ReactNode;
    show: boolean;
  }) => (show ? <div>{children}</div> : null),
}));

vi.mock('@/components/settings/ThemeSetting', () => ({ ThemeSetting: () => null }));
vi.mock('@/components/settings/HotkeySetting', () => ({ HotkeySetting: () => null }));
vi.mock('@/components/settings/UpdateSetting', () => ({ UpdateSetting: () => null }));

vi.mock('@/components/settings/ModelList', () => ({
  ModelList: ({ onAddClick }: { onAddClick: () => void }) => (
    <button onClick={onAddClick}>打开添加模型</button>
  ),
}));

vi.mock('@/components/settings/AddProviderPanel', () => ({
  AddProviderPanel: ({
    onBack,
    onAdded,
  }: {
    onBack: () => void;
    onAdded: () => void;
  }) => (
    <div>
      <button onClick={onBack}>取消添加</button>
      <button onClick={onAdded}>完成添加</button>
    </div>
  ),
}));

afterEach(cleanup);

describe('SettingsPage', () => {
  it('取消添加时留在设置页，添加成功后返回对话页', () => {
    const onBack = vi.fn();
    const { getByRole, queryByRole } = render(<SettingsPage onBack={onBack} />);

    fireEvent.click(getByRole('button', { name: '打开添加模型' }));
    fireEvent.click(getByRole('button', { name: '取消添加' }));
    expect(onBack).not.toHaveBeenCalled();
    expect(queryByRole('button', { name: '完成添加' })).toBeNull();

    fireEvent.click(getByRole('button', { name: '打开添加模型' }));
    fireEvent.click(getByRole('button', { name: '完成添加' }));
    expect(onBack).toHaveBeenCalledTimes(1);
  });
});
