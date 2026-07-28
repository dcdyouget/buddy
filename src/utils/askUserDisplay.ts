import type { QuestionOption } from '@/types';

export interface AskUserDisplay {
  header?: string;
  question: string;
  options: QuestionOption[];
  multiSelect?: boolean;
}

const QUESTION_ENDING = /[?？]\s*$/;
const HEADING_PREFIX = /^#{1,6}\s+/;
const ORDERED_PREFIX = /^\d+[.)、]\s+/;
const BULLET_PREFIX = /^[-*+]\s+/;

function cleanLine(line: string): string {
  return line
    .replace(HEADING_PREFIX, '')
    .replace(ORDERED_PREFIX, '')
    .replace(BULLET_PREFIX, '')
    .replace(/^>+\s*/, '')
    .replace(/`+/g, '')
    .replace(/\*\*/g, '')
    .trim();
}

export function extractAskUserQuestion(rawQuestion: string): string {
  const cleaned = rawQuestion.trim();
  if (!cleaned) return '';

  const lines = cleaned
    .split(/\r?\n/)
    .map(cleanLine)
    .filter(Boolean);

  for (let i = lines.length - 1; i >= 0; i -= 1) {
    if (QUESTION_ENDING.test(lines[i])) {
      return lines[i];
    }
  }

  return lines.length > 0 ? lines[lines.length - 1] : cleanLine(cleaned);
}

function normalizeOption(option: unknown): QuestionOption | null {
  if (!option || typeof option !== 'object') return null;
  const record = option as Record<string, unknown>;
  const label = typeof record.label === 'string' ? record.label.trim() : '';
  if (!label) return null;

  return {
    label,
    description:
      typeof record.description === 'string' ? record.description.trim() : undefined,
    requiresInput:
      typeof record.requiresInput === 'boolean'
        ? record.requiresInput
        : record.requires_input === true,
    inputPlaceholder:
      typeof record.inputPlaceholder === 'string'
        ? record.inputPlaceholder
        : typeof record.input_placeholder === 'string'
          ? record.input_placeholder
          : undefined,
  };
}

export function parseAskUserArguments(rawArguments: string): AskUserDisplay {
  try {
    const parsed = JSON.parse(rawArguments) as Record<string, unknown>;
    const rawQuestion =
      typeof parsed.question === 'string' ? parsed.question : rawArguments;
    const options = Array.isArray(parsed.options)
      ? parsed.options
          .map(normalizeOption)
          .filter((option): option is QuestionOption => Boolean(option))
      : [];

    return {
      header: typeof parsed.header === 'string' ? parsed.header.trim() : undefined,
      question: extractAskUserQuestion(rawQuestion),
      options,
      multiSelect: parsed.multi_select === true || parsed.multiSelect === true,
    };
  } catch {
    return {
      question: extractAskUserQuestion(rawArguments),
      options: [],
    };
  }
}
