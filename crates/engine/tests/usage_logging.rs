mod common;

use buddy_engine::{
    providers::{ChatOptions, ProviderType, create_provider},
    streaming::StreamEventEmitter,
};
use common::*;
use serde_json::{Value, json};
use std::{sync::Mutex, time::Duration};
use tokio::sync::watch;

struct Logger(Mutex<Vec<Value>>);
static LOGGER: Logger = Logger(Mutex::new(Vec::new()));
impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, record: &log::Record<'_>) {
        if let Some(text) = record.args().to_string().strip_prefix("[usage] ") {
            self.0
                .lock()
                .unwrap()
                .push(serde_json::from_str(text).unwrap());
        }
    }
    fn flush(&self) {}
}

#[tokio::test]
async fn protocol_usage_and_cache_controls_are_logged_once_per_request() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);
    let mut openai = openai_script(&["ok"], Duration::ZERO);
    openai.insert(openai.len()-1, (Duration::ZERO, format!("data: {}\n\n", json!({"choices":[],"usage":{"prompt_tokens":1000,"completion_tokens":31,"prompt_tokens_details":{"cached_tokens":800}}}))));
    let mut anthropic = anthropic_script(&["ok"], Duration::ZERO);
    anthropic[0].1 = format!(
        "event: message_start\ndata: {}\n\n",
        json!({"type":"message_start","message":{"id":"m1","type":"message","role":"assistant","content":[],"usage":{"input_tokens":100,"output_tokens":0,"cache_read_input_tokens":800,"cache_creation_input_tokens":100}}})
    );
    for (protocol, script, stage) in [
        ("openai_compatible", openai, "relevance"),
        ("anthropic", anthropic.clone(), "main"),
        ("anthropic", anthropic, "summary"),
    ] {
        let (url, server) = serve_sse(script).await;
        let provider = create_provider(&ProviderType::from_str(protocol));
        let (emitter, _) = StreamEventEmitter::channel();
        let (_cancel, receiver) = watch::channel(false);
        provider
            .stream_chat_with_options(
                &url,
                "k",
                "mock",
                stage,
                &[user_message("payload")],
                &emitter,
                receiver,
                None,
                &[],
                ChatOptions {
                    system_prompt: "fixed instructions",
                    context: None,
                    runtime_context: false,
                    max_output_tokens: Some(512),
                    stage,
                    question_id: "usage-question",
                },
            )
            .await
            .unwrap();
        let request = server.await.unwrap();
        let body: Value = serde_json::from_str(&request.body).unwrap();
        assert_eq!(body["max_tokens"], 512);
        assert!(!request.body.contains("buddy_runtime_context"));
        if protocol == "anthropic" {
            assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
            assert_eq!(body.get("cache_control").is_some(), stage == "main");
        } else {
            assert_eq!(body["stream_options"]["include_usage"], true);
        }
    }
    let records = LOGGER.0.lock().unwrap();
    assert_eq!(records.len(), 3);
    for record in records.iter() {
        assert_eq!(record["input_tokens"], 1000);
        assert_eq!(record["cache_read_tokens"], 800);
        assert_eq!(record["cache_hit_percent"], 80.0);
        assert_eq!(record["question_id"], "usage-question");
        assert_eq!(record["completed"], true);
        if record["stage"] == "relevance" {
            assert_eq!(record["output_tokens"], 31);
            assert!(record["cache_write_tokens"].is_null());
        } else {
            assert_eq!(record["output_tokens"], 2);
            assert_eq!(record["cache_write_tokens"], 100);
        }
    }
}
