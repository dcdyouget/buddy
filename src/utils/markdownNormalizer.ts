/**
 * CommonMark 会把紧邻中文标点的 `**` 判定为错误的强调边界。
 * 在合法的加粗内容两端加入不可见守卫字符，帮助解析器稳定识别；
 * 渲染 strong 节点时会移除守卫字符，不影响复制文本。
 */
export const MARKDOWN_EMPHASIS_GUARD = '\u200B';

function isEscaped(value: string, index: number): boolean {
  let slashCount = 0;
  for (let cursor = index - 1; cursor >= 0 && value[cursor] === '\\'; cursor -= 1) {
    slashCount += 1;
  }
  return slashCount % 2 === 1;
}

function findStrongDelimiter(value: string, fromIndex: number): number {
  let index = value.indexOf('**', fromIndex);

  while (index !== -1) {
    const isExactPair =
      value[index - 1] !== '*' &&
      value[index + 2] !== '*' &&
      !isEscaped(value, index);
    if (isExactPair) return index;
    index = value.indexOf('**', index + 2);
  }

  return -1;
}

function normalizeStrongInText(value: string): string {
  let result = '';
  let cursor = 0;

  while (cursor < value.length) {
    const opening = findStrongDelimiter(value, cursor);
    if (opening === -1) {
      result += value.slice(cursor);
      break;
    }

    const closing = findStrongDelimiter(value, opening + 2);
    if (closing === -1) {
      result += value.slice(cursor);
      break;
    }

    const content = value.slice(opening + 2, closing);
    const canNormalize =
      content.length > 0 &&
      !/^\s|\s$/.test(content) &&
      !content.startsWith(MARKDOWN_EMPHASIS_GUARD) &&
      !content.endsWith(MARKDOWN_EMPHASIS_GUARD);

    result += value.slice(cursor, opening);
    result += canNormalize
      ? `**${MARKDOWN_EMPHASIS_GUARD}${content}${MARKDOWN_EMPHASIS_GUARD}**`
      : value.slice(opening, closing + 2);
    cursor = closing + 2;
  }

  return result;
}

function findMatchingBacktickRun(
  value: string,
  delimiter: string,
  fromIndex: number,
): number {
  let index = value.indexOf(delimiter, fromIndex);

  while (index !== -1) {
    if (
      value[index - 1] !== '`' &&
      value[index + delimiter.length] !== '`'
    ) {
      return index;
    }
    index = value.indexOf(delimiter, index + delimiter.length);
  }

  return -1;
}

function normalizeLineOutsideInlineCode(value: string): string {
  let result = '';
  let textStart = 0;
  let cursor = 0;

  while (cursor < value.length) {
    if (value[cursor] !== '`' || isEscaped(value, cursor)) {
      cursor += 1;
      continue;
    }

    let runEnd = cursor + 1;
    while (value[runEnd] === '`') runEnd += 1;
    const delimiter = value.slice(cursor, runEnd);
    const closing = findMatchingBacktickRun(value, delimiter, runEnd);

    result += normalizeStrongInText(value.slice(textStart, cursor));
    if (closing === -1) {
      return result + value.slice(cursor);
    }

    const codeEnd = closing + delimiter.length;
    result += value.slice(cursor, codeEnd);
    cursor = codeEnd;
    textStart = codeEnd;
  }

  return result + normalizeStrongInText(value.slice(textStart));
}

interface Fence {
  marker: '`' | '~';
  length: number;
  canRepairAttachedClose: boolean;
}

function getOpeningFence(line: string): Fence | null {
  const match = line.match(/^ {0,3}(`{3,}|~{3,})/);
  if (!match) return null;

  const language = line
    .slice(match[0].length)
    .trim()
    .split(/[\t ]/, 1)[0]
    .toLowerCase();

  return {
    marker: match[1][0] as Fence['marker'],
    length: match[1].length,
    canRepairAttachedClose:
      language === '' ||
      ['plain', 'plaintext', 'text', 'txt'].includes(language),
  };
}

function isClosingFence(line: string, fence: Fence): boolean {
  const match = line.match(/^ {0,3}(`+|~+)[\t ]*$/);
  return Boolean(
    match &&
      match[1][0] === fence.marker &&
      match[1].length >= fence.length,
  );
}

function splitAttachedClosingFence(
  line: string,
  fence: Fence,
): [content: string, closingFence: string] | null {
  if (!fence.canRepairAttachedClose) return null;

  const trimmedLine = line.trimEnd();
  let markerStart = trimmedLine.length;
  while (trimmedLine[markerStart - 1] === fence.marker) markerStart -= 1;

  const markerLength = trimmedLine.length - markerStart;
  if (markerLength < fence.length) return null;

  const content = trimmedLine.slice(0, markerStart).trimEnd();
  if (!content.trim()) return null;

  return [content, fence.marker.repeat(markerLength)];
}

/**
 * 修复 AI 偶尔把纯文本代码块的结束围栏粘在正文末尾的情况。
 * 仅处理无语言或纯文本围栏，避免改写真实源码中的反引号字符串。
 */
export function repairMarkdownFences(markdown: string): string {
  let fence: Fence | null = null;
  const normalizedLines: string[] = [];

  for (const line of markdown.split('\n')) {
    if (fence) {
      if (isClosingFence(line, fence)) {
        normalizedLines.push(line);
        fence = null;
        continue;
      }

      const attachedClosingFence = splitAttachedClosingFence(line, fence);
      if (attachedClosingFence) {
        normalizedLines.push(...attachedClosingFence);
        fence = null;
        continue;
      }

      normalizedLines.push(line);
      continue;
    }

    normalizedLines.push(line);
    fence = getOpeningFence(line);
  }

  return normalizedLines.join('\n');
}

/**
 * 规范化正文中的加粗语法，同时原样保留行内代码和围栏代码块。
 */
export function normalizeMarkdownEmphasis(markdown: string): string {
  let fence: Fence | null = null;

  return markdown
    .split('\n')
    .map((line) => {
      if (fence) {
        if (isClosingFence(line, fence)) fence = null;
        return line;
      }

      const openingFence = getOpeningFence(line);
      if (openingFence) {
        fence = openingFence;
        return line;
      }

      return normalizeLineOutsideInlineCode(line);
    })
    .join('\n');
}

/** 统一执行 Markdown 结构修复和正文语法规范化。 */
export function normalizeMarkdown(markdown: string): string {
  return normalizeMarkdownEmphasis(repairMarkdownFences(markdown));
}
