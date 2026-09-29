//! S05-18 页面状态机与 engine 接入的预览与自检。
//!
//! ```text
//! cargo run -p buddy-app --example app_preview -- --mock       # 目检：本地 mock 模型（提示词含「401」「429」「500」「慢」触发对应情形）
//! cargo run -p buddy-app --example app_preview -- --no-key     # 目检：空配置，看无 Key 页与补齐流程
//! cargo run -p buddy-app --example app_preview                 # 目检：复制 v1 的 config.json 到沙盒，用真实模型
//! cargo run -p buddy-app --example app_preview -- --selftest   # 自检后退出
//! ```
//!
//! 数据都在沙盒 `target/buddy-app-preview/`，不碰 v1 的真实历史。窗口 560×480、无标题栏、透明；
//! **切页不改变窗口尺寸**（v1 的「离开紧凑页时展开」属 Phase 07）。Cmd+Q / Esc 退出。
//!
//! 自检（mock 模型 + 真实 engine 与磁盘）：
//! - T25 页面状态机端到端（S05-18）：空态发送 → 流式 → 对话；401 → 无 Key 页 → 补齐配置 → 对话；
//!   429 / 500 → 界面生成的提示消息落盘；停止 → 对话且无错误；设置叠加层往返；展开；全程窗口尺寸不变；
//! - T26 无 Key 流程：缺配置时空态发送 → 无 Key 页且草稿保留、不发送 → 点面板进设置 → 返回进对话；配置失效退回空态；
//! - T27 历史：重启后读回最新一页（起始页总是空态），触顶后经 engine 读取更早一页。

use buddy_engine::chat::ChatEngine;
use buddy_engine::models::{AppConfig, Message, MessageRole};
use buddy_engine::storage;
use buddy_ui::chat::composer::ComposerEvent;
use buddy_ui::chat::page_state::{Page, expands_window};
use buddy_ui::chat::router::{Loaded, PageRouter, RouterEvent, preload};
use buddy_ui::chat_bridge::spawn_engine;
use buddy_ui::gpui::{
    App, AppContext, AsyncApp, Bounds, Context, Entity, Focusable, IntoElement, Render, Subscription, Window, div, prelude::*, KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, PlatformInput,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowOptions, point, px, size,
};
use buddy_ui::gpui_platform::application;
use buddy_ui::markdown;
use buddy_ui::theme_system::{Appearance, Theme, fonts};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

const WIDTH: f32 = 560.0;
const HEIGHT: f32 = 480.0;
const MOCK_PARTS: usize = 20;
const SLOW_PARTS: usize = 150;
const MOCK_GAP_MS: u64 = 20;

// ───────────────────────────── mock 模型 ─────────────────────────────

/// 按**最后一条用户消息**选择应答（历史里的旧提示词不能串扰）：含「401」/「429」/「500」→ 对应错误；含「慢」→ 长流；其余 20 段
fn start_mock_server() -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let rt = Box::leak(Box::new(tokio::runtime::Runtime::new().unwrap()));
    let listener = rt.block_on(tokio::net::TcpListener::bind("127.0.0.1:0")).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    rt.spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { return };
            tokio::spawn(async move {
                // 读到请求头结束并凑够 content-length
                let mut buf = Vec::new();
                let mut chunk = vec![0u8; 65536];
                let body_start = loop {
                    let Ok(n) = sock.read(&mut chunk).await else { return };
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        break pos + 4;
                    }
                };
                if !buf.starts_with(b"POST ") {
                    return; // 本机有端口探测，只应答 POST
                }
                let head = String::from_utf8_lossy(&buf[..body_start]).to_ascii_lowercase();
                let length = head.lines().find_map(|l| l.strip_prefix("content-length:")).and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0);
                while buf.len() < body_start + length {
                    let Ok(n) = sock.read(&mut chunk).await else { return };
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
                let request: serde_json::Value = serde_json::from_slice(&buf[body_start..]).unwrap_or_default();
                let prompt = request["messages"]
                    .as_array()
                    .and_then(|m| m.iter().rev().find(|m| m["role"] == "user"))
                    .map(|m| m["content"].to_string())
                    .unwrap_or_default();
                for (needle, status) in [("401", "401 Unauthorized"), ("429", "429 Too Many Requests"), ("500", "500 Internal Server Error")] {
                    if prompt.contains(needle) {
                        let body = r#"{"error":{"message":"mock error"}}"#;
                        let response = format!("HTTP/1.1 {status}\r\ncontent-type: application/json\r\nconnection: close\r\ncontent-length: {}\r\n\r\n{body}", body.len());
                        let _ = sock.write_all(response.as_bytes()).await;
                        return;
                    }
                }
                let parts = if prompt.contains('慢') { SLOW_PARTS } else { MOCK_PARTS };
                if sock.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n").await.is_err() {
                    return;
                }
                for i in 0..parts {
                    tokio::time::sleep(Duration::from_millis(MOCK_GAP_MS)).await;
                    let data = serde_json::json!({"choices":[{"index":0,"delta":{"content":format!("{i},")}}]});
                    if sock.write_all(format!("data: {data}\n\n").as_bytes()).await.is_err() {
                        return; // 客户端取消 / 断开
                    }
                }
                let _ = sock.write_all(b"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n").await;
            });
        }
    });
    url
}

