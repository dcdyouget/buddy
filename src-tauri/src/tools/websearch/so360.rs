use super::aggregate::SearchHit;
use super::web_fetch::read_search_body;
use dom_query::{Document, Selection};
use log::info;
#[cfg(debug_assertions)]
use log::warn;
use reqwest::{
    header::{ACCEPT, CACHE_CONTROL, PRAGMA},
    Client, Url,
};
use std::collections::HashSet;

const SEARCH_ENDPOINT: &str = "https://www.so.com/s";
const MAX_SNIPPET_CHARS: usize = 600;

pub(super) const PROVIDER: &str = "so_360";

pub(super) async fn search_so360(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchHit>, String> {
    let query = normalize_text(&query.replace('"', " "));
    if query.is_empty() {
        return Err("360 搜索的 query 不能为空".to_string());
    }
    let url = build_search_url(&query)?;
    info!(
        "[websearch][so_360] 请求: query={:?}, url={}, accept_language=zh-CN,zh;q=0.9,en;q=0.7",
        query, url
    );
    let response = client
        .get(url)
        .header(
            ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.7")
        .header(CACHE_CONTROL, "no-cache, no-store, max-age=0")
        .header(PRAGMA, "no-cache")
        .header("DNT", "1")
        .send()
        .await
        .map_err(|error| format!("360 搜索请求失败：{error}"))?;
    info!(
        "[websearch][so_360] 响应: status={}, url={}",
        response.status(),
        response.url()
    );
    ensure_success_status(response.status())?;
    let html = read_search_body(response, "360 搜索").await?;
    info!(
        "[websearch][so_360] 响应正文: query={:?}, chars={}",
        query,
        html.chars().count()
    );
    save_debug_response(&query, &html).await;
    if looks_like_challenge(&html) {
        return Err("360 搜索返回了人机验证页面".to_string());
    }
    validate_response_query(&html, &query)?;

    let results = parse_search_results(&html, limit);
    if results.is_empty() {
        Err("360 搜索没有返回可解析的网页结果".to_string())
    } else {
        Ok(results)
    }
}

fn build_search_url(query: &str) -> Result<Url, String> {
    let mut url =
        Url::parse(SEARCH_ENDPOINT).map_err(|error| format!("360 搜索地址无效：{error}"))?;
    url.query_pairs_mut().append_pair("q", query);
    Ok(url)
}

fn ensure_success_status(status: reqwest::StatusCode) -> Result<(), String> {
    if status.as_u16() == 403 || status.as_u16() == 429 {
        return Err(format!("360 搜索暂时限制访问（HTTP {status}）"));
    }
    if !status.is_success() {
        return Err(format!("360 搜索返回异常状态（HTTP {status}）"));
    }
    Ok(())
}

fn validate_response_query(html: &str, expected: &str) -> Result<(), String> {
    let document = Document::from(html);
    let title = normalize_text(document.select_single("title").text().as_ref());
    let expected = compact(expected);
    if !expected.is_empty() && compact(&title).contains(&expected) {
        Ok(())
    } else {
        Err(format!(
            "360 搜索响应 query 与请求不一致（请求：{expected:?}，页面标题：{title:?}）"
        ))
    }
}

pub(super) fn parse_search_results(html: &str, limit: usize) -> Vec<SearchHit> {
    let document = Document::from(html);
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    for (index, card) in document.select("li.res-list").iter().enumerate() {
        if results.len() >= limit {
            break;
        }
        let link = card.select_single("h3.res-title a[href]");
        if !link.exists() {
            continue;
        }
        let title = normalize_text(link.text().as_ref());
        let raw_url = link
            .attr("data-mdurl")
            .filter(|value| !value.trim().is_empty())
            .or_else(|| link.attr("href"));
        let Some(url) = raw_url.and_then(|value| normalize_result_url(value.as_ref())) else {
            continue;
        };
        if title.is_empty() || !seen.insert(url.clone()) {
            continue;
        }
        let snippet =
            first_text(&card, &["p.res-desc", ".res-list-summary", "p"]).unwrap_or_default();
        let hit = SearchHit {
            source: PROVIDER,
            title,
            url,
            snippet: truncate_chars(&snippet, MAX_SNIPPET_CHARS),
        };
        info!(
            "[websearch][so_360] 匹配结果: index={}, title={:?}, url={}, snippet={:?}",
            index + 1,
            hit.title,
            hit.url,
            hit.snippet
        );
        results.push(hit);
    }
    results
}

fn normalize_result_url(raw: &str) -> Option<String> {
    let url = Url::parse(raw).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    if host == "so.com" || host.ends_with(".so.com") {
        return None;
    }
    let mut url = url;
    url.set_fragment(None);
    Some(url.to_string())
}

fn first_text(card: &Selection<'_>, selectors: &[&str]) -> Option<String> {
    selectors.iter().find_map(|selector| {
        let element = card.select_single(selector);
        if !element.exists() {
            return None;
        }
        let text = normalize_text(element.text().as_ref());
        (!text.is_empty()).then_some(text)
    })
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn compact(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn looks_like_challenge(html: &str) -> bool {
    ["请输入验证码", "访问过于频繁", "verify you are human"]
        .iter()
        .any(|needle| html.contains(needle))
}

#[cfg(debug_assertions)]
async fn save_debug_response(query: &str, html: &str) {
    let directory = std::env::temp_dir().join("buddy-websearch");
    if tokio::fs::create_dir_all(&directory).await.is_err() {
        return;
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let path = directory.join(format!("so360-response-{timestamp}.html"));
    match tokio::fs::write(&path, html).await {
        Ok(()) => info!(
            "[websearch][so_360] 响应快照已保存: query={:?}, path={}, bytes={}",
            query,
            path.display(),
            html.len()
        ),
        Err(error) => warn!(
            "[websearch][so_360] 保存响应快照失败: path={}, error={}",
            path.display(),
            error
        ),
    }
}

#[cfg(not(debug_assertions))]
async fn save_debug_response(_query: &str, _html: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_original_urls_titles_and_snippets() {
        let html = r#"
          <title>NVIDIA stock price today August 2026_360搜索</title>
          <li class="res-list"><h3 class="res-title">
            <a href="https://www.so.com/link?m=opaque" data-mdurl="https://example.com/NVDA?q=1&amp;lang=en">
              <em>NVIDIA</em> Stock Price Today
            </a></h3><p class="res-desc">2026年8月1日 - Latest NVDA quote.</p></li>
        "#;
        let results = parse_search_results(html, 5);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "NVIDIA Stock Price Today");
        assert_eq!(results[0].url, "https://example.com/NVDA?q=1&lang=en");
        assert!(results[0].snippet.contains("Latest NVDA quote"));
    }

    #[test]
    fn sends_plain_query_and_validates_response_title() {
        let query = "NVIDIA stock price today August 2026";
        let url = build_search_url(query).unwrap();
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "q")
                .map(|(_, value)| value.into_owned()),
            Some(query.to_string())
        );
        assert!(validate_response_query(
            "<title>NVIDIA stock price today August 2026_360搜索</title>",
            query
        )
        .is_ok());
    }

    #[test]
    fn rejects_internal_redirect_without_original_url() {
        assert!(normalize_result_url("https://www.so.com/link?m=opaque").is_none());
    }

    #[tokio::test]
    #[ignore = "requires external network"]
    async fn live_so360_returns_relevant_results() {
        let client = super::super::web_fetch::build_http_client().unwrap();
        for query in [
            "NVIDIA stock price today August 2026",
            "2026年7月新番 推荐 人气 口碑",
        ] {
            let results = search_so360(&client, query, 10).await.unwrap();
            eprintln!("query={query:?}, results={results:#?}");
            assert!(!results.is_empty());
        }
    }
}
