use super::{bing, duckduckgo};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub(super) struct SearchHit {
    pub source: &'static str,
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Serialize)]
pub(super) struct WebSearchProviderStatus {
    pub name: &'static str,
    pub status: &'static str,
    pub result_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub(super) fn provider_outcome(
    name: &'static str,
    result: Result<Vec<SearchHit>, String>,
) -> (Vec<SearchHit>, WebSearchProviderStatus) {
    match result {
        Ok(hits) if !hits.is_empty() => {
            let result_count = hits.len();
            (
                hits,
                WebSearchProviderStatus {
                    name,
                    status: "ok",
                    result_count,
                    error: None,
                },
            )
        }
        Ok(_) => (
            Vec::new(),
            WebSearchProviderStatus {
                name,
                status: "empty",
                result_count: 0,
                error: None,
            },
        ),
        Err(error) => (
            Vec::new(),
            WebSearchProviderStatus {
                name,
                status: "error",
                result_count: 0,
                error: Some(error),
            },
        ),
    }
}

pub(super) fn merge_interleaved(sources: Vec<Vec<SearchHit>>, limit: usize) -> Vec<SearchHit> {
    let mut iterators = sources.into_iter().map(Vec::into_iter).collect::<Vec<_>>();
    let mut seen = HashSet::new();
    let mut merged = Vec::new();

    while merged.len() < limit {
        let mut consumed = false;
        for iterator in &mut iterators {
            let Some(hit) = iterator.next() else {
                continue;
            };
            consumed = true;
            if seen.insert(hit.url.clone()) {
                merged.push(hit);
                if merged.len() == limit {
                    break;
                }
            }
        }
        if !consumed {
            break;
        }
    }

    merged
}

pub(super) fn provider_result_summary(provider: &WebSearchProviderStatus) -> String {
    match provider.status {
        "ok" => format!(
            "{} {} 条",
            provider_label(provider.name),
            provider.result_count
        ),
        "empty" => format!("{}没有结果", provider_label(provider.name)),
        _ => format!("{}搜索失败", provider_label(provider.name)),
    }
}

pub(super) fn provider_failure_summary(provider: &WebSearchProviderStatus) -> String {
    provider
        .error
        .as_ref()
        .map(|error| format!("{}：{error}", provider_label(provider.name)))
        .unwrap_or_else(|| format!("{}没有返回结果", provider_label(provider.name)))
}

fn provider_label(name: &str) -> &str {
    match name {
        bing::PROVIDER => "Bing 中国",
        duckduckgo::PROVIDER => "DuckDuckGo",
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interleaves_sources_and_deduplicates_urls() {
        let hit = |source, title: &str, url: &str| SearchHit {
            source,
            title: title.to_string(),
            url: url.to_string(),
            snippet: String::new(),
        };
        let merged = merge_interleaved(
            vec![
                vec![
                    hit(bing::PROVIDER, "Bing 1", "https://example.com/1"),
                    hit(bing::PROVIDER, "Duplicate", "https://example.com/shared"),
                    hit(bing::PROVIDER, "Bing 3", "https://example.com/3"),
                ],
                vec![
                    hit(duckduckgo::PROVIDER, "Duck 1", "https://example.com/shared"),
                    hit(duckduckgo::PROVIDER, "Duck 2", "https://example.com/2"),
                ],
            ],
            4,
        );

        assert_eq!(merged.len(), 4);
        assert_eq!(merged[0].source, bing::PROVIDER);
        assert_eq!(merged[1].source, duckduckgo::PROVIDER);
        assert_eq!(merged[2].url, "https://example.com/2");
        assert_eq!(merged[3].url, "https://example.com/3");
    }
}