fn expected_reply() -> String {
    (0..MOCK_PARTS).map(|i| format!("{i},")).collect()
}

fn mock_config(url: &str) -> AppConfig {
    serde_json::from_value(serde_json::json!({
        "theme": "light",
        "providers": [{"id":"p1","name":"Mock","base_url":url,"api_key":"k","enabled_model_ids":["p1::mock"],"provider_type":"openai_compatible"}],
        "models": [{"id":"p1::mock","provider_id":"p1","api_model_id":"mock","display_name":"Mock","context_window":128000,"latency_ms":null}],
        "selected_model_id": "p1::mock"
    }))
    .expect("mock 配置")
}

/// 沙盒数据目录（每次重建）
fn sandbox(name: &str, config: Option<&AppConfig>) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/buddy-app-preview").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("创建沙盒目录");
    if let Some(config) = config {
        storage::save_config(&dir, config).expect("写配置");
    }
    dir
}

// ───────────────────────────── 窗口与输入 ─────────────────────────────

fn options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
    WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), titlebar: None, window_background: WindowBackgroundAppearance::Transparent, ..Default::default() }
}

/// 页面切换记录（自检用）
type Transitions = Rc<RefCell<Vec<(Page, Page)>>>;

/// 经真实路径启动：tokio 上 `preload`，再开窗
async fn open_router(engine: Arc<ChatEngine>, cx: &mut AsyncApp) -> (WindowHandle<PageRouter>, Transitions) {
    let loaded: Loaded = cx.update(|cx| spawn_engine(cx, preload(engine.clone()))).await;
    let log = Transitions::default();
    let sink = log.clone();
    let handle = cx.update(|cx| {
        cx.open_window(options(cx), |window, cx| {
            let router = cx.new(|cx| PageRouter::new(engine, loaded, window, cx));
            cx.subscribe(&router, move |_, event: &RouterEvent, _| {
                let RouterEvent::PageChanged { from, to } = *event;
                sink.borrow_mut().push((from, to));
            })
            .detach();
            router
        })
        .expect("open_window 失败")
    });
    (handle, log)
}

// ───────────────────────────── 手动模式的窗口壳（Phase 07 的替身）─────────────────────────────

/// v1 `geometry.rs` 的页面尺寸：紧凑 560×60、对话 750×500、设置 760×640
fn page_size(page: Page) -> (f32, f32) {
    match page {
        Page::Empty | Page::NoApiKey => (560.0, 60.0),
        Page::Conversation | Page::Streaming => (750.0, 500.0),
        Page::Settings => (760.0, 640.0),
    }
}

/// 手动预览的根视图：只做一件事 —— 按 v1 规则在「离开紧凑页」时改窗口尺寸
/// （`expands_window`；设置页返回到紧凑页之前的页面时按 v1 展开为对话尺寸）。
/// 真正的窗口壳（底边锚定、多显示器、动画）归 S07-*；GPUI 在 macOS 上 `resize` 保持左上角，所以窗口向下 / 向右长
struct DemoShell {
    router: Entity<PageRouter>,
    _subscription: Subscription,
}

