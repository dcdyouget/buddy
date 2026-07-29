import { HelpCircle } from 'lucide-react';

interface QuestionPromptProps {
  title: string;
  question: string;
  context?: string;
  multiSelect?: boolean;
}

/**
 * ask_user 工具的问题提示块。
 * 交互控件由 AskUserCard 提供，这里只统一问题本身的视觉层级。
 */
export function QuestionPrompt({
  title,
  question,
  context,
  multiSelect = false,
}: QuestionPromptProps) {
  return (
    <div className="question-prompt">
      <span className="question-prompt-icon">
        <HelpCircle size={14} aria-hidden="true" />
      </span>

      <div className="question-prompt-body">
        <div className="question-prompt-meta">
          <strong className="question-prompt-title">{title}</strong>
          {context && (
            <span className="question-prompt-context">{context}</span>
          )}
          {multiSelect && (
            <span className="question-prompt-mode">多选</span>
          )}
        </div>
        {question && <div className="question-prompt-text">{question}</div>}
      </div>
    </div>
  );
}
