use super::aggregate::SearchHit;
use super::web_fetch::read_search_body;
use dom_query::{Document, Selection};
use reqwest::{
    header::{ACCEPT, LOCATION},
    Client, Url,
};
use std::collections::HashSet;

const SEARCH_ENDPOINT: &str = "https://cn.bing.com/search";
const MAX_SEARCH_REDIRECTS: usize = 2;
const MAX_SNIPPET_CHARS: usize = 600;

pub(super) const PROVIDER: &str = "cn_bing";

pub(super) async fn search_bing(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchHit>, String> {
    let mut url =
        Url::parse(SEARCH_ENDPOINT).map_err(|error| format!("Bing 中国搜索地址无效：{error}"))?;
    url.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("mkt", "zh-CN")
        .append_pair("setlang", "zh-hans")
        .append_pair("adlt", "off");

    for _ in 0..=MAX_SEARCH_REDIRECTS {
        let response = client
            .get(url.clone())
            .header(
                ACCEPT,
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.7")
            .header("DNT", "1")
            .send()
            .await
            .map_err(|error| format!("Bing 中国请求失败：{error}"))?;

        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "Bing 中国重定向缺少有效地址".to_string())?;
            let next_url = url
                .join(location)
                .map_err(|error| format!("Bing 中国重定向地址无效：{error}"))?;
            if !is_allowed_search_url(&next_url) {
                return Err("Bing 中国返回了不受信任的搜索重定向".to_string());
            }
            url = next_url;
            continue;
        }

        ensure_success_status(response.status())?;
        let html = read_search_body(response, "Bing 中国").await?;
        if looks_like_challenge(&html) {
            return Err("Bing 中国返回了人机验证页面".to_string());
        }

        let results = parse_search_results(&html, limit);
        return if results.is_empty() {
            Err("Bing 中国没有返回可解析的网页结果".to_string())
        } else {
            Ok(results)
        };
    }

    Err(format!("Bing 中国搜索重定向超过 {MAX_SEARCH_REDIRECTS} 次"))
}

fn ensure_success_status(status: reqwest::StatusCode) -> Result<(), String> {
    if status.as_u16() == 403 || status.as_u16() == 429 {
        return Err(format!("Bing 中国暂时限制访问（HTTP {status}）"));
    }
    if !status.is_success() {
        return Err(format!("Bing 中国返回异常状态（HTTP {status}）"));
    }
    Ok(())
}

fn is_allowed_search_url(url: &Url) -> bool {
    url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("cn.bing.com") | Some("www.bing.com") | Some("bing.com")
        )
        && url.path() == "/search"
}

pub(super) fn parse_search_results(html: &str, limit: usize) -> Vec<SearchHit> {
    let document = Document::from(html);
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    for card in document.select("ol#b_results li.b_algo").iter() {
        if results.len() >= limit {
            break;
        }
        let link = card.select_single("h2 a[href]");
        if !link.exists() {
            continue;
        }
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

        let snippet = first_text(&card, &[".b_caption p", ".b_snippet", "p"]).unwrap_or_default();
        results.push(SearchHit {
            source: PROVIDER,
            title,
            url,
            snippet: truncate_chars(&snippet, MAX_SNIPPET_CHARS),
        });
    }

    results
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

fn normalize_result_url(raw: &str) -> Option<String> {
    let base = Url::parse("https://cn.bing.com/").ok()?;
    let mut url = Url::parse(raw).or_else(|_| base.join(raw)).ok()?;

    if url.host_str().is_some_and(is_bing_host) {
        if url.path() != "/ck/a" {
            return None;
        }
        let encoded = url
            .query_pairs()
            .find(|(key, _)| key == "u")
            .map(|(_, value)| value.into_owned())?;
        let decoded = encoded
            .strip_prefix("a1")
            .and_then(decode_base64url)
            .and_then(|bytes| String::from_utf8(bytes).ok())?;
        url = Url::parse(&decoded).ok()?;
    }

    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }
    url.set_fragment(None);
    Some(url.to_string())
}

fn is_bing_host(host: &str) -> bool {
    host == "bing.com" || host.ends_with(".bing.com")
}

fn decode_base64url(value: &str) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(value.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u8;

    for byte in value.bytes().take_while(|byte| *byte != b'=') {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return None,
        } as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }

    Some(output)
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
    [
        "id=\"b_captcha\"",
        "unusual traffic",
        "verify you are human",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bing_results_and_decodes_redirects() {
        let html = r#"
        <ol id="b_results">
          <li class="b_algo">
            <h2><a href="https://example.com/docs#section">Example Docs</a></h2>
            <div class="b_caption"><p>Primary documentation result.</p></div>
          </li>
          <li class="b_algo">
            <h2><a href="https://cn.bing.com/ck/a?u=a1aHR0cHM6Ly9ydXN0LWxhbmcub3JnLw">Rust</a></h2>
            <p>Rust language homepage.</p>
          </li>
        </ol>
        "#;

        let results = parse_search_results(html, 5);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source, PROVIDER);
        assert_eq!(results[0].url, "https://example.com/docs");
        assert_eq!(results[0].snippet, "Primary documentation result.");
        assert_eq!(results[1].url, "https://rust-lang.org/");
    }

    #[test]
    fn rejects_internal_and_unsafe_result_urls() {
        assert!(normalize_result_url("https://cn.bing.com/search?q=rust").is_none());
        assert!(normalize_result_url("javascript:alert(1)").is_none());
    }

    #[test]
    fn only_allows_known_bing_search_redirects() {
        assert!(is_allowed_search_url(
            &Url::parse("https://www.bing.com/search?q=rust").unwrap()
        ));
        assert!(!is_allowed_search_url(
            &Url::parse("https://example.com/search?q=rust").unwrap()
        ));
    }
}
