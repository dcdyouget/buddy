import { useEffect, useState } from 'react';
import { Check, CornerDownRight } from 'lucide-react';
import type { QuestionOption, ToolCall } from '@/types';
import { useChatStore } from '@/stores/chatStore';
import { parseAskUserArguments } from '@/utils/askUserDisplay';
import { QuestionPrompt } from './QuestionPrompt';

interface AskUserCardProps {
  toolCall: ToolCall;
  hasResult: boolean;
  isInterrupted?: boolean;
}

export function AskUserCard({
  toolCall,
  hasResult,
  isInterrupted = false,
}: AskUserCardProps) {
  const pending = useChatStore((state) => state.pendingQuestion);
  const answerPending = useChatStore((state) => state.answerPendingQuestion);
  const parsed = parseAskUserArguments(toolCall.arguments);
  const isAwaitingAnswer = pending?.id === toolCall.id;

  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [optionInputs, setOptionInputs] = useState<Record<number, string>>({});
  const [customText, setCustomText] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const [submitError, setSubmitError] = useState('');

  useEffect(() => {
    if (!isAwaitingAnswer) return;
    setSelected(new Set());
    setOptionInputs({});
    setCustomText('');
    setIsSubmitting(false);
    setSubmitted(false);
    setSubmitError('');
  }, [isAwaitingAnswer, pending?.id]);

  const header = isAwaitingAnswer ? pending.header : parsed.header;
  const question = isAwaitingAnswer ? pending.question : parsed.question;
  const options: QuestionOption[] = isAwaitingAnswer
    ? pending.options
    : parsed.options;
  const multiSelect = isAwaitingAnswer
    ? pending.multiSelect
    : parsed.multiSelect;

  const toggleOption = (index: number) => {
    if (!isAwaitingAnswer || isSubmitting) return;
    setSelected((previous) => {
      const next = new Set(previous);
      if (multiSelect) {
        next.has(index) ? next.delete(index) : next.add(index);
      } else {
        next.clear();
        next.add(index);
      }
      return next;
    });
    setSubmitError('');
  };

  const missingRequiredInputs = Array.from(selected).filter(
    (index) =>
      options[index]?.requiresInput && !(optionInputs[index] || '').trim(),
  );
  const customTrimmed = customText.trim();
  const canSubmit =
    !isSubmitting &&
    (customTrimmed.length > 0 ||
      (selected.size > 0 && missingRequiredInputs.length === 0));

  const submitAnswer = async (
    selectedIndexes: number[],
    inputs: string[],
    custom?: string,
  ) => {
    setIsSubmitting(true);
    setSubmitError('');
    const accepted = await answerPending(selectedIndexes, inputs, custom);
    setIsSubmitting(false);
    if (accepted) {
      setSubmitted(true);
    } else {
      setSubmitError('提交失败，请重试');
    }
  };

  const handleSubmit = async () => {
    if (!canSubmit) return;
    const indexes = Array.from(selected).sort((a, b) => a - b);
    const inputs = indexes.map((index) => (optionInputs[index] || '').trim());
    if (customTrimmed) {
      // 自定义回答与已选选项（含补充输入）一起提交，避免静默丢弃用户的选择
      await submitAnswer(indexes, inputs, customTrimmed);
      return;
    }

    await submitAnswer(indexes, inputs);
  };

  const handleSkip = () => submitAnswer([], []);
  const promptTitle = isInterrupted
    ? '询问已中断'
    : hasResult
      ? '询问已完成'
      : isAwaitingAnswer
        ? '模型正在等待你的回答'
        : submitted
          ? '回答已提交'
          : '模型正在准备问题';

  return (
    <div className="tool-question-card">
      <QuestionPrompt
        title={promptTitle}
        question={question}
        context={header}
        multiSelect={multiSelect}
      />

      {options.length > 0 && (
        <div className="tool-question-options">
          {options.map((option, index) => {
            const isSelected = selected.has(index);
            if (!isAwaitingAnswer) {
              return (
                <span className="tool-question-option" key={index}>
                  <span>{option.label}</span>
                  {option.requiresInput && (
                    <small className="tool-question-input-tag">需补充</small>
                  )}
                  {option.description && (
                    <span
                      className="tool-question-description"
                      title={option.description}
                    >
                      {option.description}
                    </span>
                  )}
                </span>
              );
            }

            return (
              <button
                className={`tool-question-option is-interactive ${
                  isSelected ? 'is-selected' : ''
                }`}
                key={index}
                type="button"
                aria-pressed={isSelected}
                disabled={isSubmitting}
                onClick={() => toggleOption(index)}
              >
                {isSelected && <Check size={12} aria-hidden="true" />}
                <span>{option.label}</span>
                {option.requiresInput && (
                  <small className="tool-question-input-tag">需补充</small>
                )}
                {option.description && (
                  <span className="tool-question-description">
                    {option.description}
                  </span>
                )}
              </button>
            );
          })}
        </div>
      )}

      {isAwaitingAnswer &&
        Array.from(selected).map((index) => {
          const option = options[index];
          if (!option?.requiresInput) return null;
          return (
            <label className="tool-question-followup" key={index}>
              <span>{option.label} · 补充信息</span>
              <input
                className="tool-question-input"
                value={optionInputs[index] || ''}
                disabled={isSubmitting}
                placeholder={option.inputPlaceholder || '请输入补充内容'}
                onChange={(event) => {
                  setOptionInputs((previous) => ({
                    ...previous,
                    [index]: event.target.value,
                  }));
                  setSubmitError('');
                }}
              />
            </label>
          );
        })}

      {isAwaitingAnswer && (
        <>
          <label className="tool-question-custom">
            <span>
              <CornerDownRight size={12} aria-hidden="true" />
              或输入自定义回答
            </span>
            <textarea
              className="tool-question-textarea"
              value={customText}
              disabled={isSubmitting}
              placeholder="输入你的回答…"
              rows={2}
              onChange={(event) => {
                setCustomText(event.target.value);
                setSubmitError('');
              }}
              onKeyDown={(event) => {
                if (
                  event.key === 'Enter' &&
                  (event.metaKey || event.ctrlKey) &&
                  canSubmit
                ) {
                  event.preventDefault();
                  void handleSubmit();
                }
              }}
            />
          </label>

          <div className="tool-question-actions">
            <span className={submitError ? 'is-error' : undefined}>
              {submitError ||
                (multiSelect
                  ? '可选择多个选项'
                  : '请选择一个选项，或填写自定义回答')}
            </span>
            <button
              className="tool-question-skip tool-action-button"
              type="button"
              disabled={isSubmitting}
              onClick={() => void handleSkip()}
            >
              跳过
            </button>
            <button
              className="tool-question-submit tool-action-button"
              type="button"
              disabled={!canSubmit}
              onClick={() => void handleSubmit()}
            >
              {isSubmitting ? '提交中' : '确认'}
            </button>
          </div>
        </>
      )}

      {hasResult && toolCall.result && (
        <div className="tool-question-answer">
          <span>用户回应</span>
          <strong>{toolCall.result}</strong>
        </div>
      )}
    </div>
  );
}
