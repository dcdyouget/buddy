//! Context routing through the same disk-history entry point used by the UI.
mod common;

use buddy_engine::{
    chat::ChatEngine,
    models::{AppConfig, Message},
    storage,
    streaming::{StreamEvent, StreamEventEmitter},
};
use common::*;
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

const MODEL: &str = "p::mock";

fn configure(dir: &Path, url: &str) {
    let config: AppConfig = serde_json::from_value(json!({
        "providers":[{"id":"p","name":"Mock","base_url":url,"api_key":"k","provider_type":"openai_compatible","enabled_model_ids":[MODEL]}],
        "models":[{"id":MODEL,"api_model_id":"mock","provider_id":"p","display_name":"Mock","context_window":128000}],
        "selected_model_id":MODEL
    })).unwrap();
    storage::save_config(dir, &config).unwrap();
}

fn reply(text: &str) -> Vec<Chunk> {
    openai_script(&[text], Duration::ZERO)
}
fn summary(text: &str) -> Vec<Chunk> {
    reply(&json!({"summary":text}).to_string())
}
fn related(id: &str) -> Vec<Chunk> {
    reply(&json!({"related":true,"context_id":id,"need_more":false}).to_string())
}
fn more() -> Vec<Chunk> {
    reply(r#"{"related":null,"context_id":null,"need_more":true}"#)
}
fn question(id: &str, text: &str, time: u64) -> Message {
    let mut m = user_message(text);
    m.id = id.into();
    m.created_at = time;
    m
}
async fn ask(engine: &ChatEngine, id: &str, text: &str, time: u64) {
    let (emitter, rx) = StreamEventEmitter::channel();
    engine
        .send_message_from_history(emitter, question(id, text, time), MODEL.into())
        .await
        .unwrap();
    let events = drain(rx).await;
    assert!(
        matches!(events.last(), Some(StreamEvent::Done { .. })),
        "{events:?}"
    );
    engine.wait_for_context_tasks().await;
}
fn payload(request: &CapturedRequest) -> Value {
    serde_json::from_str(&request.body).unwrap()
}
fn background_input(request: &CapturedRequest) -> Value {
    serde_json::from_str(payload(request)["messages"][1]["content"].as_str().unwrap()).unwrap()
}
fn state(dir: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(dir.join("context/state.json")).unwrap()).unwrap()
}
fn seed(dir: &Path, turns: usize) {
    let messages: Vec<Message> = (0..turns).flat_map(|i| {
        let user = question(&format!("seed-{i}"), &format!("旧问题{i}"), 100);
        let assistant = serde_json::from_value(json!({"id":format!("answer-{i}"),"role":"assistant","content":format!("旧回答{i}"),"created_at":100})).unwrap();
        [user, assistant]
    }).collect();
    storage::append_messages(dir, &messages).unwrap();
}

#[tokio::test]
async fn unrelated_topic_and_resume_survive_restart_without_changing_raw_history() {
    let dir = tempfile::tempdir().unwrap();
    let (url, server) = serve_sse_sequence(
        vec![
            reply("Rust 用所有权管理内存"),
            summary("讨论 Rust 所有权与内存释放。"),
            reply(r#"{"related":false,"context_id":null,"need_more":false}"#),
            reply("蛋炒饭先炒蛋"),
            summary("讨论蛋炒饭做法。"),
            related("context-u1-0"),
            reply("所有权保证值只释放一次"),
            summary("回到 Rust，解释如何避免重复释放。"),
        ],
        Duration::from_secs(3),
    )
    .await;
    configure(dir.path(), &url);
    let engine = ChatEngine::new(dir.path().into());
    ask(&engine, "u1", "解释 Rust 所有权", 100).await;
    ask(&engine, "u2", "蛋炒饭怎么做", 120).await;
    drop(engine);
    let engine = ChatEngine::new(dir.path().into());
    ask(&engine, "u3", "回到 Rust，它怎么避免重复释放", 5000).await;
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 8);
    assert!(!requests[3].body.contains("解释 Rust 所有权"));
    assert!(requests[6].body.contains("解释 Rust 所有权"));
    assert!(!requests[6].body.contains("蛋炒饭怎么做"));
    let original = payload(&requests[0]);
    let resumed = payload(&requests[6]);
    assert_eq!(original["messages"][0], resumed["messages"][0]);
    assert_eq!(original["messages"][1], resumed["messages"][1]);
    assert_eq!(original["tools"], resumed["tools"]);
    for index in [1, 2, 4, 5, 7] {
        let request = payload(&requests[index]);
        assert_eq!(request["messages"].as_array().unwrap().len(), 2);
        assert!(request.get("tools").is_none());
        assert!(!requests[index].body.contains("buddy_runtime_context"));
    }
    assert_eq!(storage::message_count(dir.path()).unwrap(), 6);
    let state = state(dir.path());
    assert_eq!(state["segments"].as_array().unwrap().len(), 2);
    for turn in state["turns"].as_array().unwrap() {
        assert!(turn["summary"].as_str().unwrap().chars().count() <= 200);
    }
}

