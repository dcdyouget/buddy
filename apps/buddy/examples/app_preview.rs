//! S05-18 页面状态机与 engine 接入的预览与自检。
//!
//! ```text
//! cargo run -p buddy-app --example app_preview -- --mock       # 目检：本地 mock 模型（提示词含「401」「429」「500」「慢」触发对应情形；有 Mock / Mock 2 两个启用模型可切换）
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
//! - T27 历史：重启后读回最新一页（起始页总是空态），触顶后经 engine 读取更早一页；
//! - T30 对话页窗口拖动条：顶 / 左 / 右 / 底的不可见条按下即开始拖动窗口，消息区中部与输入区按钮不拖；
//! - T29 提问卡与审批浮层（S05-13）：mock 模型调用 ask_user / create_file，走真实 engine 的工具循环：选择 + 补充 / 跳过 / 自定义回答，Esc 拒绝、点「允许」、点「本次都允许」；
//! - T28 模型选择器（S05-15）：真实点击打开菜单窗口、只列启用的模型、按钮上方定位、点选后写盘并关闭、Esc、流式中无按钮、空列表。

use buddy_engine::chat::ChatEngine;
use buddy_engine::models::{AppConfig, Message, MessageRole};
use buddy_engine::storage;
use buddy_ui::chat::{attachments, composer::ComposerEvent};
use buddy_ui::chat::model_menu::{MENU_WIDTH, ModelMenu, ROW_HEIGHT, menu_size};
use buddy_ui::chat::page_state::{Page, expands_window};
use buddy_ui::chat::router::{Loaded, PageRouter, RouterEvent, preload};
use buddy_ui::chat_bridge::spawn_engine;
use buddy_ui::gpui::{
    App, AppContext, AsyncApp, Bounds, ClipboardItem, Context, Entity, ExternalPaths, FileDropEvent, Focusable, Image, ImageFormat, ImgResourceLoader, IntoElement, Render, Resource, Subscription, Window, div, prelude::*, KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, PlatformInput,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowOptions, point, px, size,
};
use buddy_ui::gpui_platform::application;
use buddy_ui::markdown;
use buddy_ui::theme_system::{Appearance, Theme, fonts};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
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
                let last = request["messages"].as_array().and_then(|m| m.last()).cloned().unwrap_or_default();
                let prompt = request["messages"]
                    .as_array()
                    .and_then(|m| m.iter().rev().find(|m| m["role"] == "user"))
                    .map(|m| m["content"].to_string())
                    .unwrap_or_default();
                // 工具调用：最后一条是用户消息且含触发词时，让「模型」调用 ask_user / create_file；工具结果回来后再给一句正文
                let mut tool_calls: Vec<(String, &str, String)> = Vec::new();
                if last["role"] == "user" {
                    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
                    let n = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let text = last["content"].as_str().unwrap_or_default();
                    if text.contains("请提问") {
                        let args = serde_json::json!({
                            "header": "方案", "question": "先说明背景\n\n你选哪个方案？", "multi_select": text.contains("多选"),
                            "options": [
                                {"label": "方案 A", "description": "更快"},
                                {"label": "方案 B", "requires_input": true, "input_placeholder": "写下理由"}
                            ]
                        });
                        tool_calls.push((format!("call_ask_{n}"), "ask_user", args.to_string()));
                    } else if let Some(rest) = text.split("请写文件").nth(1) {
                        // engine 会在用户文本后附加上下文，只取紧跟触发词的绝对路径
                        for (i, path) in rest.split_whitespace().take_while(|t| t.starts_with('/')).enumerate() {
                            tool_calls.push((format!("call_write_{n}_{i}"), "create_file", serde_json::json!({"path": path, "content": "hello"}).to_string()));
                        }
                    }
                }
                if !tool_calls.is_empty() {
                    if sock.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n").await.is_err() {
                        return;
                    }
                    let calls: Vec<_> = tool_calls
                        .iter()
                        .enumerate()
                        .map(|(i, (id, name, args))| serde_json::json!({"index": i, "id": id, "type": "function", "function": {"name": name, "arguments": args}}))
                        .collect();
                    let chunk = serde_json::json!({"choices":[{"index":0,"delta":{"tool_calls": calls}}]});
                    let _ = sock.write_all(format!("data: {chunk}\n\n").as_bytes()).await;
                    let _ = sock.write_all(b"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n").await;
                    return;
                }
                if last["role"] == "tool" {
                    let head = b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n";
                    if sock.write_all(head).await.is_err() {
                        return;
                    }
                    let data = serde_json::json!({"choices":[{"index":0,"delta":{"content":"已收到工具结果。"}}]});
                    let _ = sock.write_all(format!("data: {data}\n\n").as_bytes()).await;
                    let _ = sock.write_all(b"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n").await;
                    return;
                }
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
        "models": [{"id":"p1::mock","provider_id":"p1","api_model_id":"mock","display_name":"Mock","context_window":128000,"latency_ms":null,"supports_vision":true}],
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

/// 手动预览的根视图：只做一件事 —— 按 v1 规则在「离开紧凑页」时展开窗口
/// （`expands_window`；设置页返回到紧凑页之前的页面时按 v1 展开为对话尺寸），并在「进入紧凑页」时缩回（偏离 v1，见下）。
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
            } else if to.is_compact() && !from.is_compact() {
                // **偏离 v1**：v1 只在离开紧凑页时改尺寸，401 后从对话页切到无 Key 页窗口不会缩回（面板悬在大窗口中间）。
                // 目检 #17 反馈「框体应一起缩小」，这里进入紧凑页时缩回 560×60（待用户确认，见 S05-18 决策记录）
                Some(to)
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


/// 点模型菜单窗口里的位置（窗口坐标）
async fn click_menu(menu: WindowHandle<ModelMenu>, x: f32, y: f32, cx: &mut AsyncApp) {
    let p = point(px(x), px(y));
    for e in [
        PlatformInput::MouseMove(MouseMoveEvent { position: p, pressed_button: None, modifiers: Modifiers::default() }),
        PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: p, modifiers: Modifiers::default(), click_count: 1, first_mouse: false }),
        PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: p, modifiers: Modifiers::default(), click_count: 1 }),
    ] {
        let _ = cx.update_window(menu.into(), |_, window, cx| window.dispatch_event(e, cx));
        let _ = cx.update_window(menu.into(), |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
    }
}

