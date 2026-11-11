//! 对话编排（`ChatEngine::send_message`）端到端测试 —— 无 Tauri、无 UI。
//!
//! 用 mock SSE 驱动 v1 迁入的编排逻辑，UI 侧以「消费事件 + 调用 approve / answer / stop」模拟。

mod common;

use buddy_engine::chat::ChatEngine;
use buddy_engine::models::{AppConfig, Message, MessageRole};
use buddy_engine::storage;
use buddy_engine::streaming::{StopReason, StreamEvent, StreamEventEmitter};
use common::*;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

const MODEL_ID: &str = "p1::mock";
const IDLE: Duration = Duration::from_secs(3);

/// 在数据目录写入指向 mock 服务的配置
fn write_config(data_dir: &Path, base_url: &str, allowed_paths: Vec<String>) {
    let mut config: AppConfig = serde_json::from_value(json!({
        "theme": "light",
        "providers": [{
            "id": "p1", "name": "Mock", "base_url": base_url, "api_key": "k",
            "enabled_model_ids": [MODEL_ID], "provider_type": "openai_compatible"
        }],
        "models": [{
            "id": MODEL_ID, "provider_id": "p1", "api_model_id": "mock",
            "display_name": "Mock", "context_window": 128000, "latency_ms": null
        }],
        "selected_model_id": MODEL_ID,
        "auto_start": false
    }))
    .unwrap();
    config.allowed_paths = allowed_paths;
    storage::save_config(data_dir, &config).unwrap();
}

/// UI 侧对交互事件的应答
#[derive(Clone, Copy)]
enum Respond {
    /// 审批：(approved, approve_all)
    Approve(bool, bool),
    /// 提问：选第 n 项
    Answer(usize),
    /// 收到审批/提问请求时按「停止」
    Stop,
}

struct Run {
    result: Result<(), String>,
    events: Vec<StreamEvent>,
    requests: Vec<CapturedRequest>,
}

async fn run(
    engine: &Arc<ChatEngine>,
    server: tokio::task::JoinHandle<Vec<CapturedRequest>>,
    respond: Respond,
) -> Run {
    let (emitter, mut rx) = StreamEventEmitter::channel();
    let e = engine.clone();
    // 与 v2 UI 相同的用法：Arc clone 后 spawn，future 为 Send + 'static
    let task = tokio::spawn(async move {
        e.send_message(
            emitter,
            vec![user_message("帮我做件事")],
            MODEL_ID.to_string(),
        )
        .await
    });

    let mut events = Vec::new();
    while let Some(ev) = tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await
        .expect("10s 内应收到下一个事件或通道关闭")
    {
        match (&ev, respond) {
            (StreamEvent::ToolApprovalRequired { id, .. }, Respond::Approve(ok, all)) => {
                engine.approve_tool_call(id, ok, all).unwrap()
            }
            (StreamEvent::ToolQuestionRequired { id, .. }, Respond::Answer(n)) => engine
                .answer_tool_question(id, vec![n], None, None)
                .unwrap(),
            (
                StreamEvent::ToolApprovalRequired { id, .. }
                | StreamEvent::ToolQuestionRequired { id, .. },
                Respond::Stop,
            ) => {
                let saved = engine.load_messages(0, 100).await.unwrap();
                assert!(
                    saved.iter().any(|message| message
                        .tool_calls
                        .iter()
                        .flatten()
                        .any(|call| &call.id == id)),
                    "等待交互前必须保存工具调用"
                );
                engine.stop_generation();
            }
            _ => {}
        }
        events.push(ev);
    }
    let result = task.await.unwrap();
    let requests = server.await.unwrap();
    Run {
        result,
        events,
        requests,
    }
}

fn terminal(events: &[StreamEvent]) -> &StreamEvent {
    let terminals: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::Done { .. } | StreamEvent::Error { .. }))
        .collect();
    assert_eq!(terminals.len(), 1, "终态事件必须恰好 1 个: {events:?}");
    assert!(
        matches!(
            events.last(),
            Some(StreamEvent::Done { .. } | StreamEvent::Error { .. })
        ),
        "终态事件必须是最后一个事件"
    );
    terminals[0]
}

fn count(events: &[StreamEvent], pred: impl Fn(&StreamEvent) -> bool) -> usize {
    events.iter().filter(|e| pred(e)).count()
}

fn roles(messages: &[Message]) -> Vec<MessageRole> {
    messages.iter().map(|m| m.role.clone()).collect()
}

async fn persisted(engine: &ChatEngine) -> Vec<Message> {
    engine.load_messages(0, 100).await.unwrap()
}

#[tokio::test]
async fn plain_reply_emits_done_and_persists_user_and_assistant() {
    let tmp = TempDir::new().unwrap();
    let (url, server) =
        serve_sse_sequence(vec![openai_script(&["你好"], Duration::ZERO)], IDLE).await;
    write_config(tmp.path(), &url, vec![]);
    let engine = ChatEngine::new(tmp.path().to_path_buf());

    let r = run(&engine, server, Respond::Approve(true, false)).await;
    assert!(r.result.is_ok());
    assert!(
        matches!(terminal(&r.events), StreamEvent::Done { full_text, .. } if full_text == "你好")
    );
    assert_eq!(r.requests.len(), 1);
    assert_eq!(
        roles(&persisted(&engine).await),
        vec![MessageRole::User, MessageRole::Assistant]
    );
}

