//! 无 Tauri 环境下，两个 provider 对本地 mock SSE 完成一次完整流式对话。
//!
//! 契约：provider 的事件流以 `TurnEnd` 结束，结果经返回值 `StreamOutcome` 交出；
//! 终态 `Done` / `Error` 由编排层（v1 为 `commands.rs` 的 `TerminalStreamEvent`）发射，不由 provider 发射。

mod common;

use buddy_engine::providers::{ProviderType, create_provider};
use buddy_engine::streaming::{StreamEvent, StreamEventEmitter, StreamOutcome};
use common::*;
use std::time::Duration;
use tokio::sync::watch;

async fn run(provider_type: &str, script: Vec<Chunk>) -> (Vec<StreamEvent>, StreamOutcome, CapturedRequest) {
    let (base_url, server) = serve_sse(script).await;
    let provider = create_provider(&ProviderType::from_str(provider_type));
    let (emitter, rx) = StreamEventEmitter::channel();
    let (_cancel_tx, cancel_rx) = watch::channel(false);
    let messages = vec![user_message("说一句问候")];

    let outcome = provider
        .stream_chat(
            &base_url, "test-key", "mock-model", "req-1", &messages, &emitter, cancel_rx, None, &[],
        )
        .await
        .expect("stream_chat 应成功返回");
    assert!(!outcome.had_stream_error, "不应出现流错误: {outcome:?}");

    drop(emitter); // 关闭发送端，drain 才会结束
    let events = drain(rx).await;
    (events, outcome, server.await.unwrap())
}

fn assert_completed(events: &[StreamEvent], outcome: &StreamOutcome) {
    assert!(matches!(events.first(), Some(StreamEvent::Start)), "首事件应为 Start: {events:?}");
    assert_eq!(text_of(events), "你好，世界");
    assert!(
        matches!(events.last(), Some(StreamEvent::TurnEnd { tool_calls_pending: 0 })),
        "provider 事件流应以 TurnEnd 收尾: {events:?}"
    );
    assert!(!events.iter().any(|e| matches!(e, StreamEvent::Done { .. } | StreamEvent::Error { .. })));
    assert_eq!(outcome.full_text, "你好，世界");
    assert!(outcome.tool_calls.is_empty());
    assert!(outcome.terminal_error.is_none());
}

#[tokio::test]
async fn openai_compatible_streams_to_turn_end() {
    let (events, outcome, req) = run("openai_compatible", openai_script(&["你好", "，世界"], Duration::ZERO)).await;
    assert_completed(&events, &outcome);
    assert_eq!(req.request_line, "POST /chat/completions HTTP/1.1");
    assert!(req.headers.to_ascii_lowercase().contains("authorization: bearer test-key"));
    assert!(req.body.contains("\"stream\":true"), "请求体: {}", req.body);
}

#[tokio::test]
async fn anthropic_streams_to_turn_end() {
    let (events, outcome, req) = run("anthropic", anthropic_script(&["你好", "，世界"], Duration::ZERO)).await;
    assert_completed(&events, &outcome);
    assert_eq!(req.request_line, "POST /v1/messages HTTP/1.1");
    assert!(req.headers.to_ascii_lowercase().contains("x-api-key: test-key"));
}
