use super::aggregate::SearchHit;
use super::relevance::filter_relevant_hits;
use super::web_fetch::read_search_body;
use dom_query::{Document, Selection};
use log::{info, warn};
use reqwest::{
    header::{ACCEPT, CACHE_CONTROL, LOCATION, PRAGMA},
    Client, Url,
};
use std::collections::HashSet;

const SEARCH_ENDPOINT: &str = "https://cn.bing.com/search";
const MAX_SEARCH_REDIRECTS: usize = 2;
const MAX_QUERY_ATTEMPTS: usize = 3;
const MAX_SNIPPET_CHARS: usize = 600;

pub(super) const PROVIDER: &str = "cn_bing";

pub(super) async fn search_bing(
    client: &Client,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchHit>, String> {
    let mut last_query_error = None;
    let relaxed_query = relaxed_query(query).map(|value| bing_query(&value));
    let mut effective_query = bing_query(query);
    let mut bypass_cache = false;
    let mut used_relaxed_query = false;

    for query_attempt in 0..MAX_QUERY_ATTEMPTS {
        let url = build_search_url(&effective_query, bypass_cache)?;
        let html = request_search_page(client, &effective_query, url, query_attempt).await?;
        save_debug_response(&effective_query, &html).await;
        if looks_like_challenge(&html) {
            return Err("Bing 中国返回了人机验证页面".to_string());
        }
        if let Err(error) = validate_response_query(&html, &effective_query) {
            warn!(
                "[websearch][cn_bing] 响应 query 校验失败: attempt={}, expected={:?}, error={}",
                query_attempt, effective_query, error
            );
            last_query_error = Some(error);
            bypass_cache = true;
            continue;
        }

        let results = parse_search_results(&html, limit);
        let relevant = filter_relevant_hits(query, results, limit);
        if !relevant.is_empty() {
            log_search_results(query, &relevant);
            return Ok(relevant);
        }

        let error =
            format!("Bing 中国仅返回与 query 无明显关联的结果（实际搜索：{effective_query:?}）");
        warn!(
            "[websearch][cn_bing] 相关结果为空: query={:?}, effective_query={:?}",
            query, effective_query
        );
        last_query_error = Some(error);
        if !used_relaxed_query {
            if let Some(relaxed) = &relaxed_query {
                info!(
                    "[websearch][cn_bing] 使用分词降级查询: original={:?}, relaxed={:?}",
                    query, relaxed
                );
                effective_query = relaxed.clone();
                used_relaxed_query = true;
                bypass_cache = true;
                continue;
            }
        }
        break;
    }

    Err(last_query_error.unwrap_or_else(|| "Bing 中国没有返回相关搜索结果".to_string()))
}

fn bing_query(query: &str) -> String {
    let phrase = normalize_text(&query.replace('"', " "));
    if phrase.is_empty() {
        return query.to_string();
    }
    if phrase.chars().any(is_cjk_character) {
        exact_phrase_query(&phrase)
    } else {
        phrase
    }
}

fn exact_phrase_query(query: &str) -> String {
    let phrase = normalize_text(&query.replace('"', " "));
    if phrase.is_empty() {
        query.to_string()
    } else if let Some((year, topic)) = split_leading_year(&phrase) {
        format!("\"{year} {topic}\"")
    } else {
        format!("\"{phrase}\"")
    }
}

fn split_leading_year(query: &str) -> Option<(&str, &str)> {
    let digit_end = query
        .char_indices()
        .take_while(|(_, character)| character.is_ascii_digit())
        .map(|(index, character)| index + character.len_utf8())
        .last()?;
    let year = query.get(..digit_end)?;
    if year.len() != 4 {
        return None;
    }
    let suffix = query.get(digit_end..)?;
    let topic = if let Some(topic) = suffix.strip_prefix('年') {
        topic.trim()
    } else if suffix.chars().next().is_some_and(char::is_whitespace) {
        suffix.trim()
    } else {
        return None;
    };
    (!topic.is_empty()).then_some((year, topic))
}

fn relaxed_query(query: &str) -> Option<String> {
    let characters = query.chars().collect::<Vec<_>>();
    let mut relaxed = String::with_capacity(query.len());
    let mut changed = false;
    for (index, character) in characters.iter().copied().enumerate() {
        let is_inner_modifier = matches!(character, '新' | '的' | '之')
            && index > 0
            && index + 1 < characters.len()
            && is_cjk_character(characters[index - 1])
            && is_cjk_character(characters[index + 1]);
        if is_inner_modifier {
            changed = true;
        } else {
            relaxed.push(character);
        }
    }
    let relaxed = normalize_text(&relaxed);
    (changed && !relaxed.is_empty()).then_some(relaxed)
}

fn is_cjk_character(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{3040}'..='\u{30ff}'
            | '\u{ac00}'..='\u{d7af}'
    )
}

