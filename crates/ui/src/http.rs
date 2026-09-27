//! GPUI 的 HTTP 客户端（S04-08）：让 `img()` 能加载网络图片（markdown 中的 `![alt](https://…)`）
//!
//! GPUI 默认的客户端是空实现，网络图片会静默走失败占位。这里用 engine 同款 `reqwest`
//! 在 gpui_tokio 的运行时上执行请求。只服务于**无请求体**的请求（图片 GET）；带请求体时返回错误，
//! 以免被误用为通用客户端（模型请求一律走 engine）。
//!
//! 代理：`reqwest` 读取 `HTTP(S)_PROXY` 环境变量；v1 的 webview 走系统代理 —— 差异见 S04-08 决策记录。

use gpui::http_client::{AsyncBody, HttpClient, Inner, Request, Response, Url, http};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// 基于 reqwest 的 GPUI HTTP 客户端
pub struct ReqwestHttpClient {
    client: reqwest::Client,
    runtime: tokio::runtime::Handle,
}

impl ReqwestHttpClient {
    /// 在给定 tokio 运行时上执行请求
    pub fn new(runtime: tokio::runtime::Handle) -> Self {
        Self { client: reqwest::Client::new(), runtime }
    }
}

/// 安装为应用的 HTTP 客户端。须在 [`crate::chat_bridge::init`]（gpui_tokio）之后调用。
pub fn install(cx: &mut gpui::App) {
    let runtime = gpui_tokio::Tokio::handle(cx);
    cx.set_http_client(Arc::new(ReqwestHttpClient::new(runtime)));
}

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

impl HttpClient for ReqwestHttpClient {
    fn user_agent(&self) -> Option<&http::HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(&self, req: Request<AsyncBody>) -> BoxFuture<anyhow::Result<Response<AsyncBody>>> {
        let (parts, body) = req.into_parts();
        let has_body = match &body.0 {
            Inner::Empty => false,
            Inner::Bytes(cursor) => !cursor.get_ref().is_empty(),
            Inner::AsyncReader(_) => true,
        };
        let client = self.client.clone();
        let task = self.runtime.spawn(async move {
            anyhow::ensure!(!has_body, "ReqwestHttpClient 只用于无请求体的请求（图片加载）");
            let mut request = client.request(parts.method, parts.uri.to_string());
            for (name, value) in &parts.headers {
                request = request.header(name, value);
            }
            let uri = parts.uri.to_string();
            let response = request.send().await.inspect_err(|e| log::debug!("http {uri} 失败：{e}"))?;
            log::debug!("http {uri} → {}", response.status());
            let mut builder = Response::builder().status(response.status().as_u16());
            for (name, value) in response.headers() {
                builder = builder.header(name.as_str(), value.as_bytes());
            }
            let bytes = response.bytes().await?;
            Ok(builder.body(AsyncBody::from(bytes.to_vec()))?)
        });
        Box::pin(async move { task.await? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// 本地起一个只回一次的 HTTP 服务，验证状态码、响应头与字节体原样传回
    #[test]
    fn get_returns_status_headers_and_body() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let payload: &[u8] = b"\x89PNG\r\n\x1a\nfake";
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = s.read(&mut buf);
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", payload.len());
            s.write_all(head.as_bytes()).unwrap();
            s.write_all(payload).unwrap();
        });
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
        // 测试环境可能设置了代理；本地地址须直连
        let client = ReqwestHttpClient { client: reqwest::Client::builder().no_proxy().build().unwrap(), runtime: rt.handle().clone() };
        let req = Request::get(format!("http://{addr}/a.png")).body(AsyncBody::empty()).unwrap();
        let mut resp = rt.block_on(client.send(req)).unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        assert_eq!(resp.headers()["content-type"], "image/png");
        let Inner::Bytes(cursor) = &mut resp.body_mut().0 else { panic!("应为内存字节体") };
        assert_eq!(cursor.get_ref().as_ref(), payload);
    }

    #[test]
    fn request_with_body_is_rejected() {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
        let client = ReqwestHttpClient::new(rt.handle().clone());
        let req = Request::post("http://127.0.0.1:9/").body(AsyncBody::from("x".to_string())).unwrap();
        let Err(err) = rt.block_on(client.send(req)) else { panic!("带请求体的请求应被拒绝") };
        assert!(err.to_string().contains("无请求体"), "{err}");
    }
}
