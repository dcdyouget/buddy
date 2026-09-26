//! S02-06 驱动程序：GPUI 窗口经 `buddy_ui::chat_bridge` 驱动 engine 对话。
//!
//! ```text
//! cargo run -p buddy-app --example chat_bridge -- --mock   # 自动化：本地 mock，打印指标并退出
//! cargo run -p buddy-app --example chat_bridge             # 手动：用真实配置开一个最小窗口
//! ```
//!
//! 手动模式把 v1 的 `config.json` **复制**到沙盒目录 `target/buddy-dev-data/`，
//! 对话记录写在沙盒里，不碰 v1 的真实历史。

use buddy_engine::chat::ChatEngine;
use buddy_engine::models::{AppConfig, Message};
use buddy_engine::storage;
use buddy_engine::streaming::StreamEvent;
use buddy_ui::ActiveTheme;
use buddy_ui::chat_bridge::{self, ChatFinished};
use buddy_ui::gpui::{
    App, AsyncApp, Bounds, Context, Render, Task, Window, WindowBounds, WindowHandle,
    WindowOptions, div, prelude::*, px, size,
};
use buddy_ui::gpui_platform::application;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ───────────────────────────── 视图 ─────────────────────────────

struct ChatView {
    engine: Arc<ChatEngine>,
    model_id: String,
    title: String,
    text: String,
    thinking_chars: usize,
    status: String,
    sent_at: Instant,
    first_batch_ms: Option<u128>,
    run: Option<Task<()>>,
    finished: Option<ChatFinished>,
    terminal: Option<String>,
}

impl ChatView {
    fn new(engine: Arc<ChatEngine>, model_id: String, title: String) -> Self {
        Self {
            engine,
            model_id,
            title,
            text: String::new(),
            thinking_chars: 0,
            status: "就绪".into(),
            sent_at: Instant::now(),
            first_batch_ms: None,
            run: None,
            finished: None,
            terminal: None,
        }
    }

