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

async fn run(engine: &Arc<ChatEngine>, server: tokio::task::JoinHandle<Vec<CapturedRequest>>, respond: Respond) -> Run {
    let (emitter, mut rx) = StreamEventEmitter::channel();
    let e = engine.clone();
    // 与 v2 UI 相同的用法：Arc clone 后 spawn，future 为 Send + 'static
    let task = tokio::spawn(async move {
        e.send_message(emitter, vec![user_message("帮我做件事")], MODEL_ID.to_string())
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
            (StreamEvent::ToolQuestionRequired { id, .. }, Respond::Answer(n)) => {
                engine.answer_tool_question(id, vec![n], None, None).unwrap()
            }
            (StreamEvent::ToolApprovalRequired { .. } | StreamEvent::ToolQuestionRequired { .. }, Respond::Stop) => {
                engine.stop_generation()
            }
            _ => {}
        }
        events.push(ev);
    }
    let result = task.await.unwrap();
    let requests = server.await.unwrap();
    Run { result, events, requests }
}

fn terminal(events: &[StreamEvent]) -> &StreamEvent {
    let terminals: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::Done { .. } | StreamEvent::Error { .. }))
        .collect();
    assert_eq!(terminals.len(), 1, "终态事件必须恰好 1 个: {events:?}");
    assert!(
        matches!(events.last(), Some(StreamEvent::Done { .. } | StreamEvent::Error { .. })),
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
    let (url, server) = serve_sse_sequence(vec![openai_script(&["你好"], Duration::ZERO)], IDLE).await;
    write_config(tmp.path(), &url, vec![]);
    let engine = ChatEngine::new(tmp.path().to_path_buf());

    let r = run(&engine, server, Respond::Approve(true, false)).await;
    assert!(r.result.is_ok());
    assert!(matches!(terminal(&r.events), StreamEvent::Done { full_text, .. } if full_text == "你好"));
    assert_eq!(r.requests.len(), 1);
    assert_eq!(roles(&persisted(&engine).await), vec![MessageRole::User, MessageRole::Assistant]);
}

#[tokio::test]
async fn write_tool_approved_executes_then_continues_to_final_answer() {
    let tmp = TempDir::new().unwrap();
    let out = tmp.path().join("out");
    std::fs::create_dir(&out).unwrap();
    let target = out.join("a.txt");
    let (url, server) = serve_sse_sequence(
        vec![
            openai_tool_calls_script(&[("call_1", "create_file", json!({"path": target, "content": "hi"}))]),
            openai_script(&["已创建"], Duration::ZERO),
        ],
        IDLE,
    )
    .await;
    write_config(tmp.path(), &url, vec![out.to_string_lossy().into()]);
    let engine = ChatEngine::new(tmp.path().to_path_buf());

    let r = run(&engine, server, Respond::Approve(true, false)).await;
    assert!(r.result.is_ok());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "hi");
    assert_eq!(count(&r.events, |e| matches!(e, StreamEvent::ToolApprovalRequired { .. })), 1);
    assert_eq!(count(&r.events, |e| matches!(e, StreamEvent::ToolExecuting { .. })), 1);
    assert_eq!(count(&r.events, |e| matches!(e, StreamEvent::ToolResult { is_error: false, .. })), 1);
    assert!(matches!(terminal(&r.events), StreamEvent::Done { full_text, .. } if full_text == "已创建"));
    // 第二轮请求把 tool 结果回传给模型
    assert_eq!(r.requests.len(), 2);
    assert!(r.requests[1].body.contains("\"role\":\"tool\""), "{}", r.requests[1].body);
    assert!(r.requests[1].body.contains("call_1"));
    // 持久化：user → assistant(tool_calls) → tool → assistant
    assert_eq!(
        roles(&persisted(&engine).await),
        vec![MessageRole::User, MessageRole::Assistant, MessageRole::Tool, MessageRole::Assistant]
    );
}