#[tokio::test]
async fn compaction_is_batched_and_prefix_stays_frozen_after_summary_and_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (url, server) = serve_sse_sequence(
        vec![
            related("legacy-2"),
            summary("旧问答讨论同一任务，继续执行。"),
            reply("本次答复"),
            summary("本轮摘要甲"),
            related("legacy-2"),
            reply("继续答复"),
            summary("本轮摘要乙"),
        ],
        Duration::from_secs(3),
    )
    .await;
    configure(dir.path(), &url);
    seed(dir.path(), 16);
    let engine = ChatEngine::new(dir.path().into());
    ask(&engine, "u1", "接着说", 200).await;
    let checkpoint = state(dir.path())["segments"][0].clone();
    drop(engine);
    let engine = ChatEngine::new(dir.path().into());
    ask(&engine, "u2", "继续", 201).await;
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 7);
    let first = payload(&requests[2]);
    let next = payload(&requests[5]);
    let first_messages = first["messages"].as_array().unwrap();
    assert!(first_messages.len() <= 12); // <=10 raw messages + instructions/checkpoint
    assert_eq!(
        next["messages"].as_array().unwrap()[..first_messages.len()],
        first_messages[..]
    );
    assert_eq!(state(dir.path())["segments"][0], checkpoint);
    assert_eq!(checkpoint["revision"], 1);
    assert_eq!(storage::message_count(dir.path()).unwrap(), 36);
}

#[tokio::test]
async fn long_gap_search_expands_five_ten_twenty_thirty_messages_only() {
    let dir = tempfile::tempdir().unwrap();
    let (url, server) = serve_sse_sequence(
        vec![
            more(),
            more(),
            more(),
            related("legacy-0"),
            summary("旧任务摘要"),
            reply("继续"),
            summary("本轮继续旧任务"),
        ],
        Duration::from_secs(3),
    )
    .await;
    configure(dir.path(), &url);
    seed(dir.path(), 15);
    let engine = ChatEngine::new(dir.path().into());
    ask(&engine, "u1", "继续此前的任务", 2000).await;
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 7);
    for (index, expected_turns) in [3, 5, 10, 15].into_iter().enumerate() {
        let input = background_input(&requests[index]);
        assert_eq!(
            input["candidates"].as_array().unwrap().len(),
            expected_turns
        );
        assert_eq!(input["can_expand"], index < 3);
    }
}

#[tokio::test]
async fn overlong_summary_gets_one_stateless_repair_and_does_not_change_answer() {
    let dir = tempfile::tempdir().unwrap();
    let (url, server) = serve_sse_sequence(
        vec![
            reply("回答原文"),
            summary(&"长".repeat(201)),
            summary("简短摘要"),
        ],
        Duration::from_secs(3),
    )
    .await;
    configure(dir.path(), &url);
    let engine = ChatEngine::new(dir.path().into());
    ask(&engine, "u1", "请回答", 100).await;
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 3);
    let repair = payload(&requests[2]);
    assert_eq!(repair["messages"].as_array().unwrap().len(), 2);
    assert!(!requests[2].body.contains(&"长".repeat(201)));
    assert_eq!(state(dir.path())["turns"][0]["summary"], "简短摘要");
    assert_eq!(
        storage::load_messages(dir.path(), 1, 1).unwrap()[0].content,
        "回答原文"
    );
}

#[tokio::test]
async fn aborting_during_judge_releases_generation_slot() {
    let dir = tempfile::tempdir().unwrap();
    let (url, server) = serve_sse_sequence(
        vec![openai_script(&["{}"], Duration::from_millis(150))],
        Duration::from_secs(3),
    )
    .await;
    configure(dir.path(), &url);
    seed(dir.path(), 1);
    let engine = ChatEngine::new(dir.path().into());
    let copy = engine.clone();
    let task = tokio::spawn(async move {
        let (emitter, _) = StreamEventEmitter::channel();
        copy.send_message_from_history(emitter, question("u1", "继续", 200), MODEL.into())
            .await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    task.abort();
    let _ = task.await;
    let (emitter, _) = StreamEventEmitter::channel();
    // Validation fails locally, proving the old reservation was released.
    let error = engine
        .send_message(emitter, vec![user_message("test")], "missing".into())
        .await
        .unwrap_err();
    assert_eq!(error, "未找到指定的模型");
    let _ = server.await;
    assert_eq!(storage::message_count(dir.path()).unwrap(), 2);
}