fn build_search_url(query: &str, bypass_cache: bool) -> Result<Url, String> {
    let mut url =
        Url::parse(SEARCH_ENDPOINT).map_err(|error| format!("Bing 中国搜索地址无效：{error}"))?;
    url.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("mkt", "zh-CN")
        .append_pair("setlang", "zh-hans")
        .append_pair("adlt", "off");
    if bypass_cache {
        url.query_pairs_mut()
            .append_pair("_", &request_nonce().to_string());
    }
    Ok(url)
}

async fn request_search_page(
    client: &Client,
    query: &str,
    mut url: Url,
    query_attempt: usize,
) -> Result<String, String> {
    for redirect_attempt in 0..=MAX_SEARCH_REDIRECTS {
        info!(
            "[websearch][cn_bing] 请求: query_attempt={}, redirect_attempt={}, query={:?}, url={}, accept_language=zh-CN,zh;q=0.9,en;q=0.7",
            query_attempt, redirect_attempt, query, url
        );
        let response = client
            .get(url.clone())
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
            .map_err(|error| format!("Bing 中国请求失败：{error}"))?;
        info!(
            "[websearch][cn_bing] 响应: query_attempt={}, redirect_attempt={}, status={}, url={}",
            query_attempt,
            redirect_attempt,
            response.status(),
            response.url()
        );

        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "Bing 中国重定向缺少有效地址".to_string())?;
            let next_url = url
                .join(location)
                .map_err(|error| format!("Bing 中国重定向地址无效：{error}"))?;
            if !is_allowed_search_url(&next_url, query) {
                return Err("Bing 中国返回了不受信任或 query 不一致的搜索重定向".to_string());
            }
            url = next_url;
            continue;
        }

        ensure_success_status(response.status())?;
        let html = read_search_body(response, "Bing 中国").await?;
        info!(
            "[websearch][cn_bing] 响应正文: query={:?}, chars={}",
            query,
            html.chars().count()
        );
        return Ok(html);
    }

    Err(format!("Bing 中国搜索重定向超过 {MAX_SEARCH_REDIRECTS} 次"))
}

fn request_nonce() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[cfg(debug_assertions)]
async fn save_debug_response(query: &str, html: &str) {
    let directory = std::env::temp_dir().join("buddy-websearch");
    if let Err(error) = tokio::fs::create_dir_all(&directory).await {
        warn!(
            "[websearch][cn_bing] 创建响应快照目录失败: path={}, error={}",
            directory.display(),
            error
        );
        return;
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let path = directory.join(format!("bing-response-{timestamp}.html"));
    match tokio::fs::write(&path, html).await {
        Ok(()) => info!(
            "[websearch][cn_bing] 响应快照已保存: query={:?}, path={}, bytes={}",
            query,
            path.display(),
            html.len()
        ),
        Err(error) => warn!(
            "[websearch][cn_bing] 保存响应快照失败: query={:?}, path={}, error={}",
            query,
            path.display(),
            error
        ),
    }
}

#[cfg(not(debug_assertions))]
async fn save_debug_response(_query: &str, _html: &str) {}

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
        "[websearch][cn_bing] 解析结果: query={:?}, count={}, results=[{}]",
        query,
        results.len(),
        summary
    );
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

