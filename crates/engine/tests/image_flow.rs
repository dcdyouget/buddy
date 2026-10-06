//! 端到端图片上下文测试。
//!
//! 这些用例从 ChatEngine 入口验证图片附件的生命周期：新图只在首个请求
//! 发送像素，历史图片只发送引用；read_file 读图后像素只注入紧邻的下一轮。

mod common;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use buddy_engine::chat::ChatEngine;
use buddy_engine::models::{AppConfig, ImageAttachment, Message, MessageRole};
use buddy_engine::storage;
use buddy_engine::streaming::{StreamEvent, StreamEventEmitter};
use common::*;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

const MODEL_ID: &str = "p1::vision-mock";
const IDLE: Duration = Duration::from_secs(3);

// 1x1 transparent PNG. Keeping the fixture inline makes the request assertion
// independent of the checkout layout and verifies a real image signature.
const PNG_1X1_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

fn png_bytes() -> Vec<u8> {
    BASE64_STANDARD.decode(PNG_1X1_BASE64).unwrap()
}

fn image_fixture(media_type: &str) -> Vec<u8> {
    let encoded = match media_type {
        "image/png" => PNG_1X1_BASE64,
        // 1x1 GIF89a.
        "image/gif" => "R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==",
        // 1x1 WebP.
        "image/webp" => "UklGRiIAAABXRUJQVlA4IBgAAAAwAQCdASoBAAEAAUAmJaQAA3AA/vuUAAA=",
        // Minimal JPEG header; the image reader only needs the JPEG SOI/signature
        // before handing the bytes to the provider.
        "image/jpeg" => {
            "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAP///wD/AP//AP//2wBDAf///wD/AP//AP//wAARCAABAAEDASIAAhEBAxEB/8QAFQABAQAAAAAAAAAAAAAAAAAAAAX/xAAUEAEAAAAAAAAAAAAAAAAAAAAA/9oADAMBAAIQAxAAAAH/xAAUEAEAAAAAAAAAAAAAAAAAAAAA/9oACAEBAAEFAqf/xAAUEQEAAAAAAAAAAAAAAAAAAAAA/9oACAEDAQE/AYf/xAAUEQEAAAAAAAAAAAAAAAAAAAAA/9oACAECAQE/AYf/xAAUEAEAAAAAAAAAAAAAAAAAAAAA/9oACAEBAAY/Ap//xAAUEAEAAAAAAAAAAAAAAAAAAAAA/9oACAEBAAE/IV//2gAMAwEAAgADAAAAEP/EABQRAQAAAAAAAAAAAAAAAAAAABD/2gAIAQMBAT8QH//EABQRAQAAAAAAAAAAAAAAAAAAABD/2gAIAQIBAT8QH//EABQQAQAAAAAAAAAAAAAAAAAAABD/2gAIAQEAAT8QH//Z"
        }
        _ => panic!("unsupported fixture type {media_type}"),
    };
    BASE64_STANDARD.decode(encoded).unwrap()
}

fn write_config_with_type(
    data_dir: &Path,
    base_url: &str,
    supports_vision: bool,
    provider_type: &str,
) {
    let config: AppConfig = serde_json::from_value(json!({
        "theme": "light",
        "providers": [{
            "id": "p1", "name": "Mock", "base_url": base_url, "api_key": "k",
            "enabled_model_ids": [MODEL_ID], "provider_type": provider_type
        }],
        "models": [{
            "id": MODEL_ID, "provider_id": "p1", "api_model_id": "vision-mock",
            "display_name": "Vision Mock", "context_window": 128000, "latency_ms": null,
            "supports_vision": supports_vision
        }],
        "selected_model_id": MODEL_ID,
        "auto_start": false
    }))
    .unwrap();
    storage::save_config(data_dir, &config).unwrap();
}

fn write_config(data_dir: &Path, base_url: &str, supports_vision: bool) {
    write_config_with_type(data_dir, base_url, supports_vision, "openai_compatible");
}

fn image(id: &str, name: &str, media_type: &str, path: &Path) -> ImageAttachment {
    ImageAttachment {
        id: id.to_string(),
        name: name.to_string(),
        media_type: media_type.to_string(),
        path: path.to_string_lossy().into_owned(),
        data_url: String::new(),
    }
}