#[tokio::test]
async fn file_tools_are_neither_advertised_nor_executed() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("private.txt");
    let new_file = tmp.path().join("new.txt");
    std::fs::write(&target, "private content").unwrap();
    let names = [
        "read_file",
        "list_directory",
        "search_files",
        "create_file",
        "overwrite_file",
        "append_file",
        "edit_file",
    ];
    let calls: Vec<_> = names.iter().map(|name| (*name, *name, json!({
        "path": if *name == "create_file" { &new_file } else { &target },
        "content": "changed", "old_text": "private", "new_text": "changed", "query": "private"
    }))).collect();
    let (url, server) = serve_sse_sequence(
        vec![
            openai_tool_calls_script(&calls),
            openai_script(&["请把需要讨论的内容粘贴到聊天中"], Duration::ZERO),
        ],
        IDLE,
    )
    .await;
    write_config(tmp.path(), &url, vec![]);
    let engine = ChatEngine::new(tmp.path().to_path_buf());
    let r = run(&engine, server, Respond::Approve(true, true)).await;
    assert!(r.result.is_ok());
    assert!(matches!(terminal(&r.events), StreamEvent::Done { .. }));
    assert_eq!(r.requests.len(), 2);
    let request: serde_json::Value = serde_json::from_str(&r.requests[0].body).unwrap();
    for tool in request["tools"].as_array().unwrap() {
        assert!(!names.contains(&tool["function"]["name"].as_str().unwrap()));
    }
    assert_eq!(
        count(&r.events, |event| matches!(
            event,
            StreamEvent::ToolApprovalRequired { .. } | StreamEvent::ToolExecuting { .. }
        )),
        0
    );
    assert_eq!(
        count(&r.events, |event| matches!(
            event,
            StreamEvent::ToolResult { is_error: true, .. }
        )),
        names.len()
    );
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "private content");
    assert!(!new_file.exists());
    assert!(!r.requests[1].body.contains("private content"));
    assert_eq!(
        roles(&persisted(&engine).await).last(),
        Some(&MessageRole::Assistant)
    );
}

#[tokio::test]
async fn ask_user_answer_round_trips_to_model() {
    let tmp = TempDir::new().unwrap();
    let args = json!({
        "question": "选哪个？", "header": "选择",
        "options": [{"label": "方案A"}, {"label": "方案B"}]
    });
    let (url, server) = serve_sse_sequence(
        vec![
            openai_tool_calls_script(&[("call_q", "ask_user", args)]),
            openai_script(&["按方案B执行"], Duration::ZERO),
        ],
        IDLE,
    )
    .await;
    write_config(tmp.path(), &url, vec![]);
    let engine = ChatEngine::new(tmp.path().to_path_buf());

    let r = run(&engine, server, Respond::Answer(1)).await;
    assert!(r.events.iter().any(|e| matches!(
        e,
        StreamEvent::ToolQuestionRequired { question, options, .. } if question == "选哪个？" && options.len() == 2
    )));
    assert!(r.events.iter().any(
        |e| matches!(e, StreamEvent::ToolResult { is_error: false, content, .. } if content == "User selected: 方案B")
    ));
    assert!(r.requests[1].body.contains("User selected: 方案B"));
    assert!(matches!(terminal(&r.events), StreamEvent::Done { .. }));
}

#[tokio::test]
async fn stop_while_waiting_for_answer_aborts_without_continuing() {
    let tmp = TempDir::new().unwrap();
    let (url, server) = serve_sse_sequence(
        vec![
            openai_tool_calls_script(&[(
                "call_1",
                "ask_user",
                json!({"question": "选择哪个方向？", "header": "方向", "options": [{"label": "A"}, {"label": "B"}]}),
            )]),
            openai_script(&["不应到达"], Duration::ZERO),
        ],
        Duration::from_millis(500),
    )
    .await;
    write_config(tmp.path(), &url, vec![]);
    let engine = ChatEngine::new(tmp.path().to_path_buf());

    let r = run(&engine, server, Respond::Stop).await;
    assert_eq!(r.requests.len(), 1, "取消后不得发起下一轮请求");
    assert!(matches!(
        terminal(&r.events),
        StreamEvent::Error {
            reason: StopReason::Aborted,
            ..
        }
    ));
    // 等待槽位已被清理：迟到的回答返回 Err，而不是推进已结束的对话
    assert!(
        engine
            .answer_tool_question("call_1", vec![0], None, None)
            .is_err()
    );
    // 生成占用已释放：可以立即开始下一次对话
    let (url2, server2) =
        serve_sse_sequence(vec![openai_script(&["再来"], Duration::ZERO)], IDLE).await;
    write_config(tmp.path(), &url2, vec![]);
    let r2 = run(&engine, server2, Respond::Stop).await;
    assert!(matches!(terminal(&r2.events), StreamEvent::Done { .. }));
}

