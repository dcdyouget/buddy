/**
 * thinkParser.ts — 解析消息中的 <think>...</think> 标签
 *
 * 将消息内容按 think 标签切分为文本块和思考块，
 * 供 MessageBubble 分段渲染（思考块用 ThinkSection 折叠展示）。
 *
 * 算法：手动字符扫描（非正则），O(n) 时间复杂度。
 * 嵌套 <think> 当作字面文本处理 —— 始终匹配第一个 </think>。
 */

/** 思考块：被 <think>...</think> 包裹的内容 */
export interface ThinkBlock {
  type: 'think';
  /** 标签内部的原始内容（可能含 markdown） */
  content: string;
  /** 是否缺少闭合标签（流式输出中 </think> 尚未到达） */
  isOpen: boolean;
}

/** 文本块：正常的 markdown 内容 */
export interface TextBlock {
  type: 'text';
  content: string;
}

/** 内容分段：思考块或文本块 */
export type ContentSegment = ThinkBlock | TextBlock;

/** 占位符，用于在流式输出中替换未闭合的 <think> 标签，防止 markdown 渲染器误解析 */
const THINK_OPEN = '<think>';
const THINK_CLOSE = '</think>';

/**
 * 解析消息内容，将 <think>...</think> 标签切分为独立的分段数组。
 *
 * 处理逻辑：
 * - 遇到 <think> → 前面的文本作为 TextBlock，开始收集 think 内容
 * - 遇到 </think> → think 内容作为 ThinkBlock(isOpen=false)，回到文本模式
 * - 到字符串末尾仍未闭合 → think 内容作为 ThinkBlock(isOpen=true)
 * - 内部再次出现的 <think> 当作字面文本（按第一个 </think> 闭合）
 * - 空的 TextBlock 会被过滤，但空的 ThinkBlock 保留
 */
export function parseThinkBlocks(content: string): ContentSegment[] {
  const segments: ContentSegment[] = [];
  const len = content.length;
  let i = 0;

  while (i < len) {
    const thinkOpen = content.indexOf(THINK_OPEN, i);

    // 没有更多 <think> 标签 → 剩余全部是文本
    if (thinkOpen === -1) {
      if (i < len) {
        segments.push({ type: 'text', content: content.substring(i) });
      }
      break;
    }

    // thinkOpen 之前的文本作为一个 TextBlock
    if (thinkOpen > i) {
      segments.push({ type: 'text', content: content.substring(i, thinkOpen) });
    }

    // 查找对应的闭合标签
    const thinkStart = thinkOpen + THINK_OPEN.length;
    const thinkClose = content.indexOf(THINK_CLOSE, thinkStart);

    if (thinkClose === -1) {
      // 没有闭合标签：流式输出中 </think> 尚未到达
      const thinkContent = content.substring(thinkStart);
      segments.push({ type: 'think', content: thinkContent, isOpen: true });
      break;
    }

    // 完整闭合的 think 块
    const thinkContent = content.substring(thinkStart, thinkClose);
    segments.push({ type: 'think', content: thinkContent, isOpen: false });
    i = thinkClose + THINK_CLOSE.length;
  }

  return segments;
}
