import { Sparkles, ChevronDown } from 'lucide-react';
import type { ModelInfo } from '@/types';

/** 模型选择器按钮的 Props */
interface ModelPickerProps {
  /** 当前选中的模型信息，为 null 时表示尚未选择 */
  model: ModelInfo | null;
  /** 点击按钮时的回调，通常用于打开模型下拉菜单 */
  onClick: () => void;
}

/**
 * 模型选择器
 * 在聊天输入框上方显示当前选中的模型名称，点击可弹出模型下拉菜单切换模型。
 * 渲染为一个紧凑的 pill 按钮，包含 sparkles 图标、模型名和下拉箭头。
 */
export function ModelPicker({ model, onClick }: ModelPickerProps) {
  return (
    <button
      onClick={onClick}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: '4px',
        padding: '2px 8px',
        borderRadius: 'var(--radius-full)',
        border: '1px solid var(--border-subtle)',
        background: 'var(--bg-elevated)',
        color: 'var(--text-primary)',
        cursor: 'pointer',
        fontFamily: 'var(--font-sans)',
        fontSize: '12px',
        fontWeight: 500,
        whiteSpace: 'nowrap',
        transition: `all var(--duration-fast) var(--ease-standard)`,
        flexShrink: 0,
      }}
    >
      {/* 品牌色 sparkles 图标 */}
      <Sparkles size={12} style={{ color: 'var(--buddy-primary)' }} />

      {/* 模型名称：已选择则显示 display_name，否则显示提示文字 */}
      <span>{model?.display_name || '选择模型'}</span>
      {/* 下拉箭头，提示可展开 */}
      <ChevronDown size={12} style={{ color: 'var(--text-muted)' }} />
    </button>
  );
}