fn menu_of(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> Option<WindowHandle<ModelMenu>> {
    let menu = handle.read_with(cx, |r, _| r.model_menu()).ok().flatten()?;
    // 已关闭的窗口读取会失败
    menu.read_with(cx, |_, _| ()).ok().map(|_| menu)
}

/// 点输入区的模型按钮（真实点击）
async fn click_model_button(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> bool {
    let bounds = handle.read_with(cx, |r, cx| r.composer().read(cx).model_button_bounds()).unwrap();
    let Some(b) = bounds else { return false };
    click(handle, f32::from(b.center().x), f32::from(b.center().y), cx).await;
    true
}

fn models_config(url: &str, enabled: &[&str]) -> AppConfig {
    serde_json::from_value(serde_json::json!({
        "theme": "light",
        "providers": [{"id":"p1","name":"Mock","base_url":url,"api_key":"k","enabled_model_ids":enabled,"provider_type":"openai_compatible"}],
        "models": [
            {"id":"p1::mock","provider_id":"p1","api_model_id":"mock","display_name":"Mock","context_window":128000,"latency_ms":null,"supports_vision":true},
            {"id":"p1::mock2","provider_id":"p1","api_model_id":"mock2","display_name":"Mock 2","context_window":32000,"latency_ms":120,"supports_vision":false},
            {"id":"p1::hidden","provider_id":"p1","api_model_id":"hidden","display_name":"Hidden","context_window":8000,"latency_ms":null}
        ],
        "selected_model_id": "p1::mock"
    }))
    .expect("models 配置")
}

/// T28：模型选择器
async fn selftest_models(url: &str, cx: &mut AsyncApp) -> bool {
    let engine = ChatEngine::new(sandbox("models", Some(&models_config(url, &["p1::mock", "p1::mock2"]))));
    let (handle, _) = open_router(engine.clone(), cx).await;
    for _ in 0..3 {
        draw(handle, cx).await;
    }
    let mut checks: Vec<(&str, bool)> = Vec::new();
    let mut check = |name: &'static str, ok: bool| checks.push((name, ok));
    let selected_on_disk = |engine: &Arc<ChatEngine>, cx: &mut AsyncApp| {
        let engine = engine.clone();
        let task = cx.update(|cx| spawn_engine(cx, async move { engine.get_config().await.map(|c| c.selected_model_id).unwrap_or_default() }));
        task
    };

    // 打开：只列出启用的两个，当前模型高亮
    check("点模型按钮 → 打开菜单", click_model_button(handle, cx).await && menu_of(handle, cx).is_some());
    let Some(menu) = menu_of(handle, cx) else {
        println!("FAIL S05-15 T28 模型选择器（菜单未打开）");
        return false;
    };
    let rows = menu.read_with(cx, |m, _| m.rows().to_vec()).unwrap();
    check("只列出启用的模型（未启用的不出现）", rows.len() == 2 && rows[0].id == "p1::mock" && rows[1].id == "p1::mock2");
    check("副标题按 v1 模板", rows[1].detail == "Mock · 32K 上下文 · 120ms");
    check("当前模型高亮", menu.read_with(cx, |m, _| m.highlighted().to_string()).unwrap() == "p1::mock");
    let (w, h) = menu_size(2);
    let menu_bounds = cx.update_window(menu.into(), |_, window, _| window.bounds()).unwrap();
    check("菜单窗口尺寸 = 面板尺寸（无透明边距）", menu_bounds.size.width == px(w) && menu_bounds.size.height == px(h));
    // 位置：面板底边在模型按钮上方 8px（窗口坐标 → 屏幕坐标）
    let parent = cx.update_window(handle.into(), |_, window, _| window.bounds()).unwrap();
    let button = handle.read_with(cx, |r, cx| r.composer().read(cx).model_button_bounds()).unwrap().unwrap();
    let panel_bottom = menu_bounds.origin.y + menu_bounds.size.height;
    check("面板底边在模型按钮上方 8px", (f32::from(panel_bottom) - f32::from(parent.origin.y + button.top() - px(8.0))).abs() < 1.0);
    let panel_right = menu_bounds.origin.x + menu_bounds.size.width;
    check("面板右缘距父窗口右缘 8px", (f32::from(panel_right) - f32::from(parent.origin.x + parent.size.width - px(8.0))).abs() < 1.0);

    // 选择第二行：内存立即生效，写盘串行完成，菜单在延迟后关闭
    click_menu(menu, MENU_WIDTH / 2.0, 1.0 + ROW_HEIGHT + ROW_HEIGHT / 2.0, cx).await;
    check("选择后内存配置立即更新", handle.read_with(cx, |r, _| r.config().selected_model_id.clone()).unwrap() == "p1::mock2");
    check("选中反馈：高亮已移到新行", menu_of(handle, cx).map(|m| m.read_with(cx, |m, _| m.highlighted().to_string()).unwrap()) == Some("p1::mock2".into()) || menu_of(handle, cx).is_none());
    let save = handle.update(cx, |r, _, _| r.take_config_save()).unwrap();
    if let Some(task) = save {
        task.await;
    }
    check("配置已写盘", selected_on_disk(&engine, cx).await == "p1::mock2");
    check("约 120ms 后菜单关闭", wait_until(handle, cx, |cx| menu_of(handle, cx).is_none()).await);

    // 再打开：高亮为新的默认模型；Esc 关闭
    check("再次打开", click_model_button(handle, cx).await && menu_of(handle, cx).is_some());
    if let Some(menu) = menu_of(handle, cx) {
        check("高亮为新的默认模型", menu.read_with(cx, |m, _| m.highlighted().to_string()).unwrap() == "p1::mock2");
        let esc = PlatformInput::KeyDown(KeyDownEvent { keystroke: Keystroke::parse("escape").unwrap(), is_held: false, prefer_character_input: false });
        let _ = cx.update_window(menu.into(), |_, window, cx| window.dispatch_event(esc, cx));
    }
    check("Esc 关闭菜单", wait_until(handle, cx, |cx| menu_of(handle, cx).is_none()).await);

    // 连续快速选择：串行写盘，最后一次生效
    let _ = handle.update(cx, |r, _, cx| {
        for i in 0..41 {
            r.select_model(if i % 2 == 0 { "p1::mock2" } else { "p1::mock" }.into(), cx);
        }
    });
    if let Some(task) = handle.update(cx, |r, _, _| r.take_config_save()).unwrap() {
        task.await;
    }
    check("快速连选 41 次：盘上是最后一次", selected_on_disk(&engine, cx).await == "p1::mock2");

    // 流式中没有模型按钮
    type_and_send(handle, "请慢慢说", cx).await;
    check("慢速流已开始", wait_until(handle, cx, |cx| streaming(handle, cx)).await);
    draw(handle, cx).await;
    check("流式中模型按钮不存在（不可选）", handle.read_with(cx, |r, cx| r.composer().read(cx).model_button_bounds()).unwrap().is_none());
    let composer = handle.read_with(cx, |r, _| r.composer().clone()).unwrap();
    let _ = cx.update_window(handle.into(), |_, _, cx| composer.update(cx, |_, cx| cx.emit(ComposerEvent::Stop)));
    wait_until(handle, cx, |cx| !streaming(handle, cx)).await;
    draw(handle, cx).await;

    // 没有启用的模型：提示文案
    let _ = handle.update(cx, |r, _, cx| r.set_config(models_config(url, &[]), cx));
    draw(handle, cx).await;
    let _ = click_model_button(handle, cx).await;
    let empty = menu_of(handle, cx);
    check("无启用模型 → 菜单为空提示", empty.map(|m| m.read_with(cx, |m, _| m.rows().is_empty()).unwrap()) == Some(true));
    if let Some(menu) = menu_of(handle, cx) {
        let esc = PlatformInput::KeyDown(KeyDownEvent { keystroke: Keystroke::parse("escape").unwrap(), is_held: false, prefer_character_input: false });
        let _ = cx.update_window(menu.into(), |_, window, cx| window.dispatch_event(esc, cx));
    }

    let ok = checks.iter().all(|(_, ok)| *ok);
    for (name, ok) in &checks {
        println!("  {} {name}", if *ok { "ok  " } else { "FAIL" });
    }
    println!("{} S05-15 T28 模型选择器（列表 / 定位 / 选择并写盘 / Esc / 串行保存 / 流式中禁用 / 空列表）", if ok { "PASS" } else { "FAIL" });
    ok
}


/// T29：提问卡与审批浮层（S05-13）—— 真实 engine 的工具循环：mock 模型调用 ask_user / create_file
async fn selftest_interactions(url: &str, cx: &mut AsyncApp) -> bool {
    let dir = sandbox("tools", Some(&mock_config(url)));
    let engine = ChatEngine::new(dir.clone());
    let (handle, _) = open_router(engine, cx).await;
    for _ in 0..3 {
        draw(handle, cx).await;
    }
    let conversation = handle.read_with(cx, |r, _| r.conversation().clone()).unwrap();
    let transcript = handle.read_with(cx, |r, cx| r.transcript(cx)).unwrap();
    let mut checks: Vec<(String, bool)> = Vec::new();
    let mut check = |name: &str, ok: bool| checks.push((name.to_string(), ok));
    let question_id = |cx: &mut AsyncApp| conversation.read_with(cx, |c, _| c.state.question.as_ref().map(|q| q.id.clone()));
    let approval_id = |cx: &mut AsyncApp| conversation.read_with(cx, |c, _| c.state.approval.as_ref().map(|a| a.id.clone()));
    let tool_result = |cx: &mut AsyncApp, id: &str| conversation.read_with(cx, |c, _| c.state.tools.get(id).and_then(|t| t.result.clone()));
    let card_of = |cx: &mut AsyncApp, id: &str| transcript.read_with(cx, |t, _| t.ask_card_for_test(id));

    // ── ask_user：单选 + 要求补充的选项 ──
    type_and_send(handle, "请提问", cx).await;
    check("模型调用 ask_user → 出现待答问题", wait_until(handle, cx, |cx| question_id(cx).is_some()).await);
    let Some(id) = question_id(cx) else {
        println!("FAIL S05-13 T29 提问卡与审批（问题未出现）");
        return false;
    };
    check("提问卡已建立", wait_until(handle, cx, |cx| card_of(cx, &id).is_some()).await);
    let Some(card) = card_of(cx, &id) else {
        println!("FAIL S05-13 T29 提问卡与审批（提问卡未建立）");
        return false;
    };
    let display = conversation.read_with(cx, |c, _| c.state.question.clone()).unwrap();
    check("问题与选项来自 engine 事件", display.question.contains("你选哪个方案") && display.options.len() == 2 && display.header == "方案" && !display.multi_select);
    check("初始未选、无补充框", card.read_with(cx, |c, _| c.selected().is_empty() && c.option_input(1).is_none()));
    let _ = cx.update_window(handle.into(), |_, _, cx| card.update(cx, |c, cx| c.toggle_option(1, cx)));
    draw(handle, cx).await;
    check("选「方案 B」→ 已选且出现补充框", card.read_with(cx, |c, _| c.selected() == vec![1] && c.option_input(1).is_some()));
    let _ = cx.update_window(handle.into(), |_, _, cx| card.update(cx, |c, cx| c.submit(cx)));
    cx.background_executor().timer(Duration::from_millis(150)).await;
    check("要求补充却没填 → 确认无效（问题仍在等待）", question_id(cx).is_some() && !card.read_with(cx, |c, _| c.submitted()));
    let input = card.read_with(cx, |c, _| c.option_input(1).cloned()).unwrap();
    let _ = cx.update_window(handle.into(), |_, _, cx| input.update(cx, |a, cx| a.set_text("更快", cx)));
    let _ = cx.update_window(handle.into(), |_, _, cx| card.update(cx, |c, cx| c.submit(cx)));
    check("补充后确认 → 已提交、问题清除", card.read_with(cx, |c, _| c.submitted()) && wait_until(handle, cx, |cx| question_id(cx).is_none()).await);
    check("回合结束", wait_until(handle, cx, |cx| !streaming(handle, cx)).await);
    let result = tool_result(cx, &id).unwrap_or_default();
    check("engine 收到选项与补充输入（按 id 配对）", result.contains("User selected: 方案 B") && result.contains("User input: 更快"));
    // 回答之后这一行与其他工具（创建文件等）统一：默认折叠；手动展开也是「调用参数 / 执行结果」，不再渲染提问卡
    check("回答后的工具行默认折叠（与已完成的其他工具一致）", !buddy_ui::chat::tool_card::default_expanded(&conversation.read_with(cx, |c, _| c.state.tools.get(&id).cloned()).unwrap(), false, false));
    let row_id = transcript.read_with(cx, |t, _| t.rows().iter().find(|r| r.id.ends_with(&format!(".t.{id}"))).map(|r| r.id.clone())).unwrap_or_default();
    check("找到该工具行", !row_id.is_empty());
    let renders = transcript.read_with(cx, |t, _| t.ask_card_renders);
    let _ = cx.update_window(handle.into(), |_, _, cx| transcript.update(cx, |t, cx| t.toggle_tool_for_test(&row_id, cx)));
    for _ in 0..5 {
        cx.background_executor().timer(Duration::from_millis(30)).await;
        draw(handle, cx).await;
    }
    check("手动展开已回答的提问行：不再渲染提问卡（与创建文件的展示统一）", transcript.read_with(cx, |t, _| t.ask_card_renders) == renders && renders > 0);
    let _ = cx.update_window(handle.into(), |_, _, cx| transcript.update(cx, |t, cx| t.toggle_tool_for_test(&row_id, cx)));

    // ── ask_user：跳过 ──
    type_and_send(handle, "请提问", cx).await;
    check("第二次提问出现", wait_until(handle, cx, |cx| question_id(cx).is_some_and(|q| q != id)).await);
    let id2 = question_id(cx).unwrap_or_default();
    wait_until(handle, cx, |cx| card_of(cx, &id2).is_some()).await;
    if let Some(card) = card_of(cx, &id2) {
        let _ = cx.update_window(handle.into(), |_, _, cx| card.update(cx, |c, cx| c.skip(cx)));
    }
    check("跳过 → 问题清除、回合结束", wait_until(handle, cx, |cx| question_id(cx).is_none() && !streaming(handle, cx)).await);
    let skipped = tool_result(cx, &id2).unwrap_or_default();
    check("跳过的结果不含选项", !skipped.contains("User selected"));

    // ── ask_user：只有自定义回答 ──
    type_and_send(handle, "请提问", cx).await;
    check("第三次提问出现", wait_until(handle, cx, |cx| question_id(cx).is_some_and(|q| q != id && q != id2)).await);
    let id3 = question_id(cx).unwrap_or_default();
    wait_until(handle, cx, |cx| card_of(cx, &id3).is_some()).await;
    if let Some(card) = card_of(cx, &id3) {
        let custom = card.read_with(cx, |c, _| c.custom_area().clone());
        let _ = cx.update_window(handle.into(), |_, _, cx| custom.update(cx, |a, cx| a.set_text("我想自己定", cx)));
        let _ = cx.update_window(handle.into(), |_, _, cx| card.update(cx, |c, cx| c.submit(cx)));
    }
    check("自定义回答 → 回合结束", wait_until(handle, cx, |cx| question_id(cx).is_none() && !streaming(handle, cx)).await);
    check("engine 收到自定义回答", tool_result(cx, &id3).unwrap_or_default().contains("我想自己定"));

    // ── 审批：Esc 拒绝 ──
    let p1 = dir.join("approved-1.txt");
    let p2 = dir.join("approved-2.txt");
    let p3 = dir.join("approved-3.txt");
    let (s1, s2, s3) = (p1.to_string_lossy().to_string(), p2.to_string_lossy().to_string(), p3.to_string_lossy().to_string());
    type_and_send(handle, &format!("请写文件 {s1}"), cx).await;
    check("写入类工具 → 出现审批", wait_until(handle, cx, |cx| approval_id(cx).is_some()).await);
    let approval = conversation.read_with(cx, |c, _| c.state.approval.clone());
    check("审批内容：工具名与原因", approval.as_ref().is_some_and(|a| a.name == "create_file" && a.reason.contains("approved-1.txt")));
    draw(handle, cx).await;
    press(handle, "escape", cx).await;
    check("Esc → 拒绝：审批清除", wait_until(handle, cx, |cx| approval_id(cx).is_none()).await);
    check("回合结束", wait_until(handle, cx, |cx| !streaming(handle, cx)).await);
    let denied_id = approval.map(|a| a.id).unwrap_or_default();
    check("engine 收到拒绝、文件未创建", tool_result(cx, &denied_id).unwrap_or_default().contains("拒绝") && !p1.exists());

    // ── 审批：点「允许」（真实点击；按钮行纵向位置由布局决定，自下而上扫描）──
    type_and_send(handle, &format!("请写文件 {s1}"), cx).await;
    check("再次出现审批", wait_until(handle, cx, |cx| approval_id(cx).is_some()).await);
    draw(handle, cx).await;
    let mut y = 400.0;
    while approval_id(cx).is_some() && y > 250.0 {
        click(handle, 410.0, y, cx).await; // 「允许」按钮（宽 440 面板居中，三按钮 1 : 1 : 1.2）
        y -= 3.0;
    }
    check("点「允许」→ 审批清除", approval_id(cx).is_none());
    check("回合结束", wait_until(handle, cx, |cx| !streaming(handle, cx)).await);
    check("允许后文件已创建", std::fs::read_to_string(&p1).is_ok_and(|t| t == "hello"));

    // ── 审批：「本次都允许」——同一回合的第二个写入不再询问 ──
    type_and_send(handle, &format!("请写文件 {s2} {s3}"), cx).await;
    check("两个写入 → 先出现第一个审批", wait_until(handle, cx, |cx| approval_id(cx).is_some()).await);
    draw(handle, cx).await;
    let first_approval = approval_id(cx);
    let mut seen = std::collections::BTreeSet::new();
    let mut y = 400.0;
    while approval_id(cx).is_some() && y > 250.0 {
        click(handle, 268.0, y, cx).await; // 「本次都允许」按钮
        y -= 3.0;
    }
    let start = Instant::now();
    while streaming(handle, cx) && start.elapsed() < Duration::from_secs(15) {
        if let Some(id) = approval_id(cx) {
            seen.insert(id);
        }
        cx.background_executor().timer(Duration::from_millis(10)).await;
        draw(handle, cx).await;
    }
    check("点「本次都允许」→ 第一个审批清除", first_approval.is_some());
    check("后续写入不再询问", seen.is_empty());
    check("两个文件都已创建", p2.exists() && p3.exists());

    // ── 折叠后的样子：提问行与创建文件行一样高（目检 #19 反馈：多行问题曾把折叠态撑成三行）──
    let row_ids = transcript.read_with(cx, |t, _| t.rows().iter().map(|r| r.id.clone()).collect::<Vec<_>>());
    let ask_row = row_ids.iter().find(|r| r.contains(".t.call_ask_")).cloned().unwrap_or_default();
    let write_row = row_ids.iter().find(|r| r.contains(".t.call_write_")).cloned().unwrap_or_default();
    let mut heights = Vec::new();
    for row in [&ask_row, &write_row] {
        let _ = cx.update_window(handle.into(), |_, _, cx| transcript.update(cx, |t, cx| t.scroll_to_row(row, cx)));
        for _ in 0..4 {
            cx.background_executor().timer(Duration::from_millis(30)).await;
            draw(handle, cx).await;
        }
        heights.push(transcript.read_with(cx, |t, _| t.painted_row_bounds(row)).map(|b| f32::from(b.size.height)));
    }
    check(
        &format!("折叠后提问行与创建文件行等高（{:?}）", heights),
        matches!(heights.as_slice(), [Some(a), Some(b)] if (a - b).abs() < 0.5),
    );

    let ok = checks.iter().all(|(_, ok)| *ok);
    for (name, ok) in &checks {
        println!("  {} {name}", if *ok { "ok  " } else { "FAIL" });
    }
    println!("{} S05-13 T29 提问卡与审批浮层（回答 / 跳过 / 自定义 / Esc 拒绝 / 允许 / 本次都允许）", if ok { "PASS" } else { "FAIL" });
    ok
}


/// T30：对话页的窗口拖动条（目检 #19 反馈：气泡态能拖，对话态不能）
async fn selftest_drag(url: &str, cx: &mut AsyncApp) -> bool {
    let engine = ChatEngine::new(sandbox("drag", Some(&mock_config(url))));
    let (handle, _) = open_router(engine, cx).await;
    for _ in 0..3 {
        draw(handle, cx).await;
    }
    type_and_send(handle, "你好", cx).await;
    let done = wait_turn_done(handle, cx).await;
    let counter = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let c = counter.clone();
    let chat = handle.read_with(cx, |r, _| r.chat_page().clone()).unwrap();
    let _ = cx.update_window(handle.into(), |_, _, cx| chat.update(cx, |p, _| p.set_drag_handler(std::rc::Rc::new(move |_| c.set(c.get() + 1)))));
    draw(handle, cx).await;
    let mut checks: Vec<(&str, bool)> = Vec::new();
    let mut check = |name: &'static str, ok: bool| checks.push((name, ok));
    check("进入对话页", done && page(handle, cx) == Page::Conversation);
    for (name, x, y) in [("顶部", WIDTH / 2.0, 6.0), ("左边缘", 3.0, HEIGHT / 2.0), ("右边缘", WIDTH - 3.0, HEIGHT / 2.0), ("底边缘", WIDTH / 2.0, HEIGHT - 3.0)] {
        let before = counter.get();
        click(handle, x, y, cx).await;
        check(match name { "顶部" => "点顶部拖动条 → 开始拖动窗口", "左边缘" => "点左边缘 → 开始拖动窗口", "右边缘" => "点右边缘 → 开始拖动窗口", _ => "点底边缘 → 开始拖动窗口" }, counter.get() == before + 1);
    }
    let before = counter.get();
    click(handle, WIDTH / 2.0, HEIGHT / 2.0, cx).await;
    check("点消息区中部 → 不拖动窗口（保留文本选择）", counter.get() == before);
    let composer = handle.read_with(cx, |r, _| r.composer().clone()).unwrap();
    let bounds = composer.read_with(cx, |c, _| c.model_button_bounds()).unwrap();
    click(handle, f32::from(bounds.center().x), f32::from(bounds.center().y), cx).await;
    check("点输入区按钮 → 不拖动窗口", counter.get() == before);
    if let Some(menu) = handle.read_with(cx, |r, _| r.model_menu()).unwrap() {
        let esc = PlatformInput::KeyDown(KeyDownEvent { keystroke: Keystroke::parse("escape").unwrap(), is_held: false, prefer_character_input: false });
        let _ = cx.update_window(menu.into(), |_, window, cx| window.dispatch_event(esc, cx));
    }

    let ok = checks.iter().all(|(_, ok)| *ok);
    for (name, ok) in &checks {
        println!("  {} {name}", if *ok { "ok  " } else { "FAIL" });
    }
    println!("{} S05-13 T30 对话页窗口拖动条（顶 / 左 / 右 / 底可拖，消息区与按钮不拖）", if ok { "PASS" } else { "FAIL" });
    ok
}

/// T34：真实 engine 附件链路——Cmd-V、路径保存、切页保留、点击删除、纯图片发送。
async fn selftest_attachments(url: &str, cx: &mut AsyncApp) -> bool {
    let data_dir = sandbox("attachments", Some(&mock_config(url)));
    let engine = ChatEngine::new(data_dir.clone());
    let (handle, _) = open_router(engine.clone(), cx).await;
    let composer = handle.read_with(cx, |router, _| router.composer().clone()).unwrap();
    let mut config = mock_config(url);
    config.models[0].supports_vision = true;
    let config_for_engine = config.clone();
    let engine_for_config = engine.clone();
    let config_saved = cx
        .update(|cx| {
            spawn_engine(cx, async move { engine_for_config.save_config(config_for_engine).await })
        })
        .await
        .is_ok();
    let _ = handle.update(cx, |router, _, cx| router.set_config(config, cx));
    let bytes = attachments::decode_data_url(
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
        "image/png",
    ).expect("valid one-pixel PNG");
    let image = Image::from_bytes(ImageFormat::Png, bytes.clone());
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_image(&image)));
    let focus = composer.read_with(cx, |composer, cx| composer.focus_handle(cx));
    let _ = cx.update_window(handle.into(), |_, window, cx| window.focus(&focus, cx));
    draw(handle, cx).await;
    let paste = PlatformInput::KeyDown(KeyDownEvent { keystroke: Keystroke::parse("cmd-v").expect("cmd-v"), is_held: false, prefer_character_input: false });
    let _ = cx.update_window(handle.into(), |_, window, cx| window.dispatch_event(paste, cx));
    draw(handle, cx).await;
    let pasted = composer.read_with(cx, |composer, _| composer.image_count() == 1);
    let saved = wait_until(handle, cx, |cx| composer.read_with(cx, |composer, _| composer.images().first().is_some_and(|image| !image.path.is_empty()))).await;
    let saved_path = composer.read_with(cx, |composer, _| composer.images().first().map(|image| image.path.clone())).unwrap_or_default();
    let saved_on_disk = pasted && saved && Path::new(&saved_path).exists();
    let _ = handle.update(cx, |router, _, cx| router.open_settings(cx));
    let _ = handle.update(cx, |router, _, cx| router.close_settings(cx));
    let preserved = composer.read_with(cx, |composer, _| composer.image_count() == 1);
    let mut clicked_removed = false;
    for y in [HEIGHT - 86.0, HEIGHT - 120.0, HEIGHT - 52.0] {
        if clicked_removed { break; }
        click(handle, 46.0, y, cx).await;
        clicked_removed = composer.read_with(cx, |composer, _| composer.image_count() == 0);
    }
    let deleted = wait_until(handle, cx, |_| !Path::new(&saved_path).exists()).await;

    // GPUI 原生文件拖放事件：用 Entered → Pending → Submit 复现平台路径。
    let drop_source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/buddy-app-preview/attachments/drop.png");
    std::fs::write(&drop_source, &bytes).expect("写入拖放图片");
    let position = point(px(20.0), px(HEIGHT - 20.0));
    let paths = ExternalPaths(std::iter::once(drop_source.clone()).collect());
    for event in [
        FileDropEvent::Entered { position, paths },
        FileDropEvent::Pending { position },
        FileDropEvent::Submit { position },
    ] {
        let _ = cx.update_window(handle.into(), |_, window, cx| window.dispatch_event(PlatformInput::FileDrop(event), cx));
        draw(handle, cx).await;
    }
    let dropped = composer.read_with(cx, |composer, _| composer.image_count() == 1);
    let drop_saved = wait_until(handle, cx, |cx| composer.read_with(cx, |composer, _| composer.images().first().is_some_and(|image| !image.path.is_empty()))).await;
    let drop_saved_path = composer.read_with(cx, |composer, _| composer.images().first().map(|image| image.path.clone())).unwrap_or_default();
    let drop_on_disk = dropped && drop_saved && Path::new(&drop_saved_path).exists();
    let mut drop_removed = false;
    for y in [HEIGHT - 86.0, HEIGHT - 120.0, HEIGHT - 52.0] {
        if drop_removed { break; }
        click(handle, 46.0, y, cx).await;
        drop_removed = composer.read_with(cx, |composer, _| composer.image_count() == 0);
    }
    let drop_deleted = wait_until(handle, cx, |_| !Path::new(&drop_saved_path).exists()).await;
    let _ = std::fs::remove_file(drop_source);

    // 用同一 engine 打开新的空态路由器，覆盖 classify_empty_send 的真实生产路径。
    let (pure_handle, _) = open_router(engine.clone(), cx).await;
    let pure_composer = pure_handle.read_with(cx, |router, _| router.composer().clone()).unwrap();
    let pure_empty = page(pure_handle, cx) == Page::Empty;

    // 生产 Router 路径：再次经剪贴板 Cmd-V 注入，只带图片、无文字也创建带图用户消息；
    // 发送断言先检查会话，再等待真实 engine 落盘，不依赖回复正文。
    let pure_image = Image::from_bytes(ImageFormat::Png, bytes.clone());
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_image(&pure_image)));
    let focus = pure_composer.read_with(cx, |composer, cx| composer.focus_handle(cx));
    let _ = cx.update_window(pure_handle.into(), |_, window, cx| window.focus(&focus, cx));
    draw(pure_handle, cx).await;
    let paste = PlatformInput::KeyDown(KeyDownEvent { keystroke: Keystroke::parse("cmd-v").expect("cmd-v"), is_held: false, prefer_character_input: false });
    let _ = cx.update_window(pure_handle.into(), |_, window, cx| window.dispatch_event(paste, cx));
    draw(pure_handle, cx).await;
    let pure_pasted = pure_composer.read_with(cx, |composer, _| composer.image_count() == 1);
    let pure_saved = wait_until(pure_handle, cx, |cx| pure_composer.read_with(cx, |composer, _| composer.images().first().is_some_and(|image| !image.path.is_empty()))).await;
    let focus = pure_composer.read_with(cx, |composer, cx| composer.focus_handle(cx));
    let _ = cx.update_window(pure_handle.into(), |_, window, cx| window.focus(&focus, cx));
    draw(pure_handle, cx).await;
    pure_composer.update(cx, |composer, cx| composer.set_draft("", cx));
    press(pure_handle, "enter", cx).await;
    let pure_message = wait_until(pure_handle, cx, |cx| messages(pure_handle, cx).iter().any(|message| message.role == MessageRole::User && message.images.len() == 1 && message.content.is_empty())).await;
    let mut stored_pure_message = false;
    if pure_message {
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(15) {
            let stored_messages = stored(&engine, cx).await;
            if stored_messages.iter().any(|message| message.role == MessageRole::User && message.images.len() == 1 && message.content.is_empty()) {
                stored_pure_message = true;
                break;
            }
            cx.background_executor().timer(Duration::from_millis(20)).await;
        }
    }
    let pure_draft_cleared = pure_composer.read_with(cx, |composer, app| composer.draft(app).is_empty() && composer.image_count() == 0);
    engine.stop_generation();
    let stopped = wait_until(pure_handle, cx, |cx| !pure_handle.read_with(cx, |router, cx| router.conversation().read(cx).state.is_streaming()).unwrap()).await;

    // 调用生产下载边界，校验系统下载目录中的实际字节，并清理本次文件。
    let downloaded_image = messages(pure_handle, cx).iter().find(|message| message.role == MessageRole::User)
        .and_then(|message| message.images.first()).cloned();
    let downloaded = if let Some(image) = downloaded_image {
        let download_engine = engine.clone();
        match cx.update(|cx| spawn_engine(cx, async move { download_engine.download_generated_image(image).await })).await {
            Ok(path) => {
                let matches = std::fs::read(&path).is_ok_and(|actual| actual == bytes);
                let cleaned = std::fs::remove_file(path).is_ok();
                matches && cleaned
            }
            Err(error) => { eprintln!("T34 下载失败：{error}"); false }
        }
    } else { false };

    // 历史附件真实加载失败 → 修复文件 → 鼠标点击重试 → GPUI 缓存返回解码图片。
    let retry_path = std::env::temp_dir().join(format!("buddy-history-selftest-{}.png", std::process::id()));
    let _ = std::fs::remove_file(&retry_path);
    let user_id = pure_handle.update(cx, |router, _, cx| router.conversation().update(cx, |conversation, cx| {
        let message = conversation.state.messages.iter_mut().find(|message| message.role == MessageRole::User).expect("pure-image user");
        message.images[0].id = "history-retry-selftest".into();
        message.images[0].path = retry_path.to_string_lossy().into_owned();
        let id = message.id.clone();
        conversation.state.revision += 1;
        cx.notify();
        id
    })).unwrap();
    let transcript = pure_handle.read_with(cx, |router, cx| router.transcript(cx)).unwrap();
    transcript.update(cx, |transcript, cx| transcript.scroll_to_row(&user_id, cx));
    for _ in 0..12 { draw(pure_handle, cx).await; }
    let resource: Resource = retry_path.clone().into();
    let history_failed = wait_until(pure_handle, cx, |cx| cx.update_window(pure_handle.into(), |_, window, cx| matches!(window.get_asset::<ImgResourceLoader>(&resource, cx), Some(Err(_)))).unwrap_or(false)).await;
    let fixture = attachments::decode_data_url("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=", "image/png").unwrap();
    std::fs::write(&retry_path, fixture).expect("repair fixture");
    let bounds = transcript.read_with(cx, |transcript, _| transcript.painted_row_bounds(&user_id));
    let mut history_retried = false;
    if let Some(bounds) = bounds {
        'retry: for dy in [38.0, 44.0, 50.0, 56.0, 62.0, 68.0] {
            for dx in [146.0, 154.0, 138.0] {
                click(pure_handle, f32::from(bounds.right()) - dx, f32::from(bounds.top()) + dy, cx).await;
                cx.background_executor().timer(Duration::from_millis(30)).await;
                for _ in 0..3 { draw(pure_handle, cx).await; }
                history_retried = cx.update_window(pure_handle.into(), |_, window, cx| {
                    window.get_asset::<ImgResourceLoader>(&resource, cx).is_some_and(|result| result.is_ok_and(|image| image.size(0).width.0 == 1 && image.size(0).height.0 == 1))
                }).unwrap_or(false);
                if history_retried { break 'retry; }
            }
        }
    }
    let _ = std::fs::remove_file(retry_path);
    // Drop a Composer before its engine write completes. Pause this foreground
    // task briefly so the Tokio write is observable before the detached cleanup runs.
    let attachment_dir = data_dir.join("attachments");
    let paths = |dir: &Path| -> std::collections::HashSet<PathBuf> {
        std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok).map(|entry| entry.path()).collect()
    };
    let before = paths(&attachment_dir);
    let orphan_engine = engine.clone();
    let orphan_draft = attachments::draft_from_bytes("销毁清理.png", "image/png", bytes.clone()).unwrap();
    let _ = cx.update_window(pure_handle.into(), |_, window, cx| {
        let orphan_composer = cx.new(|cx| buddy_ui::chat::composer::Composer::new_with_engine(Some(orphan_engine), window, cx));
        orphan_composer.update(cx, |composer, cx| composer.add_images(vec![orphan_draft], cx));
        drop(orphan_composer);
    });
    let started = Instant::now();
    let mut orphan_path = None;
    while started.elapsed() < Duration::from_secs(2) {
        orphan_path = paths(&attachment_dir).difference(&before).next().cloned();
        if orphan_path.is_some() { break; }
        std::thread::sleep(Duration::from_millis(2));
    }
    let orphan_saved = orphan_path.as_ref().is_some_and(|path| std::fs::read(path).is_ok_and(|actual| actual == bytes));
    let orphan_cleaned = if let Some(path) = orphan_path {
        wait_until(pure_handle, cx, |_| !path.exists()).await
    } else { false };

    let ok = config_saved && pasted && saved_on_disk && preserved && clicked_removed && deleted && dropped && drop_on_disk && drop_removed && drop_deleted && pure_empty && pure_pasted && pure_saved && pure_message && stored_pure_message && pure_draft_cleared && stopped && downloaded && history_failed && history_retried && orphan_saved && orphan_cleaned;
    println!("T34: 配置落盘 {config_saved}；Cmd-V {pasted}；保存路径 {saved_on_disk}；切页保留 {preserved}；点击删除 {clicked_removed}；文件清理 {deleted}；拖放入口 {dropped}；拖放保存 {drop_on_disk}；拖放删除 {drop_removed}；拖放清理 {drop_deleted}；空态纯图页 {pure_empty}；纯图 Cmd-V {pure_pasted}；纯图片保存 {pure_saved}；内存用户消息带图 {pure_message}；磁盘用户消息带图 {stored_pure_message}；草稿清空 {pure_draft_cleared}；真实下载字节与清理 {downloaded}；历史图片失败 {history_failed} → 真实点击重试解码 {history_retried}；实体销毁仍完成保存 {orphan_saved} → 清理 {orphan_cleaned}");
    println!("{} S05-07 T34 真实 engine 附件保存 / 删除 / 纯图片发送", if ok { "PASS" } else { "FAIL" });
    ok
}

