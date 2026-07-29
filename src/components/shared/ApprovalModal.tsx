/**
 * ApprovalModal.tsx — Tool 执行审批弹窗
 *
 * 与工具交互卡片共享统一视觉语言：
 * - 玻璃面板 + backdrop blur + 淡入动画
 * - header icon + 标题
 * - 横向并排操作按钮
 * - 底部补充信息(Esc 提示)
 */

import { useEffect } from 'react';
import { Shield, ShieldCheck, ShieldX } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { useChatStore } from '@/stores/chatStore';
import { resolveApproval } from '@/hooks/useStreaming';

export function ApprovalModal() {
  const approval = useChatStore((s) => s.toolApproval);

  useEffect(() => {
    if (!approval) return;
    const onKey = (e: Event) => {
      if ((e as KeyboardEvent).key === 'Escape') handleDeny();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [approval]);

  if (!approval) return null;

  const dismiss = () => useChatStore.getState().setToolApproval(null);
  const handleAllow = () => { resolveApproval(true, false); dismiss(); };
  const handleAllowAll = () => { resolveApproval(true, true); dismiss(); };
  const handleDeny = () => { resolveApproval(false, false); dismiss(); };

  return (
    <AnimatePresence>
      {approval && (
        <motion.div
          className="tool-interaction-layer"
          initial={{ opacity: 0, y: 8, scale: 0.99 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: 6, scale: 0.99 }}
          transition={{ duration: 0.2, ease: 'easeOut' }}
        >
        <div
          className="tool-interaction-shell"
          style={{
            width: 440,
            maxWidth: '100%',
          }}
        >
          <div
            className="tool-interaction-panel"
          >
            {/* ── Header ── */}
            <div
              className="tool-interaction-header"
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 'var(--space-3)',
                padding: 'var(--space-4) var(--space-4) var(--space-2)',
              }}
            >
              <div
                className="tool-interaction-icon"
                style={{
                  width: 28,
                  height: 28,
                  borderRadius: 'var(--radius-md)',
                  background: 'var(--tool-ui-accent-soft)',
                  color: 'var(--tool-ui-accent-strong)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  flexShrink: 0,
                }}
              >
                <Shield size={16} />
              </div>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div
                  className="tool-interaction-title"
                  style={{
                    fontSize: 'var(--font-size-md)',
                    fontWeight: 600,
                    color: 'var(--text-primary)',
                    letterSpacing: 'var(--letter-spacing-tight)',
                  }}
                >
                  工具调用审批
                </div>
                <div
                  style={{
                    fontSize: 'var(--font-size-sm)',
                    color: 'var(--text-muted)',
                    marginTop: 1,
                    fontFamily: 'var(--font-mono)',
                  }}
                >
                  {approval.name}
                </div>
              </div>
            </div>

            {/* ── 参数预览 ── */}
            <div
              className="tool-interaction-preview"
              style={{
                margin: '0 var(--space-4) var(--space-3)',
                padding: 'var(--space-2) var(--space-3)',
                background: 'var(--bg-sunken)',
                borderRadius: 'var(--radius-md)',
                fontSize: 'var(--font-size-xs)',
                fontFamily: 'var(--font-mono)',
                color: 'var(--text-muted)',
                maxHeight: 100,
                overflow: 'auto',
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-all',
                lineHeight: 1.5,
              }}
            >
              {approval.reason}
            </div>

            {/* ── 操作按钮(横向并排) ── */}
            <div
              style={{
                display: 'flex',
                gap: 'var(--space-2)',
                padding: '0 var(--space-4) var(--space-4)',
              }}
            >
              <button
                className="tool-action-button"
                onClick={handleDeny}
                style={{
                  flex: 1,
                  padding: '8px 0',
                  border: '1px solid var(--border-default)',
                  background: 'transparent',
                  color: 'var(--text-muted)',
                  borderRadius: 'var(--radius-md)',
                  cursor: 'pointer',
                  fontSize: 'var(--font-size-sm)',
                  fontWeight: 500,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  gap: 5,
                  transition: 'all 0.12s',
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = 'var(--bg-sunken)';
                  e.currentTarget.style.borderColor = 'var(--state-error)';
                  e.currentTarget.style.color = 'var(--state-error)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = 'transparent';
                  e.currentTarget.style.borderColor = 'var(--border-default)';
                  e.currentTarget.style.color = 'var(--text-muted)';
                }}
              >
                <ShieldX size={13} />
                拒绝
              </button>
              <button
                className="tool-action-button"
                onClick={handleAllowAll}
                style={{
                  flex: 1,
                  padding: '8px 0',
                  border: '1px solid var(--tool-ui-accent)',
                  background: 'transparent',
                  color: 'var(--tool-ui-accent-strong)',
                  borderRadius: 'var(--radius-md)',
                  cursor: 'pointer',
                  fontSize: 'var(--font-size-sm)',
                  fontWeight: 500,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  gap: 5,
                  transition: 'all 0.12s',
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = 'var(--tool-ui-accent-soft)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = 'transparent';
                }}
              >
                <ShieldCheck size={13} />
                本次都允许
              </button>
              <button
                className="tool-action-button"
                onClick={handleAllow}
                style={{
                  flex: 1.2,
                  padding: '8px 0',
                  border: 'none',
                  background: 'var(--tool-ui-action)',
                  color: 'var(--text-on-primary)',
                  borderRadius: 'var(--radius-md)',
                  cursor: 'pointer',
                  fontSize: 'var(--font-size-sm)',
                  fontWeight: 600,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  gap: 5,
                  transition: 'opacity 0.12s',
                }}
                onMouseEnter={(e) => { e.currentTarget.style.opacity = '0.92'; }}
                onMouseLeave={(e) => { e.currentTarget.style.opacity = '1'; }}
              >
                <ShieldCheck size={13} />
                允许
              </button>
            </div>

            {/* ── 底部提示 ── */}
            <div
              style={{
                padding: '0 var(--space-4) var(--space-3)',
                fontSize: 'var(--font-size-xs)',
                color: 'var(--text-tertiary)',
                textAlign: 'center',
              }}
            >
              Esc · 拒绝
            </div>
          </div>
        </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
