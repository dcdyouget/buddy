use super::aggregate::SearchHit;
use super::web_fetch::read_search_body;
use dom_query::{Document, Selection};
use log::info;
use reqwest::{Client, Url};
use serde::Deserialize;
use std::collections::HashSet;

const SEARCH_PAGE: &str = "https://duckduckgo.com/";
const SEARCH_ENDPOINT: &str = "https://html.duckduckgo.com/html/";
const MAX_SNIPPET_CHARS: usize = 600;

pub(super) const PROVIDER: &str = "duckduckgo";

pub(super) async fn search_duckduckgo(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchHit>, String> {
    let modern_result = search_modern_page(client, query, limit).await;
    let result = if let Ok(results) = &modern_result {
        if !results.is_empty() {
            Ok(results.clone())
        } else {
            search_legacy_page(client, query, limit).await
        }
    } else {
        match search_legacy_page(client, query, limit).await {
            Ok(results) if !results.is_empty() => Ok(results),
            Ok(_) => modern_result,
            Err(legacy_error) => match modern_result {
                Ok(_) => Err(legacy_error),
                Err(modern_error) => Err(format!(
                    "DuckDuckGo 搜索失败：主页面链路：{modern_error}；HTML 链路：{legacy_error}"
                )),
            },
        }
    };

    match &result {
        Ok(results) => log_search_results(query, results),
        Err(error) => info!(
            "[websearch][duckduckgo] 搜索失败: query={:?}, error={}",
            query, error
        ),
    }
    result
}

async fn search_modern_page(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchHit>, String> {
    let mut search_url =
        Url::parse(SEARCH_PAGE).map_err(|error| format!("主搜索页地址无效：{error}"))?;
    search_url
        .query_pairs_mut()
        .extend_pairs([("q", query), ("ia", "web")]);
    info!(
        "[websearch][duckduckgo] 主搜索页请求: query={:?}, url={}, accept_language=zh-CN,zh;q=0.9,en;q=0.8",
        query, search_url
    );
    let response = client
        .get(search_url)
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .send()
        .await
        .map_err(|error| format!("主搜索页请求失败：{error}"))?;
    info!(
        "[websearch][duckduckgo] 主搜索页响应: status={}, url={}",
        response.status(),
        response.url()
    );
    ensure_success_status(response.status())?;

    let html = read_search_body(response, "主搜索页").await?;
    info!(
        "[websearch][duckduckgo] 主搜索页正文: query={:?}, chars={}",
        query,
        html.chars().count()
    );
    if looks_like_challenge(&html) {
        return Err("主搜索页返回了人机验证".to_string());
    }

    let script_url = extract_results_script_url(&html)
        .ok_or_else(|| "主搜索页没有返回结果数据地址".to_string())?;
    info!(
        "[websearch][duckduckgo] 结果数据请求: query={:?}, url={}",
        query, script_url
    );
    let response = client
        .get(script_url)
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .header("Referer", SEARCH_PAGE)
        .send()
        .await
        .map_err(|error| format!("结果数据请求失败：{error}"))?;
    info!(
        "[websearch][duckduckgo] 结果数据响应: status={}, url={}",
        response.status(),
        response.url()
    );
    ensure_success_status(response.status())?;

    let script = read_search_body(response, "结果数据").await?;
    info!(
        "[websearch][duckduckgo] 结果数据正文: query={:?}, chars={}",
        query,
        script.chars().count()
    );
    if looks_like_challenge(&script) {
        return Err("结果数据返回了人机验证".to_string());
    }

    let results = parse_modern_results(&script, limit);
    if results.is_empty() {
        Err("结果数据中没有可解析的网页结果".to_string())
    } else {
        Ok(results)
    }
}

async fn search_legacy_page(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchHit>, String> {
    info!(
        "[websearch][duckduckgo] HTML 请求: query={:?}, url={}, region=wt-wt, accept_language=zh-CN,zh;q=0.9,en;q=0.8",
        query, SEARCH_ENDPOINT
    );
    let response = client
        .post(SEARCH_ENDPOINT)
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .header("Referer", "https://duckduckgo.com/")
        .form(&[("q", query), ("b", ""), ("l", "wt-wt")])
        .send()
        .await
        .map_err(|error| format!("DuckDuckGo 请求失败：{error}"))?;
    info!(
        "[websearch][duckduckgo] HTML 响应: status={}, url={}",
        response.status(),
        response.url()
    );

    ensure_success_status(response.status())?;

    let html = read_search_body(response, "DuckDuckGo").await?;
    info!(
        "[websearch][duckduckgo] HTML 正文: query={:?}, chars={}",
        query,
        html.chars().count()
    );
    if looks_like_challenge(&html) {
        return Err("DuckDuckGo 返回了人机验证页面".to_string());
    }

    Ok(parse_search_results(&html, limit))
}

fn log_search_results(query: &str, results: &[SearchHit]) {
    let summary = results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            format!(
                "#{} title={:?} url={} snippet={:?}",
                index + 1,
                result.title,
                result.url,
                result.snippet
            )
        })
        .collect::<Vec<_>>()
        .join(" | ");
    info!(
        "[websearch][duckduckgo] 解析结果: query={:?}, count={}, results=[{}]",
        query,
        results.len(),
        summary
    );
}

fn ensure_success_status(status: reqwest::StatusCode) -> Result<(), String> {
    if status.as_u16() == 403 || status.as_u16() == 429 {
        return Err(format!("DuckDuckGo 暂时限制访问（HTTP {status}）"));
    }
    if !status.is_success() {
        return Err(format!("DuckDuckGo 返回异常状态（HTTP {status}）"));
    }
    Ok(())
}

fn extract_results_script_url(html: &str) -> Option<Url> {
    let document = Document::from(html);
    let source = document
        .select_single("script#deep_preload_script[src]")
        .attr("src")?;
    let url = Url::parse(SEARCH_PAGE).ok()?.join(source.as_ref()).ok()?;
    if url.scheme() == "https"
        && url.host_str() == Some("links.duckduckgo.com")
        && url.path() == "/d.js"
    {
        Some(url)
    } else {
        None
    }
}

#[derive(Deserialize)]
struct ModernResult {
    #[serde(default)]
    t: String,
    #[serde(default)]
    u: String,
    #[serde(default)]
    a: String,
}

fn parse_modern_results(script: &str, limit: usize) -> Vec<SearchHit> {
    const MARKER: &str = "pageLayout.load('d',";
    let mut remaining = script;

    while let Some(marker_index) = remaining.find(MARKER) {
        let after_marker = &remaining[marker_index + MARKER.len()..];
        let Some(array_start) = after_marker.find('[') else {
            break;
        };
        let array_source = &after_marker[array_start..];
        let Some(array) = extract_json_array(array_source) else {
            break;
        };

        if let Ok(raw_results) = serde_json::from_str::<Vec<ModernResult>>(array) {
            let mut seen = HashSet::new();
            let results = raw_results
                .into_iter()
                .filter_map(|result| {
                    let title = normalize_html_text(&result.t);
                    let url = normalize_result_url(&result.u)?;
                    if title.is_empty() || !seen.insert(url.clone()) {
                        return None;
                    }
                    Some(SearchHit {
                        source: PROVIDER,
                        title,
                        url,
                        snippet: truncate_chars(&normalize_html_text(&result.a), MAX_SNIPPET_CHARS),
                    })
                })
                .take(limit)
                .collect::<Vec<_>>();
            if !results.is_empty() {
                return results;
            }
        }

        remaining = &array_source[array.len()..];
    }

    Vec::new()
}

fn extract_json_array(source: &str) -> Option<&str> {
    if !source.starts_with('[') {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, character) in source.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }

        match character {
            '"' => in_string = true,
            '[' => depth += 1,
            ']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return source.get(..=index);
                }
            }
            _ => {}
        }
    }
    None
}

