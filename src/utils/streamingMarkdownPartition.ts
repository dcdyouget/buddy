interface OpenFence {
  marker: '`' | '~';
  length: number;
  start: number;
  contentStart: number;
  language: string;
}

export interface StreamingMarkdownPartitionCache {
  content: string;
  scanOffset: number;
  stableBoundary: number;
  fence: OpenFence | null;
}

export interface StreamingMarkdownPartition {
  stablePart: string;
  unstablePart: string;
  openCode: { language: string; source: string } | null;
}

function openingFence(line: string, start: number, contentStart: number): OpenFence | null {
  const match = line.match(/^ {0,3}(`{3,}|~{3,})([^\r\n]*)$/);
  if (!match) return null;

  const info = match[2].trim();
  if (match[1][0] === '`' && info.includes('`')) return null;
  return {
    marker: match[1][0] as OpenFence['marker'],
    length: match[1].length,
    start,
    contentStart,
    language: info.split(/[\t ]/, 1)[0] || 'text',
  };
}

function closesFence(line: string, fence: OpenFence): boolean {
  const match = line.match(/^ {0,3}(`+|~+)[\t ]*$/);
  return Boolean(
    match &&
      match[1][0] === fence.marker &&
      match[1].length >= fence.length,
  );
}

function emptyCache(): StreamingMarkdownPartitionCache {
  return {
    content: '',
    scanOffset: 0,
    stableBoundary: 0,
    fence: null,
  };
}

/**
 * 只扫描相对上一帧新增的完整行，记录代码围栏和最后一个安全段落边界。
 * 内容不是单纯追加时自动回退到全量重建，兼容历史消息替换。
 */
export function partitionStreamingMarkdown(
  content: string,
  previous?: StreamingMarkdownPartitionCache,
): {
  cache: StreamingMarkdownPartitionCache;
  partition: StreamingMarkdownPartition;
} {
  const canContinue = previous && content.startsWith(previous.content);
  const cache = canContinue ? { ...previous } : emptyCache();
  let cursor = cache.scanOffset;

  while (cursor < content.length) {
    const newline = content.indexOf('\n', cursor);
    if (newline === -1) break;
    const line = content.slice(cursor, newline).replace(/\r$/, '');
    const nextLine = newline + 1;

    if (cache.fence) {
      if (closesFence(line, cache.fence)) cache.fence = null;
    } else {
      cache.fence = openingFence(line, cursor, nextLine);
      if (!cache.fence && line.trim() === '') {
        cache.stableBoundary = nextLine;
      }
    }
    cursor = nextLine;
  }

  cache.content = content;
  cache.scanOffset = cursor;

  const stablePart = content.slice(0, cache.stableBoundary);
  if (!cache.fence) {
    return {
      cache,
      partition: {
        stablePart,
        unstablePart: content.slice(cache.stableBoundary),
        openCode: null,
      },
    };
  }

  return {
    cache,
    partition: {
      stablePart,
      unstablePart: content.slice(cache.stableBoundary, cache.fence.start),
      openCode: {
        language: cache.fence.language,
        source: content.slice(cache.fence.contentStart),
      },
    },
  };
}
