//! 集成测试共用：本地 mock SSE 服务端与事件收集。
//!
//! 只接受一个连接，读完请求后按脚本逐块写出 SSE 响应，写完即关闭连接
//! （`Connection: close`，响应体以连接关闭为界）。

#![allow(dead_code)] // 各测试文件只用到其中一部分

use buddy_engine::models::Message;
use buddy_engine::streaming::StreamEvent;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::JoinHandle;

/// mock 服务端收到的请求（用于断言 provider 拼出的路径与鉴权头）
#[derive(Debug)]
pub struct CapturedRequest {
    pub request_line: String,
    pub headers: String,
    pub body: String,
}

/// 服务端写出一块数据前等待的时长 + 数据本身
pub type Chunk = (Duration, String);

/// 启动 mock 服务端，返回 base_url 与服务端任务句柄
pub async fn serve_sse(chunks: Vec<Chunk>) -> (String, JoinHandle<CapturedRequest>) {
    serve_sse_with_header_delay(Duration::ZERO, chunks).await
}

/// 同上，但读完请求后先等待 `header_delay` 再发响应头（模拟慢首包）
pub async fn serve_sse_with_header_delay(
    header_delay: Duration,
    chunks: Vec<Chunk>,
) -> (String, JoinHandle<CapturedRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let captured = read_request(&mut sock).await;
        // 等待期间同时监听对端关闭：客户端取消会断开连接，read 返回 0 / Err 即结束
        let mut probe = [0u8; 1];
        tokio::select! {
            _ = tokio::time::sleep(header_delay) => {}
            r = sock.read(&mut probe) => {
                if matches!(r, Ok(0) | Err(_)) {
                    return captured;
                }
            }
        }
        // 客户端在等响应头时取消会断开连接，写失败即结束
        if sock.write_all(
            b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\nconnection: close\r\n\r\n",
        )
        .await
        .is_err()
        {
            return captured;
        }
        for (delay, data) in chunks {
            tokio::time::sleep(delay).await;
            // 客户端取消后会断开连接，写失败即停止
            if sock.write_all(data.as_bytes()).await.is_err() {
                break;
            }
            let _ = sock.flush().await;
        }
        let _ = sock.shutdown().await;
        captured
    });
    (base_url, handle)
}

async fn read_request(sock: &mut tokio::net::TcpStream) -> CapturedRequest {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        let n = sock.read(&mut tmp).await.unwrap();
        assert!(n > 0, "客户端在发完请求头之前断开");
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let content_length = head
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    while buf.len() < header_end + content_length {
        let n = sock.read(&mut tmp).await.unwrap();
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    let (request_line, headers) = head.split_once("\r\n").unwrap();
    CapturedRequest {
        request_line: request_line.to_string(),
        headers: headers.to_string(),
        body: String::from_utf8_lossy(&buf[header_end..]).to_string(),
    }
}

/// 构造一条 user 消息（`Message` 无 `Default`，经 serde 构造以免逐字段列举）
pub fn user_message(content: &str) -> Message {
    serde_json::from_value(serde_json::json!({
        "id": "u1",
        "role": "user",
        "content": content,
        "model_id": null,
        "created_at": 0,
    }))
    .unwrap()
}

/// 收完 channel 中的全部事件（发送端 drop 后返回）
pub async fn drain(mut rx: UnboundedReceiver<StreamEvent>) -> Vec<StreamEvent> {
    let mut out = Vec::new();
    while let Some(ev) = rx.recv().await {
        out.push(ev);
    }
    out
}

/// 拼接全部 `TextDelta`
pub fn text_of(events: &[StreamEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            StreamEvent::TextDelta { delta, .. } => Some(delta.as_str()),
            _ => None,
        })
        .collect()
}

/// OpenAI 兼容协议的 SSE 脚本：逐段文本增量 + finish_reason + `[DONE]`
pub fn openai_script(parts: &[&str], gap: Duration) -> Vec<Chunk> {
    let mut v: Vec<Chunk> = parts
        .iter()
        .map(|p| {
            let data = serde_json::json!({"choices":[{"index":0,"delta":{"content":p}}]});
            (gap, format!("data: {data}\n\n"))
        })
        .collect();
    v.push((
        gap,
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2,\"total_tokens\":5}}\n\n".into(),
    ));
    v.push((gap, "data: [DONE]\n\n".into()));
    v
}

/// Anthropic Messages 协议的 SSE 脚本
pub fn anthropic_script(parts: &[&str], gap: Duration) -> Vec<Chunk> {
    let ev = |name: &str, data: serde_json::Value| (gap, format!("event: {name}\ndata: {data}\n\n"));
    let mut v = vec![
        ev(
            "message_start",
            serde_json::json!({"type":"message_start","message":{"id":"m1","type":"message","role":"assistant","content":[],"usage":{"input_tokens":3,"output_tokens":0}}}),
        ),
        ev(
            "content_block_start",
            serde_json::json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        ),
    ];
    for p in parts {
        v.push(ev(
            "content_block_delta",
            serde_json::json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":p}}),
        ));
    }
    v.push(ev("content_block_stop", serde_json::json!({"type":"content_block_stop","index":0})));
    v.push(ev(
        "message_delta",
        serde_json::json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}),
    ));
    v.push(ev("message_stop", serde_json::json!({"type":"message_stop"})));
    v
}
