//! S06-02 provider loopback fixture.  It serves only the requests made by the
//! real engine adapter; no HTTP client is used by the fixture itself.

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/// Response cases used by T37.  The mode is sampled when a request is
/// accepted, so a delayed response can be made stale by a later form edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MockMode {
    /// `/models` returns two raw model ids and latency succeeds.
    Success,
    /// `/models` returns a valid empty list.
    Empty,
    /// `/models` returns an authentication error.
    Unauthorized,
    /// `/models` returns a body that is not JSON.
    InvalidJson,
    /// Model discovery succeeds; latency returns an HTTP error.
    LatencyError,
    /// The model response is delayed long enough for a revision change.
    DelayedSuccess,
}

/// A disposable loopback HTTP server for settings selftests.
pub(crate) struct ProviderMock {
    address: String,
    mode: Arc<Mutex<MockMode>>,
    requests: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl ProviderMock {
    /// Start an ephemeral listener on 127.0.0.1.
    pub(crate) fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("绑定 provider loopback");
        listener
            .set_nonblocking(true)
            .expect("设置 loopback 非阻塞");
        let address = format!(
            "http://127.0.0.1:{}/v1",
            listener.local_addr().expect("读取 loopback 地址").port()
        );
        let mode = Arc::new(Mutex::new(MockMode::Success));
        let requests = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_mode = mode.clone();
        let thread_requests = requests.clone();
        let thread_stop = stop.clone();
        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let current = thread_mode
                            .lock()
                            .map(|mode| *mode)
                            .unwrap_or(MockMode::InvalidJson);
                        serve(stream, current, &thread_requests);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            address,
            mode,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    pub(crate) fn url(&self) -> String {
        self.address.clone()
    }

    pub(crate) fn set_mode(&self, mode: MockMode) {
        if let Ok(mut current) = self.mode.lock() {
            *current = mode;
        }
    }

    pub(crate) fn request_count(&self) -> usize {
        self.requests.load(Ordering::Relaxed)
    }
}

impl Drop for ProviderMock {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Wake a nonblocking accept loop without relying on a fixed sleep.
        let _ = TcpStream::connect(
            self.address
                .trim_start_matches("http://")
                .split('/')
                .next()
                .unwrap_or_default(),
        );
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(mut stream: TcpStream, mode: MockMode, requests: &AtomicUsize) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut request = [0_u8; 16 * 1024];
    let size = stream.read(&mut request).unwrap_or(0);
    let first_line = std::str::from_utf8(&request[..size])
        .ok()
        .and_then(|request| request.lines().next())
        .unwrap_or_default();
    // 只计 engine 的 GET/POST；本地探测 HEAD / 与唤醒连接不属于模型调用。
    if first_line.starts_with("GET ") || first_line.starts_with("POST ") {
        requests.fetch_add(1, Ordering::Relaxed);
    }

    let is_latency = first_line.starts_with("POST ") && first_line.contains("/chat/completions");
    if mode == MockMode::DelayedSuccess && !is_latency {
        thread::sleep(Duration::from_millis(180));
    }
    let (status, body) = if is_latency && mode == MockMode::LatencyError {
        (
            "500 Internal Server Error",
            r#"{"error":"latency fixture"}"#.to_string(),
        )
    } else if !is_latency {
        match mode {
            MockMode::Empty => ("200 OK", r#"{"data":[]}"#.to_string()),
            MockMode::Unauthorized => (
                "401 Unauthorized",
                r#"{"error":{"message":"invalid api key"}}"#.to_string(),
            ),
            MockMode::InvalidJson => ("200 OK", "not-json".to_string()),
            _ => (
                "200 OK",
                r#"{"object":"list","data":[{"id":"alpha"},{"id":"shared"}]}"#.to_string(),
            ),
        }
    } else {
        (
            "200 OK",
            r#"{"choices":[{"message":{"content":"ok"}}]}"#.to_string(),
        )
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}
