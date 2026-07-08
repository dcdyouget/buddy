/**
 * QuestionModal.tsx — ask_user tool 的弹窗 UI
 */

import { useEffect, useState } from 'react';
import {
  ArrowRight,
  CheckSquare,
  CornerDownRight,
  HelpCircle,
  X,
} from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { useChatStore } from '@/stores/chatStore';

export function QuestionModal() {
  const pending = useChatStore((s) => s.pendingQuestion);
  const answerPending = useChatStore((s) => s.answerPendingQuestion);

  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [optionInputs, setOptionInputs] = useState<Record<number, string>>({});
  const [customText, setCustomText] = useState('');

  useEffect(() => {
    setSelected(new Set());
    setOptionInputs({});
    setCustomText('');
  }, [pending?.id]);

  useEffect(() => {
    if (!pending) return;
    const onKey = (e: Event) => {
      if ((e as KeyboardEvent).key === 'Escape') handleSkip();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [pending]);

  if (!pending) return null;

  const toggleOption = (idx: number) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (pending.multiSelect) {
        next.has(idx) ? next.delete(idx) : next.add(idx);
      } else {
        next.clear();
        next.add(idx);
      }
      return next;
    });
  };

  const missingRequiredInputs = Array.from(selected).filter(
    (idx) => pending.options[idx]?.requiresInput && !(optionInputs[idx] || '').trim(),
  );
  const canSubmit =
    (selected.size > 0 && missingRequiredInputs.length === 0) ||
    customText.trim().length > 0;

  const handleSubmit = async () => {
    if (!canSubmit) return;
    const sel = Array.from(selected).sort((a, b) => a - b);
    const inputs = sel.map((idx) => (optionInputs[idx] || '').trim());
    await answerPending(sel, inputs, customText.trim() || undefined);
  };

  const handleSkip = () => answerPending([], [], undefined);

  const setOptionInput = (idx: number, v: string) =>
    setOptionInputs((prev) => ({ ...prev, [idx]: v }));

  return (
    <AnimatePresence>
      {pending && (
        <div
          style={{
            position: 'absolute',
            bottom: 'calc(100% + var(--space-2))',
            left: 0,
            right: 0,
            display: 'flex',
            justifyContent: 'center',
            zIndex: 1001,
            pointerEvents: 'none',
          }}
        >
        <motion.div
          initial={{ opacity: 0, y: 20, scale: 0.97 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: 10, scale: 0.97 }}
          transition={{ duration: 0.18, ease: 'easeOut' }}
          style={{
            width: 440,
            maxWidth: 'calc(100vw - 32px)',
            pointerEvents: 'auto',
          }}
        >
          <div
            style={{
              background: 'var(--bg-elevated)',
              backdropFilter: 'blur(var(--blur-surface)) saturate(160%)',
              WebkitBackdropFilter: 'blur(var(--blur-surface)) saturate(160%)',
              border: '1px solid var(--border-default)',
              borderRadius: 'var(--radius-xl)',
              overflow: 'hidden',
              boxShadow: 'var(--shadow-floating-md)',
            }}
          >
            {/* ── Header ── */}
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 'var(--space-3)',
                padding: 'var(--space-4) var(--space-4) var(--space-2)',
              }}
            >
              <div
                style={{
                  width: 32,
                  height: 32,
                  borderRadius: 'var(--radius-md)',
                  background: 'var(--primary-tint-soft)',
                  color: 'var(--buddy-primary)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  flexShrink: 0,
                }}
              >
                <HelpCircle size={16} />
              </div>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div
                  style={{
                    fontSize: 'var(--font-size-md)',
                    fontWeight: 600,
                    color: 'var(--text-primary)',
                    letterSpacing: 'var(--letter-spacing-tight)',
                  }}
                >
                  {pending.header}
                </div>
              </div>
              <button
                onClick={handleSkip}
                style={{
                  width: 28,
                  height: 28,
                  display: 'inline-flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  border: 'none',
                  borderRadius: 'var(--radius-sm)',
                  background: 'transparent',
                  color: 'var(--text-tertiary)',
                  cursor: 'pointer',
                  flexShrink: 0,
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = 'var(--bg-sunken)';
                  e.currentTarget.style.color = 'var(--text-primary)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = 'transparent';
                  e.currentTarget.style.color = 'var(--text-tertiary)';
                }}
              >
                <X size={15} />
              </button>
            </div>

            {/* ── 问题 ── */}
            <div
              style={{
                padding: '0 var(--space-4) var(--space-3)',
                fontSize: 'var(--font-size-md)',
                color: 'var(--text-primary)',
                lineHeight: 'var(--line-height-relaxed)',
              }}
            >
              {pending.question}
            </div>

            {/* ── 选项 ── */}
            <div
              style={{
                padding: '0 var(--space-4) var(--space-4)',
                display: 'flex',
                flexDirection: 'column',
                gap: 'var(--space-2)',
              }}
            >
              {pending.options.map((opt, idx) => {
                const isSelected = selected.has(idx);
                const showInput = opt.requiresInput && isSelected;
                const inputVal = optionInputs[idx] || '';
                const inputMissing = opt.requiresInput && isSelected && !inputVal.trim();

                return (
                  <div
                    key={idx}
                    style={{
                      border: isSelected
                        ? '1px solid var(--buddy-primary)'
                        : '1px solid var(--border-subtle)',
                      borderRadius: 'var(--radius-md)',
                      background: isSelected
                        ? 'var(--primary-tint-soft)'
                        : 'var(--bg-surface)',
                      overflow: 'hidden',
                      transition: 'border-color 0.15s, background 0.15s',
                    }}
                    onMouseEnter={(e) => {
                      if (!isSelected) e.currentTarget.style.borderColor = 'var(--border-default)';
                    }}
                    onMouseLeave={(e) => {
                      if (!isSelected) e.currentTarget.style.borderColor = 'var(--border-subtle)';
                    }}
                  >
                    <button
                      onClick={() => toggleOption(idx)}
                      style={{
                        display: 'flex',
                        alignItems: 'flex-start',
                        gap: 'var(--space-3)',
                        width: '100%',
                        padding: 'var(--space-3) var(--space-3)',
                        border: 'none',
                        background: 'transparent',
                        color: 'var(--text-primary)',
                        cursor: 'pointer',
                        textAlign: 'left',
                        fontSize: 'var(--font-size-base)',
                        lineHeight: 'var(--line-height-base)',
                      }}
                    >
                      {/* 单选圆点 / 多选方块 */}
                      <div
                        style={{
                          width: 18,
                          height: 18,
                          flexShrink: 0,
                          marginTop: 1,
                          borderRadius: pending.multiSelect ? 'var(--radius-sm)' : 'var(--radius-full)',
                          border: isSelected
                            ? `5px solid var(--buddy-primary)`
                            : '1.5px solid var(--text-tertiary)',
                          background: isSelected ? 'var(--text-on-primary)' : 'transparent',
                          display: 'flex',
                          alignItems: 'center',
                          justifyContent: 'center',
                          transition: 'all 0.15s',
                        }}
                      >
                        {pending.multiSelect && isSelected && (
                          <CheckSquare size={10} style={{ color: 'var(--buddy-primary)' }} />
                        )}
                      </div>

                      <div style={{ flex: 1, minWidth: 0 }}>
                        <div
                          style={{
                            display: 'flex',
                            alignItems: 'center',
                            gap: 'var(--space-2)',
                            flexWrap: 'wrap',
                          }}
                        >
                          <span style={{ fontWeight: 500 }}>{opt.label}</span>
                          {opt.requiresInput && (
                            <span
                              style={{
                                fontSize: 10,
                                padding: '1px 6px',
                                borderRadius: 'var(--radius-full)',
                                background: 'var(--bg-sunken)',
                                color: 'var(--text-tertiary)',
                                fontWeight: 600,
                                letterSpacing: '0.03em',
                                textTransform: 'uppercase',
                              }}
                            >
                              input
                            </span>
                          )}
                        </div>
                        {opt.description && (
                          <div
                            style={{
                              fontSize: 'var(--font-size-sm)',
                              color: 'var(--text-muted)',
                              marginTop: 3,
                              lineHeight: 1.4,
                            }}
                          >
                            {opt.description}
                          </div>
                        )}
                      </div>

                      {isSelected && !opt.requiresInput && (
                        <ArrowRight
                          size={14}
                          style={{
                            color: 'var(--buddy-primary)',
                            flexShrink: 0,
                            marginTop: 2,
                          }}
                        />
                      )}
                    </button>

                    {/* per-option input */}
                    {showInput && (
                      <div style={{ padding: '0 var(--space-3) var(--space-3) var(--space-3)', paddingLeft: 39 }}>
                        <input
                          type="text"
                          value={inputVal}
                          onChange={(e) => setOptionInput(idx, e.target.value)}
                          onClick={(e) => e.stopPropagation()}
                          placeholder={opt.inputPlaceholder || '请输入...'}
                          autoFocus
                          style={{
                            width: '100%',
                            padding: '6px 10px',
                            border: inputMissing
                              ? '1px solid var(--state-error)'
                              : '1px solid var(--border-default)',
                            borderRadius: 'var(--radius-sm)',
                            background: 'var(--bg-surface)',
                            color: 'var(--text-primary)',
                            fontFamily: 'var(--font-mono)',
                            fontSize: 'var(--font-size-sm)',
                            lineHeight: 1.4,
                            outline: 'none',
                            boxSizing: 'border-box',
                          }}
                        />
                      </div>
                    )}
                  </div>
                );
              })}
            </div>

            {/* ── 自定义回答 ── */}
            <div
              style={{
                padding: '0 var(--space-4) var(--space-3)',
              }}
            >
              <div
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 4,
                  fontSize: 'var(--font-size-xs)',
                  color: 'var(--text-tertiary)',
                  marginBottom: 4,
                  fontWeight: 600,
                  textTransform: 'uppercase',
                  letterSpacing: '0.03em',
                }}
              >
                <CornerDownRight size={11} />
                或输入自定义回答
              </div>
              <textarea
                value={customText}
                onChange={(e) => setCustomText(e.target.value)}
                placeholder="输入你的回答..."
                rows={2}
                style={{
                  width: '100%',
                  padding: '8px 10px',
                  border: '1px solid var(--border-subtle)',
                  borderRadius: 'var(--radius-md)',
                  background: 'var(--bg-surface)',
                  color: 'var(--text-primary)',
                  fontFamily: 'var(--font-sans)',
                  fontSize: 'var(--font-size-base)',
                  lineHeight: 1.4,
                  resize: 'vertical',
                  outline: 'none',
                  boxSizing: 'border-box',
                }}
              />
            </div>

            {/* ── Footer ── */}
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                padding: '0 var(--space-4) var(--space-4)',
                gap: 'var(--space-2)',
              }}
            >
              <div
                style={{
                  fontSize: 'var(--font-size-xs)',
                  color: 'var(--text-tertiary)',
                }}
              >
                Esc · 跳过
              </div>
              <div style={{ display: 'flex', gap: 'var(--space-2)' }}>
                <button
                  onClick={handleSkip}
                  style={{
                    padding: '6px 14px',
                    border: '1px solid var(--border-default)',
                    background: 'transparent',
                    color: 'var(--text-muted)',
                    borderRadius: 'var(--radius-md)',
                    cursor: 'pointer',
                    fontSize: 'var(--font-size-sm)',
                    fontWeight: 500,
                    transition: 'all 0.12s',
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.background = 'var(--bg-sunken)';
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.background = 'transparent';
                  }}
                >
                  跳过
                </button>
                <button
                  onClick={handleSubmit}
                  disabled={!canSubmit}
                  style={{
                    padding: '6px 20px',
                    border: 'none',
                    background: canSubmit
                      ? 'var(--buddy-primary)'
                      : 'var(--border-subtle)',
                    color: canSubmit
                      ? 'var(--text-on-primary)'
                      : 'var(--text-tertiary)',
                    borderRadius: 'var(--radius-md)',
                    cursor: canSubmit ? 'pointer' : 'not-allowed',
                    fontSize: 'var(--font-size-sm)',
                    fontWeight: 600,
                    transition: 'all 0.12s',
                  }}
                  onMouseEnter={(e) => {
                    if (canSubmit) e.currentTarget.style.opacity = '0.92';
                  }}
                  onMouseLeave={(e) => {
                    if (canSubmit) e.currentTarget.style.opacity = '1';
                  }}
                >
                  确认{canSubmit && selected.size > 0 && ` (${selected.size})`}
                </button>
              </div>
            </div>
          </div>
        </motion.div>
        </div>
      )}
    </AnimatePresence>
  );
}