impl DemoShell {
    fn new(router: Entity<PageRouter>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut settings_from_compact = false;
        let subscription = cx.subscribe_in(&router, window, move |_, _, event: &RouterEvent, window, _| {
            let RouterEvent::PageChanged { from, to } = *event;
            println!("页面切换：{from:?} → {to:?}");
            let target = if expands_window(from, to) {
                Some(to)
            } else if from == Page::Settings && to == Page::Conversation && settings_from_compact {
                Some(Page::Conversation) // v1 设置页 onBack：上一页是紧凑页时先把窗口调到对话尺寸
            } else {
                None
            };
            if to == Page::Settings {
                settings_from_compact = from.is_compact();
            }
            if let Some(page) = target {
                let (w, h) = page_size(page);
                println!("窗口尺寸：→ {w}×{h}");
                window.resize(size(px(w), px(h)));
            }
        });
        Self { router, _subscription: subscription }
    }
}

impl Render for DemoShell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.router.clone())
    }
}

/// 手动预览：紧凑窗口（560×60）启动，窗口壳按上面的规则改尺寸
async fn open_manual(engine: Arc<ChatEngine>, cx: &mut AsyncApp) {
    let loaded: Loaded = cx.update(|cx| spawn_engine(cx, preload(engine.clone()))).await;
    cx.update(|cx| {
        let (w, h) = page_size(Page::Empty);
        // 放在屏幕偏上处：窗口只会向下 / 向右长
        let bounds = Bounds::centered(None, size(px(w), px(h)), cx);
        let bounds = Bounds::new(point(bounds.origin.x - px(95.0), bounds.origin.y - px(200.0)), bounds.size);
        let mut opts = options(cx);
        opts.window_bounds = Some(WindowBounds::Windowed(bounds));
        cx.open_window(opts, |window, cx| {
            let router = cx.new(|cx| PageRouter::new(engine, loaded, window, cx));
            cx.new(|cx| DemoShell::new(router, window, cx))
        })
        .expect("open_window 失败");
    });
}

async fn draw(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) {
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

async fn press(handle: WindowHandle<PageRouter>, keys: &str, cx: &mut AsyncApp) {
    let key = PlatformInput::KeyDown(KeyDownEvent { keystroke: Keystroke::parse(keys).expect("按键"), is_held: false, prefer_character_input: false });
    let _ = cx.update_window(handle.into(), |_, window, cx| window.dispatch_event(key, cx));
    draw(handle, cx).await;
}

async fn click(handle: WindowHandle<PageRouter>, x: f32, y: f32, cx: &mut AsyncApp) {
    let p = point(px(x), px(y));
    for e in [
        PlatformInput::MouseMove(MouseMoveEvent { position: p, pressed_button: None, modifiers: Modifiers::default() }),
        PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: p, modifiers: Modifiers::default(), click_count: 1, first_mouse: false }),
        PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: p, modifiers: Modifiers::default(), click_count: 1 }),
    ] {
        let _ = cx.update_window(handle.into(), |_, window, cx| window.dispatch_event(e, cx));
        draw(handle, cx).await;
    }
}

fn page(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> Page {
    handle.read_with(cx, |r, _| r.page()).unwrap()
}

fn window_size(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> (Pixels, Pixels) {
    let bounds = cx.update_window(handle.into(), |_, window, _| window.bounds()).unwrap();
    (bounds.size.width, bounds.size.height)
}

async fn wait_until(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp, mut pred: impl FnMut(&mut AsyncApp) -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(15) {
        draw(handle, cx).await;
        if pred(cx) {
            return true;
        }
        cx.background_executor().timer(Duration::from_millis(20)).await;
    }
    false
}

/// 在输入区键入并按 Enter（真实按键路径：Enter → 输入框提交 → 输入区发送事件 → 路由器）
async fn type_and_send(handle: WindowHandle<PageRouter>, text: &str, cx: &mut AsyncApp) {
    let composer = handle.read_with(cx, |r, _| r.composer().clone()).unwrap();
    let focus = composer.read_with(cx, |c, cx| c.focus_handle(cx));
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.focus(&focus, cx);
        composer.update(cx, |c, cx| c.set_draft(text, cx));
    });
    draw(handle, cx).await;
    press(handle, "enter", cx).await;
}

