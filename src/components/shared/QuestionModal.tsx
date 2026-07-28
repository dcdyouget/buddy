/**
 * QuestionModal.tsx — ask_user tool 的弹窗 UI
 *
 * 重新设计：
 * - 选项按钮横向排列（flexWrap 换行）
 * - 按钮下方显示选中选项的补充输入框
 * - 再下方是自定义回答输入区
 * - 弹窗位于输入框正上方，撑起回复消息（不再用 absolute 叠加）
 */

import { useEffect, useState } from 'react';
import {
  Check,
  CornerDownRight,
  HelpCircle,
  X,
} from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { useChatStore } from '@/stores/chatStore';
import { extractAskUserQuestion } from '@/utils/askUserDisplay';

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
    // 不在这里监听 Esc —— App.tsx 已有 window-level Esc → 隐藏窗口。
    // 若这里再监听并调 handleSkip(),同一次 Esc 会同时:
    //   ① 把 question 当作『User skipped the question』发给模型
    //   ② 隐藏窗口
    // 违反 CLAUDE.md 硬约束 #7『Esc/click-outside closes window, does NOT stop streaming』。
    // 用户想『跳过』请点右下角『跳过』按钮(显式)。
  }, [pending]);

  const toggleOption = (idx: number) => {
    if (!pending) return;
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

  const missingRequiredInputs = pending
    ? Array.from(selected).filter(
        (idx) => pending.options[idx]?.requiresInput && !(optionInputs[idx] || '').trim(),
      )
    : [];

  // 选项分支(customText 为空)必须所有 requiresInput 都填;
  // 或直接走 customText 分支(customText 非空)。
  const customTrimmed = customText.trim();
  const canSubmit =
    customTrimmed.length > 0 ||
    (selected.size > 0 && missingRequiredInputs.length === 0);

  const handleSubmit = async () => {
    if (!canSubmit) return;
    // customText 与 selected 互斥: 后端在 selected 非空时丢 custom,
    // 提前在 UI 端决断走哪条路,避免『自定义回答』文字被静默吞掉。
    if (customTrimmed.length > 0) {
      await answerPending([], [], customTrimmed);
    } else {
      const sel = Array.from(selected).sort((a, b) => a - b);
      const inputs = sel.map((idx) => (optionInputs[idx] || '').trim());
      await answerPending(sel, inputs, undefined);
    }
  };

  const handleSkip = () => answerPending([], [], undefined);

  const setOptionInput = (idx: number, v: string) =>
    setOptionInputs((prev) => ({ ...prev, [idx]: v }));

  // 选中选项的描述（单选且选中项有描述时展示）
  const selectedWithDesc =
    pending && selected.size > 0
      ? Array.from(selected).filter((idx) => pending.options[idx]?.description)
      : [];
  const displayQuestion = pending ? extractAskUserQuestion(pending.question) : '';

  return (
    <AnimatePresence>
      {pending && (
        <motion.div
          initial={{ opacity: 0, height: 0, marginBottom: 0 }}
          animate={{ opacity: 1, height: 'auto', marginBottom: 'var(--space-2)' }}
          exit={{ opacity: 0, height: 0, marginBottom: 0 }}
          transition={{ duration: 0.2, ease: 'easeOut' }}
          style={{
            overflow: 'hidden',
            display: 'flex',
            justifyContent: 'center',
          }}
        >
          <div
            style={{
              width: 440,
              maxWidth: '100%',
              background: 'var(--bg-elevated)',
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
                padding: 'var(--space-3) var(--space-4) var(--space-2)',
              }}
            >
              <div
                style={{
                  width: 28,
                  height: 28,
                  borderRadius: 'var(--radius-md)',
                  background: 'var(--primary-tint-soft)',
                  color: 'var(--buddy-primary)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  flexShrink: 0,
                }}
              >
                <HelpCircle size={14} />
              </div>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div
                  style={{
                    fontSize: 'var(--font-size-base)',
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
                  width: 26,
                  height: 26,
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
                <X size={14} />
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
              {displayQuestion}
            </div>

            {/* ── 选项按钮(横向排列) ── */}
            <div
              style={{
                padding: '0 var(--space-4) var(--space-2)',
                display: 'flex',
                flexWrap: 'wrap',
                gap: 'var(--space-2)',
              }}
            >
              {pending.options.map((opt, idx) => {
                const isSelected = selected.has(idx);

                return (
                  <button
                    key={idx}
                    onClick={() => toggleOption(idx)}
                    style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: 5,
                      padding: '6px 12px',
                      border: isSelected
                        ? '1px solid var(--buddy-primary)'
                        : '1px solid var(--border-subtle)',
                      borderRadius: 'var(--radius-md)',
                      background: isSelected
                        ? 'var(--primary-tint-soft)'
                        : 'var(--bg-surface)',
                      color: isSelected
                        ? 'var(--buddy-primary)'
                        : 'var(--text-primary)',
                      fontSize: 'var(--font-size-sm)',
                      fontWeight: isSelected ? 600 : 500,
                      cursor: 'pointer',
                      transition: 'all 0.15s',
                      whiteSpace: 'nowrap',
                    }}
                    onMouseEnter={(e) => {
                      if (!isSelected) {
                        e.currentTarget.style.borderColor = 'var(--border-default)';
                        e.currentTarget.style.background = 'var(--bg-sunken)';
                      }
                    }}
                    onMouseLeave={(e) => {
                      if (!isSelected) {
                        e.currentTarget.style.borderColor = 'var(--border-subtle)';
                        e.currentTarget.style.background = 'var(--bg-surface)';
                      }
                    }}
                  >
                    {isSelected && <Check size={12} />}
                    {opt.label}
                    {opt.requiresInput && (
                      <span
                        style={{
                          fontSize: 9,
                          padding: '0px 4px',
                          borderRadius: 'var(--radius-full)',
                          background: isSelected
                            ? 'var(--buddy-primary)'
                            : 'var(--bg-sunken)',
                          color: isSelected
                            ? 'var(--text-on-primary)'
                            : 'var(--text-tertiary)',
                          fontWeight: 700,
                          letterSpacing: '0.03em',
                          textTransform: 'uppercase',
                          lineHeight: '16px',
                        }}
                      >
                        input
                      </span>
                    )}
                  </button>
                );
              })}
            </div>

            {/* ── 选中选项的描述 ── */}
            {selectedWithDesc.length > 0 && (
              <div
                style={{
                  padding: '0 var(--space-4) var(--space-2)',
                  display: 'flex',
                  flexDirection: 'column',
                  gap: 2,
                }}
              >
                {selectedWithDesc.map((idx) => {
                  const opt = pending.options[idx];
                  return (
                    <div
                      key={idx}
                      style={{
                        fontSize: 'var(--font-size-xs)',
                        color: 'var(--text-muted)',
                        lineHeight: 1.4,
                        paddingLeft: 4,
                        borderLeft: '2px solid var(--border-default)',
                      }}
                    >
                      {opt.description}
                    </div>
                  );
                })}
              </div>
            )}

            {/* ── 选中选项的补充输入框 ── */}
            {Array.from(selected)
              .filter((idx) => pending.options[idx]?.requiresInput)
              .map((idx) => {
                const opt = pending.options[idx];
                const inputVal = optionInputs[idx] || '';
                const inputMissing = !inputVal.trim();

                return (
                  <div
                    key={`input-${idx}`}
                    style={{
                      padding: '0 var(--space-4) var(--space-2)',
                    }}
                  >
                    <div
                      style={{
                        fontSize: 10,
                        fontWeight: 600,
                        color: 'var(--text-tertiary)',
                        textTransform: 'uppercase',
                        letterSpacing: '0.04em',
                        marginBottom: 4,
                      }}
                    >
                      {opt.label}
                    </div>
                    <input
                      type="text"
                      value={inputVal}
                      onChange={(e) => setOptionInput(idx, e.target.value)}
                      onClick={(e) => e.stopPropagation()}
                      placeholder={opt.inputPlaceholder || '请输入...'}
                      autoFocus
                      style={{
                        width: '100%',
                        padding: '7px 10px',
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
                );
              })}

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
                  fontSize: 10,
                  color: 'var(--text-tertiary)',
                  marginBottom: 4,
                  fontWeight: 600,
                  textTransform: 'uppercase',
                  letterSpacing: '0.04em',
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
                padding: '0 var(--space-4) var(--space-3)',
                gap: 'var(--space-2)',
              }}
            >
              <div
                style={{
                  fontSize: 'var(--font-size-xs)',
                  color: 'var(--text-tertiary)',
                }}
              >
                请选择一个选项，或直接输入自定义回答
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
      )}
    </AnimatePresence>
  );
}