    fn send(&mut self, prompt: &str, cx: &mut Context<Self>) {
        let message: Message = serde_json::from_value(serde_json::json!({
            "id": format!("demo-{}", chrono_ms()), "role": "user", "content": prompt,
            "model_id": null, "created_at": chrono_ms() / 1000
        }))
        .expect("构造消息");
        self.text.clear();
        self.thinking_chars = 0;
        self.first_batch_ms = None;
        self.finished = None;
        self.terminal = None;
        self.status = "生成中…".into();
        self.sent_at = Instant::now();

        let task = chat_bridge::start_chat(
            self.engine.clone(),
            vec![message],
            self.model_id.clone(),
            cx,
            |view: &mut ChatView, batch, cx| {
                view.first_batch_ms
                    .get_or_insert_with(|| view.sent_at.elapsed().as_millis());
                for ev in batch {
                    match ev {
                        // 显示只用 TextDelta 累加（Done.full_text 含思考标签，见 S00-08 §3.1）
                        StreamEvent::TextDelta { delta, .. } => view.text.push_str(&delta),
                        StreamEvent::ThinkingDelta { delta, .. } => {
                            view.thinking_chars += delta.chars().count()
                        }
                        StreamEvent::Done { .. } => view.terminal = Some("Done".into()),
                        StreamEvent::Error { reason, message, .. } => {
                            view.terminal = Some(format!("Error({reason:?}): {message}"))
                        }
                        _ => {}
                    }
                }
                cx.notify();
            },
        );
        // 视图被销毁时这个 Task 随之 drop：只停止前台交付，engine 的生成不受影响（见 chat_bridge 文档）
        self.run = Some(cx.spawn(async move |this, cx| {
            let finished = task.await;
            this.update(cx, |view, cx| {
                view.status = match (&finished.result, &view.terminal) {
                    (Err(e), _) => format!("被拒绝：{e}"),
                    (Ok(()), Some(t)) => format!("结束：{t}"),
                    (Ok(()), None) => "结束（无终态事件？）".into(),
                };
                view.finished = Some(finished);
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }
}

impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .px_3()
                .py_1()
                .rounded_md()
                .bg(colors.element_background)
                .border_1()
                .border_color(colors.border)
                .cursor_pointer()
                .child(label)
        };
        let metrics = format!(
            "首批事件到达前台：{}　|　事件 {} / 批次 {}　|　思考 {} 字",
            self.first_batch_ms
                .map(|ms| format!("{ms} ms"))
                .unwrap_or_else(|| "—".into()),
            self.finished.as_ref().map_or(0, |f| f.events),
            self.finished.as_ref().map_or(0, |f| f.batches),
            self.thinking_chars,
        );
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_3()
            .bg(colors.background)
            .text_color(colors.text)
            .child(div().text_color(colors.text_muted).child(self.title.clone()))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(button("send", "发送示例问题").on_click(cx.listener(
                        |view, _, _, cx| view.send("用三句话介绍一下你自己。", cx),
                    )))
                    .child(button("stop", "停止生成").on_click(cx.listener(
                        |view, _, _, _| view.engine.stop_generation(),
                    ))),
            )
            .child(
                div()
                    .flex_1()
                    .p_3()
                    .rounded_lg()
                    .bg(colors.editor_background)
                    .child(if self.text.is_empty() { "（等待回复）".to_string() } else { self.text.clone() }),
            )
            .child(div().text_color(colors.text_muted).child(self.status.clone()))
            .child(div().text_color(colors.text_muted).child(metrics))
    }
}

fn chrono_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

fn open_chat_window(cx: &mut App, view: impl FnOnce() -> ChatView + 'static) -> WindowHandle<ChatView> {
    let bounds = Bounds::centered(None, size(px(640.0), px(480.0)), cx);
    cx.open_window(
        WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
        |_, cx| cx.new(|_| view()),
    )
    .expect("open_window 失败")
}

// ───────────────────────────── 手动模式（真实配置）─────────────────────────────

fn run_real(cx: &mut App) {
    let sandbox = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/buddy-dev-data");
    std::fs::create_dir_all(&sandbox).expect("创建沙盒目录");
    let v1_dir = storage::default_data_dir().expect("默认数据目录");
    std::fs::copy(v1_dir.join("config.json"), sandbox.join("config.json"))
        .expect("复制 v1 config.json 到沙盒");
    let config: AppConfig = storage::get_config(&sandbox).expect("读取配置");
    let model = config
        .models
        .iter()
        .find(|m| m.id == config.selected_model_id)
        .or(config.models.first())
        .expect("配置中没有模型");
    let title = format!("模型：{}（沙盒数据目录，不影响 v1 聊天记录）", model.display_name);
    let engine = ChatEngine::new(sandbox);
    let model_id = model.id.clone();
    open_chat_window(cx, move || ChatView::new(engine, model_id, title));
    cx.activate(true);
}

// ───────────────────────────── 自动化模式（mock）─────────────────────────────

const MOCK_PARTS: usize = 20;
const MOCK_GAP_MS: u64 = 20;

/// 每个 POST 连接都按同一脚本应答：20 段文本、每段间隔 20ms
fn start_mock_server() -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let rt = Box::leak(Box::new(tokio::runtime::Runtime::new().unwrap()));
    let listener = rt
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    rt.spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { return };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 65536];
                let Ok(n) = sock.read(&mut buf).await else { return };
                // 本机有端口探测（S02-07 实测 HEAD /），只应答 POST
                if !buf[..n].starts_with(b"POST ") {
                    return;
                }
                let head = b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n";
                if sock.write_all(head).await.is_err() {
                    return;
                }
                for i in 0..MOCK_PARTS {
                    tokio::time::sleep(Duration::from_millis(MOCK_GAP_MS)).await;
                    let data = serde_json::json!({"choices":[{"index":0,"delta":{"content":format!("{i},")}}]});
                    if sock.write_all(format!("data: {data}\n\n").as_bytes()).await.is_err() {
                        return; // 客户端取消 / 断开
                    }
                }
                let _ = sock
                    .write_all(b"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n")
                    .await;
            });
        }
    });
    url
}

fn expected_text() -> String {
    (0..MOCK_PARTS).map(|i| format!("{i},")).collect()
}

async fn wait_until(cx: &mut AsyncApp, timeout: Duration, mut pred: impl FnMut(&mut AsyncApp) -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if pred(cx) {
            return true;
        }
        cx.background_executor().timer(Duration::from_millis(10)).await;
    }
    false
}

