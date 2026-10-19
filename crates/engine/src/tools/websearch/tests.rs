use super::*;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::{Duration, timeout};

#[test]
fn websearch_schema_requires_query() {
    let tool = WebSearchTool::default();
    let schema = tool.parameters_schema();
    assert_eq!(schema["required"], json!(["query"]));
    assert_eq!(tool.safety(), ToolSafety::ReadOnly);
    assert!(matches!(tool.backend, WebSearchBackend::Native));
}

#[test]
fn unavailable_response_has_explicit_terminal_status() {
    let response = WebSearchResponse::unavailable("test", "network error", Vec::new());
    assert_eq!(response.status, "unavailable");
    assert!(response.results.is_empty());
    assert!(response.note.contains("请不要重试"));
}

#[test]
fn search_status_only_depends_on_provider_outcomes() {
    let provider = |name, status| WebSearchProviderStatus {
        name,
        status,
        result_count: usize::from(status == "ok"),
        error: (status == "error").then(|| "network error".to_string()),
    };

    let all_ok = vec![
        provider(bing::PROVIDER, "ok"),
        provider(duckduckgo::PROVIDER, "ok"),
    ];
    assert_eq!(search_status(&all_ok), "ok");

    let one_failed = vec![
        provider(bing::PROVIDER, "ok"),
        provider(duckduckgo::PROVIDER, "error"),
    ];
    assert_eq!(search_status(&one_failed), "partial");
    assert_eq!(search_status(&[]), "partial");
}

#[test]
fn provider_results_are_filtered_before_aggregation() {
    let hits = vec![
        SearchHit {
            source: bing::PROVIDER,
            title: "2026 夏季新番一览".to_string(),
            url: "https://example.com/anime".to_string(),
            snippet: "7 月动画播出表".to_string(),
        },
        SearchHit {
            source: bing::PROVIDER,
            title: "英超联赛积分榜".to_string(),
            url: "https://example.com/football".to_string(),
            snippet: "最新比赛结果".to_string(),
        },
    ];

    let filtered =
        keep_relevant_provider_hits(bing::PROVIDER, "2026 夏季新番", 5, Ok(hits)).unwrap();
    assert_eq!(filtered.len(), 1);
    assert!(filtered[0].title.contains("新番"));
}

#[test]
fn provider_fails_closed_when_all_candidates_are_irrelevant() {
    let hits = vec![SearchHit {
        source: bing::PROVIDER,
        title: "英超联赛积分榜".to_string(),
        url: "https://example.com/football".to_string(),
        snippet: "最新比赛结果".to_string(),
    }];

    let result = keep_relevant_provider_hits(bing::PROVIDER, "Rust language", 5, Ok(hits));
    assert!(result.is_err());
}

#[tokio::test]
async fn invalid_query_is_rejected() {
    let result = WebSearchTool::default()
        .execute(json!({ "query": "   " }), ToolContext::default())
        .await;
    assert!(matches!(result, Err(ToolError::InvalidArgs(_))));
}

#[tokio::test]
async fn deepseek_maps_results_and_preserves_service_order() {
    let hits = (0..4)
        .map(|index| {
            json!({
                "rerank_score": 3.0 - index as f64 * 0.1,
                "title": format!("标题 {index}"),
                "url": format!("https://example.com/{index}"),
                "snippet": format!("界面摘要 {index}"),
                "summary": format!("模型摘要 {index}"),
                "content": "正文。".repeat(5_000),
                "site_name": "Example",
                "site_icon": null,
                "date": null,
                "authority": 3
            })
        })
        .collect::<Vec<_>>();
    let (endpoint, request_rx, _server) =
        mock_deepseek(200, json!({ "results": hits }).to_string()).await;
    let tool = WebSearchTool::with_deepseek_endpoint(Some("test-secret".to_string()), endpoint);
    let output = tool
        .execute(
            json!({ "query": "DeepSeek site:example.com", "max_results": 4 }),
            ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!output.is_error);
    let response: Value = serde_json::from_str(&output.content).unwrap();
    assert_eq!(response["provider"], "deepseek");
    assert_eq!(response["providers"].as_array().unwrap().len(), 1);
    assert_eq!(response["status"], "ok");
    assert_eq!(response["results"][0]["title"], "标题 0");
    assert_eq!(response["results"][1]["title"], "标题 1");
    assert_eq!(response["results"][0]["snippet"], "界面摘要 0");
    assert_eq!(response["results"][0]["summary"], "模型摘要 0");
    let first_content = response["results"][0]["content"].as_str().unwrap();
    assert!(first_content.starts_with("【不可信网页正文开始："));
    assert!(first_content.ends_with("【不可信网页正文结束】"));
    let result_content_chars = response["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|result| result["content"].as_str())
        .map(|content| content.chars().count())
        .sum::<usize>();
    // 4 个边界标记和 URL 不计入正文预算。
    assert!(result_content_chars <= 12_000 + 4 * 100);

    let request = request_rx.await.unwrap();
    let (headers, body) = request.split_once("\r\n\r\n").unwrap();
    assert!(
        headers
            .to_ascii_lowercase()
            .contains("authorization: bearer test-secret")
    );
    let body: Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["query"], "DeepSeek site:example.com");
    assert_eq!(body["count"], 4);
    assert_eq!(body["need_content"], true);
}

