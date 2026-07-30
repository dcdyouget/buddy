import { memo, useCallback, useState } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { open as openExternal } from '@tauri-apps/plugin-shell';
import {
  ChevronDown,
  ChevronRight,
  ExternalLink,
  Search,
} from 'lucide-react';
import type { ToolCall } from '@/types';

interface WebSearchSectionProps {
  toolCall: ToolCall;
}

interface WebSearchResultItem {
  rank?: number;
  source?: string;
  title?: string;
  url?: string;
  snippet?: string;
  content?: string;
  fetch_error?: string;
}

interface WebSearchProviderStatus {
  name?: string;
  status?: 'ok' | 'empty' | 'error';
  result_count?: number;
  error?: string;
}

interface WebSearchResultPayload {
  status?: 'ok' | 'partial' | 'unavailable';
  query?: string;
  provider?: string;
  providers?: WebSearchProviderStatus[];
  note?: string;
  results?: WebSearchResultItem[];
}

function parseQuery(argumentsJson: string): string {
  try {
    const query = JSON.parse(argumentsJson)?.query;
    return typeof query === 'string' ? query.trim() : '';
  } catch {
    return '';
  }
}

function parseResult(result: string | undefined): WebSearchResultPayload {
  if (!result) return {};
  try {
    const payload = JSON.parse(result);
    return payload && typeof payload === 'object'
      ? (payload as WebSearchResultPayload)
      : {};
  } catch {
    return {};
  }
}

function providerName(provider: string): string {
  switch (provider.toLowerCase()) {
    case 'cn_bing':
      return 'Bing 中国';
    case 'duckduckgo':
      return 'DuckDuckGo';
    default:
      return provider;
  }
}

function providerLabel(result: WebSearchResultPayload): string {
  if (Array.isArray(result.providers) && result.providers.length > 0) {
    return result.providers
      .map((provider) => provider.name?.trim())
      .filter((name): name is string => Boolean(name))
      .map(providerName)
      .join(' + ');
  }
  if (result.provider) {
    return result.provider
      .split('+')
      .filter(Boolean)
      .map(providerName)
      .join(' + ');
  }
  return 'Bing 中国 + DuckDuckGo';
}

async function openSource(url: string) {
  try {
    if (isTauri()) {
      await openExternal(url);
    } else {
      window.open(url, '_blank', 'noopener,noreferrer');
    }
  } catch (error) {
    console.error('[WebSearchSection] failed to open source', url, error);
  }
}

export const WebSearchSection = memo(function WebSearchSection({
  toolCall,
}: WebSearchSectionProps) {
  const [expanded, setExpanded] = useState(false);
  const toggle = useCallback(() => {
    setExpanded((previous) => !previous);
  }, []);
  const status = toolCall.status ?? 'calling';
  const isActive = status === 'calling' || status === 'executing';
  const result = parseResult(toolCall.result);
  const results = Array.isArray(result.results) ? result.results : [];
  const isUnavailable =
    status === 'error' || result.status === 'unavailable';
  const label = isActive
    ? '正在搜索网络'
    : isUnavailable
      ? '网络搜索不可用，已继续回答'
      : status === 'interrupted'
        ? '网络搜索已中断'
        : '网络搜索完成';
  const query = result.query?.trim() || parseQuery(toolCall.arguments);

  return (
    <div
      className={`think-section websearch-section ${
        isActive ? 'is-streaming' : ''
      } ${isUnavailable ? 'is-unavailable' : ''} ${
        expanded ? 'is-expanded' : ''
      }`}
      role="status"
      aria-live="polite"
    >
      <button
        className="websearch-section-header"
        type="button"
        onClick={toggle}
        aria-expanded={expanded}
        aria-label={`网络搜索：${query || label}`}
      >
        <Search
          className="websearch-section-icon"
          size={14}
          aria-hidden="true"
        />
        <span className="websearch-section-label">{label}</span>
        {isActive && (
          <span className="think-section-loader" aria-label="搜索中">
            <span />
            <span />
            <span />
          </span>
        )}
        {query && (
          <span className="websearch-section-query" title={query}>
            {query}
          </span>
        )}
        <span className="websearch-section-chevron" aria-hidden="true">
          {expanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
        </span>
      </button>

      {expanded && (
        <div className="websearch-section-body">
          <div className="websearch-section-meta">
            <div className="websearch-section-meta-row">
              <span>搜索内容</span>
              <strong>{query || '未获得搜索关键词'}</strong>
            </div>
            <div className="websearch-section-meta-row">
              <span>搜索引擎</span>
              <strong>{providerLabel(result)}</strong>
            </div>
          </div>

          {result.note && (
            <p className="websearch-section-note">{result.note}</p>
          )}

          {isActive ? (
            <div className="websearch-section-empty">
              正在等待搜索结果…
            </div>
          ) : results.length > 0 ? (
            <section className="websearch-results">
              <div className="websearch-results-title">
                搜索结果（{results.length}）
              </div>
              <ol className="websearch-results-list">
                {results.map((item, index) => {
                  const rank = item.rank ?? index + 1;
                  const title = item.title?.trim() || item.url || `结果 ${rank}`;
                  return (
                    <li
                      className="websearch-result-item"
                      key={`${rank}-${item.url || title}`}
                    >
                      <div className="websearch-result-heading">
                        <span className="websearch-result-rank">{rank}</span>
                        {item.source && (
                          <span className="websearch-result-source">
                            {providerName(item.source)}
                          </span>
                        )}
                        {item.url ? (
                          <a
                            className="websearch-result-link"
                            href={item.url}
                            title={item.url}
                            rel="noopener noreferrer"
                            onClick={(event) => {
                              event.preventDefault();
                              void openSource(item.url!);
                            }}
                          >
                            <span>{title}</span>
                            <ExternalLink size={12} aria-hidden="true" />
                          </a>
                        ) : (
                          <span className="websearch-result-title">{title}</span>
                        )}
                      </div>

                      {item.snippet && (
                        <p className="websearch-result-snippet">
                          {item.snippet}
                        </p>
                      )}
                      <span
                        className={`websearch-result-fetch ${
                          item.content
                            ? 'is-fetched'
                            : item.fetch_error
                              ? 'is-failed'
                              : ''
                        }`}
                        title={item.fetch_error}
                      >
                        {item.content
                          ? '已读取网页正文'
                          : item.fetch_error
                            ? '网页正文读取失败，使用搜索摘要'
                            : '使用搜索摘要'}
                      </span>
                    </li>
                  );
                })}
              </ol>
            </section>
          ) : (
            <div className="websearch-section-empty">
              {isUnavailable ? '没有可展示的搜索结果' : '搜索未返回结果'}
            </div>
          )}
        </div>
      )}
    </div>
  );
});
