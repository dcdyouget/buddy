use futures_util::StreamExt;
use reqwest::redirect::Policy;
use serde::Deserialize;
use std::future::Future;
use std::time::Duration;
use tokio::sync::watch;

pub(super) const SEARCH_ENDPOINT: &str = "https://api.deepseek.com/v1/tools/search";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RESPONSE_BYTES: usize = 1_000_000;
const MAX_SUMMARY_CHARS: usize = 1_200;
const MAX_SNIPPET_CHARS: usize = 600;
const MAX_CONTENT_CHARS_PER_RESULT: usize = 4_000;
const MAX_CONTENT_CHARS_TOTAL: usize = 12_000;

#[derive(Debug)]
pub(super) struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub summary: String,
    pub content: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum SearchError {
    #[error("cancelled")]
    Cancelled,
    #[error("unauthorized")]
    Unauthorized,
    #[error("insufficient balance")]
    InsufficientBalance,
    #[error("HTTP {0}")]
    HttpStatus(u16),
    #[error("request timed out")]
    Timeout,
    #[error("request failed")]
    Request,
    #[error("response too large")]
    ResponseTooLarge,
    #[error("invalid response")]
    InvalidResponse,
}

#[derive(Deserialize)]
struct SearchResponse {
    results: Vec<SearchHit>,
}

#[derive(Deserialize)]
struct SearchHit {
    title: String,
    url: String,
    snippet: String,
    summary: String,
    #[serde(default)]
    content: String,
}

pub(super) async fn search(
    endpoint: &str,
    api_key: &str,
    query: &str,
    count: usize,
    cancel_rx: &mut Option<watch::Receiver<bool>>,
) -> Result<Vec<SearchResult>, SearchError> {
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .redirect(Policy::none())
        .build()
        .map_err(|_| SearchError::Request)?;

    let request = client
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "query": query,
            "count": count,
            "need_content": true,
        }));
    let response = await_cancellable(request.send(), cancel_rx)
        .await
        .ok_or(SearchError::Cancelled)?
        .map_err(|error| {
            if error.is_timeout() {
                SearchError::Timeout
            } else {
                SearchError::Request
            }
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => SearchError::Unauthorized,
            402 => SearchError::InsufficientBalance,
            code => SearchError::HttpStatus(code),
        });
    }

    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(SearchError::ResponseTooLarge);
    }

    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    loop {
        let next = await_cancellable(stream.next(), cancel_rx)
            .await
            .ok_or(SearchError::Cancelled)?;
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk.map_err(|error| {
            if error.is_timeout() {
                SearchError::Timeout
            } else {
                SearchError::Request
            }
        })?;
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(SearchError::ResponseTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }

    let parsed: SearchResponse =
        serde_json::from_slice(&bytes).map_err(|_| SearchError::InvalidResponse)?;
    let mut remaining_content_chars = MAX_CONTENT_CHARS_TOTAL;
    Ok(parsed
        .results
        .into_iter()
        .take(count)
        .map(|hit| {
            let summary = truncate_chars(&hit.summary, MAX_SUMMARY_CHARS);
            let snippet = truncate_chars(&hit.snippet, MAX_SNIPPET_CHARS);
            let content = if hit.content.is_empty() || remaining_content_chars == 0 {
                None
            } else {
                let budget = MAX_CONTENT_CHARS_PER_RESULT.min(remaining_content_chars);
                let content = truncate_chars(&hit.content, budget);
                remaining_content_chars =
                    remaining_content_chars.saturating_sub(content.chars().count());
                if content.is_empty() {
                    None
                } else {
                    Some(format!(
                        "【不可信网页正文开始：{}】\n{}\n【不可信网页正文结束】",
                        hit.url, content
                    ))
                }
            };
            SearchResult {
                title: hit.title,
                url: hit.url,
                snippet,
                summary,
                content,
            }
        })
        .collect())
}

async fn await_cancellable<T, F>(
    request: F,
    cancel_rx: &mut Option<watch::Receiver<bool>>,
) -> Option<T>
where
    F: Future<Output = T>,
{
    let Some(cancel_rx) = cancel_rx.as_mut() else {
        return Some(request.await);
    };
    if *cancel_rx.borrow() {
        return None;
    }

    tokio::pin!(request);
    loop {
        tokio::select! {
            output = &mut request => return Some(output),
            changed = cancel_rx.changed() => {
                if *cancel_rx.borrow() {
                    return None;
                }
                if changed.is_err() {
                    return Some(request.await);
                }
            }
        }
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}