fn user(id: &str, content: &str, images: Vec<ImageAttachment>) -> Message {
    serde_json::from_value(json!({
        "id": id, "role": "user", "content": content, "images": images,
        "model_id": null, "created_at": 0
    }))
    .unwrap()
}

fn assistant_with_call(id: &str, call_id: &str, name: &str) -> Message {
    serde_json::from_value(json!({
        "id": id, "role": "assistant", "content": "",
        "model_id": MODEL_ID, "created_at": 0,
        "tool_calls": [{"id": call_id, "name": name, "arguments": "{}"}]
    }))
    .unwrap()
}

fn tool_result(
    id: &str,
    call_id: &str,
    name: &str,
    content: &str,
    images: Vec<ImageAttachment>,
) -> Message {
    serde_json::from_value(json!({
        "id": id, "role": "tool", "content": content, "images": images,
        "model_id": null, "created_at": 0,
        "tool_call_id": call_id, "tool_name": name, "is_error": false
    }))
    .unwrap()
}

struct Run {
    result: Result<(), String>,
    events: Vec<StreamEvent>,
    requests: Vec<CapturedRequest>,
}

async fn run(
    engine: &Arc<ChatEngine>,
    messages: Vec<Message>,
    server: tokio::task::JoinHandle<Vec<CapturedRequest>>,
) -> Run {
    let (emitter, rx) = StreamEventEmitter::channel();
    let task_engine = engine.clone();
    let task = tokio::spawn(async move {
        task_engine
            .send_message(emitter, messages, MODEL_ID.to_string())
            .await
    });
    let events = drain(rx).await;
    let result = task.await.unwrap();
    let requests = server.await.unwrap();
    Run {
        result,
        events,
        requests,
    }
}

fn assert_done(events: &[StreamEvent]) {
    assert!(
        matches!(events.last(), Some(StreamEvent::Done { .. })),
        "终态事件: {events:?}"
    );
}

fn image_body_marker(media_type: &str) -> String {
    format!("data:{media_type};base64,{PNG_1X1_BASE64}")
}

fn attachment_count(data_dir: &Path) -> usize {
    std::fs::read_dir(data_dir.join("attachments"))
        .unwrap()
        .count()
}

fn anthropic_tool_call_script(
    call_id: &str,
    name: &str,
    arguments: &serde_json::Value,
) -> Vec<Chunk> {
    let event = |event_name: &str, data: serde_json::Value| {
        (
            Duration::ZERO,
            format!("event: {event_name}\ndata: {data}\n\n"),
        )
    };
    vec![
        event(
            "message_start",
            json!({"type":"message_start","message":{"id":"m-tool","type":"message","role":"assistant","content":[],"usage":{"input_tokens":3,"output_tokens":0}}}),
        ),
        event(
            "content_block_start",
            json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":call_id,"name":name}}),
        ),
        event(
            "content_block_delta",
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":arguments.to_string()}}),
        ),
        event(
            "content_block_stop",
            json!({"type":"content_block_stop","index":0}),
        ),
        event(
            "message_delta",
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":2}}),
        ),
        event("message_stop", json!({"type":"message_stop"})),
    ]
}