fn streaming(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> bool {
    let conversation = handle.read_with(cx, |r, _| r.conversation().clone()).unwrap();
    conversation.read_with(cx, |c, _| c.state.is_streaming())
}

/// 等一轮对话完整结束（流式结束且节奏器放完）
async fn wait_turn_done(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> bool {
    let started = wait_until(handle, cx, |cx| streaming(handle, cx)).await;
    started && wait_until(handle, cx, |cx| !streaming(handle, cx)).await
}

fn messages(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> Vec<Message> {
    let conversation = handle.read_with(cx, |r, _| r.conversation().clone()).unwrap();
    conversation.read_with(cx, |c, _| c.state.messages.clone())
}

/// 盘上的消息总数与全部内容（经 engine，走 tokio）
async fn stored(engine: &Arc<ChatEngine>, cx: &mut AsyncApp) -> Vec<Message> {
    let engine = engine.clone();
    cx.update(|cx| spawn_engine(cx, async move { engine.load_messages(0, 1000).await.unwrap_or_default() })).await
}

// ───────────────────────────── 自检 ─────────────────────────────

/// T25：页面状态机端到端
async fn selftest_flow(url: &str, cx: &mut AsyncApp) -> bool {
    let engine = ChatEngine::new(sandbox("flow", Some(&mock_config(url))));
    let (handle, transitions) = open_router(engine.clone(), cx).await;
    draw(handle, cx).await;
    let size0 = window_size(handle, cx);
    assert_eq!(size0, (px(WIDTH), px(HEIGHT)), "窗口应为请求的尺寸（否则尺寸比较没有意义）");
    let mut checks: Vec<(&str, bool)> = Vec::new();
    let mut check = |name: &'static str, ok: bool| checks.push((name, ok));

    check("启动为空态", page(handle, cx) == Page::Empty);

    // 空态输入区的设置小齿轮（独立气泡：无外边距；从右往下量：发送 28、间隔 4、模型 24、间隔 4、齿轮 24）
    click(handle, WIDTH - 5.0 - 28.0 - 4.0 - 24.0 - 4.0 - 12.0, HEIGHT - 5.0 - 16.0, cx).await;
    check("空态点小齿轮 → 设置", page(handle, cx) == Page::Settings);
    click(handle, WIDTH / 2.0, HEIGHT / 2.0 + 44.0, cx).await;
    check("设置返回 → 对话页（上一页是紧凑页）", page(handle, cx) == Page::Conversation);
    let _ = handle.update(cx, |r, _, cx| r.set_config(mock_config(url), cx));
    let mut bad = mock_config(url);
    bad.selected_model_id.clear();
    let _ = handle.update(cx, |r, _, cx| r.set_config(bad, cx));
    draw(handle, cx).await;
    check("配置失效 → 退回空态", page(handle, cx) == Page::Empty);
    let _ = handle.update(cx, |r, _, cx| r.set_config(mock_config(url), cx));

    // 空态发送 → streaming → 完成 → conversation
    type_and_send(handle, "你好", cx).await;
    check("空态发送后进入流式页", page(handle, cx) == Page::Streaming);
    check("流式完成", wait_turn_done(handle, cx).await || !streaming(handle, cx));
    check("完成后进入对话页", page(handle, cx) == Page::Conversation);
    let m = messages(handle, cx);
    check("用户与助手消息", m.len() == 2 && m[0].content == "你好" && m[1].role == MessageRole::Assistant);
    check("助手正文为 mock 全文", m.get(1).map(|a| a.content.trim_end_matches('\n') == expected_reply()).unwrap_or(false));
    check("输入区已清空", handle.read_with(cx, |r, cx| r.composer().read(cx).draft(cx).is_empty()).unwrap());
    check("engine 已持久化两条", stored(&engine, cx).await.len() == 2);

    // 401 → 无 Key 页；补齐配置 → 对话
    type_and_send(handle, "触发401", cx).await;
    check("401 后进入无 Key 页", wait_until(handle, cx, |cx| page(handle, cx) == Page::NoApiKey).await);
    let config = handle.read_with(cx, |r, _| r.config().clone()).unwrap();
    let _ = handle.update(cx, |r, _, cx| r.set_config(config.clone(), cx));
    draw(handle, cx).await;
    check("无 Key 页补齐配置 → 对话页", page(handle, cx) == Page::Conversation);

    // 429 / 500：界面生成的提示消息落盘
    let before = stored(&engine, cx).await.len();
    type_and_send(handle, "触发429", cx).await;
    check("429 后回到对话页", wait_until(handle, cx, |cx| !streaming(handle, cx) && page(handle, cx) == Page::Conversation).await);
    type_and_send(handle, "触发500", cx).await;
    check("500 后回到对话页", wait_until(handle, cx, |cx| !streaming(handle, cx) && page(handle, cx) == Page::Conversation).await);
    cx.background_executor().timer(Duration::from_millis(300)).await;
    let saved = stored(&engine, cx).await;
    let notices: Vec<_> = saved.iter().filter(|m| m.id.starts_with("err-") || m.id.starts_with("warn-")).collect();
    check("配额提示消息已落盘", saved.iter().any(|m| m.content.contains("配额已用尽")));
    check("服务器错误提示消息已落盘", saved.iter().any(|m| m.content.contains("500")));
    check("提示消息为界面生成（不来自 engine 的两条）", notices.len() >= 2 && saved.len() > before);

    // 停止：回到对话页，不显示错误
    type_and_send(handle, "请慢慢说", cx).await;
    check("慢速流已开始", wait_until(handle, cx, |cx| streaming(handle, cx)).await);
    let composer = handle.read_with(cx, |r, _| r.composer().clone()).unwrap();
    let _ = cx.update_window(handle.into(), |_, _, cx| composer.update(cx, |_, cx| cx.emit(ComposerEvent::Stop)));
    let stopped_at = Instant::now();
    check("停止后流式结束", wait_until(handle, cx, |cx| !streaming(handle, cx)).await);
    // 慢速流自然结束要 3 秒以上：能在 1.5 秒内结束说明确实是被 stop_generation 打断的
    check("停止在 1.5 秒内生效", stopped_at.elapsed() < Duration::from_millis(1500));
    let no_error = handle.read_with(cx, |r, cx| r.conversation().read(cx).state.error.is_none()).unwrap();
    check("停止不显示错误、停在对话页", no_error && page(handle, cx) == Page::Conversation);

    // 设置叠加层往返：底层页不变。对话页输入区的小齿轮（外边距 8：距右 8+1+4，距底 8+1+4）
    click(handle, WIDTH - 13.0 - 28.0 - 4.0 - 24.0 - 4.0 - 12.0, HEIGHT - 13.0 - 16.0, cx).await;
    check("对话页点小齿轮 → 设置", page(handle, cx) == Page::Settings);
    check("设置叠加在对话页之上（底层页不卸载）", handle.read_with(cx, |r, _| r.base_page()).unwrap() == Page::Conversation);
    click(handle, WIDTH / 2.0, HEIGHT / 2.0 + 44.0, cx).await; // 占位设置页的「返回」
    check("返回 → 对话页", page(handle, cx) == Page::Conversation);

    // 页面切换事件（窗口壳据此改尺寸）：离开紧凑页的那次会展开，内容页之间 / 回到紧凑页不展开
    let seen = transitions.borrow().clone();
    check("发送时 empty → streaming 且需展开窗口", seen.contains(&(Page::Empty, Page::Streaming)) && expands_window(Page::Empty, Page::Streaming));
    check("流式结束 streaming → conversation 不展开", seen.contains(&(Page::Streaming, Page::Conversation)) && !expands_window(Page::Streaming, Page::Conversation));
    check("401 时 conversation → noapikey 不改窗口（v1 只在离开紧凑页时改尺寸）", seen.contains(&(Page::Conversation, Page::NoApiKey)) && !expands_window(Page::Conversation, Page::NoApiKey));
    check("每次事件都是真实切换", seen.iter().all(|(a, b)| a != b));

    // 窗口尺寸
    let size1 = window_size(handle, cx);
    check("全程窗口尺寸不变", size0 == size1);

    // 空态 → 展开：配置失效退回空态后，点顶部「展开」
    let mut bad = config.clone();
    bad.selected_model_id.clear();
    let _ = handle.update(cx, |r, _, cx| r.set_config(bad, cx));
    draw(handle, cx).await;
    check("配置失效 → 退回空态", page(handle, cx) == Page::Empty);
    let _ = handle.update(cx, |r, _, cx| r.set_config(config, cx));
    click(handle, WIDTH / 2.0, 12.0, cx).await; // 空态页顶部居中的「展开」
    check("点「展开」→ 对话页", page(handle, cx) == Page::Conversation);
    check("展开后窗口尺寸仍不变", window_size(handle, cx) == size0);

    let ok = checks.iter().all(|(_, ok)| *ok);
    for (name, ok) in &checks {
        println!("  {} {name}", if *ok { "ok  " } else { "FAIL" });
    }
    println!("{} S05-18 T25 页面状态机端到端（发送 / 401 / 429 / 500 / 停止 / 设置 / 展开 / 窗口尺寸）", if ok { "PASS" } else { "FAIL" });
    ok
}

/// T26：缺配置的流程
async fn selftest_no_key(url: &str, cx: &mut AsyncApp) -> bool {
    let engine = ChatEngine::new(sandbox("nokey", None));
    let (handle, _transitions) = open_router(engine.clone(), cx).await;
    draw(handle, cx).await;
    let mut checks: Vec<(&str, bool)> = Vec::new();
    let mut check = |name: &'static str, ok: bool| checks.push((name, ok));

    check("启动为空态", page(handle, cx) == Page::Empty);
    type_and_send(handle, "你好", cx).await;
    check("缺配置发送 → 无 Key 页", page(handle, cx) == Page::NoApiKey);
    check("未发送任何消息", messages(handle, cx).is_empty() && stored(&engine, cx).await.is_empty());
    check("草稿保留", handle.read_with(cx, |r, cx| r.composer().read(cx).draft(cx)).unwrap() == "你好");
    click(handle, WIDTH / 2.0, HEIGHT / 2.0, cx).await; // 无 Key 页整块面板
    check("点面板 → 设置", page(handle, cx) == Page::Settings);
    click(handle, WIDTH / 2.0, HEIGHT / 2.0 + 44.0, cx).await;
    check("设置返回 → 对话页（上一页是紧凑页）", page(handle, cx) == Page::Conversation);

    // 只在内存里补齐配置、盘上仍没有：engine 拒绝发送（占用生成通道前就失败）→ 移除空占位并报错，用户消息保留
    let _ = handle.update(cx, |r, _, cx| r.set_config(mock_config(url), cx));
    draw(handle, cx).await;
    type_and_send(handle, "再来", cx).await;
    let rejected = wait_until(handle, cx, |cx| handle.read_with(cx, |r, cx| r.conversation().read(cx).state.error.is_some()).unwrap()).await;
    let m = messages(handle, cx);
    check("engine 拒绝发送 → 报错、无空占位、用户消息保留", rejected && m.len() == 1 && m[0].content == "再来" && !streaming(handle, cx));
    check("拒绝后停在对话页", page(handle, cx) == Page::Conversation);

    // 配置写盘并更新后可以发送（设置页保存 = engine.save_config + set_config）
    let saved = {
        let (engine, config) = (engine.clone(), mock_config(url));
        cx.update(|cx| spawn_engine(cx, async move { engine.save_config(config).await })).await
    };
    let _ = handle.update(cx, |r, _, cx| r.set_config(mock_config(url), cx));
    type_and_send(handle, "再来一次", cx).await;
    check("配置写盘后可发送", saved.is_ok() && wait_turn_done(handle, cx).await && messages(handle, cx).len() == 3);

    // 配置失效 → 内容页退回空态
    let mut bad = mock_config(url);
    bad.providers.clear();
    let _ = handle.update(cx, |r, _, cx| r.set_config(bad, cx));
    draw(handle, cx).await;
    check("Provider 清空 → 退回空态", page(handle, cx) == Page::Empty);

    let ok = checks.iter().all(|(_, ok)| *ok);
    for (name, ok) in &checks {
        println!("  {} {name}", if *ok { "ok  " } else { "FAIL" });
    }
    println!("{} S05-18 T26 缺配置流程（无 Key 页 / 草稿保留 / 设置往返 / 配置失效）", if ok { "PASS" } else { "FAIL" });
    ok
}

/// T27：重启读回历史并经 engine 分页
async fn selftest_history(url: &str, cx: &mut AsyncApp) -> bool {
    let engine = ChatEngine::new(sandbox("history", Some(&mock_config(url))));
    // 预置 25 条（交替问答）
    for i in 0..25u32 {
        let mut message = buddy_ui::chat::state::user_message(&format!("第 {i} 条"));
        message.id = format!("seed-{i:02}");
        if i % 2 == 1 {
            message.role = MessageRole::Assistant;
        }
        let engine = engine.clone();
        cx.update(|cx| spawn_engine(cx, async move { engine.save_message(message).await })).await.expect("预置消息");
    }
    let (handle, _transitions) = open_router(engine.clone(), cx).await;
    draw(handle, cx).await;
    let conversation = handle.read_with(cx, |r, _| r.conversation().clone()).unwrap();
    let mut checks: Vec<(&str, bool)> = Vec::new();
    let mut check = |name: &'static str, ok: bool| checks.push((name, ok));

    check("重启后仍为空态", page(handle, cx) == Page::Empty);
    let first = messages(handle, cx);
    check("载入最新 10 条（seed-15 … seed-24）", first.len() == 10 && first[0].id == "seed-15" && first[9].id == "seed-24");
    let has_more = conversation.read_with(cx, |c, _| c.state.history.has_more);
    check("还有更早历史", has_more);
    let _ = handle.update(cx, |r, _, cx| r.conversation().clone().update(cx, |c, cx| c.load_older(cx)));
    check("经 engine 读到更早一页", wait_until(handle, cx, |cx| messages(handle, cx).len() == 20).await);
    let second = messages(handle, cx);
    check("更早一页并入开头且顺序正确", second[0].id == "seed-05" && second[10].id == "seed-15");
    let _ = handle.update(cx, |r, _, cx| r.conversation().clone().update(cx, |c, cx| c.load_older(cx)));
    check("读到最早的 5 条", wait_until(handle, cx, |cx| messages(handle, cx).len() == 25).await);
    check("已无更早历史", !conversation.read_with(cx, |c, _| c.state.history.has_more));

    let ok = checks.iter().all(|(_, ok)| *ok);
    for (name, ok) in &checks {
        println!("  {} {name}", if *ok { "ok  " } else { "FAIL" });
    }
    println!("{} S05-18 T27 历史读回与分页（重启 / 最新一页 / 触顶加载）", if ok { "PASS" } else { "FAIL" });
    ok
}

// ───────────────────────────── 入口 ─────────────────────────────

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let (self_test, mock, no_key) = (flag("--selftest"), flag("--mock"), flag("--no-key"));
    application().with_assets(buddy_ui::icons::Assets).run(move |cx: &mut App| {
        buddy_ui::init_theme(cx);
        buddy_ui::chat_bridge::init(cx);
        Theme::install(Appearance::Light, cx);
        fonts::install_text_rendering(cx);
        markdown::init(cx);
        buddy_ui::chat::init(cx);
        // 无标题栏，没有关闭按钮：Cmd+Q / Esc 退出
        cx.observe_keystrokes(|event, _, cx| {
            if event.keystroke.unparse() == "cmd-q" || event.keystroke.key == "escape" {
                cx.quit();
            }
        })
        .detach();

        if self_test {
            let url = start_mock_server();
            cx.spawn(async move |cx: &mut AsyncApp| {
                let t25 = selftest_flow(&url, cx).await;
                let t26 = selftest_no_key(&url, cx).await;
                let t27 = selftest_history(&url, cx).await;
                std::process::exit(if t25 && t26 && t27 { 0 } else { 1 });
            })
            .detach();
            return;
        }
        let dir = if mock {
            sandbox("manual-mock", Some(&mock_config(&start_mock_server())))
        } else if no_key {
            sandbox("manual-nokey", None)
        } else {
            let dir = sandbox("manual-real", None);
            let v1 = storage::default_data_dir().expect("默认数据目录");
            std::fs::copy(v1.join("config.json"), dir.join("config.json")).expect("复制 v1 config.json 到沙盒");
            dir
        };
        let engine = ChatEngine::new(dir);
        cx.spawn(async move |cx: &mut AsyncApp| {
            open_manual(engine, cx).await;
            cx.update(|cx| cx.activate(true));
        })
        .detach();
    });
}