#[tokio::test]
async fn second_send_while_generating_is_rejected_without_events() {
    let tmp = TempDir::new().unwrap();
    let (url, server) = serve_sse_sequence(
        vec![openai_script(&["慢"; 10], Duration::from_millis(200))],
        IDLE,
    )
    .await;
    write_config(tmp.path(), &url, vec![]);
    let engine = ChatEngine::new(tmp.path().to_path_buf());

    let (emitter1, mut rx1) = StreamEventEmitter::channel();
    let e = engine.clone();
    let first = tokio::spawn(async move {
        e.send_message(emitter1, vec![user_message("一")], MODEL_ID.to_string())
            .await
    });
    // 等第一次对话真正开始（收到首个事件）
    rx1.recv().await.unwrap();
    let saved = engine.load_messages(0, 100).await.unwrap();
    assert_eq!(saved.len(), 1, "流式回复结束前应已保存用户输入");
    assert_eq!(saved[0].content, "一");

    let (emitter2, rx2) = StreamEventEmitter::channel();
    let second = engine
        .send_message(emitter2, vec![user_message("二")], MODEL_ID.to_string())
        .await;
    assert_eq!(second.unwrap_err(), "已有生成任务正在进行中");
    assert!(drain(rx2).await.is_empty(), "被拒绝的请求不应发出任何事件");

    engine.stop_generation();
    assert!(first.await.unwrap().is_ok());
    let rest = drain(rx1).await;
    assert!(matches!(
        terminal(&rest),
        StreamEvent::Error {
            reason: StopReason::Aborted,
            ..
        }
    ));
    let _ = server.await;
}

#[tokio::test]
async fn unknown_model_returns_err_before_any_event() {
    let tmp = TempDir::new().unwrap();
    write_config(tmp.path(), "http://127.0.0.1:9", vec![]);
    let engine = ChatEngine::new(tmp.path().to_path_buf());
    let (emitter, rx) = StreamEventEmitter::channel();
    let r = engine
        .send_message(emitter, vec![user_message("hi")], "nope::x".to_string())
        .await;
    assert_eq!(r.unwrap_err(), "未找到指定的模型");
    assert!(drain(rx).await.is_empty());
    assert_eq!(
        engine.get_message_count().await.unwrap(),
        0,
        "失败的请求不落盘"
    );
}

#[tokio::test]
async fn engine_context_uses_disk_history_instead_of_ui_page() {
    let tmp = TempDir::new().unwrap();
    let (url, server) = serve_sse_sequence(
        vec![
            openai_script(
                &[r#"{"related":true,"context_id":"legacy-30","need_more":false}"#],
                Duration::ZERO,
            ),
            openai_script(
                &[r#"{"summary":"先前讨论了 history-30 至 history-51。"}"#],
                Duration::ZERO,
            ),
            openai_script(&["ok"], Duration::ZERO),
            openai_script(
                &[r#"{"summary":"用户提出 current，得到 ok 回复。"}"#],
                Duration::ZERO,
            ),
        ],
        IDLE,
    )
    .await;
    write_config(tmp.path(), &url, vec![]);
    let history: Vec<_> = (0..60)
        .map(|index| {
            let mut message = user_message(&format!("history-{index}"));
            message.id = format!("historical-{index}");
            message
        })
        .collect();
    storage::append_messages(tmp.path(), &history).unwrap();
    let engine = ChatEngine::new(tmp.path().to_path_buf());
    let (emitter, mut rx) = StreamEventEmitter::channel();
    engine
        .send_message_from_history(emitter, user_message("current"), MODEL_ID.into())
        .await
        .unwrap();
    while rx.recv().await.is_some() {}
    engine.wait_for_context_tasks().await;
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 4);
    assert!(!requests[2].body.contains("history-0"));
    assert!(requests[2].body.contains("history-59"));
    assert!(requests[2].body.contains("current"));
    let request: serde_json::Value = serde_json::from_str(&requests[2].body).unwrap();
    assert!(request["messages"].as_array().unwrap().len() <= 32); // up to 30 + system + checkpoint
    assert_eq!(engine.get_message_count().await.unwrap(), 62);
}

#[tokio::test]
async fn oversized_current_turn_is_rejected_before_network_request() {
    let tmp = TempDir::new().unwrap();
    write_config(tmp.path(), "http://127.0.0.1:1", vec![]);
    let mut config = storage::get_config(tmp.path()).unwrap();
    config.models[0].context_window = 1024;
    storage::save_config(tmp.path(), &config).unwrap();
    let engine = ChatEngine::new(tmp.path().to_path_buf());
    let (emitter, mut rx) = StreamEventEmitter::channel();
    engine
        .send_message(
            emitter,
            vec![user_message(&"x".repeat(6000))],
            MODEL_ID.into(),
        )
        .await
        .unwrap();
    let mut events = Vec::new();
    while let Some(event) = rx.recv().await {
        events.push(event);
    }
    assert!(
        matches!(terminal(&events), StreamEvent::Error { message, .. } if message.contains("上下文预算"))
    );
    assert_eq!(engine.get_message_count().await.unwrap(), 1);
}