#[tokio::test]
async fn deepseek_missing_key_returns_terminal_error_json() {
    for api_key in [None, Some(" \t".to_string())] {
        let tool = WebSearchTool::new(WebSearchBackend::DeepSeek, api_key);
        let output = tool
            .execute(json!({ "query": "测试" }), ToolContext::default())
            .await
            .unwrap();

        assert!(output.is_error);
        let response: Value = serde_json::from_str(&output.content).unwrap();
        assert_eq!(response["status"], "unavailable");
        assert_eq!(response["query"], "测试");
        assert_eq!(response["provider"], "deepseek");
        assert_eq!(response["providers"][0]["status"], "error");
        assert!(response["results"].as_array().unwrap().is_empty());
        assert!(
            response["note"].as_str().unwrap().contains(
                "未配置 DeepSeek API Key，请在设置中的服务商配置 DeepSeek API Key 后重试。"
            )
        );
        assert!(tool.description().contains("不要在本轮重复调用"));
    }
}

#[tokio::test]
async fn deepseek_accepts_128_chinese_characters_and_rejects_129() {
    let query = "中".repeat(128);
    let (endpoint, request_rx, _server) =
        mock_deepseek(200, json!({ "results": [] }).to_string()).await;
    let tool = WebSearchTool::with_deepseek_endpoint(Some("key".to_string()), endpoint);
    let output = tool
        .execute(json!({ "query": query }), ToolContext::default())
        .await
        .unwrap();
    assert!(!output.is_error);
    let request = request_rx.await.unwrap();
    let body = request.split_once("\r\n\r\n").unwrap().1;
    let body: Value = serde_json::from_str(body).unwrap();
    assert_eq!(body["query"].as_str().unwrap().chars().count(), 128);

    let tool = WebSearchTool::with_deepseek_endpoint(
        Some("key".to_string()),
        "http://127.0.0.1:9/should-not-be-requested",
    );
    let output = tool
        .execute(json!({ "query": "中".repeat(129) }), ToolContext::default())
        .await
        .unwrap();
    assert!(output.is_error);
    let response: Value = serde_json::from_str(&output.content).unwrap();
    assert_eq!(response["status"], "unavailable");
    assert!(
        response["note"]
            .as_str()
            .unwrap()
            .contains("128 个 Unicode 字符")
    );
}

#[tokio::test]
async fn deepseek_http_errors_are_reported_without_key_or_native_fallback() {
    for (status, expected) in [(401, "HTTP 401"), (402, "HTTP 402"), (500, "HTTP 500")] {
        let (endpoint, _request_rx, _server) = mock_deepseek(
            status,
            json!({ "error": { "message": "test-secret should not be reflected" } }).to_string(),
        )
        .await;
        let tool = WebSearchTool::with_deepseek_endpoint(Some("test-secret".to_string()), endpoint);
        let output = tool
            .execute(json!({ "query": "测试" }), ToolContext::default())
            .await
            .unwrap();

        assert!(output.is_error);
        assert!(!output.content.contains("test-secret"));
        let response: Value = serde_json::from_str(&output.content).unwrap();
        assert_eq!(response["provider"], "deepseek");
        assert_eq!(response["providers"].as_array().unwrap().len(), 1);
        assert_eq!(response["providers"][0]["status"], "error");
        assert!(response["note"].as_str().unwrap().contains(expected));
        if status == 401 {
            assert!(response["note"].as_str().unwrap().contains("API Key 无效"));
        }
        if status == 402 {
            assert!(response["note"].as_str().unwrap().contains("余额不足"));
        }
    }
}