#[tokio::test]
async fn write_tool_rejected_is_not_executed_and_model_is_told() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("b.txt");
    let (url, server) = serve_sse_sequence(
        vec![
            openai_tool_calls_script(&[("call_1", "create_file", json!({"path": target, "content": "x"}))]),
            openai_script(&["好的，不创建"], Duration::ZERO),
        ],
        IDLE,
    )
    .await;
    write_config(tmp.path(), &url, vec![]);
    let engine = ChatEngine::new(tmp.path().to_path_buf());

    let r = run(&engine, server, Respond::Approve(false, false)).await;
    assert!(!target.exists(), "拒绝后不得执行写操作");
    assert_eq!(count(&r.events, |e| matches!(e, StreamEvent::ToolExecuting { .. })), 0);
    assert!(r.events.iter().any(
        |e| matches!(e, StreamEvent::ToolResult { is_error: true, content, .. } if content == "用户拒绝执行")
    ));
    assert!(r.requests[1].body.contains("用户拒绝执行"));
    assert!(matches!(terminal(&r.events), StreamEvent::Done { .. }));
}

#[tokio::test]
async fn approve_all_skips_approval_for_rest_of_turn() {
    let tmp = TempDir::new().unwrap();
    let (a, b) = (tmp.path().join("a.txt"), tmp.path().join("b.txt"));
    let (url, server) = serve_sse_sequence(
        vec![
            openai_tool_calls_script(&[
                ("call_1", "create_file", json!({"path": a, "content": "1"})),
                ("call_2", "create_file", json!({"path": b, "content": "2"})),
            ]),
            openai_script(&["都建好了"], Duration::ZERO),
        ],
        IDLE,
    )
    .await;
    write_config(tmp.path(), &url, vec![]);
    let engine = ChatEngine::new(tmp.path().to_path_buf());

    let r = run(&engine, server, Respond::Approve(true, true)).await;
    assert_eq!(count(&r.events, |e| matches!(e, StreamEvent::ToolApprovalRequired { .. })), 1);
    assert!(a.exists() && b.exists());
    assert!(matches!(terminal(&r.events), StreamEvent::Done { .. }));
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
async fn stop_while_waiting_approval_aborts_without_executing() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("c.txt");
    let (url, server) = serve_sse_sequence(
        vec![
            openai_tool_calls_script(&[("call_1", "create_file", json!({"path": target, "content": "x"}))]),
            openai_script(&["不应到达"], Duration::ZERO),
        ],
        Duration::from_millis(500),
    )
    .await;
    write_config(tmp.path(), &url, vec![]);
    let engine = ChatEngine::new(tmp.path().to_path_buf());

    let r = run(&engine, server, Respond::Stop).await;
    assert!(!target.exists());
    assert_eq!(r.requests.len(), 1, "取消后不得发起下一轮请求");
    assert!(matches!(terminal(&r.events), StreamEvent::Error { reason: StopReason::Aborted, .. }));
    // 等待槽位已被清理：迟到的审批返回 Err，而不是推进已结束的对话
    assert!(engine.approve_tool_call("call_1", true, false).is_err());
    // 生成占用已释放：可以立即开始下一次对话
    let (url2, server2) = serve_sse_sequence(vec![openai_script(&["再来"], Duration::ZERO)], IDLE).await;
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
        e.send_message(emitter1, vec![user_message("一")], MODEL_ID.to_string()).await
    });
    // 等第一次对话真正开始（收到首个事件）
    rx1.recv().await.unwrap();

    let (emitter2, rx2) = StreamEventEmitter::channel();
    let second = engine
        .send_message(emitter2, vec![user_message("二")], MODEL_ID.to_string())
        .await;
    assert_eq!(second.unwrap_err(), "已有生成任务正在进行中");
    assert!(drain(rx2).await.is_empty(), "被拒绝的请求不应发出任何事件");

    engine.stop_generation();
    assert!(first.await.unwrap().is_ok());
    let rest = drain(rx1).await;
    assert!(matches!(terminal(&rest), StreamEvent::Error { reason: StopReason::Aborted, .. }));
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
    assert_eq!(engine.get_message_count().await.unwrap(), 0, "失败的请求不落盘");
}