pub(super) fn parse_search_results(html: &str, limit: usize) -> Vec<SearchHit> {
    let document = Document::from(html);
    let card_selectors = [
        "div.result",
        "div.web-result",
        "div.results_links",
        "div.body",
    ];
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    for card_selector in card_selectors {
        for card in document.select(card_selector).iter() {
            if results.len() >= limit {
                return results;
            }
            let Some(link) = first_match(&card, &["a.result__a", "h2 a", "a[href]"]) else {
                continue;
            };
            let title = normalize_text(link.text().as_ref());
            let Some(raw_url) = link.attr("href") else {
                continue;
            };
            let Some(url) = normalize_result_url(raw_url.as_ref()) else {
                continue;
            };
            if title.is_empty() || !seen.insert(url.clone()) {
                continue;
            }

            let snippet = first_match(
                &card,
                &[
                    ".result__snippet",
                    ".result-snippet",
                    "a.result__snippet",
                    ".snippet",
                ],
            )
            .map(|element| normalize_text(element.text().as_ref()))
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| fallback_snippet(&card, &title));

            results.push(SearchHit {
                source: PROVIDER,
                title,
                url,
                snippet: truncate_chars(&snippet, MAX_SNIPPET_CHARS),
            });
        }
    }

    results
}

fn first_match<'a>(card: &Selection<'a>, selectors: &[&str]) -> Option<Selection<'a>> {
    for selector in selectors {
        let element = card.select_single(selector);
        if element.exists() {
            return Some(element);
        }
    }
    None
}