// ───────────────────────────── 入口 ─────────────────────────────

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let (self_test, self_test_attachments, mock, no_key) = (flag("--selftest"), flag("--selftest-attachments"), flag("--mock"), flag("--no-key"));
    application().with_assets(buddy_ui::icons::Assets).run(move |cx: &mut App| {
        buddy_ui::init_theme(cx);
        buddy_ui::chat_bridge::init(cx);
        Theme::install(Appearance::Light, cx);
        fonts::install_text_rendering(cx);
        markdown::init(cx);
        buddy_ui::chat::init(cx);
        // 无标题栏，没有关闭按钮：手动模式下 Cmd+Q / Esc 退出（自检不注册，否则未被处理的 Esc 会让自检静默退出）
        if !self_test && !self_test_attachments {
            cx.observe_keystrokes(|event, _, cx| {
                if event.keystroke.unparse() == "cmd-q" || event.keystroke.key == "escape" {
                    cx.quit();
                }
            })
            .detach();
        }

        if self_test || self_test_attachments {
            let url = start_mock_server();
            cx.spawn(async move |cx: &mut AsyncApp| {
                if self_test_attachments {
                    std::process::exit(if selftest_attachments(&url, cx).await { 0 } else { 1 });
                }
                let t25 = selftest_flow(&url, cx).await;
                let t26 = selftest_no_key(&url, cx).await;
                let t27 = selftest_history(&url, cx).await;
                let t28 = selftest_models(&url, cx).await;
                let t29 = selftest_interactions(&url, cx).await;
                let t30 = selftest_drag(&url, cx).await;
                let t34 = selftest_attachments(&url, cx).await;
                std::process::exit(if t25 && t26 && t27 && t28 && t29 && t30 && t34 { 0 } else { 1 });
            })
            .detach();
            return;
        }
        let dir = if mock {
            sandbox("manual-mock", Some(&models_config(&start_mock_server(), &["p1::mock", "p1::mock2"])))
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