#[tokio::test]
async fn deepseek_request_observes_tool_cancellation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (request_seen_tx, request_seen_rx) = oneshot::channel();
    let server: JoinHandle<()> = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_request(&mut stream).await;
        let _ = request_seen_tx.send(request);
        std::future::pending::<()>().await;
    });
    let tool = WebSearchTool::with_deepseek_endpoint(
        Some("test-secret".to_string()),
        format!("http://{address}/search"),
    );
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let call = tokio::spawn(async move {
        tool.execute(
            json!({ "query": "测试" }),
            ToolContext {
                cancel_rx: Some(cancel_rx),
                ..ToolContext::default()
            },
        )
        .await
    });
    request_seen_rx.await.unwrap();
    cancel_tx.send(true).unwrap();
    let output = timeout(Duration::from_secs(2), call)
        .await
        .expect("cancel should interrupt a pending HTTP response")
        .unwrap()
        .unwrap();
    assert!(output.is_error);
    let response: Value = serde_json::from_str(&output.content).unwrap();
    assert_eq!(response["status"], "unavailable");
    assert_eq!(response["provider"], "deepseek");
    server.abort();
}

async fn mock_deepseek(
    status: u16,
    body: String,
) -> (String, oneshot::Receiver<String>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (request_tx, request_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_request(&mut stream).await;
        let _ = request_tx.send(request);
        let reason = match status {
            200 => "OK",
            401 => "Unauthorized",
            402 => "Payment Required",
            _ => "Internal Server Error",
        };
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes()).await;
    });
    (format!("http://{address}/search"), request_rx, server)
}

async fn read_request(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    loop {
        let mut buffer = [0u8; 4096];
        let count = stream.read(&mut buffer).await.unwrap();
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        let Some(header_end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        let body_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        if bytes.len() >= header_end + 4 + body_length {
            break;
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[tokio::test]
#[ignore = "requires external network"]
async fn live_websearch_returns_structured_output() {
    let response = run_websearch("Rust programming language", 3).await;
    eprintln!("{response:#?}");
    assert_ne!(response.status, "unavailable", "{response:?}");
    assert_eq!(response.providers.len(), 2, "{response:?}");
    assert_eq!(response.providers[0].name, so360::PROVIDER);
    assert_eq!(response.providers[1].name, duckduckgo::PROVIDER);
    assert!(!response.results.is_empty(), "{response:?}");
    assert!(
        response
            .results
            .iter()
            .any(|result| result.content.is_some()),
        "{response:?}"
    );
}

#[tokio::test]
#[ignore = "requires external network"]
async fn live_websearch_preserves_query_relevance() {
    let response = run_websearch("2026 夏季新番", 5).await;

    eprintln!(
        "status={}, providers={:?}",
        response.status, response.providers
    );
    for result in &response.results {
        eprintln!(
            "#{} [{}] {:?}\t{}",
            result.rank, result.source, result.title, result.url
        );
    }
    assert!(
        response.results.iter().any(|result| {
            let text = format!("{} {}", result.title, result.snippet);
            text.contains("アニメ")
                || text.contains("新番")
                || text.contains("动画")
                || text.contains("夏季番")
                || text.contains("夏番")
        }),
        "websearch returned only low-relevance results: {response:#?}"
    );
}

#[tokio::test]
#[ignore = "requires external network"]
async fn live_websearch_handles_current_stock_query() {
    let response = run_websearch("NVIDIA stock price today August 2026", 5).await;

    eprintln!("{response:#?}");
    assert_ne!(response.status, "unavailable", "{response:#?}");
    assert!(
        response.results.iter().any(|result| {
            let text = format!("{} {}", result.title, result.snippet).to_lowercase();
            (text.contains("nvidia") || text.contains("英伟达"))
                && (text.contains("stock") || text.contains("股票") || text.contains("股价"))
        }),
        "websearch returned no NVIDIA stock result: {response:#?}"
    );
}