fn normalize_result_url(raw: &str) -> Option<String> {
    let base = Url::parse("https://duckduckgo.com/").ok()?;
    let mut url = Url::parse(raw).or_else(|_| base.join(raw)).ok()?;

    if url
        .host_str()
        .is_some_and(|host| host.ends_with("duckduckgo.com"))
    {
        if url.path().contains("/y.js") {
            return None;
        }
        if let Some(target) = url
            .query_pairs()
            .find(|(key, _)| key == "uddg")
            .map(|(_, value)| value.into_owned())
        {
            url = Url::parse(&target).ok()?;
        }
    }

    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }
    url.set_fragment(None);
    Some(url.to_string())
}

fn fallback_snippet(card: &Selection<'_>, title: &str) -> String {
    let card_text = card.text();
    let full = normalize_text(card_text.as_ref());
    full.strip_prefix(title)
        .map(str::trim)
        .unwrap_or(&full)
        .to_string()
}

fn normalize_html_text(value: &str) -> String {
    let fragment = Document::fragment(value);
    normalize_text(fragment.text().as_ref())
}

fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
    let lower = html.to_ascii_lowercase();
    ["anomaly-modal", "captcha", "verify you are human"]
        .iter()
        .any(|needle| lower.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_html_results_and_decodes_redirects() {
        let html = r#"
        <html><body>
          <div class="result results_links">
            <h2>
              <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fdocs%3Fid%3D1">
                Example documentation
              </a>
            </h2>
            <a class="result__snippet">A concise documentation result.</a>
          </div>
          <div class="result">
            <h2><a class="result__a" href="https://rust-lang.org/">Rust</a></h2>
            <div class="result__snippet">Rust language homepage.</div>
          </div>
        </body></html>
        "#;

        let results = parse_search_results(html, 5);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Example documentation");
        assert_eq!(results[0].url, "https://example.com/docs?id=1");
        assert_eq!(results[0].snippet, "A concise documentation result.");
        assert_eq!(results[1].url, "https://rust-lang.org/");
    }

    #[test]
    fn filters_ad_redirects_and_duplicate_urls() {
        let html = r#"
        <div class="result"><h2><a href="https://duckduckgo.com/y.js?ad=1">Ad</a></h2></div>
        <div class="result"><h2><a href="https://example.com/">One</a></h2></div>
        <div class="result"><h2><a href="https://example.com/">Duplicate</a></h2></div>
        "#;
        let results = parse_search_results(html, 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "One");
    }

    #[test]
    fn extracts_modern_result_script_url() {
        let html = r#"
        <html><head>
          <script id="deep_preload_script"
            src="https://links.duckduckgo.com/d.js?q=rust&amp;vqd=4-123"></script>
        </head></html>
        "#;
        let url = extract_results_script_url(html).expect("script URL");
        assert_eq!(
            url.as_str(),
            "https://links.duckduckgo.com/d.js?q=rust&vqd=4-123"
        );

        let untrusted = r#"
        <script id="deep_preload_script"
          src="https://example.com/d.js?q=rust"></script>
        "#;
        assert!(extract_results_script_url(untrusted).is_none());
    }

    #[test]
    fn parses_modern_script_results_and_nested_arrays() {
        let script = r#"
        if (DDG.pageLayout) DDG.pageLayout.load('d',[
          {
            "a":"A <b>safe</b> systems language.",
            "l":[{"text":"nested ] value"}],
            "t":"The <span>Rust</span> Language",
            "u":"https://www.rust-lang.org/"
          },
          {"a":"Documentation","t":"Rust Book","u":"https://doc.rust-lang.org/book/"},
          {"n":"/d.js?page=2"}
        ]);
        "#;

        let results = parse_modern_results(script, 5);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "The Rust Language");
        assert_eq!(results[0].snippet, "A safe systems language.");
        assert_eq!(results[1].url, "https://doc.rust-lang.org/book/");
    }
}