fn is_allowed_search_url(url: &Url, query: &str) -> bool {
    url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("cn.bing.com") | Some("www.bing.com") | Some("bing.com")
        )
        && url.path() == "/search"
        && url
            .query_pairs()
            .find(|(key, _)| key == "q")
            .is_some_and(|(_, value)| queries_match(value.as_ref(), query))
}

fn validate_response_query(html: &str, expected: &str) -> Result<(), String> {
    let actual = extract_response_query(html)
        .ok_or_else(|| "Bing 中国响应中缺少可验证的 query".to_string())?;
    if queries_match(&actual, expected) {
        Ok(())
    } else {
        Err(format!(
            "Bing 中国响应 query 与请求不一致（请求：{expected:?}，响应：{actual:?}）"
        ))
    }
}

fn extract_response_query(html: &str) -> Option<String> {
    let document = Document::from(html);
    ["#sb_form_q[value]", "input[name=q][value]"]
        .iter()
        .find_map(|selector| {
            let input = document.select_single(selector);
            let value = input.attr("value")?;
            let normalized = normalize_text(value.as_ref());
            (!normalized.is_empty()).then_some(normalized)
        })
}

fn queries_match(left: &str, right: &str) -> bool {
    normalize_query(left) == normalize_query(right)
}

fn normalize_query(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

pub(super) fn parse_search_results(html: &str, limit: usize) -> Vec<SearchHit> {
    let document = Document::from(html);
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    for (card_index, card) in document.select("ol#b_results li.b_algo").iter().enumerate() {
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
        let result = SearchHit {
            source: PROVIDER,
            title,
            url,
            snippet: truncate_chars(&snippet, MAX_SNIPPET_CHARS),
        };
        info!(
            "[websearch][cn_bing] 匹配结果: css=ol#b_results li.b_algo > h2 a[href], xpath=//*[@id=\"b_results\"]/li[contains(concat(\" \", normalize-space(@class), \" \"), \" b_algo \")][{}]/h2/a, title={:?}, url={}, snippet={:?}",
            card_index + 1,
            result.title,
            result.url,
            result.snippet
        );
        results.push(result);
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
            &Url::parse("https://www.bing.com/search?q=rust").unwrap(),
            "rust"
        ));
        assert!(!is_allowed_search_url(
            &Url::parse("https://example.com/search?q=rust").unwrap(),
            "rust"
        ));
        assert!(!is_allowed_search_url(
            &Url::parse("https://www.bing.com/search?q=stale").unwrap(),
            "rust"
        ));
    }

    #[test]
    fn validates_query_embedded_in_bing_response() {
        let html = r#"<input id="sb_form_q" name="q" value="Rust language">"#;
        assert!(validate_response_query(html, "rust-language").is_ok());
        assert!(validate_response_query(html, "unrelated query").is_err());
    }

    #[test]
    fn cache_bypass_keeps_query_and_changes_request_url() {
        let normal = build_search_url("缓存 查询", false).unwrap();
        let bypassed = build_search_url("缓存 查询", true).unwrap();

        assert_eq!(
            normal
                .query_pairs()
                .find(|(key, _)| key == "q")
                .map(|(_, value)| value.into_owned()),
            Some("缓存 查询".to_string())
        );
        assert_ne!(normal, bypassed);
        assert!(bypassed.query_pairs().any(|(key, _)| key == "_"));
    }

    #[test]
    fn wraps_bing_query_as_an_exact_phrase() {
        assert_eq!(
            exact_phrase_query("  2026年夏季新番  "),
            "\"2026 夏季新番\""
        );
        assert_eq!(
            exact_phrase_query("2026年7月新番 推荐 人气 关注度 口碑"),
            "\"2026 7月新番 推荐 人气 关注度 口碑\""
        );
        assert_eq!(exact_phrase_query("2026 夏季新番"), "\"2026 夏季新番\"");
        assert_eq!(exact_phrase_query("\"英伟达\""), "\"英伟达\"");
        assert_eq!(exact_phrase_query("2026世界杯"), "\"2026世界杯\"");
    }

    #[test]
    fn sends_plain_english_queries_without_exact_phrase_quotes() {
        let query = bing_query("\"NVIDIA stock price today August 2026\"");
        let url = build_search_url(&query, false).unwrap();

        assert_eq!(query, "NVIDIA stock price today August 2026");
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "q")
                .map(|(_, value)| value.into_owned()),
            Some("NVIDIA stock price today August 2026".to_string())
        );
    }

    #[test]
    fn sends_the_whole_exact_phrase_as_the_bing_q_parameter() {
        let exact = exact_phrase_query("2026年7月新番 推荐 人气 关注度 口碑");
        let url = build_search_url(&exact, false).unwrap();

        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "q")
                .map(|(_, value)| value.into_owned()),
            Some("\"2026 7月新番 推荐 人气 关注度 口碑\"".to_string())
        );
    }

    #[test]
    fn relaxes_inner_cjk_modifiers_for_bing_tokenization_fallback() {
        assert_eq!(
            relaxed_query("2026 夏季新番"),
            Some("2026 夏季番".to_string())
        );
        assert_eq!(relaxed_query("法国的总统"), Some("法国总统".to_string()));
        assert_eq!(relaxed_query("新世界"), None);
    }

    #[tokio::test]
    #[ignore = "requires external network"]
    async fn live_bing_preserves_query_relevance() {
        let client = super::super::web_fetch::build_http_client().unwrap();
        let results = search_bing(&client, "2026年夏季新番", 5).await.unwrap();

        eprintln!("{results:#?}");
        assert!(
            results.iter().any(|result| {
                let text = format!("{} {}", result.title, result.snippet);
                text.contains("夏季新番") || text.contains("7月新番") || text.contains("夏季番")
            }),
            "Bing returned only low-relevance results: {results:#?}"
        );
    }

    #[tokio::test]
    #[ignore = "requires external network"]
    async fn live_bing_exact_entity_search_keeps_entity_in_title() {
        let client = super::super::web_fetch::build_http_client().unwrap();
        let results = search_bing(&client, "英伟达", 5).await.unwrap();

        eprintln!("{results:#?}");
        assert!(
            results
                .iter()
                .all(|result| result.title.contains("英伟达") || result.title.contains("NVIDIA")),
            "Bing returned competitor-only titles: {results:#?}"
        );
    }

    #[tokio::test]
    #[ignore = "requires external network"]
    async fn live_bing_raw_response_snapshot() {
        let client = super::super::web_fetch::build_http_client().unwrap();
        let query = "2026 夏季新番";
        let mut url = Url::parse(SEARCH_ENDPOINT).unwrap();
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("mkt", "zh-CN")
            .append_pair("setlang", "zh-hans")
            .append_pair("adlt", "off");

        let response = client
            .get(url)
            .header(
                ACCEPT,
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.7")
            .header("DNT", "1")
            .send()
            .await
            .unwrap();
        eprintln!(
            "status={}, version={:?}, url={}, remote_addr={:?}",
            response.status(),
            response.version(),
            response.url(),
            response.remote_addr()
        );
        for name in [
            "content-type",
            "content-encoding",
            "content-length",
            "server",
            "vary",
            "x-cache",
            "x-msedge-ref",
        ] {
            if let Some(value) = response.headers().get(name) {
                eprintln!("header {name}={:?}", value);
            }
        }

        let html = read_search_body(response, "Bing 中国").await.unwrap();
        let output = std::path::Path::new("/private/tmp/buddy-bing-rust-response.html");
        std::fs::write(output, &html).unwrap();
        eprintln!(
            "body_path={}, bytes={}, chars={}",
            output.display(),
            html.len(),
            html.chars().count()
        );
        for (index, result) in parse_search_results(&html, 5).iter().enumerate() {
            eprintln!("#{} {:?}\t{}", index + 1, result.title, result.url);
        }
    }
}
