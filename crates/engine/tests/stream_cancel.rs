//! S02-05：流式取消语义（S00-08 未实测的交接项）。
//!
//! 锁定的 v1 行为：取消后 `stream_chat` 返回 `Ok(StreamOutcome::failed(已累积文本, _, Aborted, "用户取消"))`，
//! provider 不发射终态事件；HTTP 连接随之断开（服务端写失败退出 = 无残留连接）。

mod common;

use buddy_engine::providers::{ProviderType, create_provider};
use buddy_engine::streaming::{StopReason, StreamEvent, StreamEventEmitter, StreamOutcome};
use common::*;
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tokio::time::timeout;

/// 慢速脚本：首段立即到达，之后每 300ms 一段，共约 6s —— 远长于取消后的等待上限
const SLOW_PARTS: [&str; 20] = ["你好"; 20];
const GAP: Duration = Duration::from_millis(300);
/// 取消到返回的上限。v1 用 `select!` 监听取消，理论上立即返回
const CANCEL_DEADLINE: Duration = Duration::from_millis(500);

struct Cancelled {
    outcome: StreamOutcome,
    events: Vec<StreamEvent>,
    cancel_to_return: Duration,
    server_exited: bool,
}

/// 发起流式请求；`cancel_when_first_delta` 为 true 时在收到首个 TextDelta 后取消，否则在 `cancel_after` 后取消
async fn run_and_cancel(
    provider_type: &str,
    base_url: String,
    server: tokio::task::JoinHandle<CapturedRequest>,
    cancel_when_first_delta: bool,
    cancel_after: Duration,
) -> Cancelled {
    let provider = create_provider(&ProviderType::from_str(provider_type));
    let (emitter, mut rx) = StreamEventEmitter::channel();
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let messages = vec![user_message("慢慢说")];

    // stream_chat 返回 Pin<Box<..>>，可直接 `&mut call` 轮询，且能 drop 释放对 emitter 的借用
    let mut call = provider.stream_chat(
        &base_url, "k", "mock-model", "req-cancel", &messages, &emitter, cancel_rx, None, &[],
    );

    // 驱动请求直到触发取消条件
    let mut events = Vec::new();
    let deadline = tokio::time::sleep(cancel_after);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            r = &mut call => panic!("取消前流不应结束: {r:?}"),
            Some(ev) = rx.recv(), if cancel_when_first_delta => {
                let is_delta = matches!(ev, StreamEvent::TextDelta { .. });
                events.push(ev);
                if is_delta { break; }
            }
            _ = &mut deadline, if !cancel_when_first_delta => break,
        }
    }

    let t0 = Instant::now();
    cancel_tx.send(true).unwrap();
    let outcome = timeout(Duration::from_secs(5), &mut call)
        .await
        .expect("取消后 stream_chat 必须返回")
        .expect("取消应返回 Ok(StreamOutcome) 而非 Err");
    let cancel_to_return = t0.elapsed();

    drop(call); // 释放对 emitter 的借用
    drop(emitter);
    while let Some(ev) = rx.recv().await {
        events.push(ev);
    }
    // 连接已断开 → 服务端下一次写失败即退出（脚本本身要跑约 6s）
    let server_exited = timeout(Duration::from_secs(2), server).await.is_ok();

    Cancelled { outcome, events, cancel_to_return, server_exited }
}

fn assert_aborted(c: &Cancelled, expect_partial: &str) {
    println!("cancel_to_return = {:?}", c.cancel_to_return);
    assert!(c.cancel_to_return < CANCEL_DEADLINE, "取消到返回耗时 {:?}", c.cancel_to_return);
    assert!(c.outcome.had_stream_error);
    let failure = c.outcome.terminal_error.as_ref().expect("取消必须带 terminal_error");
    assert!(matches!(failure.reason, StopReason::Aborted), "{:?}", failure.reason);
    assert_eq!(failure.message, "用户取消");
    assert_eq!(c.outcome.full_text, expect_partial, "应保留取消前已累积的文本");
    // provider 不发射终态事件（由编排层发射，见 tests/mock_sse.rs 头注释）
    assert!(
        !c.events.iter().any(|e| matches!(e, StreamEvent::Done { .. } | StreamEvent::Error { .. } | StreamEvent::TurnEnd { .. })),
        "{:?}",
        c.events
    );
    assert!(c.server_exited, "取消后 HTTP 连接应断开，服务端应在 2s 内退出");
}

#[tokio::test]
async fn openai_compatible_cancel_mid_stream() {
    let (url, server) = serve_sse(openai_script(&SLOW_PARTS, GAP)).await;
    let c = run_and_cancel("openai_compatible", url, server, true, Duration::ZERO).await;
    assert_aborted(&c, "你好");
    assert_eq!(text_of(&c.events), "你好", "取消后不应再有增量");
}

#[tokio::test]
async fn anthropic_cancel_mid_stream() {
    let (url, server) = serve_sse(anthropic_script(&SLOW_PARTS, GAP)).await;
    let c = run_and_cancel("anthropic", url, server, true, Duration::ZERO).await;
    assert_aborted(&c, "你好");
    assert_eq!(text_of(&c.events), "你好", "取消后不应再有增量");
}

#[tokio::test]
async fn openai_compatible_cancel_while_waiting_headers() {
    let (url, server) = serve_sse_with_header_delay(Duration::from_secs(10), openai_script(&["x"], GAP)).await;
    let c = run_and_cancel("openai_compatible", url, server, false, Duration::from_millis(200)).await;
    assert_aborted(&c, "");
    assert!(c.events.is_empty(), "响应头之前不应有任何事件: {:?}", c.events);
}

#[tokio::test]
async fn anthropic_cancel_while_waiting_headers() {
    let (url, server) = serve_sse_with_header_delay(Duration::from_secs(10), anthropic_script(&["x"], GAP)).await;
    let c = run_and_cancel("anthropic", url, server, false, Duration::from_millis(200)).await;
    assert_aborted(&c, "");
    assert!(c.events.is_empty(), "响应头之前不应有任何事件: {:?}", c.events);
}
