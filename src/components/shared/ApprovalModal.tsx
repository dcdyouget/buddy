/**
 * ApprovalModal.tsx — Tool 执行审批弹窗
 *
 * 当后端发送 tool_approval_required 事件时，
 * useStreaming 会设置 toolApproval state。
 * 本组件监听该 state，显示「允许/本次都允许/拒绝」三个按钮。
 */

import { useEffect } from 'react';
import { useChatStore } from '@/stores/chatStore';
import { resolveApproval } from '@/hooks/useStreaming';

export function ApprovalModal() {
  const approval = useChatStore((s) => s.toolApproval);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        handleDeny();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  if (!approval) return null;

  const dismiss = () => {
    // 通过重新调用 handleToolApprovalRequired 传空值来清除
    useChatStore.getState().setToolApproval(null);
  };

  const handleAllow = () => {
    resolveApproval(true, false);
    dismiss();
  };

  const handleAllowAll = () => {
    resolveApproval(true, true);
    dismiss();
  };

  const handleDeny = () => {
    resolveApproval(false, false);
    dismiss();
  };

  // 需要清空 toolApproval:直接通过 getState 修改

  return (
    <div
      style={{
        position: 'fixed',
        bottom: 80,
        left: '50%',
        transform: 'translateX(-50%)',
        background: 'var(--glass-bg, rgba(255,255,255,0.9))',
        backdropFilter: 'blur(16px)',
        border: '1px solid var(--border, #ddd)',
        borderRadius: 12,
        padding: '16px 20px',
        zIndex: 1000,
        minWidth: 320,
        maxWidth: 480,
        boxShadow: '0 4px 24px rgba(0,0,0,0.2)',
      }}
    >
      <div style={{ marginBottom: 8, fontSize: 13, fontWeight: 600 }}>
        工具调用审批
      </div>
      <div style={{ marginBottom: 6, fontSize: 13, color: 'var(--fg-secondary, #666)' }}>
        要修改的文件: {approval.name}
      </div>
      <div
        style={{
          marginBottom: 14,
          fontSize: 11,
          fontFamily: 'monospace',
          color: 'var(--fg-muted, #999)',
          background: 'var(--code-bg, #f5f5f5)',
          padding: '8px 10px',
          borderRadius: 6,
          maxHeight: 120,
          overflow: 'auto',
          whiteSpace: 'pre-wrap',
          wordBreak: 'break-all',
        }}
      >
        {approval.reason}
      </div>
      <div style={{ display: 'flex', gap: 8 }}>
        <button
          onClick={() => { handleAllow(); }}
          style={{
            flex: 1,
            padding: '8px 0',
            border: '1px solid var(--brand, #5B5FE9)',
            background: 'var(--brand, #5B5FE9)',
            color: 'var(--fg-on-brand, #fff)',
            borderRadius: 8,
            cursor: 'pointer',
            fontSize: 13,
          }}
        >
          允许
        </button>
        <button
          onClick={() => { handleAllowAll(); }}
          style={{
            flex: 1,
            padding: '8px 0',
            border: '1px solid var(--brand, #5B5FE9)',
            background: 'transparent',
            color: 'var(--brand, #5B5FE9)',
            borderRadius: 8,
            cursor: 'pointer',
            fontSize: 13,
          }}
        >
          本次都允许
        </button>
        <button
          onClick={() => { handleDeny(); }}
          style={{
            padding: '8px 16px',
            border: '1px solid var(--border, #ddd)',
            background: 'transparent',
            color: 'var(--fg, #333)',
            borderRadius: 8,
            cursor: 'pointer',
            fontSize: 13,
          }}
        >
          拒绝
        </button>
      </div>
    </div>
  );
}