fn attachment_paths(body: &str) -> Vec<String> {
    let request: serde_json::Value = serde_json::from_str(body).unwrap();
    let paths: Vec<String> = request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|message| {
            let content = &message["content"];
            if let Some(text) = content.as_str() {
                vec![text.to_string()]
            } else {
                content
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|part| part["text"].as_str().map(str::to_owned))
                    .collect()
            }
        })
        .flat_map(|text| {
            let Some(references) = text.split("<buddy_attachments>\n").nth(1) else {
                return Vec::new();
            };
            let references: serde_json::Value =
                serde_json::from_str(references.split("\nUse read_file").next().unwrap()).unwrap();
            references
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|image| image["path"].as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .collect();
    paths
}

#[test]
fn attachment_paths_decode_windows_paths_from_nested_json() {
    let path = r"\\?\C:\Users\Buddy\attachments\image.png";
    let references = json!([{"path":path}]);
    let content =
        format!("<buddy_attachments>\n{references}\nUse read_file(path)\n</buddy_attachments>");
    for content in [json!(content), json!([{"type":"text","text":content}])] {
        let body = json!({"messages":[{"content":content}]}).to_string();
        assert_eq!(attachment_paths(&body), vec![path]);
    }
}

#[tokio::test]
async fn new_image_is_sent_once_and_history_images_are_json_references() {
    let tmp = TempDir::new().unwrap();
    let stored = storage::store_image_bytes(
        tmp.path(),
        "history.png",
        "image/png",
        &png_bytes(),
        "history",
    )
    .unwrap();
    let history_user_image = image(
        "history-user",
        "history.png",
        "image/png",
        Path::new(&stored.path),
    );
    let history_tool_image = image(
        "history-tool",
        "history-tool.png",
        "image/png",
        Path::new(&stored.path),
    );
    let new_image = image("new-image", "new.png", "image/png", Path::new(&stored.path));

    let history = vec![
        user("old-user", "旧图片问题", vec![history_user_image.clone()]),
        assistant_with_call("old-assistant", "old-call", "read_file"),
        tool_result(
            "old-tool",
            "old-call",
            "read_file",
            "旧图片已读取",
            vec![history_tool_image.clone()],
        ),
    ];
    storage::append_messages(tmp.path(), &history).unwrap();

    let note_path = tmp.path().join("note.txt");
    std::fs::write(&note_path, "文本读取正常").unwrap();
    let (url, server) = serve_sse_sequence(
        vec![
            openai_tool_calls_script(&[("call-text", "read_file", json!({"path": note_path}))]),
            openai_script(&["收到"], Duration::ZERO),
        ],
        IDLE,
    )
    .await;
    write_config(tmp.path(), &url, true);
    let engine = ChatEngine::new(tmp.path().to_path_buf());
    let run = run(
        &engine,
        [
            history,
            vec![user("new-user", "请看这张图", vec![new_image])],
        ]
        .concat(),
        server,
    )
    .await;

    assert!(
        run.result.is_ok(),
        "result={:?}, events={:?}, requests={}",
        run.result.as_ref().err(),
        run.events,
        run.requests.len()
    );
    assert_done(&run.events);
    assert_eq!(run.requests.len(), 2);
    let body = &run.requests[0].body;
    assert_eq!(body.matches(&image_body_marker("image/png")).count(), 1);
    for marker in [
        "history-user",
        "history-tool",
        "history.png",
        "history-tool.png",
        "image/png",
    ] {
        assert!(
            body.contains(marker),
            "请求缺少历史附件引用 {marker}: {body}"
        );
    }
    let paths = attachment_paths(body);
    assert!(paths.contains(&stored.path));
    assert!(
        !run.requests[1]
            .body
            .contains(&image_body_marker("image/png"))
    );
    assert!(run.requests[1].body.contains("文本读取正常"));
    // 历史与当前消息都只保存 path；写盘 JSON 不应携带任何 base64 像素。
    let raw = std::fs::read_to_string(tmp.path().join("chunk_001.json")).unwrap();
    assert!(
        !raw.contains("data:image/png;base64,"),
        "持久化泄漏图片像素: {raw}"
    );
    for message in engine.load_messages(0, 100).await.unwrap() {
        for image in &message.images {
            assert!(
                image.data_url.is_empty(),
                "历史消息携带 base64: {message:?}"
            );
        }
    }
}

#[tokio::test]
async fn read_file_image_is_visible_for_one_follow_up_then_becomes_reference() {
    let tmp = TempDir::new().unwrap();
    let stored = storage::store_image_bytes(
        tmp.path(),
        "picture.png",
        "image/png",
        &png_bytes(),
        "read-file",
    )
    .unwrap();
    let attachments_before = attachment_count(tmp.path());
    let text_path = tmp.path().join("note.txt");
    std::fs::write(&text_path, "纯文本文件").unwrap();

    let (url, server) = serve_sse_sequence(
        vec![
            openai_tool_calls_script(&[("call-image", "read_file", json!({"path": stored.path}))]),
            openai_tool_calls_script(&[("call-text", "read_file", json!({"path": text_path}))]),
            openai_script(&["图片和文本都已读取"], Duration::ZERO),
        ],
        IDLE,
    )
    .await;
    write_config(tmp.path(), &url, true);
    let engine = ChatEngine::new(tmp.path().to_path_buf());
    let run = run(
        &engine,
        vec![user("current", "请读取图片和文本", Vec::new())],
        server,
    )
    .await;

    assert!(
        run.result.is_ok(),
        "result={:?}, events={:?}, requests={}",
        run.result.as_ref().err(),
        run.events,
        run.requests.len()
    );
    assert_done(&run.events);
    assert_eq!(run.requests.len(), 3);
    let image_marker = image_body_marker("image/png");
    assert_eq!(run.requests[1].body.matches(&image_marker).count(), 1);
    assert!(run.requests[1].body.contains("call-image"));
    assert!(run.requests[1].body.contains("\"role\":\"tool\""));
    assert!(run.requests[2].body.contains("call-text"));
    assert!(!run.requests[2].body.contains(&image_marker));
    let managed_attachments = std::fs::canonicalize(tmp.path().join("attachments")).unwrap();
    let paths = attachment_paths(&run.requests[2].body);
    assert!(
        paths
            .iter()
            .any(|path| Path::new(path).starts_with(&managed_attachments)),
        "请求应保留可回读的附件路径：{paths:?}"
    );
    for marker in ["read-image", "image/png"] {
        assert!(
            run.requests[2].body.contains(marker),
            "缺少图片引用 {marker}: {}",
            run.requests[2].body
        );
    }

    let persisted = engine.load_messages(0, 100).await.unwrap();
    assert_eq!(
        persisted.iter().map(|m| m.role.clone()).collect::<Vec<_>>(),
        vec![
            MessageRole::User,
            MessageRole::Assistant,
            MessageRole::Tool,
            MessageRole::Assistant,
            MessageRole::Tool,
            MessageRole::Assistant,
        ]
    );
    let image_tool = persisted
        .iter()
        .find(|m| m.tool_call_id.as_deref() == Some("call-image"))
        .expect("图片工具结果应落盘");
    assert_eq!(image_tool.images.len(), 1);
    assert!(image_tool.images[0].data_url.is_empty());
    let persisted_path = std::fs::canonicalize(&image_tool.images[0].path).unwrap();
    let stored_path = std::fs::canonicalize(&stored.path).unwrap();
    assert_eq!(persisted_path, stored_path);
    assert_eq!(attachment_count(tmp.path()), attachments_before);
}

#[tokio::test]
async fn anthropic_read_file_image_is_injected_once_then_referenced() {
    let tmp = TempDir::new().unwrap();
    let source = tmp.path().join("picture.png");
    std::fs::write(&source, png_bytes()).unwrap();
    let text_path = tmp.path().join("note.txt");
    std::fs::write(&text_path, "Anthropic 文本读取正常").unwrap();

    let (url, server) = serve_sse_sequence(
        vec![
            anthropic_tool_call_script("anthropic-image", "read_file", &json!({"path": source})),
            anthropic_tool_call_script("anthropic-text", "read_file", &json!({"path": text_path})),
            anthropic_script(&["已读取"], Duration::ZERO),
        ],
        IDLE,
    )
    .await;
    write_config_with_type(tmp.path(), &url, true, "anthropic");
    let engine = ChatEngine::new(tmp.path().to_path_buf());
    let run = run(
        &engine,
        vec![user("current", "请读取图片和文本", Vec::new())],
        server,
    )
    .await;

    assert!(
        run.result.is_ok(),
        "result={:?}, events={:?}, requests={}",
        run.result.as_ref().err(),
        run.events,
        run.requests.len()
    );
    assert_done(&run.events);
    assert_eq!(run.requests.len(), 3);
    let image_marker = BASE64_STANDARD.decode(PNG_1X1_BASE64).unwrap();
    let image_data = BASE64_STANDARD.encode(image_marker);
    assert!(run.requests[1].body.contains("\"type\":\"image\""));
    assert!(run.requests[1].body.contains(&image_data));
    assert!(run.requests[1].body.contains("anthropic-image"));
    assert!(run.requests[2].body.contains("anthropic-text"));
    assert!(!run.requests[2].body.contains(&image_data));
    assert!(run.requests[2].body.contains("image/png"));
}

#[tokio::test]
async fn read_file_recognizes_supported_image_signatures() {
    for (extension, media_type) in [
        ("png", "image/png"),
        ("jpg", "image/jpeg"),
        ("gif", "image/gif"),
        ("webp", "image/webp"),
    ] {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(format!("fixture.{extension}"));
        let bytes = image_fixture(media_type);
        std::fs::write(&path, &bytes).unwrap();
        let (url, server) = serve_sse_sequence(
            vec![
                openai_tool_calls_script(&[("call-image", "read_file", json!({"path": path}))]),
                openai_script(&["已读取"], Duration::ZERO),
            ],
            IDLE,
        )
        .await;
        write_config(tmp.path(), &url, true);
        let engine = ChatEngine::new(tmp.path().to_path_buf());
        let run = run(
            &engine,
            vec![user("current", "读取图片", Vec::new())],
            server,
        )
        .await;

        assert!(
            run.result.is_ok(),
            "{media_type}: result={:?}, events={:?}, requests={}",
            run.result.as_ref().err(),
            run.events,
            run.requests.len()
        );
        assert_done(&run.events);
        assert!(
            run.events.iter().any(|event| matches!(
                event,
                StreamEvent::ToolResult {
                    is_error: false,
                    ..
                }
            )),
            "{media_type}: 图片应被识别"
        );
        let persisted = engine.load_messages(0, 100).await.unwrap();
        let tool = persisted
            .iter()
            .find(|message| message.tool_call_id.as_deref() == Some("call-image"))
            .expect("图片工具结果应落盘");
        assert_eq!(tool.images.len(), 1, "{media_type}");
        assert_eq!(tool.images[0].media_type, media_type, "{media_type}");
        assert!(
            tool.images[0].data_url.is_empty(),
            "{media_type} 不应把像素写入历史"
        );
        assert!(
            run.requests[1]
                .body
                .contains(&format!("data:{media_type};base64,"))
        );
    }
}

#[tokio::test]
async fn image_read_reports_error_when_model_has_no_vision_support() {
    let tmp = TempDir::new().unwrap();
    let stored = storage::store_image_bytes(
        tmp.path(),
        "no-vision.png",
        "image/png",
        &png_bytes(),
        "no-vision",
    )
    .unwrap();
    let (url, server) = serve_sse_sequence(
        vec![
            openai_tool_calls_script(&[(
                "call-no-vision",
                "read_file",
                json!({"path": stored.path}),
            )]),
            openai_script(&["当前模型不能读取图片"], Duration::ZERO),
        ],
        IDLE,
    )
    .await;
    write_config(tmp.path(), &url, false);
    let engine = ChatEngine::new(tmp.path().to_path_buf());
    let run = run(&engine, vec![user("current", "请读图", Vec::new())], server).await;

    assert!(
        run.result.is_ok(),
        "result={:?}, events={:?}, requests={}",
        run.result.as_ref().err(),
        run.events,
        run.requests.len()
    );
    assert_done(&run.events);
    let error = run
        .events
        .iter()
        .find_map(|event| match event {
            StreamEvent::ToolResult {
                is_error: true,
                content,
                ..
            } => Some(content),
            _ => None,
        })
        .expect("无视觉能力时图片工具应返回错误");
    assert!(
        error.contains("图片") || error.contains("视觉"),
        "错误应说明视觉能力限制: {error}"
    );
    assert!(!run.requests[1].body.contains("data:image/png;base64,"));
}

#[tokio::test]
async fn read_file_rejects_images_over_provider_limit() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("too-large.png");
    let mut bytes = vec![0_u8; buddy_engine::models::DEFAULT_IMAGE_BYTES + 1];
    bytes[..8].copy_from_slice(&png_bytes()[..8]);
    std::fs::write(&path, bytes).unwrap();
    let (url, server) = serve_sse_sequence(
        vec![
            openai_tool_calls_script(&[("call-too-large", "read_file", json!({"path": path}))]),
            openai_script(&["图片过大"], Duration::ZERO),
        ],
        IDLE,
    )
    .await;
    write_config(tmp.path(), &url, true);
    let engine = ChatEngine::new(tmp.path().to_path_buf());
    let run = run(
        &engine,
        vec![user("current", "请读取大图片", Vec::new())],
        server,
    )
    .await;

    assert!(run.result.is_ok());
    assert_done(&run.events);
    let error = run
        .events
        .iter()
        .find_map(|event| match event {
            StreamEvent::ToolResult {
                is_error: true,
                content,
                ..
            } => Some(content),
            _ => None,
        })
        .expect("超限图片应返回工具错误");
    assert!(
        error.contains("过大") || error.contains("5 MB"),
        "错误应说明图片大小限制: {error}"
    );
    assert!(!run.requests[1].body.contains("data:image/png;base64,"));
}
