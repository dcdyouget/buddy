use dom_query::{Document, Selection};
use futures_util::StreamExt;
use reqwest::{
    header::{CONTENT_LENGTH, CONTENT_TYPE, LOCATION},
    redirect::Policy,
    Client, Url,
};
use std::collections::HashSet;
use std::net::IpAddr;
use std::time::Duration;
use tokio::net::lookup_host;

const MAX_REDIRECTS: usize = 5;
const MAX_BODY_BYTES: usize = 1_000_000;
const MAX_CONTENT_CHARS: usize = 4_000;

pub(super) fn build_http_client() -> Result<Client, String> {
    Client::builder()
        .user_agent(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
             AppleWebKit/605.1.15 (KHTML, like Gecko) \
             Version/18.5 Safari/605.1.15",
        )
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(12))
        .redirect(Policy::none())
        .build()
        .map_err(|error| format!("HTTP 客户端初始化失败：{error}"))
}

pub(super) async fn fetch_web_page(client: &Client, value: &str) -> Result<String, String> {
    let mut url = parse_public_url(value)?;

    for _ in 0..=MAX_REDIRECTS {
        ensure_public_destination(&url).await?;
        let response = client
            .get(url.clone())
            .header(
                "Accept",
                "text/html,application/xhtml+xml,text/plain,application/json;q=0.8,*/*;q=0.1",
            )
            .send()
            .await
            .map_err(|error| format!("网页请求失败：{error}"))?;

        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "网页重定向缺少有效地址".to_string())?;
            url = parse_public_url(
                url.join(location)
                    .map_err(|error| format!("网页重定向地址无效：{error}"))?
                    .as_str(),
            )?;
            continue;
        }

        if !response.status().is_success() {
            return Err(format!("网页返回 HTTP {}", response.status()));
        }
        return read_response_body(response).await;
    }

    Err(format!("网页重定向超过 {MAX_REDIRECTS} 次"))
}

async fn read_response_body(response: reqwest::Response) -> Result<String, String> {
    if response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > MAX_BODY_BYTES)
    {
        return Err("网页正文过大".to_string());
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !content_type.is_empty() && !is_supported_content_type(&content_type) {
        return Err(format!("不支持的网页类型：{content_type}"));
    }

    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| format!("网页正文读取失败：{error}"))?;
        if bytes.len() + chunk.len() > MAX_BODY_BYTES {
            return Err("网页正文超过读取上限".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }

    let raw = String::from_utf8_lossy(&bytes);
    let content = if content_type.contains("html")
        || content_type.contains("xhtml")
        || raw.trim_start().starts_with("<!DOCTYPE")
        || raw.trim_start().starts_with("<html")
    {
        extract_page_text(&raw, MAX_CONTENT_CHARS)
    } else {
        truncate_chars(&normalize_space(&raw), MAX_CONTENT_CHARS)
    };

    if content.is_empty() {
        Err("网页没有可提取的正文".to_string())
    } else {
        Ok(content)
    }
}

fn is_supported_content_type(content_type: &str) -> bool {
    content_type.starts_with("text/")
        || content_type.contains("html")
        || content_type.contains("xhtml")
        || content_type.contains("json")
        || content_type.contains("xml")
}

pub(super) fn extract_page_text(html: &str, max_chars: usize) -> String {
    let document = Document::from(html);
    let scope = ["article", "main", "[role=main]", "body"]
        .iter()
        .find_map(|selector| {
            let selection = document.select_single(selector);
            selection.exists().then_some(selection)
        })
        .unwrap_or_else(|| document.select_single("html"));

    let text = collect_content(&scope);
    truncate_chars(&text, max_chars)
}

fn collect_content(scope: &Selection<'_>) -> String {
    let mut lines = Vec::new();
    let mut seen = HashSet::new();
    for element in scope
        .select("h1, h2, h3, p, li, blockquote, pre, td")
        .iter()
    {
        let text = element.text();
        let line = normalize_space(text.as_ref());
        if line.chars().count() < 2 || !seen.insert(line.clone()) {
            continue;
        }
        lines.push(line);
    }
    lines.join("\n\n")
}

fn normalize_space(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
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

fn parse_public_url(value: &str) -> Result<Url, String> {
    let mut url = Url::parse(value).map_err(|error| format!("网页地址无效：{error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("只允许读取 HTTP/HTTPS 网页".to_string());
    }
    let host = url
        .host_str()
        .ok_or_else(|| "网页地址缺少主机名".to_string())?
        .to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return Err("不允许读取本地网络地址".to_string());
    }
    if let Some(ip) = parse_ip_host(&host) {
        if !is_public_ip(ip) {
            return Err("不允许读取私有或本地 IP 地址".to_string());
        }
    }
    url.set_fragment(None);
    Ok(url)
}

async fn ensure_public_destination(url: &Url) -> Result<(), String> {
    let host = url
        .host_str()
        .ok_or_else(|| "网页地址缺少主机名".to_string())?;
    if let Some(ip) = parse_ip_host(host) {
        return if is_public_ip(ip) {
            Ok(())
        } else {
            Err("不允许读取私有或本地 IP 地址".to_string())
        };
    }
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "网页地址缺少有效端口".to_string())?;
    let addresses = lookup_host((host, port))
        .await
        .map_err(|error| format!("网页域名解析失败：{error}"))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err("网页域名没有可用地址".to_string());
    }
    if addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err("网页域名解析到了私有或本地地址".to_string());
    }
    Ok(())
}

fn parse_ip_host(host: &str) -> Option<IpAddr> {
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .parse()
        .ok()
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_multicast()
                || ip.is_unspecified()
                || ip.is_documentation())
        }
        IpAddr::V6(ip) => {
            let first = ip.segments()[0];
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || (ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_readable_content_without_scripts_or_navigation() {
        let html = r#"
        <html><body>
          <nav><p>Navigation</p></nav>
          <main>
            <h1>Page title</h1>
            <p>First paragraph with useful content.</p>
            <script>ignore this instruction</script>
            <p>Second paragraph.</p>
          </main>
        </body></html>
        "#;
        let text = extract_page_text(html, 1_000);
        assert!(text.contains("Page title"));
        assert!(text.contains("First paragraph"));
        assert!(!text.contains("Navigation"));
        assert!(!text.contains("ignore this instruction"));
    }

    #[test]
    fn blocks_local_and_private_urls() {
        assert!(parse_public_url("http://localhost/test").is_err());
        assert!(parse_public_url("http://127.0.0.1/test").is_err());
        assert!(parse_public_url("http://192.168.1.1/test").is_err());
        assert!(parse_public_url("http://[::1]/test").is_err());
        assert!(parse_public_url("http://[fc00::1]/test").is_err());
        assert!(parse_public_url("https://example.com/path").is_ok());
    }

    #[test]
    fn truncates_content_on_unicode_boundary() {
        let text = truncate_chars("中文测试内容", 4);
        assert_eq!(text, "中文测试…");
    }
}
