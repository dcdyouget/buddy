//! Shell 自测的独立数据目录与可用模型配置。

use buddy_engine::models::{AppConfig, ModelInfo, ProviderConfig, Theme};
use buddy_ui::shell::config::ShellConfig;
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) fn sandbox(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/buddy-shell-preview")
        .join(std::process::id().to_string())
        .join(format!("{name}-{sequence}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("创建 shell 沙盒");
    path
}

pub(crate) fn config() -> AppConfig {
    AppConfig {
        theme: Theme::Light,
        hotkey: "CmdOrCtrl+J".into(),
        providers: vec![ProviderConfig {
            id: "shell-preview".into(),
            name: "Shell 预览服务".into(),
            base_url: "http://127.0.0.1:1".into(),
            api_key: "shell-preview-key".into(),
            enabled_model_ids: vec!["shell-preview::mock".into()],
            provider_type: "openai_compatible".into(),
            compat: None,
        }],
        models: vec![ModelInfo {
            id: "shell-preview::mock".into(),
            provider_id: "shell-preview".into(),
            api_model_id: Some("mock".into()),
            display_name: "Shell 预览模型".into(),
            context_window: 128_000,
            latency_ms: Some(10),
            supports_vision: false,
            supports_image_generation: false,
        }],
        selected_model_id: "shell-preview::mock".into(),
        auto_start: false,
        allowed_paths: Vec::new(),
        mcp_servers: Vec::new(),
    }
}

pub(crate) fn shell_config() -> ShellConfig {
    ShellConfig::default()
}

/// 创建带本地 SSE mock 的真实 ChatEngine；请求中的 401/慢 可用于 shell 手测。
pub(crate) fn manual_engine(dark: bool) -> Arc<buddy_engine::chat::ChatEngine> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let runtime = Box::leak(Box::new(
        tokio::runtime::Runtime::new().expect("mock runtime"),
    ));
    let listener = runtime
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .expect("mock listener");
    let url = format!("http://{}", listener.local_addr().expect("mock address"));
    runtime.spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { break };
            tokio::spawn(async move {
                let mut bytes = Vec::new();
                let mut chunk = [0_u8; 8192];
                let body_start = loop {
                    let Ok(size) = socket.read(&mut chunk).await else { return };
                    if size == 0 { return }
                    bytes.extend_from_slice(&chunk[..size]);
                    if let Some(position) = bytes.windows(4).position(|item| item == b"\r\n\r\n") {
                        break position + 4;
                    }
                };
                let headers = String::from_utf8_lossy(&bytes[..body_start]).to_ascii_lowercase();
                let length = headers.lines().find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|value| value.trim().parse::<usize>().ok()).unwrap_or(0);
                while bytes.len() < body_start + length {
                    let Ok(size) = socket.read(&mut chunk).await else { return };
                    if size == 0 { return }
                    bytes.extend_from_slice(&chunk[..size]);
                }
                let body = serde_json::from_slice::<serde_json::Value>(&bytes[body_start..body_start + length]).unwrap_or_default();
                let prompt = body["messages"].as_array().and_then(|messages| messages.iter().rev().find(|message| message["role"] == "user"))
                    .and_then(|message| message["content"].as_str()).unwrap_or_default();
                if prompt.contains("401") {
                    let response = b"HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";
                    let _ = socket.write_all(response).await;
                    return;
                }
                let _ = socket.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n").await;
                let last_is_tool = body["messages"].as_array().and_then(|messages| messages.last())
                    .is_some_and(|message| message["role"] == "tool");
                if !last_is_tool && prompt.contains("请写文件") {
                    if let Some(path) = prompt.split_whitespace().find(|word| word.starts_with('/')) {
                        let arguments = serde_json::json!({"path":path,"content":"hello"}).to_string();
                        let data = serde_json::json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"shell-write","type":"function","function":{"name":"create_file","arguments":arguments}}]}}]});
                        let _ = socket.write_all(format!("data: {data}\n\n").as_bytes()).await;
                        let _ = socket.write_all(b"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n").await;
                        return;
                    }
                }
                let count = if prompt.contains('慢') { 80 } else { 6 };
                for index in 0..count {
                    if prompt.contains('慢') { tokio::time::sleep(std::time::Duration::from_millis(20)).await; }
                    let data = serde_json::json!({"choices":[{"index":0,"delta":{"content":format!("{index},")}}]});
                    if socket.write_all(format!("data: {data}\n\n").as_bytes()).await.is_err() { return }
                }
                let _ = socket.write_all(b"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n").await;
            });
        }
    });
    let data_dir = sandbox(if dark { "manual-dark" } else { "manual" });
    let mut config = config();
    config.theme = if dark { Theme::Dark } else { Theme::Light };
    config.providers[0].base_url = url;
    buddy_engine::storage::save_config(&data_dir, &config).expect("写入 mock 配置");
    buddy_engine::chat::ChatEngine::new(data_dir)
}

/// Full SSE payload, not merely its last token: catches drops while the window is hidden.
pub(crate) fn complete_slow_response(text: &str) -> bool {
    text == (0..80).map(|index| format!("{index},")).collect::<String>()
}