fn run_mock(cx: &mut App) {
    let url = start_mock_server();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/buddy-bridge-mock")
        .join(chrono_ms().to_string());
    std::fs::create_dir_all(&dir).unwrap();
    let config: AppConfig = serde_json::from_value(serde_json::json!({
        "theme": "light",
        "providers": [{"id":"p1","name":"Mock","base_url":url,"api_key":"k",
                       "enabled_model_ids":["p1::mock"],"provider_type":"openai_compatible"}],
        "models": [{"id":"p1::mock","provider_id":"p1","api_model_id":"mock","display_name":"Mock",
                    "context_window":128000,"latency_ms":null}],
        "selected_model_id": "p1::mock"
    }))
    .unwrap();
    storage::save_config(&dir, &config).unwrap();
    let engine = ChatEngine::new(dir);

    let e1 = engine.clone();
    let w1 = open_chat_window(cx, move || ChatView::new(e1, "p1::mock".into(), "mock #1".into()));
    w1.update(cx, |v, _, cx| v.send("mock", cx)).unwrap();

    cx.spawn(async move |cx: &mut AsyncApp| {
        let mut checks: Vec<(String, bool)> = Vec::new();

        // ── 场景 1：完整对话，前台收齐事件 ──
        let done = wait_until(cx, Duration::from_secs(10), |cx| {
            w1.update(cx, |v, _, _| v.finished.is_some()).unwrap_or(false)
        })
        .await;
        let (text, terminal, first_ms, fin) = w1
            .update(cx, |v, _, _| {
                let f = v.finished.as_ref();
                (v.text.clone(), v.terminal.clone(), v.first_batch_ms,
                 f.map(|f| (f.events, f.batches, f.result.clone())))
            })
            .unwrap();
        println!("[场景1] first_batch_ms={first_ms:?} events/batches/result={fin:?} terminal={terminal:?}");
        checks.push(("场景1 对话结束".into(), done));
        checks.push(("场景1 文本完整".into(), text == expected_text()));
        checks.push(("场景1 终态 Done".into(), terminal.as_deref() == Some("Done")));
        checks.push(("场景1 首批事件 < 200ms".into(), first_ms.is_some_and(|ms| ms < 200)));
        checks.push(("场景1 批次数 ≤ 事件数".into(), fin.as_ref().is_some_and(|(e, b, _)| b <= e && *b > 0)));

        // ── 场景 2：生成中途关闭窗口（视图销毁）→ 生成不中断，照常落盘并释放占用 ──
        // engine 的 async 方法必须经 spawn_engine 在 tokio 上执行（直接 await 会 panic，见 chat_bridge 文档）
        let count = |cx: &mut AsyncApp, e: Arc<ChatEngine>| {
            cx.update(|cx| chat_bridge::spawn_engine(cx, async move { e.get_message_count().await.unwrap() }))
        };
        let before = count(cx, engine.clone()).await;
        let e2 = engine.clone();
        let w2 = cx.update(|cx| open_chat_window(cx, move || ChatView::new(e2, "p1::mock".into(), "mock #2".into())));
        w2.update(cx, |v, _, cx| v.send("mock", cx)).unwrap();
        wait_until(cx, Duration::from_secs(5), |cx| {
            w2.update(cx, |v, _, _| v.first_batch_ms.is_some()).unwrap_or(false)
        })
        .await;
        w2.update(cx, |_, window, _| window.remove_window()).unwrap();
        let closed_at = Instant::now();
        let e = engine.clone();
        let mut persisted = false;
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if count(cx, e.clone()).await == before + 2 {
                persisted = true;
                break;
            }
            cx.background_executor().timer(Duration::from_millis(20)).await;
        }
        println!("[场景2] 关窗后 {} ms 内落盘 user+assistant：{persisted}", closed_at.elapsed().as_millis());
        checks.push(("场景2 关窗后生成仍完成并落盘".into(), persisted));

        // ── 场景 3：生成占用已释放，可立即再次对话 ──
        let e3 = engine.clone();
        let w3 = cx.update(|cx| open_chat_window(cx, move || ChatView::new(e3, "p1::mock".into(), "mock #3".into())));
        w3.update(cx, |v, _, cx| v.send("mock", cx)).unwrap();
        wait_until(cx, Duration::from_secs(10), |cx| {
            w3.update(cx, |v, _, _| v.finished.is_some()).unwrap_or(false)
        })
        .await;
        let ok3 = w3
            .update(cx, |v, _, _| v.finished.as_ref().is_some_and(|f| f.result.is_ok()) && v.terminal.as_deref() == Some("Done"))
            .unwrap_or(false);
        checks.push(("场景3 占用已释放，可再次对话".into(), ok3));

        // ── 场景 4：生成中按「停止」→ 终态 Aborted ──
        w3.update(cx, |v, _, cx| v.send("mock", cx)).unwrap();
        wait_until(cx, Duration::from_secs(5), |cx| {
            w3.update(cx, |v, _, _| v.first_batch_ms.is_some()).unwrap_or(false)
        })
        .await;
        engine.stop_generation();
        wait_until(cx, Duration::from_secs(5), |cx| {
            w3.update(cx, |v, _, _| v.finished.is_some()).unwrap_or(false)
        })
        .await;
        let t4 = w3.update(cx, |v, _, _| v.terminal.clone()).ok().flatten();
        println!("[场景4] terminal={t4:?}");
        checks.push(("场景4 停止 → Error(Aborted)".into(), t4.as_deref().is_some_and(|t| t.starts_with("Error(Aborted)"))));

        let mut all = true;
        for (name, ok) in &checks {
            println!("{} {name}", if *ok { "PASS" } else { "FAIL" });
            all &= ok;
        }
        println!("RESULT: {}", if all { "PASS" } else { "FAIL" });
        std::process::exit(if all { 0 } else { 1 });
    })
    .detach();
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let mock = std::env::args().any(|a| a == "--mock");
    application().run(move |cx: &mut App| {
        buddy_ui::init_theme(cx);
        chat_bridge::init(cx);
        if mock { run_mock(cx) } else { run_real(cx) }
    });
}
