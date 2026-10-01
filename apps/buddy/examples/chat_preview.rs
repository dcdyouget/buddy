//! Phase 05 聊天界面预览与自检。
//!
//! ```text
//! cargo run -p buddy-app --example chat_preview                          # 目检：1000 条历史 + 模拟流式回复
//! cargo run -p buddy-app --example chat_preview -- --messages 5000       # 指定历史条数
//! cargo run -p buddy-app --example chat_preview -- --selftest            # 自检后退出
//! cargo run -p buddy-app --example chat_preview -- --selftest-image     # 仅自检图片生成卡片
//! ```
//!
//! 不连接 engine：历史为合成消息；「模拟流式回复」按固定种子把样例切成片段，模拟网络到达后经
//! [`Conversation::apply_events`] 注入（与真实 `chat_bridge` 批次同一路径）。
//!
//! 自检：
//! - T11 虚拟化（S05-01）：1000 条消息（约 2000+ 行）中，滚动 60 帧，平均每帧布局的行数远小于总行数；
//! - T12 流式只重测一行（S05-02 / S05-17）：纯正文流式期间，每次行同步最多重测 1 行、行集合不变；
//! - T13 重绘预算：1000 条消息时整窗重绘中位耗时 < 50ms；
//! - T14 视口上方行高变化（S05-03）：可见内容不移动（视口首行与行内偏移不变），且只重测那一行；
//! - T15 用户消息保留换行（S05-08，v1 `white-space: pre-wrap`）；
//! - T22 历史分页（S05-05）：另开窗口，5001 条只载入最新 10 条（首条是助手消息 → 行锚 `head-a4991`）；
//!   真实滚轮滚到顶 → 加载更早 10 条，视口首行（改锚的边界行）与行内偏移、可见行屏幕位置均不变。
//!
//! 目检模式按 v1 分页：先载最新 10 条，滚到顶再读更早的（读取延迟 150ms，便于看到「正在加载更早消息…」）。

use buddy_engine::chat::ChatEngine;
use buddy_engine::models::{ImageAttachment, Message, MessageRole};
use buddy_engine::streaming::{ContentBlock, StopReason, StreamEvent};
use buddy_ui::chat::{
    composer::{Composer, ComposerEvent},
    session::Conversation,
    transcript::Transcript,
};
use buddy_ui::gpui::{
    App, AppContext, AsyncApp, Bounds, ClipboardItem, Context, Entity, Image, ImageFormat, Render,
    Window, WindowBounds, WindowHandle, WindowOptions, div, prelude::*, px, size,
};
use buddy_ui::gpui::{EntityInputHandler, Focusable, KeyDownEvent, Keystroke, PlatformInput};
use buddy_ui::gpui_platform::application;
use buddy_ui::markdown;
use buddy_ui::theme_system::{
    Appearance, BuddyTheme, Theme, fonts, set_appearance, tokens::metrics,
};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

const ANSWERS: &[&str] = &[
    "好的。这是一个**简短**的回答。",
    "分三点说明：\n\n1. 第一点，说明背景\n2. 第二点，给出做法\n3. 第三点，注意事项\n\n以上。",
    "示例代码：\n\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\n\n运行后会打印 hello。",
    "| 方案 | 优点 |\n|------|------|\n| A | 简单 |\n| B | 灵活 |\n\n一般选 A。",
    "这是一段较长的回答，用来制造不同高度的行。它包含多句话，会在窄窗口中折成多行显示。继续补充一些内容，让高度更有变化；再加一句收尾。",
];

const STREAM_REPLY: &str = "下面是一段模拟的流式回复，用来观察列表贴底与逐字渐显。\n\n## 要点\n\n- 列表只布局可见的行\n- 流式时只有最后一行在变\n- 历史越长，滚动依然流畅\n\n```python\ndef hello():\n    return \"world\"\n```\n\n以上就是全部内容。";

fn message(id: String, role: MessageRole, text: &str) -> Message {
    Message {
        id,
        role: role.clone(),
        content: text.into(),
        images: Vec::new(),
        blocks: (role == MessageRole::Assistant).then(|| {
            vec![ContentBlock::Text {
                content: text.into(),
            }]
        }),
        model_id: None,
        created_at: 0,
        tool_calls: None,
        tool_call_id: None,
        tool_name: None,
        is_error: None,
        parent_message_id: None,
    }
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as usize;
        let b = chunk.get(1).copied().unwrap_or(0) as usize;
        let c = chunk.get(2).copied().unwrap_or(0) as usize;
        out.push(TABLE[a >> 2] as char);
        out.push(TABLE[((a & 3) << 4) | (b >> 4)] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((b & 15) << 2) | (c >> 6)] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[c & 63] as char
        } else {
            '='
        });
    }
    out
}

fn history(count: usize) -> Vec<Message> {
    (0..count)
        .map(|i| {
            if i == 0 {
                // T15：多行用户消息（v1 `white-space: pre-wrap` 保留换行）
                message("u0".into(), MessageRole::User, "第一行\n第二行\n第三行")
            } else if i % 2 == 0 {
                message(
                    format!("u{i}"),
                    MessageRole::User,
                    &format!("第 {} 个问题：请解释一下这个概念？", i / 2 + 1),
                )
            } else {
                message(
                    format!("a{i}"),
                    MessageRole::Assistant,
                    ANSWERS[(i / 2) % ANSWERS.len()],
                )
            }
        })
        .collect()
}

/// 分页读取合成历史（替代 engine `load_messages`）。`gate` 为 `Some` 时读取挂起到放行（自检用）
fn paged_conversation(
    all: Vec<Message>,
    delay: Duration,
    gate: Option<Rc<Cell<bool>>>,
) -> Conversation {
    let all = Rc::new(all);
    let offset = all
        .len()
        .saturating_sub(buddy_ui::chat::state::HISTORY_PAGE_SIZE as usize);
    let page = all[offset..].to_vec();
    let loader: buddy_ui::chat::session::HistoryLoader =
        Rc::new(move |offset, limit, cx: &mut App| {
            let all = all.clone();
            let gate = gate.clone();
            cx.spawn(async move |cx: &mut AsyncApp| {
                cx.background_executor().timer(delay).await;
                while gate.as_ref().is_some_and(|g| !g.get()) {
                    cx.background_executor()
                        .timer(Duration::from_millis(10))
                        .await;
                }
                let (start, end) = (offset as usize, (offset + limit) as usize);
                Ok(all[start..end.min(all.len())].to_vec())
            })
        });
    Conversation::with_history_page(page, offset as u64, loader)
}

struct ChatPreview {
    conversation: Entity<Conversation>,
    transcript: Entity<Transcript>,
    composer: Entity<Composer>,
    /// 发送过的草稿（T16 计数）
    sent: Vec<String>,
    /// 模拟流式的停止标志
    stop: Rc<Cell<bool>>,
    _subscriptions: Vec<buddy_ui::gpui::Subscription>,
}

impl ChatPreview {
    fn new(
        conversation: Entity<Conversation>,
        transcript: Entity<Transcript>,
        engine: Arc<ChatEngine>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            // v1：窗口失焦 / 隐藏时立即放出缓冲，重新聚焦后追赶再恢复逐字
            cx.observe_window_activation(window, |this: &mut Self, window, cx| {
                let active = window.is_window_active();
                this.conversation.update(cx, |c, cx| {
                    if active {
                        c.window_shown(cx)
                    } else {
                        c.window_hidden(cx)
                    }
                });
            }),
            // 流式状态同步到输入区（流式中显示「生成中」+ 停止按钮）
            cx.observe_in(
                &conversation,
                window,
                |this: &mut Self, conversation, window, cx| {
                    let streaming = conversation.read(cx).state.is_streaming();
                    this.composer.update(cx, |c, cx| {
                        c.set_streaming(streaming, Some("mock-model".into()), window, cx)
                    });
                    cx.notify();
                },
            ),
        ];
        let composer = cx.new(|cx| Composer::new(window, cx));
        // The preview's default mock model supports vision; T33 explicitly
        // switches this off to exercise the rejection path.
        composer.update(cx, |c, cx| c.set_supports_vision(true, cx));
        // 预览使用沙盒 engine 的真实下载实现；自测会另行注入可控失败 / 成功回调。
        let download_engine = engine.clone();
        transcript.update(cx, |t, _| {
            t.set_download_handler(Rc::new(move |image, cx| {
                let engine = download_engine.clone();
                buddy_ui::chat_bridge::spawn_engine(cx, async move {
                    engine.download_generated_image(image).await
                })
            }));
        });
        let mut subscriptions = subscriptions;
        subscriptions.push(cx.subscribe(
            &composer,
            |this: &mut Self, composer, event: &ComposerEvent, cx| match event {
                ComposerEvent::Send(text) => {
                    let images = composer.update(cx, |c, cx| c.take_images(cx));
                    this.sent.push(text.clone());
                    composer.update(cx, |c, cx| c.set_draft("", cx));
                    let text = text.clone();
                    this.simulate_reply(&text, images, cx);
                }
                // v1：停止 → engine 发出 Error(Aborted) → 按正常结束处理
                ComposerEvent::Stop => this.stop.set(true),
                ComposerEvent::OpenSettings | ComposerEvent::PickModel => {}
            },
        ));
        window.focus(&composer.focus_handle(cx), cx);
        Self {
            conversation,
            transcript,
            composer,
            sent: Vec::new(),
            stop: Rc::default(),
            _subscriptions: subscriptions,
        }
    }

    /// 模拟一次出错：发送后先到一段正文，再收到网络错误（v1：已显示正文保留、提示条显示错误）
    fn simulate_error(&mut self, cx: &mut Context<Self>) {
        let user = message(
            format!("u-err-{}", buddy_ui::chat::state::unique_suffix()),
            MessageRole::User,
            "请模拟一次出错",
        );
        self.conversation.update(cx, |c, cx| {
            c.begin_send(user, "mock-model", cx);
            c.apply_events(
                vec![
                    StreamEvent::TextStart { content_index: 0 },
                    StreamEvent::TextDelta {
                        content_index: 0,
                        delta: "这是出错前已经生成的一段内容。".into(),
                    },
                    StreamEvent::Error {
                        reason: StopReason::Error,
                        message: "网络错误：连接被重置（模拟）".into(),
                        partial_text: String::new(),
                    },
                ],
                cx,
            );
        });
    }

    /// 模拟联网搜索（S05-11）：搜索中停留 `hold` → 三条结果（读到正文 / 正文读取失败 / 仅摘要）→ 最终回答
    fn simulate_search(&mut self, hold: Duration, cx: &mut Context<Self>) {
        let conversation = self.conversation.clone();
        let user = message(
            format!("u-search-{}", buddy_ui::chat::state::unique_suffix()),
            MessageRole::User,
            "帮我搜索一下 Buddy 桌面 AI 聊天",
        );
        conversation.update(cx, |c, cx| c.begin_send(user, "mock-model", cx));
        cx.spawn(async move |_, cx: &mut AsyncApp| {
            let send = |events: Vec<StreamEvent>, cx: &mut AsyncApp| {
                let _ = conversation.update(cx, |c, cx| c.apply_events(events, cx));
            };
            let args = r#"{"query":"Buddy 桌面 AI 聊天"}"#;
            send(
                vec![
                    StreamEvent::Start,
                    StreamEvent::ToolCallStart { id: "call-search".into(), name: "websearch".into(), content_index: 0 },
                    StreamEvent::ToolCallEnd { id: "call-search".into(), name: "websearch".into(), arguments: args.into() },
                    StreamEvent::TurnEnd { tool_calls_pending: 1 },
                    StreamEvent::ToolExecuting { id: "call-search".into(), name: "websearch".into() },
                ],
                cx,
            );
            cx.background_executor().timer(hold).await;
            let payload = serde_json::json!({
                "status": "partial",
                "query": "Buddy 桌面 AI 聊天",
                "provider": "so_360+duckduckgo",
                "providers": [{"name": "so_360", "status": "ok", "result_count": 2}, {"name": "duckduckgo", "status": "ok", "result_count": 1}],
                "note": "DuckDuckGo 只返回了 1 条结果，已与 360 搜索合并。",
                "results": [
                    {"rank": 1, "source": "so_360", "title": "Buddy —— 一按即用的桌面 AI 聊天", "url": "https://example.com/buddy", "snippet": "按下全局快捷键，弹出轻量无边框窗口，与 AI 对话，点击外部即收起。支持流式输出、思考过程、工具调用与联网搜索。这是一段比较长的摘要，用来观察最多三行的截断效果。", "content": "……网页正文……"},
                    {"rank": 2, "source": "duckduckgo", "title": "", "url": "https://example.org/a/very/long/path/that/should/be/truncated/in/the/title/row", "snippet": "没有标题时用链接作为标题。", "fetch_error": "请求超时"},
                    {"rank": 3, "source": "so_360", "title": "另一个来源", "url": "https://example.net/", "snippet": "只有搜索摘要。"}
                ]
            })
            .to_string();
            send(
                vec![
                    StreamEvent::ToolResult { id: "call-search".into(), name: "websearch".into(), content: payload, images: Vec::new(), is_error: false },
                    StreamEvent::TextStart { content_index: 0 },
                    StreamEvent::TextDelta { content_index: 0, delta: "Buddy 是一个桌面 AI 聊天工具，详见[官网](https://example.com/buddy)。".into() },
                    StreamEvent::TextEnd { content_index: 0, content: "Buddy 是一个桌面 AI 聊天工具，详见[官网](https://example.com/buddy)。".into() },
                    StreamEvent::TurnEnd { tool_calls_pending: 0 },
                    StreamEvent::Done { reason: StopReason::Stop, full_text: String::new() },
                ],
                cx,
            );
        })
        .detach();
    }

    /// 模拟图片生成（S05-12）：停在生成中，随后返回本地图片与结果元数据。
    /// 使用仓库内已有 PNG，验证真实 `ToolResult` 图片路径与 GPUI 图片加载链路，不联网。
    fn simulate_image(&mut self, hold: Duration, cx: &mut Context<Self>) {
        let conversation = self.conversation.clone();
        let user = message(
            format!("u-image-{}", buddy_ui::chat::state::unique_suffix()),
            MessageRole::User,
            "帮我生成一张蓝色星空下的猫",
        );
        conversation.update(cx, |c, cx| c.begin_send(user, "mock-model", cx));
        cx.spawn(async move |_, cx: &mut AsyncApp| {
            let send = |events: Vec<StreamEvent>, cx: &mut AsyncApp| {
                let _ = conversation.update(cx, |c, cx| c.apply_events(events, cx));
            };
            let args = r#"{"prompt":"蓝色星空下的一只猫，数字插画"}"#;
            send(
                vec![
                    StreamEvent::Start,
                    StreamEvent::ToolCallStart {
                        id: "call-image".into(),
                        name: "generate_image".into(),
                        content_index: 0,
                    },
                    StreamEvent::ToolCallEnd {
                        id: "call-image".into(),
                        name: "generate_image".into(),
                        arguments: args.into(),
                    },
                    StreamEvent::TurnEnd {
                        tool_calls_pending: 1,
                    },
                    StreamEvent::ToolExecuting {
                        id: "call-image".into(),
                        name: "generate_image".into(),
                    },
                ],
                cx,
            );
            cx.background_executor().timer(hold).await;
            let path = std::env::current_dir()
                .unwrap_or_default()
                .join("src-tauri/icons/icon.png");
            let data_url = std::fs::read(&path)
                .map(|bytes| format!("data:image/png;base64,{}", encode_base64(&bytes)))
                .unwrap_or_default();
            let image = ImageAttachment {
                id: "generated-preview".into(),
                name: "Buddy-生成图片.png".into(),
                media_type: "image/png".into(),
                path: String::new(),
                data_url,
            };
            let result = serde_json::json!({
                "status": "ok",
                "model": "mock-image-model",
                "prompt": "蓝色星空下的一只猫，数字插画",
                "image_count": 1,
                "revised_prompts": ["蓝色星空下的一只猫，柔和数字插画"],
                "note": "模拟结果，图片来自仓库本地资源。"
            })
            .to_string();
            send(
                vec![
                    StreamEvent::ToolResult {
                        id: "call-image".into(),
                        name: "generate_image".into(),
                        content: result,
                        images: vec![image],
                        is_error: false,
                    },
                    StreamEvent::TextStart { content_index: 0 },
                    StreamEvent::TextDelta {
                        content_index: 0,
                        delta: "图片已经生成好了。".into(),
                    },
                    StreamEvent::TextEnd {
                        content_index: 0,
                        content: "图片已经生成好了。".into(),
                    },
                    StreamEvent::TurnEnd {
                        tool_calls_pending: 0,
                    },
                    StreamEvent::Done {
                        reason: StopReason::Stop,
                        full_text: String::new(),
                    },
                ],
                cx,
            );
        })
        .detach();
    }

    /// 模拟思考 + 工具调用（S05-09 / S05-10）：思考流式 → 读取文件（成功，长结果）→ 浏览目录（失败）→ 最终回答。
    /// `hold` 期间工具停在「执行中」，便于观察与自检
    fn simulate_tools(&mut self, hold: Duration, cx: &mut Context<Self>) {
        let conversation = self.conversation.clone();
        let user = message(
            format!("u-tool-{}", buddy_ui::chat::state::unique_suffix()),
            MessageRole::User,
            "帮我看看项目里的配置文件",
        );
        conversation.update(cx, |c, cx| c.begin_send(user, "mock-model", cx));
        cx.spawn(async move |_, cx: &mut AsyncApp| {
            let send = |events: Vec<StreamEvent>, cx: &mut AsyncApp| {
                let _ = conversation.update(cx, |c, cx| c.apply_events(events, cx));
            };
            let tick = Duration::from_millis(60);
            send(vec![StreamEvent::Start, StreamEvent::ThinkingStart { content_index: 0 }], cx);
            for piece in ["用户想看配置文件。", "先读取 Cargo.toml，", "再列一下 config 目录，", "然后总结给用户。"] {
                send(vec![StreamEvent::ThinkingDelta { content_index: 0, delta: piece.into() }], cx);
                cx.background_executor().timer(tick * 4).await;
            }
            send(vec![StreamEvent::ThinkingEnd { content_index: 0, content: "用户想看配置文件。先读取 Cargo.toml，再列一下 config 目录，然后总结给用户。".into() }], cx);
            let args1 = r#"{"path":"/Users/me/buddy/Cargo.toml"}"#;
            let args2 = r#"{"path":"/Users/me/buddy/config"}"#;
            send(
                vec![
                    StreamEvent::ToolCallStart { id: "call-read".into(), name: "read_file".into(), content_index: 0 },
                    StreamEvent::ToolCallEnd { id: "call-read".into(), name: "read_file".into(), arguments: args1.into() },
                    StreamEvent::ToolCallStart { id: "call-list".into(), name: "list_directory".into(), content_index: 1 },
                    StreamEvent::ToolCallEnd { id: "call-list".into(), name: "list_directory".into(), arguments: args2.into() },
                    StreamEvent::TurnEnd { tool_calls_pending: 2 },
                    StreamEvent::ToolExecuting { id: "call-read".into(), name: "read_file".into() },
                    StreamEvent::ToolExecuting { id: "call-list".into(), name: "list_directory".into() },
                ],
                cx,
            );
            cx.background_executor().timer(hold).await;
            let long: String = (1..=40).map(|i| format!("line {i:02} = \"value\"\n")).collect();
            send(
                vec![
                    StreamEvent::ToolResult { id: "call-read".into(), name: "read_file".into(), content: format!("[package]\nname = \"buddy\"\n{long}"), images: Vec::new(), is_error: false },
                    StreamEvent::ToolResult { id: "call-list".into(), name: "list_directory".into(), content: "目录不存在：/Users/me/buddy/config".into(), images: Vec::new(), is_error: true },
                    StreamEvent::TextStart { content_index: 0 },
                    StreamEvent::TextDelta { content_index: 0, delta: "已读取 `Cargo.toml`：包名为 **buddy**。config 目录不存在。".into() },
                    StreamEvent::TextEnd { content_index: 0, content: "已读取 `Cargo.toml`：包名为 **buddy**。config 目录不存在。".into() },
                    StreamEvent::TurnEnd { tool_calls_pending: 0 },
                    StreamEvent::Done { reason: StopReason::Stop, full_text: String::new() },
                ],
                cx,
            );
        })
        .detach();
    }

    /// 模拟一次流式回复：固定种子切片，20–120ms 间隔到达
    fn simulate_reply(
        &mut self,
        question: &str,
        images: Vec<ImageAttachment>,
        cx: &mut Context<Self>,
    ) {
        let conversation = self.conversation.clone();
        let mut user = message(
            format!("u-live-{}", buddy_ui::chat::state::unique_suffix()),
            MessageRole::User,
            question,
        );
        user.images = images;
        conversation.update(cx, |c, cx| c.begin_send(user, "mock-model", cx));
        let stop = self.stop.clone();
        stop.set(false);
        cx.spawn(async move |_, cx: &mut AsyncApp| {
            let mut seed: u64 = 7;
            let chars: Vec<char> = STREAM_REPLY.chars().collect();
            let mut i = 0;
            let _ = conversation.update(cx, |c, cx| {
                c.apply_events(
                    vec![
                        StreamEvent::Start,
                        StreamEvent::TextStart { content_index: 0 },
                    ],
                    cx,
                )
            });
            while i < chars.len() {
                if stop.get() {
                    let _ = conversation.update(cx, |c, cx| {
                        c.apply_events(
                            vec![StreamEvent::Error {
                                reason: StopReason::Aborted,
                                message: "用户取消".into(),
                                partial_text: String::new(),
                            }],
                            cx,
                        )
                    });
                    return;
                }
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let n = (1 + (seed >> 33) % 8) as usize;
                let piece: String = chars[i..(i + n).min(chars.len())].iter().collect();
                i += n;
                let _ = conversation.update(cx, |c, cx| {
                    c.apply_events(
                        vec![StreamEvent::TextDelta {
                            content_index: 0,
                            delta: piece,
                        }],
                        cx,
                    )
                });
                cx.background_executor()
                    .timer(Duration::from_millis(20 + (seed >> 40) % 100))
                    .await;
            }
            let _ = conversation.update(cx, |c, cx| {
                c.apply_events(
                    vec![
                        StreamEvent::TextEnd {
                            content_index: 0,
                            content: STREAM_REPLY.into(),
                        },
                        StreamEvent::TurnEnd {
                            tool_calls_pending: 0,
                        },
                        StreamEvent::Done {
                            reason: StopReason::Stop,
                            full_text: STREAM_REPLY.into(),
                        },
                    ],
                    cx,
                )
            });
        })
        .detach();
    }
}

impl Render for ChatPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.buddy_theme();
        let next = match theme.appearance {
            Appearance::Light => Appearance::Dark,
            Appearance::Dark => Appearance::Light,
        };
        let streaming = self.conversation.read(cx).state.is_streaming();
        let error = self.conversation.read(cx).state.error.clone();
        let button = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .px(px(metrics::SPACE_3))
                .py(px(metrics::SPACE_1))
                .rounded(px(metrics::RADIUS_SM))
                .border_1()
                .border_color(theme.colors.border_default)
                .text_color(theme.colors.text_primary)
                .cursor_pointer()
                .child(label)
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.colors.bg_surface)
            .child(
                div()
                    .flex()
                    .gap(px(metrics::SPACE_2))
                    .p(px(metrics::SPACE_3))
                    .child(
                        button(
                            "toggle",
                            if next == Appearance::Dark {
                                "切换到深色"
                            } else {
                                "切换到浅色"
                            },
                        )
                        .on_click(move |_, _, cx| set_appearance(next, cx)),
                    )
                    .when(!streaming, |d| {
                        d.child(button("reply", "模拟流式回复").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.simulate_reply("请模拟一段流式回复", Vec::new(), cx)
                            },
                        )))
                        .child(
                            button("error", "模拟出错")
                                .on_click(cx.listener(|this, _, _, cx| this.simulate_error(cx))),
                        )
                        .child(button("tools", "模拟思考与工具").on_click(cx.listener(
                            |this, _, _, cx| this.simulate_tools(Duration::from_secs(3), cx),
                        )))
                        .child(button("search", "模拟联网搜索").on_click(cx.listener(
                            |this, _, _, cx| this.simulate_search(Duration::from_secs(3), cx),
                        )))
                        .child(button("image", "模拟生图").on_click(cx.listener(
                            |this, _, _, cx| this.simulate_image(Duration::from_secs(3), cx),
                        )))
                    })
                    .child(
                        div()
                            .text_color(theme.colors.text_muted)
                            .child(format!("{} 行", self.transcript.read(cx).rows().len())),
                    ),
            )
            .child(div().flex_1().min_h_0().child(self.transcript.clone()))
            .when_some(error, |d, error| {
                let conversation = self.conversation.clone();
                d.child(buddy_ui::chat::message_row::error_banner(
                    &error,
                    move |_, cx| {
                        conversation.update(cx, |c, cx| {
                            c.state.error = None;
                            c.state.revision += 1;
                            cx.notify();
                        })
                    },
                    cx,
                ))
            })
            .child(self.composer.clone())
    }
}

async fn draw(handle: WindowHandle<ChatPreview>, cx: &mut AsyncApp) -> f64 {
    let t = Instant::now();
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    t.elapsed().as_secs_f64() * 1000.0
}

/// 模拟按键（经真实键位绑定与动作分发）
async fn press(handle: WindowHandle<ChatPreview>, keys: &str, cx: &mut AsyncApp) {
    let key = PlatformInput::KeyDown(KeyDownEvent {
        keystroke: Keystroke::parse(keys).expect("按键"),
        is_held: false,
        prefer_character_input: false,
    });
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_event(key, cx)
    });
    draw(handle, cx).await;
}

/// 在列表上滚动滚轮（正值 = 向上翻看历史）
async fn wheel(handle: WindowHandle<ChatPreview>, dy: f32, cx: &mut AsyncApp) {
    use buddy_ui::gpui::{
        Modifiers, MouseMoveEvent, ScrollDelta, ScrollWheelEvent, TouchPhase, point,
    };
    let position = point(px(280.0), px(300.0));
    let events = [
        PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers: Modifiers::default(),
        }),
        PlatformInput::ScrollWheel(ScrollWheelEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(0.0), px(dy))),
            modifiers: Modifiers::default(),
            touch_phase: TouchPhase::Moved,
        }),
    ];
    for e in events {
        let _ = cx.update_window(handle.into(), |_, window, cx| window.dispatch_event(e, cx));
        draw(handle, cx).await;
    }
    // 等平滑滚动停稳（S05-04 修订：滚轮按 v1 逐帧缓动）
    for _ in 0..40 {
        cx.background_executor()
            .timer(Duration::from_millis(16))
            .await;
        draw(handle, cx).await;
    }
}

/// T19 滚轮平滑滚动（v1 `useSmoothWheelScroll`）：一次「向上 3 行」应在多帧内逐渐走完 60px
async fn selftest_wheel(handle: WindowHandle<ChatPreview>, cx: &mut AsyncApp) -> bool {
    use buddy_ui::gpui::{
        Modifiers, MouseMoveEvent, ScrollDelta, ScrollWheelEvent, TouchPhase, point,
    };
    let transcript = handle.read_with(cx, |p, _| p.transcript.clone()).unwrap();
    wait_idle(handle, cx).await;
    transcript.update(cx, |t, _| {
        t.list_state().scroll_to(buddy_ui::gpui::ListOffset {
            item_ix: 400,
            offset_in_item: px(0.0),
        })
    });
    draw(handle, cx).await;
    let offset = |cx: &mut AsyncApp| {
        transcript.read_with(cx, |t, _| {
            -f32::from(t.list_state().scroll_px_offset_for_scrollbar().y)
        })
    };
    let position = point(px(280.0), px(300.0));
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: None,
                modifiers: Modifiers::default(),
            }),
            cx,
        )
    });
    draw(handle, cx).await;
    let start = offset(cx);
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_event(
            PlatformInput::ScrollWheel(ScrollWheelEvent {
                position,
                delta: ScrollDelta::Lines(point(0.0, 3.0)),
                modifiers: Modifiers::default(),
                touch_phase: TouchPhase::Moved,
            }),
            cx,
        )
    });
    draw(handle, cx).await;
    let mut trace = vec![start - offset(cx)];
    for _ in 0..40 {
        cx.background_executor()
            .timer(Duration::from_millis(16))
            .await;
        draw(handle, cx).await;
        trace.push(start - offset(cx));
    }
    let moving_frames = trace
        .windows(2)
        .filter(|w| (w[1] - w[0]).abs() > 0.01)
        .count();
    let monotonic = trace.windows(2).all(|w| w[1] >= w[0] - 0.01);
    let total = *trace.last().unwrap();
    println!(
        "T19: 向上 3 行：首帧后已移动 {:.1}px，逐帧移动 {moving_frames} 帧，最终 {total:.1}px（期望 60px）；前 8 帧 {:?}",
        trace[0],
        trace
            .iter()
            .take(8)
            .map(|v| (v * 10.0).round() / 10.0)
            .collect::<Vec<_>>()
    );
    let ok = trace[0] < 30.0 && moving_frames >= 5 && monotonic && (total - 60.0).abs() < 1.0;
    println!(
        "{} S05-04 T19 滚轮逐帧缓动（无极滚动，v1 useSmoothWheelScroll）",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

async fn wait_idle(handle: WindowHandle<ChatPreview>, cx: &mut AsyncApp) {
    while handle
        .read_with(cx, |p, cx| p.conversation.read(cx).state.is_streaming())
        .unwrap()
    {
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
        draw(handle, cx).await;
    }
    draw(handle, cx).await;
}

/// T18 跟随与回到底部（S05-04）
async fn selftest_follow(handle: WindowHandle<ChatPreview>, cx: &mut AsyncApp) -> bool {
    let transcript = handle.read_with(cx, |p, _| p.transcript.clone()).unwrap();
    wait_idle(handle, cx).await;
    let following =
        |cx: &mut AsyncApp| transcript.read_with(cx, |t, _| t.list_state().is_following_tail());
    let button = |cx: &mut AsyncApp| transcript.read_with(cx, |t, cx| t.scroll_button_visible(cx));
    let at_rest = following(cx) && !button(cx);

    // (a) 流式中上滑：脱离跟随，视口不再被新内容拉动；流式中不显示按钮
    handle
        .update(cx, |p, _, cx| {
            p.simulate_reply("请模拟一段流式回复", Vec::new(), cx)
        })
        .unwrap();
    for _ in 0..10 {
        cx.background_executor()
            .timer(Duration::from_millis(30))
            .await;
        draw(handle, cx).await;
    }
    wheel(handle, 400.0, cx).await;
    let detached = !following(cx);
    let top_a = transcript.read_with(cx, |t, _| t.list_state().logical_scroll_top());
    for _ in 0..20 {
        cx.background_executor()
            .timer(Duration::from_millis(30))
            .await;
        draw(handle, cx).await;
    }
    let top_b = transcript.read_with(cx, |t, _| t.list_state().logical_scroll_top());
    let stays = top_a.item_ix == top_b.item_ix && top_a.offset_in_item == top_b.offset_in_item;
    let hidden_while_streaming = !button(cx);
    // (b) 结束后显示按钮
    wait_idle(handle, cx).await;
    let shown_after = button(cx);
    // (c) 点按钮：平滑回到底部并恢复跟随
    transcript.update(cx, |t, cx| t.scroll_to_bottom(cx));
    for _ in 0..30 {
        cx.background_executor()
            .timer(Duration::from_millis(16))
            .await;
        draw(handle, cx).await;
    }
    let back = following(cx)
        && transcript.read_with(cx, |t, _| t.list_state().is_scrolled_to_end()) == Some(true)
        && !button(cx);
    // (d) 脱离状态下发送：v1 流式开始时重置为跟随
    wheel(handle, 400.0, cx).await;
    let detached_again = !following(cx);
    handle
        .update(cx, |p, _, cx| p.simulate_reply("再来一段", Vec::new(), cx))
        .unwrap();
    draw(handle, cx).await;
    let refollow = following(cx);
    wait_idle(handle, cx).await;
    println!(
        "T18: 静止时贴底 {at_rest}；流式中上滑脱离 {detached}、视口不动 {stays}、流式中无按钮 {hidden_while_streaming}；结束后显示按钮 {shown_after}；点按钮回底并跟随 {back}；再次脱离 {detached_again} 后发送恢复跟随 {refollow}"
    );
    let ok = at_rest
        && detached
        && stays
        && hidden_while_streaming
        && shown_after
        && back
        && detached_again
        && refollow;
    println!(
        "{} S05-04 T18 跟随 / 脱离 / 回到底部与 v1 一致",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

/// T21 回答操作栏（S05-14）：真实点击「复制」、「已复制」反馈与恢复、回到问题
async fn selftest_actions(handle: WindowHandle<ChatPreview>, cx: &mut AsyncApp) -> bool {
    use buddy_ui::gpui::{
        Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, point,
    };
    let transcript = handle.read_with(cx, |p, _| p.transcript.clone()).unwrap();
    wait_idle(handle, cx).await;
    handle
        .update(cx, |p, _, cx| {
            p.simulate_reply("请模拟一段流式回复", Vec::new(), cx)
        })
        .unwrap();
    wait_idle(handle, cx).await;
    transcript.update(cx, |t, cx| t.scroll_to_bottom(cx));
    for _ in 0..20 {
        cx.background_executor()
            .timer(Duration::from_millis(16))
            .await;
        draw(handle, cx).await;
    }
    let row_id = transcript
        .read_with(cx, |t, _| t.rows().last().map(|r| r.id.clone()))
        .unwrap_or_default();
    let bounds = transcript.read_with(cx, |t, _| t.painted_row_bounds(&row_id));
    let saved = cx.update(|cx| cx.read_from_clipboard());
    // 按 v1 版式定位「复制」按钮：行左 space-4 + 框边 1 + 框内左 space-2；行顶 space-1 + 框上边距 space-3 + 边 1 + 内 2 + 按钮半高 12
    let target = bounds.map(|b| {
        point(
            b.left() + px(16.0 + 1.0 + 8.0 + 20.0),
            b.top() + px(4.0 + 12.0 + 1.0 + 2.0 + 12.0),
        )
    });
    if let Some(p) = target {
        for e in [
            PlatformInput::MouseMove(MouseMoveEvent {
                position: p,
                pressed_button: None,
                modifiers: Modifiers::default(),
            }),
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: p,
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }),
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position: p,
                modifiers: Modifiers::default(),
                click_count: 1,
            }),
        ] {
            let _ = cx.update_window(handle.into(), |_, window, cx| window.dispatch_event(e, cx));
            draw(handle, cx).await;
        }
    }
    let copied_text = cx
        .update(|cx| cx.read_from_clipboard())
        .and_then(|c| c.text())
        .unwrap_or_default();
    let copied_state =
        transcript.read_with(cx, |t, _| t.copied_row_for_test() == Some(row_id.clone()));
    cx.background_executor()
        .timer(Duration::from_millis(1800))
        .await;
    draw(handle, cx).await;
    let restored = transcript.read_with(cx, |t, _| t.copied_row_for_test().is_none());
    if let Some(saved) = saved {
        cx.update(|cx| cx.write_to_clipboard(saved));
    }
    // 回到问题：取历史中段一轮（最后几轮离底部太近，列表会把滚动位置夹到末尾），其用户消息应滚到视口顶部
    let anchor = transcript.read_with(cx, |t, _| {
        let actions: Vec<_> = t
            .rows()
            .iter()
            .filter(|r| r.id.ends_with(".actions"))
            .collect();
        actions
            .get(actions.len() / 2)
            .map(|r| r.id.split('#').next().unwrap_or_default().to_string())
            .unwrap_or_default()
    });
    transcript.update(cx, |t, cx| t.scroll_to_row(&anchor, cx));
    draw(handle, cx).await;
    let top_is_question = transcript.read_with(cx, |t, _| {
        let ix = t.rows().iter().position(|r| r.id == anchor);
        ix == Some(t.list_state().logical_scroll_top().item_ix)
    });
    let ok = row_id.ends_with(".actions")
        && copied_text == STREAM_REPLY.trim()
        && copied_state
        && restored
        && top_is_question;
    println!(
        "T21: 操作栏行 {row_id}；复制得到 {} 字（与回答一致 {}）；「已复制」{copied_state} → 1.6 秒后恢复 {restored}；回到问题 {top_is_question}",
        copied_text.chars().count(),
        copied_text == STREAM_REPLY.trim()
    );
    println!(
        "{} S05-14 T21 回答操作栏（复制 / 反馈 / 回到问题）",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

/// T20 思考块与工具卡片（S05-09 / S05-10）
async fn selftest_tools(handle: WindowHandle<ChatPreview>, cx: &mut AsyncApp) -> bool {
    use buddy_ui::chat::rows::RowKind;
    let transcript = handle.read_with(cx, |p, _| p.transcript.clone()).unwrap();
    wait_idle(handle, cx).await;
    handle
        .update(cx, |p, _, cx| {
            p.simulate_tools(Duration::from_millis(1500), cx)
        })
        .unwrap();
    // 等到两个工具进入「执行中」
    // 只有已渲染的行才有边界：先把该行滚入视口、画一帧再量
    async fn row_height(
        transcript: &Entity<Transcript>,
        handle: WindowHandle<ChatPreview>,
        id: &str,
        cx: &mut AsyncApp,
    ) -> Option<f32> {
        let ix =
            transcript.read_with(cx, |t, _| t.rows().iter().rposition(|r| r.id.ends_with(id)))?;
        // 贴底列表对末尾几行的 bounds_for_item 恒为 None → 读取行的实际绘制边界
        let row_id = transcript.read_with(cx, |t, _| t.rows()[ix].id.clone());
        draw(handle, cx).await;
        draw(handle, cx).await;
        transcript.read_with(cx, |t, _| {
            t.painted_row_bounds(&row_id)
                .map(|b| f32::from(b.size.height))
        })
    }
    let mut executing_h = None;
    for _ in 0..80 {
        cx.background_executor()
            .timer(Duration::from_millis(25))
            .await;
        draw(handle, cx).await;
        let executing = handle
            .read_with(cx, |p, cx| {
                p.conversation
                    .read(cx)
                    .state
                    .tools
                    .get("call-read")
                    .map(|t| t.status == buddy_ui::chat::state::ToolStatus::Executing)
            })
            .unwrap();
        if executing == Some(true) {
            draw(handle, cx).await;
            executing_h = row_height(&transcript, handle, ".t.call-read", cx).await;
            break;
        }
    }
    let kinds = transcript.read_with(cx, |t, cx| {
        let state = &t.conversation_state(cx);
        t.rows()
            .iter()
            .filter_map(|r| match &r.kind {
                RowKind::Block { msg, block, .. } => state.messages[*msg]
                    .blocks
                    .as_ref()
                    .and_then(|b| b.get(*block))
                    .map(|b| matches!(b, buddy_engine::streaming::ContentBlock::Thinking { .. }))
                    .filter(|t| *t)
                    .map(|_| "think"),
                RowKind::Tool { .. } => Some("tool"),
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    wait_idle(handle, cx).await;
    draw(handle, cx).await;
    let done_h = row_height(&transcript, handle, ".t.call-read", cx).await;
    // 执行中默认展开（有详情）→ 完成后收起：高度应明显变小
    let collapsed_after_done = executing_h.zip(done_h).is_some_and(|(e, d)| e > d + 40.0);
    // 用户点开：行重测、高度变大
    let (id, before) = transcript.read_with(cx, |t, _| {
        let row = t
            .rows()
            .iter()
            .rev()
            .find(|r| r.id.ends_with(".t.call-read"))
            .unwrap();
        (row.id.clone(), t.remeasured_rows)
    });
    transcript.update(cx, |t, cx| t.toggle_tool_for_test(&id, cx));
    draw(handle, cx).await;
    draw(handle, cx).await;
    let expanded_h = row_height(&transcript, handle, ".t.call-read", cx).await;
    let remeasured = transcript.read_with(cx, |t, _| t.remeasured_rows) - before;
    let expands = expanded_h.zip(done_h).is_some_and(|(e, d)| e > d + 100.0) && remeasured >= 1;
    // 滚轮落在展开的长结果上：先滚卡片内部，列表不动
    transcript.update(cx, |t, _| {
        let ix = t.rows().iter().position(|r| r.id == id).unwrap();
        t.list_state().scroll_to(buddy_ui::gpui::ListOffset {
            item_ix: ix,
            offset_in_item: px(0.0),
        })
    });
    draw(handle, cx).await;
    let detail_center = transcript.read_with(cx, |t, _| {
        t.detail_scroll_for_test(&format!("{id}#result"))
            .map(|h| h.bounds().center())
    });
    let list_before = transcript.read_with(cx, |t, _| t.list_state().logical_scroll_top());
    let inner_before = transcript.read_with(cx, |t, _| {
        t.detail_scroll_for_test(&format!("{id}#result"))
            .map(|h| h.offset().y)
    });
    if let Some(p) = detail_center {
        use buddy_ui::gpui::{
            Modifiers, MouseMoveEvent, ScrollDelta, ScrollWheelEvent, TouchPhase, point,
        };
        for e in [
            PlatformInput::MouseMove(MouseMoveEvent {
                position: p,
                pressed_button: None,
                modifiers: Modifiers::default(),
            }),
            PlatformInput::ScrollWheel(ScrollWheelEvent {
                position: p,
                delta: ScrollDelta::Pixels(point(px(0.0), px(-60.0))),
                modifiers: Modifiers::default(),
                touch_phase: TouchPhase::Moved,
            }),
        ] {
            let _ = cx.update_window(handle.into(), |_, window, cx| window.dispatch_event(e, cx));
            draw(handle, cx).await;
        }
    }
    let list_after = transcript.read_with(cx, |t, _| t.list_state().logical_scroll_top());
    let inner_after = transcript.read_with(cx, |t, _| {
        t.detail_scroll_for_test(&format!("{id}#result"))
            .map(|h| h.offset().y)
    });
    let inner_first = list_before.item_ix == list_after.item_ix
        && list_before.offset_in_item == list_after.offset_in_item
        && inner_before.zip(inner_after).is_some_and(|(b, a)| a < b);
    println!(
        "T20: 行类型 {kinds:?}；执行中高 {executing_h:?} → 完成后 {done_h:?}（自动收起 {collapsed_after_done}）；点开后 {expanded_h:?}、重测 {remeasured} 行；卡内滚动 {inner_before:?} → {inner_after:?}，列表不动 {}",
        list_before.item_ix == list_after.item_ix
    );
    let ok = kinds.contains(&"think")
        && kinds.iter().filter(|k| **k == "tool").count() == 2
        && collapsed_after_done
        && expands
        && inner_first;
    println!(
        "{} S05-09 / S05-10 T20 思考块与工具卡片（展开规则、行重测、卡内滚动优先）",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

/// T16 回车三态 / T17 组字选区（缺陷 4 回归）
async fn selftest_keyboard(handle: WindowHandle<ChatPreview>, cx: &mut AsyncApp) -> (bool, bool) {
    // 等上一段流式结束，输入区回到可发送状态
    while handle
        .read_with(cx, |p, cx| p.conversation.read(cx).state.is_streaming())
        .unwrap()
    {
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
    }
    let area = handle
        .read_with(cx, |p, cx| p.composer.read(cx).text_area().clone())
        .unwrap();
    let focus = area.read_with(cx, |a, cx| a.focus_handle(cx));
    let _ = cx.update_window(handle.into(), |_, window, cx| window.focus(&focus, cx));
    draw(handle, cx).await;
    let sent = |cx: &mut AsyncApp| handle.read_with(cx, |p, _| p.sent.len()).unwrap();
    let text = |cx: &mut AsyncApp| area.read_with(cx, |a, _| a.text().to_string());
    let with_input = |f: Box<
        dyn FnOnce(
            &mut buddy_ui::text_area::TextArea,
            &mut Window,
            &mut Context<buddy_ui::text_area::TextArea>,
        ),
    >,
                      cx: &mut AsyncApp| {
        let area = area.clone();
        let _ = cx.update_window(handle.into(), move |_, window, cx| {
            area.update(cx, |a, cx| f(a, window, cx))
        });
    };
    let base = sent(cx);

    // 1) 组字中（拼音 "ni" 为标记文本）按 Enter：不发送
    with_input(
        Box::new(|a, w, cx| a.replace_and_mark_text_in_range(None, "ni", Some(2..2), w, cx)),
        cx,
    );
    press(handle, "enter", cx).await;
    let composing_blocked = sent(cx) == base && text(cx) == "ni";
    // 2) 上屏「你」，再按 Enter：发送 "你"
    with_input(
        Box::new(|a, w, cx| a.replace_text_in_range(None, "你", w, cx)),
        cx,
    );
    press(handle, "enter", cx).await;
    let enter_sends = sent(cx) == base + 1
        && handle
            .read_with(cx, |p, _| p.sent.last().cloned())
            .unwrap()
            .as_deref()
            == Some("你")
        && text(cx).is_empty();
    // 等模拟回复结束
    while handle
        .read_with(cx, |p, cx| p.conversation.read(cx).state.is_streaming())
        .unwrap()
    {
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
    }
    // 3) Cmd+Enter 换行、不发送
    with_input(
        Box::new(|a, w, cx| a.replace_text_in_range(None, "第一行", w, cx)),
        cx,
    );
    press(
        handle,
        &format!(
            "{}-enter",
            if cfg!(target_os = "macos") {
                "cmd"
            } else {
                "ctrl"
            }
        ),
        cx,
    )
    .await;
    let newline = sent(cx) == base + 1 && text(cx) == "第一行\n";
    // 4) Shift+Enter：v1 同样发送
    with_input(
        Box::new(|a, w, cx| a.replace_text_in_range(None, "第二行", w, cx)),
        cx,
    );
    press(handle, "shift-enter", cx).await;
    let shift_sends = sent(cx) == base + 2;
    println!(
        "T16: 组字中 Enter 不发送 {composing_blocked}；Enter 发送 {enter_sends}；Cmd+Enter 换行 {newline}；Shift+Enter 发送 {shift_sends}（v1 同）"
    );
    let t16 = composing_blocked && enter_sends && newline && shift_sends;
    println!(
        "{} S05-06 T16 回车三态与 v1 一致",
        if t16 { "PASS" } else { "FAIL" }
    );
    while handle
        .read_with(cx, |p, cx| p.conversation.read(cx).state.is_streaming())
        .unwrap()
    {
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
    }

    // T17：「你好」之后组字 "dian"，输入法给出相对标记文本的光标 1..1（在 d 之后）→ 正确为字节 6+1=7；
    // 按整段内容换算（官方示例 / S00-05 缺陷 4 根因）会得到 6+3=9 —— 仍在长度内，防御性 clamp 掩盖不了
    with_input(
        Box::new(|a, w, cx| {
            a.set_text("你好", cx);
            a.replace_and_mark_text_in_range(None, "dian", Some(1..1), w, cx);
        }),
        cx,
    );
    let (content, selection) = area.read_with(cx, |a, _| {
        (a.text().to_string(), a.selected_range_for_test())
    });
    press(
        handle,
        &format!(
            "{}-c",
            if cfg!(target_os = "macos") {
                "cmd"
            } else {
                "ctrl"
            }
        ),
        cx,
    )
    .await;
    let expected = "你好d".len();
    let t17 = content == "你好dian" && selection == (expected..expected);
    println!(
        "T17: 内容 {content:?}，选区 {selection:?}（期望 {expected}..{expected}）；组字中 Cmd+C 未崩溃"
    );
    println!(
        "{} S05-06 T17 组字选区按标记文本换算（S00-05 缺陷 4 回归）",
        if t17 { "PASS" } else { "FAIL" }
    );
    (t16, t17)
}

/// T31 网络搜索卡片（S05-11）：默认折叠、点开看来源、点链接打开、搜索中/完成的状态
async fn selftest_search(handle: WindowHandle<ChatPreview>, cx: &mut AsyncApp) -> bool {
    use buddy_ui::chat::state::ToolStatus;
    use buddy_ui::gpui::{
        Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, point,
    };
    let transcript = handle.read_with(cx, |p, _| p.transcript.clone()).unwrap();
    wait_idle(handle, cx).await;
    let opened: Rc<std::cell::RefCell<Vec<String>>> = Rc::default();
    let sink = opened.clone();
    transcript.update(cx, |t, _| {
        t.set_open_handler(Rc::new(move |url, _| {
            sink.borrow_mut().push(url.to_string())
        }))
    });
    handle
        .update(cx, |p, _, cx| {
            p.simulate_search(Duration::from_millis(1200), cx)
        })
        .unwrap();
    let status = |cx: &mut AsyncApp| {
        handle
            .read_with(cx, |p, cx| {
                p.conversation
                    .read(cx)
                    .state
                    .tools
                    .get("call-search")
                    .map(|t| t.status)
            })
            .unwrap()
    };
    let mut executing = false;
    for _ in 0..80 {
        cx.background_executor()
            .timer(Duration::from_millis(25))
            .await;
        draw(handle, cx).await;
        if status(cx) == Some(ToolStatus::Executing) {
            executing = true;
            break;
        }
    }
    let row_id = transcript
        .read_with(cx, |t, _| {
            t.rows()
                .iter()
                .rev()
                .find(|r| r.id.ends_with(".t.call-search"))
                .map(|r| r.id.clone())
        })
        .unwrap_or_default();
    let height = |cx: &mut AsyncApp| {
        transcript.read_with(cx, |t, _| {
            t.painted_row_bounds(&row_id)
                .map(|b| f32::from(b.size.height))
        })
    };
    for _ in 0..3 {
        draw(handle, cx).await;
    }
    let collapsed_running = height(cx);
    // 完成后仍是折叠的（v1 `useState(false)`）
    wait_idle(handle, cx).await;
    for _ in 0..3 {
        draw(handle, cx).await;
    }
    let collapsed_done = height(cx);
    let done = status(cx) == Some(ToolStatus::Done);
    // 点开：行重测、变高（三条来源）
    let before = transcript.read_with(cx, |t, _| t.remeasured_rows);
    transcript.update(cx, |t, cx| t.toggle_tool_for_test(&row_id, cx));
    for _ in 0..3 {
        draw(handle, cx).await;
    }
    let expanded_h = height(cx);
    let remeasured = transcript.read_with(cx, |t, _| t.remeasured_rows) - before;
    let expands = collapsed_done
        .zip(expanded_h)
        .is_some_and(|(c, e)| e > c + 150.0)
        && remeasured >= 1;
    // 点第一条来源的标题链接：在行内扫描（避开标题栏，点标题栏会折叠）
    let bounds = transcript.read_with(cx, |t, _| t.painted_row_bounds(&row_id));
    let mut hit = None;
    if let Some(b) = bounds {
        let (left, top) = (f32::from(b.left()), f32::from(b.top()));
        'scan: for dy in (90..260).step_by(6) {
            for dx in (110..520).step_by(18) {
                let p = point(px(left + dx as f32), px(top + dy as f32));
                for e in [
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position: p,
                        pressed_button: None,
                        modifiers: Modifiers::default(),
                    }),
                    PlatformInput::MouseDown(MouseDownEvent {
                        button: MouseButton::Left,
                        position: p,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                        first_mouse: false,
                    }),
                    PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position: p,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                    }),
                ] {
                    let _ = cx
                        .update_window(handle.into(), |_, window, cx| window.dispatch_event(e, cx));
                }
                draw(handle, cx).await;
                if !opened.borrow().is_empty() {
                    hit = Some((dx, dy));
                    break 'scan;
                }
            }
        }
    }
    let url_ok = opened.borrow().first().map(String::as_str) == Some("https://example.com/buddy");
    // 再点标题栏：收起（行的上边距 + 卡片外边距之下才是标题栏，自上而下扫描直到高度变化）
    if let Some(b) = bounds {
        for dy in (10..70).step_by(4) {
            let p = point(b.left() + px(200.0), b.top() + px(dy as f32));
            for e in [
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: p,
                    pressed_button: None,
                    modifiers: Modifiers::default(),
                }),
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position: p,
                    modifiers: Modifiers::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                PlatformInput::MouseUp(MouseUpEvent {
                    button: MouseButton::Left,
                    position: p,
                    modifiers: Modifiers::default(),
                    click_count: 1,
                }),
            ] {
                let _ =
                    cx.update_window(handle.into(), |_, window, cx| window.dispatch_event(e, cx));
                draw(handle, cx).await;
            }
            draw(handle, cx).await;
            if height(cx).is_some_and(|h| h < 200.0) {
                break;
            }
        }
    }
    for _ in 0..3 {
        draw(handle, cx).await;
    }
    let refolded_h = height(cx);
    let refolded = refolded_h
        .zip(collapsed_done)
        .is_some_and(|(h, c)| (h - c).abs() < 1.0);
    println!(
        "T31: 搜索中出现 {executing}；折叠高（搜索中 {collapsed_running:?} / 完成后 {collapsed_done:?}）；完成 {done}；点开后 {expanded_h:?}、重测 {remeasured} 行；点链接命中 {hit:?} → {:?}；再点标题栏收起 {refolded}（{refolded_h:?}）",
        opened.borrow()
    );
    let ok = executing
        && collapsed_running.is_some_and(|h| h < 100.0)
        && collapsed_done.is_some_and(|h| h < 100.0)
        && done
        && expands
        && url_ok
        && refolded;
    println!(
        "{} S05-11 T31 网络搜索卡片（默认折叠 / 点开 / 点链接打开 / 收起）",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

/// T32 图片生成卡片（S05-12）：本地图片、生成中 / 完成状态、真实点击展开与复制提示词。
async fn selftest_image(handle: WindowHandle<ChatPreview>, cx: &mut AsyncApp) -> bool {
    use buddy_ui::chat::state::ToolStatus;
    use buddy_ui::gpui::{
        Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, point,
    };
    let transcript = handle.read_with(cx, |p, _| p.transcript.clone()).unwrap();
    // Isolate the image interaction from the long transcript built by T11–T31;
    // otherwise the virtual list may keep the growing tool row below the small
    // self-test viewport while it is being remeasured.
    handle
        .update(cx, |p, _, cx| {
            p.conversation.update(cx, |conversation, cx| {
                conversation.state = buddy_ui::chat::state::ChatState::from_history(Vec::new());
                cx.notify();
            });
        })
        .unwrap();
    for _ in 0..4 {
        draw(handle, cx).await;
    }
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempt_counter = attempts.clone();
    transcript.update(cx, |t, _| {
        t.set_download_handler(Rc::new(move |image, cx| {
            let attempt = attempt_counter.fetch_add(1, Ordering::SeqCst);
            cx.background_spawn(async move {
                if attempt == 0 {
                    Err("模拟下载失败".to_string())
                } else {
                    Ok(format!("mock-download/{}", image.name))
                }
            })
        }));
    });
    wait_idle(handle, cx).await;
    handle
        .update(cx, |p, _, cx| {
            p.simulate_image(Duration::from_millis(1000), cx)
        })
        .unwrap();
    let status = |cx: &mut AsyncApp| {
        handle
            .read_with(cx, |p, cx| {
                p.conversation
                    .read(cx)
                    .state
                    .tools
                    .get("call-image")
                    .map(|t| t.status)
            })
            .unwrap()
    };
    let mut generating = false;
    for _ in 0..80 {
        cx.background_executor()
            .timer(Duration::from_millis(25))
            .await;
        draw(handle, cx).await;
        if status(cx) == Some(ToolStatus::Executing) {
            generating = true;
            break;
        }
    }
    let row_id = transcript
        .read_with(cx, |t, _| {
            t.rows()
                .iter()
                .rev()
                .find(|r| r.id.ends_with(".t.call-image"))
                .map(|r| r.id.clone())
        })
        .unwrap_or_default();
    let height = |cx: &mut AsyncApp| {
        transcript.read_with(cx, |t, _| {
            t.painted_row_bounds(&row_id)
                .map(|b| f32::from(b.size.height))
        })
    };
    for _ in 0..3 {
        draw(handle, cx).await;
    }
    let collapsed_running = height(cx);
    wait_idle(handle, cx).await;
    for _ in 0..12 {
        draw(handle, cx).await;
    }
    let done = status(cx) == Some(ToolStatus::Done);
    let attachment_ok = handle
        .read_with(cx, |p, cx| {
            p.conversation.read(cx).state.messages.iter().any(|m| {
                m.images
                    .iter()
                    .any(|i| i.id == "generated-preview" && !i.data_url.is_empty())
            })
        })
        .unwrap_or(false);
    // Keep the image tool row at the tail while exercising its controls; this
    // prevents the mock assistant's trailing sentence from pushing the gallery
    // below the small self-test viewport.
    handle
        .update(cx, |p, _, cx| {
            p.conversation.update(cx, |conversation, cx| {
                if let Some(index) = conversation.state.messages.iter().position(|message| {
                    message.role == MessageRole::Tool
                        && message.tool_call_id.as_deref() == Some("call-image")
                }) {
                    conversation.state.messages.truncate(index + 1);
                    conversation.state.revision += 1;
                    cx.notify();
                }
            });
        })
        .unwrap();
    transcript.update(cx, |t, cx| {
        t.stop_following_for_test(cx);
        t.scroll_to_bottom(cx);
    });
    for _ in 0..6 {
        draw(handle, cx).await;
    }
    let collapsed_done = height(cx);
    // Open the card first so the save control is laid out in its final gallery
    // position; this is still a real heading click dispatched to the window.
    if let Some(b) = transcript.read_with(cx, |t, _| t.painted_row_bounds(&row_id)) {
        'open_before_save: for dy in [26.0] {
            for dx in [80.0] {
                let p = point(b.left() + px(dx as f32), b.top() + px(dy as f32));
                for e in [
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position: p,
                        pressed_button: None,
                        modifiers: Modifiers::default(),
                    }),
                    PlatformInput::MouseDown(MouseDownEvent {
                        button: MouseButton::Left,
                        position: p,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                        first_mouse: false,
                    }),
                    PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position: p,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                    }),
                ] {
                    let _ = cx
                        .update_window(handle.into(), |_, window, cx| window.dispatch_event(e, cx));
                }
                draw(handle, cx).await;
                if height(cx)
                    .zip(collapsed_done)
                    .is_some_and(|(h, collapsed)| h > collapsed + 100.0)
                {
                    break 'open_before_save;
                }
            }
        }
    }
    // 真实点击图片右上角保存：第一次失败显示重试，第二次成功显示已下载。
    let mut download_failed = false;
    let mut download_succeeded = false;
    let mut download_point = None;
    if let Some(b) = transcript.read_with(cx, |t, _| t.painted_row_bounds(&row_id)) {
        // The button is top-right of the gallery image. Scan only that small
        // region so this remains a real pointer event without toggling the card
        // heading underneath it.
        'download: for dx in [128.0, 136.0, 120.0] {
            for dy in [88.0, 80.0, 96.0] {
                let p = point(b.right() - px(dx as f32), b.top() + px(dy as f32));
                for e in [
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position: p,
                        pressed_button: None,
                        modifiers: Modifiers::default(),
                    }),
                    PlatformInput::MouseDown(MouseDownEvent {
                        button: MouseButton::Left,
                        position: p,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                        first_mouse: false,
                    }),
                    PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position: p,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                    }),
                ] {
                    let _ = cx
                        .update_window(handle.into(), |_, window, cx| window.dispatch_event(e, cx));
                }
                draw(handle, cx).await;
                cx.background_executor()
                    .timer(Duration::from_millis(20))
                    .await;
                if transcript.read_with(cx, |t, _| t.download_state_for_test("generated-preview"))
                    == Some(buddy_ui::chat::image_gen_state::DownloadState::Error(
                        "模拟下载失败".into(),
                    ))
                {
                    download_failed = true;
                    download_point = Some(p);
                    break 'download;
                }
            }
        }
        if download_point.is_some() {
            // The failure footer changes the row height and its tail anchor.
            // Paint that state before locating the retry button.
            draw(handle, cx).await;
            let p = transcript
                .read_with(cx, |t, _| t.painted_row_bounds(&row_id))
                .map(|b| point(b.right() - px(128.0), b.top() + px(88.0)))
                .unwrap_or(point(px(496.0), px(245.5)));
            for e in [
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: p,
                    pressed_button: None,
                    modifiers: Modifiers::default(),
                }),
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position: p,
                    modifiers: Modifiers::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                PlatformInput::MouseUp(MouseUpEvent {
                    button: MouseButton::Left,
                    position: p,
                    modifiers: Modifiers::default(),
                    click_count: 1,
                }),
            ] {
                let _ =
                    cx.update_window(handle.into(), |_, window, cx| window.dispatch_event(e, cx));
                draw(handle, cx).await;
            }
            for _ in 0..20 {
                draw(handle, cx).await;
                if matches!(
                    transcript.read_with(cx, |t, _| t.download_state_for_test("generated-preview")),
                    Some(buddy_ui::chat::image_gen_state::DownloadState::Saved(_))
                ) {
                    download_succeeded = true;
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(20))
                    .await;
            }
        }
    }
    // Resource 失败→修复文件→真实点击“重试”：验证 GPUI 图片缓存确实被清掉并重新加载。
    let retry_path = std::env::temp_dir().join(format!(
        "buddy-image-retry-{}-{}.png",
        std::process::id(),
        buddy_ui::chat::state::unique_suffix()
    ));
    let retry_path_string = retry_path.to_string_lossy().into_owned();
    let retry_image_id = format!(
        "generated-preview-retry-{}",
        buddy_ui::chat::state::unique_suffix()
    );
    let original_data_url = handle
        .read_with(cx, |p, cx| {
            p.conversation
                .read(cx)
                .state
                .messages
                .iter()
                .find_map(|message| {
                    (message.role == MessageRole::Tool
                        && message.tool_call_id.as_deref() == Some("call-image"))
                    .then(|| message.images.first().map(|image| image.data_url.clone()))
                    .flatten()
                })
        })
        .unwrap_or_default()
        .unwrap_or_default();
    let _ = std::fs::remove_file(&retry_path);
    handle
        .update(cx, |p, _, cx| {
            p.conversation.update(cx, |conversation, cx| {
                if let Some(message) = conversation.state.messages.iter_mut().find(|message| {
                    message.role == MessageRole::Tool
                        && message.tool_call_id.as_deref() == Some("call-image")
                }) {
                    if let Some(image) = message.images.first_mut() {
                        image.id = retry_image_id.clone();
                        image.path = retry_path_string.clone();
                        image.data_url.clear();
                    }
                }
                if let Some(tool) = conversation.state.tools.get_mut("call-image") {
                    tool.images = vec![retry_image_id.clone()];
                }
                conversation.state.revision += 1;
                cx.notify();
            });
        })
        .unwrap();
    transcript.update(cx, |t, cx| {
        t.stop_following_for_test(cx);
        t.scroll_to_row(&row_id, cx);
    });
    for _ in 0..12 {
        draw(handle, cx).await;
    }
    transcript.update(cx, |t, cx| t.scroll_to_row(&row_id, cx));
    for _ in 0..6 {
        draw(handle, cx).await;
    }
    let mut failed_resource_cached = false;
    for _ in 0..80 {
        draw(handle, cx).await;
        failed_resource_cached = transcript.read_with(cx, |t, _| {
            matches!(t.image_load_state_for_test(&retry_image_id), Some(buddy_ui::chat::image_gen_state::ImageLoadState::Failed))
        });
        if failed_resource_cached { break; }
        cx.background_executor().timer(Duration::from_millis(20)).await;
    }
    transcript.update(cx, |t, cx| {
        t.list_state().set_follow_mode(buddy_ui::gpui::FollowMode::Tail);
        t.list_state().scroll_to_end();
        cx.notify();
    });
    cx.background_executor().timer(Duration::from_millis(30)).await;
    for _ in 0..6 { draw(handle, cx).await; }
    let failed_height = height(cx);
    let remeasures_before = transcript.read_with(cx, |t, _| t.remeasured_rows);
    let icon_path = std::env::current_dir()
        .unwrap_or_default()
        .join("src-tauri/icons/icon.png");
    let repaired = std::fs::read(&icon_path)
        .and_then(|bytes| std::fs::write(&retry_path, bytes))
        .is_ok();
    let mut retry_clicked = false;
    if let Some(b) = transcript.read_with(cx, |t, _| t.painted_row_bounds(&row_id)) {
        'retry: for dy in [173.0, 181.0, 165.0] {
            for dx in [318.0, 326.0, 310.0] {
                let p = point(b.left() + px(dx as f32), b.top() + px(dy as f32));
                for e in [
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position: p,
                        pressed_button: None,
                        modifiers: Modifiers::default(),
                    }),
                    PlatformInput::MouseDown(MouseDownEvent {
                        button: MouseButton::Left,
                        position: p,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                        first_mouse: false,
                    }),
                    PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position: p,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                    }),
                ] {
                    let _ = cx
                        .update_window(handle.into(), |_, window, cx| window.dispatch_event(e, cx));
                    draw(handle, cx).await;
                }
                if transcript.read_with(cx, |t, _| t.retry_state_for_test(&retry_image_id)) {
                    retry_clicked = true;
                    break 'retry;
                }
            }
        }
    }
    let mut reloaded_size = None;
    for _ in 0..20 {
        draw(handle, cx).await;
        let load_state =
            transcript.read_with(cx, |t, _| t.image_load_state_for_test(&retry_image_id));
        if let Some(buddy_ui::chat::image_gen_state::ImageLoadState::Loaded { width, height }) =
            load_state
        {
            reloaded_size = Some((width, height));
            break;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    cx.background_executor().timer(Duration::from_millis(30)).await;
    for _ in 0..3 { draw(handle, cx).await; }
    let loaded_height = height(cx);
    let async_remeasured = transcript.read_with(cx, |t, _| t.remeasured_rows > remeasures_before);
    let height_updated = loaded_height.zip(failed_height).is_some_and(|(loaded, failed)| loaded > failed + 100.0);
    let tail_preserved = transcript.read_with(cx, |t, _| t.list_state().is_following_tail() && (t.list_state().max_offset_for_scrollbar().y - t.list_state().scroll_px_offset_for_scrollbar().y.abs()).abs() < px(1.0));
    let retry_reloaded = retry_clicked
        && repaired
        && reloaded_size.is_some_and(|(width, height)| width > 0 && height > 0);
    handle
        .update(cx, |p, _, cx| {
            p.conversation.update(cx, |conversation, cx| {
                if let Some(message) = conversation.state.messages.iter_mut().find(|message| {
                    message.role == MessageRole::Tool
                        && message.tool_call_id.as_deref() == Some("call-image")
                }) {
                    if let Some(image) = message.images.first_mut() {
                        image.id = "generated-preview".into();
                        image.path.clear();
                        image.data_url = original_data_url.clone();
                    }
                }
                if let Some(tool) = conversation.state.tools.get_mut("call-image") {
                    tool.images = vec!["generated-preview".into()];
                }
                conversation.state.revision += 1;
                cx.notify();
            });
        })
        .unwrap();
    let _ = std::fs::remove_file(&retry_path);
    let bounds = transcript.read_with(cx, |t, _| t.painted_row_bounds(&row_id));
    let mut expanded_by_click = height(cx)
        .zip(collapsed_done)
        .is_some_and(|(expanded, collapsed)| expanded > collapsed + 100.0);
    if let Some(b) = bounds {
        'expand: for dy in [26.0] {
            if expanded_by_click {
                break 'expand;
            }
            for dx in [80.0] {
                let p = point(b.left() + px(dx as f32), b.top() + px(dy as f32));
                for e in [
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position: p,
                        pressed_button: None,
                        modifiers: Modifiers::default(),
                    }),
                    PlatformInput::MouseDown(MouseDownEvent {
                        button: MouseButton::Left,
                        position: p,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                        first_mouse: false,
                    }),
                    PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position: p,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                    }),
                ] {
                    let _ = cx
                        .update_window(handle.into(), |_, window, cx| window.dispatch_event(e, cx));
                    draw(handle, cx).await;
                }
                for _ in 0..3 {
                    draw(handle, cx).await;
                }
                expanded_by_click = height(cx)
                    .zip(collapsed_done)
                    .is_some_and(|(expanded, collapsed)| expanded > collapsed + 100.0);
                if expanded_by_click {
                    break 'expand;
                }
            }
        }
    }
    let expanded_h = height(cx);
    let copied_before = cx.update(|cx| cx.read_from_clipboard());
    // 展开详情中的复制按钮：扫描卡片底部，仍使用真实鼠标事件。
    let mut copied = false;
    if let Some(b) = transcript.read_with(cx, |t, _| t.painted_row_bounds(&row_id)) {
        'scan: for dy in [88.0, 96.0, 80.0, 104.0, 72.0] {
            if !expanded_by_click { break; }
            for dx in [224.0, 232.0, 216.0, 240.0, 208.0] {
                let p = point(b.left() + px(dx), b.bottom() - px(dy));
                for e in [
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position: p,
                        pressed_button: None,
                        modifiers: Modifiers::default(),
                    }),
                    PlatformInput::MouseDown(MouseDownEvent {
                        button: MouseButton::Left,
                        position: p,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                        first_mouse: false,
                    }),
                    PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position: p,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                    }),
                ] {
                    let _ = cx
                        .update_window(handle.into(), |_, window, cx| window.dispatch_event(e, cx));
                }
                draw(handle, cx).await;
                copied = cx
                    .update(|cx| cx.read_from_clipboard())
                    .and_then(|clip| clip.text())
                    .is_some_and(|text| text == "蓝色星空下的一只猫，数字插画");
                if copied {
                    break 'scan;
                }
            }
        }
    }
    if let Some(clip) = copied_before {
        cx.update(|cx| cx.write_to_clipboard(clip));
    }
    let copied_feedback =
        transcript.read_with(cx, |t, _| t.copy_state_for_test(&format!("image-{row_id}")));
    cx.background_executor()
        .timer(Duration::from_millis(1700))
        .await;
    draw(handle, cx).await;
    let copied_feedback_reset =
        !transcript.read_with(cx, |t, _| t.copy_state_for_test(&format!("image-{row_id}")));
    let mut refolded = false;
    let mut refolded_h = None;
    if let Some(b) = transcript.read_with(cx, |t, _| t.painted_row_bounds(&row_id)) {
        'collapse: for dy in [26.0] {
            for dx in [80.0] {
                let p = point(b.left() + px(dx as f32), b.top() + px(dy as f32));
                for e in [
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position: p,
                        pressed_button: None,
                        modifiers: Modifiers::default(),
                    }),
                    PlatformInput::MouseDown(MouseDownEvent {
                        button: MouseButton::Left,
                        position: p,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                        first_mouse: false,
                    }),
                    PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position: p,
                        modifiers: Modifiers::default(),
                        click_count: 1,
                    }),
                ] {
                    let _ = cx
                        .update_window(handle.into(), |_, window, cx| window.dispatch_event(e, cx));
                    draw(handle, cx).await;
                }
                for _ in 0..3 {
                    draw(handle, cx).await;
                }
                refolded_h = height(cx);
                if refolded_h
                    .zip(collapsed_done)
                    .is_some_and(|(collapsed, expected)| (collapsed - expected).abs() < 1.0)
                {
                    refolded = true;
                    break 'collapse;
                }
            }
        }
    }
    println!(
        "T32: 生成中 {generating}；完成 {done}；图片数据源 {attachment_ok}；下载失败 {download_failed} → 重试成功 {download_succeeded}；图片加载失败缓存 {failed_resource_cached} → 修复后真实重试 {retry_reloaded}；异步重测 {async_remeasured}、行高更新 {height_updated}（{failed_height:?}→{loaded_height:?}）、Tail 保持 {tail_preserved}；折叠高（生成中 {collapsed_running:?} / 完成 {collapsed_done:?}）；真实点击展开 {expanded_by_click}（{expanded_h:?}）；收起 {refolded}（{refolded_h:?}）；复制提示词 {copied}、反馈 {copied_feedback}、1.6s 后恢复 {copied_feedback_reset}"
    );
    let ok = generating
        && done
        && attachment_ok
        && collapsed_running.is_some_and(|h| h < 100.0)
        && expanded_by_click
        && download_failed
        && download_succeeded
        && failed_resource_cached
        && retry_reloaded
        && async_remeasured
        && height_updated
        && tail_preserved
        && refolded
        && copied
        && copied_feedback
        && copied_feedback_reset;
    println!(
        "{} S05-12 T32 图片生成卡片（本地图片 / 状态 / 真实展开 / 复制提示词）",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

/// T33 附件草稿：真实剪贴板图片经 `window.dispatch_event` 进入 Composer，随后验证
/// 上限、缩略图草稿跨页面实体保留，以及视觉模型不支持图片时不会发送。
async fn selftest_attachments(handle: WindowHandle<ChatPreview>, cx: &mut AsyncApp) -> bool {
    use buddy_ui::chat::attachments::{decode_data_url, draft_from_bytes};
    let composer = handle.read_with(cx, |p, _| p.composer.clone()).unwrap();
    let previous_clipboard = cx.update(|cx| cx.read_from_clipboard());
    let png = decode_data_url(
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
        "image/png",
    )
    .expect("valid one-pixel PNG");
    let image = Image::from_bytes(ImageFormat::Png, png.clone());
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_image(&image)));
    let clipboard_entries = cx.update(|cx| {
        cx.read_from_clipboard()
            .map(|item| item.entries().len())
            .unwrap_or(0)
    });
    let focus = composer.read_with(cx, |c, cx| c.focus_handle(cx));
    let _ = cx.update_window(handle.into(), |_, window, cx| window.focus(&focus, cx));
    draw(handle, cx).await;
    let paste = PlatformInput::KeyDown(KeyDownEvent {
        keystroke: Keystroke::parse("cmd-v").expect("cmd-v"),
        is_held: false,
        prefer_character_input: false,
    });
    let _ = cx.update_window(handle.into(), |_, window, cx| {
        window.dispatch_event(paste, cx)
    });
    draw(handle, cx).await;
    let pasted = composer.read_with(cx, |c, _| c.image_count() == 1);

    let mut extra = Vec::new();
    for i in 0..3 {
        extra.push(draft_from_bytes(format!("extra-{i}.png"), "image/png", png.clone()).unwrap());
    }
    composer.update(cx, |c, cx| c.add_images(extra, cx));
    draw(handle, cx).await;
    let capped = composer.read_with(cx, |c, _| c.image_count() == 4);
    let fifth = draft_from_bytes("fifth.png", "image/png", png).unwrap();
    composer.update(cx, |c, cx| c.add_images(vec![fifth], cx));
    let fifth_rejected = composer.read_with(cx, |c, _| {
        c.image_count() == 4 && c.attachment_error().is_some()
    });
    let sent_before = handle.read_with(cx, |p, _| p.sent.len()).unwrap_or_default();
    composer.update(cx, |c, cx| c.set_supports_vision(false, cx));
    composer.update(cx, |c, cx| c.set_draft("带图文本", cx));
    press(handle, "enter", cx).await;
    let blocked = handle
        .read_with(cx, |p, _| p.sent.len() == sent_before)
        .unwrap_or(false);
    composer.update(cx, |c, cx| c.set_supports_vision(true, cx));
    composer.update(cx, |c, cx| c.set_draft("", cx));
    let draft_kept_before_send = composer.read_with(cx, |c, _| c.image_count() == 4);
    press(handle, "enter", cx).await;
    let image_only = handle
        .read_with(cx, |p, _| p.sent.len() == sent_before + 1 && p.sent.last().is_some_and(String::is_empty))
        .unwrap_or(false);
    let draft_cleared_after_send = composer.read_with(cx, |c, _| c.image_count() == 0);
    let sent_user_images = handle
        .read_with(cx, |p, cx| {
            p.conversation
                .read(cx)
                .state
                .messages
                .iter()
                .rev()
                .find(|message| message.role == MessageRole::User)
                .is_some_and(|message| message.images.len() == 4)
        })
        .unwrap_or(false);
    if let Some(previous_clipboard) = previous_clipboard {
        cx.update(|cx| cx.write_to_clipboard(previous_clipboard));
    }
    let ok = pasted
        && capped
        && fifth_rejected
        && blocked
        && image_only
        && draft_kept_before_send
        && draft_cleared_after_send
        && sent_user_images;
    println!(
        "T33: 剪贴板条目 {clipboard_entries}，图片 {pasted}；4 张上限 {capped}；第 5 张拒绝 {fifth_rejected}；不支持视觉模型阻止发送 {blocked}；纯图片发送 {image_only}；发送前草稿保留 {draft_kept_before_send}；发送后草稿清空 {draft_cleared_after_send}；用户消息带 4 张图 {sent_user_images}"
    );
    println!(
        "{} S05-07 T33 附件草稿 / 限制 / 视觉模型校验",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

/// T22：历史分页 —— 触顶加载更早一页，可见内容不移动
async fn selftest_paging(cx: &mut AsyncApp) -> bool {
    let gate = Rc::new(Cell::new(false));
    // 矮窗口：最新一页（10 条）须超过一屏，才能真正滚到「距顶 ≤56px」而不是整页贴底
    let bounds = cx.update(|cx| Bounds::centered(None, size(px(560.0), px(460.0)), cx));
    let g = gate.clone();
    let handle = cx
        .update(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    let conversation =
                        cx.new(|_| paged_conversation(history(5001), Duration::ZERO, Some(g)));
                    let transcript = cx.new(|cx| Transcript::new(conversation.clone(), cx));
                    let engine = ChatEngine::new(
                        std::env::temp_dir()
                            .join(format!("buddy-chat-preview-paging-{}", std::process::id())),
                    );
                    cx.new(|cx| ChatPreview::new(conversation, transcript, engine, window, cx))
                },
            )
        })
        .expect("open_window 失败");
    let (transcript, conversation) = handle
        .read_with(cx, |p, _| (p.transcript.clone(), p.conversation.clone()))
        .unwrap();
    for _ in 0..3 {
        draw(handle, cx).await;
    }
    let paging = |cx: &mut AsyncApp| conversation.read_with(cx, |c, _| c.state.history);
    let initial = paging(cx);
    let first_row = transcript
        .read_with(cx, |t, _| t.rows().first().map(|r| r.id.clone()))
        .unwrap_or_default();
    // 真实滚轮滚到顶：距顶 ≤56px 触发加载，读取被闸门挂起
    wheel(handle, 4000.0, cx).await;
    let during = paging(cx);
    let top_before = transcript.read_with(cx, |t, _| t.list_state().logical_scroll_top());
    let top_id_before = transcript
        .read_with(cx, |t, _| {
            t.rows().get(top_before.item_ix).map(|r| r.id.clone())
        })
        .unwrap_or_default();
    // 可见的一条用户消息（行 id 不随并入而变）的屏幕位置
    let probe = transcript.read_with(cx, |t, _| {
        t.rows()
            .iter()
            .filter(|r| matches!(r.kind, buddy_ui::chat::rows::RowKind::User { .. }))
            .find_map(|r| {
                t.painted_row_bounds(&r.id)
                    .map(|b| (r.id.clone(), b.origin.y))
            })
    });
    let rows_before = transcript.read_with(cx, |t, _| t.rows().len());
    gate.set(true);
    for _ in 0..20 {
        cx.background_executor()
            .timer(Duration::from_millis(16))
            .await;
        draw(handle, cx).await;
    }
    let after = paging(cx);
    let top_after = transcript.read_with(cx, |t, _| t.list_state().logical_scroll_top());
    let top_id_after = transcript
        .read_with(cx, |t, _| {
            t.rows().get(top_after.item_ix).map(|r| r.id.clone())
        })
        .unwrap_or_default();
    let probe_after = probe.as_ref().and_then(|(id, _)| {
        transcript.read_with(cx, |t, _| t.painted_row_bounds(id).map(|b| b.origin.y))
    });
    let rows_after = transcript.read_with(cx, |t, _| t.rows().len());
    let at_end = transcript.read_with(cx, |t, _| {
        t.list_state().logical_scroll_top().item_ix >= t.rows().len()
    });
    println!("T22: 加载后仍贴底 {at_end}（须 false：否则没有真正滚动过）");
    println!(
        "T22: 初始 offset {} 更早 {}，首行 {first_row}；触顶后加载中 {}；首行 {top_id_before} + {:?} → {top_id_after} + {:?}；探针 {:?} → {probe_after:?}；行 {rows_before} → {rows_after}；offset {} → {}",
        initial.offset,
        initial.has_more,
        during.loading,
        top_before.offset_in_item,
        top_after.offset_in_item,
        probe,
        during.offset,
        after.offset
    );
    let ok = initial.offset == 4991
        && initial.has_more
        && first_row.starts_with("head-a4991#")
        && during.loading
        && !after.loading
        && after.offset == 4981
        && rows_after > rows_before
        && top_id_before.starts_with("head-a4991#")
        && !top_id_after.starts_with("head-")
        // 边界行 a4991（回答 u4990）并入前锚为 head，并入后改锚为 u4990：视口首行应仍是它
        && top_id_after == top_id_before.replacen("head-a4991", "u4990", 1)
        && top_before.offset_in_item == top_after.offset_in_item
        && probe.as_ref().map(|(_, y)| *y) == probe_after
        && probe.is_some()
        && !at_end;
    let _ = cx.update_window(handle.into(), |_, window, _| window.remove_window());
    println!(
        "{} S05-05 T22 触顶加载更早历史，可见内容不移动",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

fn selftest(handle: WindowHandle<ChatPreview>, cx: &mut App) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        let transcript = handle.read_with(cx, |p, _| p.transcript.clone()).unwrap();
        let before_first = transcript.read_with(cx, |t, _| t.rendered_rows);
        // 首帧（measure_all 会布局全部行一次，S00-07 已知的一次性成本）
        let first = draw(handle, cx).await;
        let laid_out_first = transcript.read_with(cx, |t, _| t.rendered_rows) - before_first;
        println!("T11: 启动到首帧完成 {:.0} ms；首帧前已布局 {before_first} 行，首帧布局 {laid_out_first} 行", LAUNCH.get().unwrap().elapsed().as_secs_f64() * 1000.0);
        let total_rows = transcript.read_with(cx, |t, _| t.rows().len());
        let at_end = transcript.read_with(cx, |t, _| t.list_state().is_scrolled_to_end());
        println!("T11: 共 {total_rows} 行；首帧 {first:.0} ms；贴底 {at_end:?}");

        // ── T11：滚动 60 帧，统计每帧布局的行数 ──
        let before = transcript.read_with(cx, |t, _| t.rendered_rows);
        for _ in 0..60 {
            transcript.update(cx, |t, _| t.list_state().scroll_by(px(-120.0)));
            draw(handle, cx).await;
        }
        let per_frame = (transcript.read_with(cx, |t, _| t.rendered_rows) - before) as f64 / 60.0;
        let t11 = at_end == Some(true) && per_frame > 0.0 && per_frame < 40.0 && (total_rows as f64) > per_frame * 20.0;
        println!("T11: 滚动 60 帧，平均每帧布局 {per_frame:.1} 行（总行数 {total_rows}）");
        println!("{} S05-01 T11 虚拟化：每帧只布局可见行", if t11 { "PASS" } else { "FAIL" });

        // ── T13：1000 条消息时的重绘耗时 ──
        let mut samples = Vec::new();
        for _ in 0..30 {
            samples.push(draw(handle, cx).await);
            cx.background_executor().timer(Duration::from_millis(5)).await;
        }
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = samples[samples.len() / 2];
        let t13 = median < 50.0;
        println!("T13: 整窗重绘中位 {median:.2} ms（预算 50 ms）");
        println!("{} S05-01 T13 长列表重绘耗时在预算内", if t13 { "PASS" } else { "FAIL" });

        // ── T14：视口上方的行高变化不移动可见内容（S05-03） ──
        transcript.update(cx, |t, _| t.list_state().scroll_to(buddy_ui::gpui::ListOffset { item_ix: total_rows / 2, offset_in_item: px(10.0) }));
        draw(handle, cx).await;
        let top_before = transcript.read_with(cx, |t, _| t.list_state().logical_scroll_top());
        let (screen_before, height_before, target_above) = transcript.read_with(cx, |t, _| {
            let l = t.list_state();
            (l.bounds_for_item(top_before.item_ix + 1).map(|b| b.origin.y), l.max_offset_for_scrollbar().y, l.item_is_above_viewport(top_before.item_ix - 10))
        });
        let remeasured_before = transcript.read_with(cx, |t, _| t.remeasured_rows);
        // 把视口上方第 10 行所在的助手消息改长（模拟图片加载 / 展开等造成的高度变化）
        let target = handle
            .update(cx, |p, _, cx| {
                let rows = p.transcript.read(cx).rows().to_vec();
                let row = rows[top_before.item_ix - 10].clone();
                p.conversation.update(cx, |c, cx| {
                    let msg = match row.kind {
                        buddy_ui::chat::rows::RowKind::Block { msg, .. }
                        | buddy_ui::chat::rows::RowKind::User { msg }
                        | buddy_ui::chat::rows::RowKind::Actions { msg } => msg,
                        _ => unreachable!(),
                    };
                    let longer = format!("{}\n\n{}", c.state.messages[msg].content, ANSWERS[4].repeat(3));
                    c.state.messages[msg].content = longer.clone();
                    if let Some(blocks) = c.state.messages[msg].blocks.as_mut() {
                        blocks[0] = ContentBlock::Text { content: longer };
                    }
                    c.state.revision += 1;
                    cx.notify();
                });
                row.id
            })
            .unwrap();
        draw(handle, cx).await;
        draw(handle, cx).await;
        let top_after = transcript.read_with(cx, |t, _| t.list_state().logical_scroll_top());
        let (screen_after, height_after) = transcript.read_with(cx, |t, _| {
            let l = t.list_state();
            (l.bounds_for_item(top_before.item_ix + 1).map(|b| b.origin.y), l.max_offset_for_scrollbar().y)
        });
        let remeasured = transcript.read_with(cx, |t, _| t.remeasured_rows) - remeasured_before;
        println!("T14: 视口内某行屏幕位置 {screen_before:?} → {screen_after:?}；内容总高（可滚动量）{height_before:?} → {height_after:?}（反证：须增加）；改动行在视口上方 {target_above:?}");
        let t14 = top_before.item_ix == top_after.item_ix
            && top_before.offset_in_item == top_after.offset_in_item
            && screen_before.is_some()
            && screen_before == screen_after
            && height_after > height_before
            && target_above == Some(true)
            && remeasured == 1;
        println!("T14: 改动视口上方的行 {target}（重测 {remeasured} 行）；视口首行 {} + {:?} → {} + {:?}", top_before.item_ix, top_before.offset_in_item, top_after.item_ix, top_after.offset_in_item);
        println!("{} S05-03 T14 视口上方行高变化不移动可见内容", if t14 { "PASS" } else { "FAIL" });

        // ── T15：用户消息保留换行 ──
        transcript.update(cx, |t, _| t.list_state().scroll_to(buddy_ui::gpui::ListOffset { item_ix: 0, offset_in_item: px(0.0) }));
        draw(handle, cx).await;
        draw(handle, cx).await;
        let (h_multi, h_single) = transcript.read_with(cx, |t, _| {
            let l = t.list_state();
            (l.bounds_for_item(0).map(|b| f32::from(b.size.height)), l.bounds_for_item(2).map(|b| f32::from(b.size.height)))
        });
        let extra = h_multi.zip(h_single).map(|(a, b)| a - b);
        let t15 = extra.is_some_and(|d| (d - 2.0 * metrics::FONT_SIZE_MD * 1.5).abs() < 4.0);
        println!("T15: 三行用户消息高 {h_multi:?}，单行 {h_single:?}，差 {extra:?}（期望约 2 × 21px）");
        println!("{} S05-08 T15 用户消息按换行分行（v1 pre-wrap）", if t15 { "PASS" } else { "FAIL" });

        // ── T12：流式只重测一行 ──
        transcript.update(cx, |t, _| t.list_state().scroll_to_end());
        handle.update(cx, |p, _, cx| p.simulate_reply("请模拟一段流式回复", Vec::new(), cx)).unwrap();
        draw(handle, cx).await;
        let start_rows = transcript.read_with(cx, |t, _| t.rows().len());
        transcript.update(cx, |t, _| t.max_remeasured_per_sync = 0);
        let (mut syncs, mut prev_remeasured) = (0usize, transcript.read_with(cx, |t, _| t.remeasured_rows));
        let mut row_count_stable = true;
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut first_text_seen = false;
        loop {
            cx.background_executor().timer(Duration::from_millis(16)).await;
            draw(handle, cx).await;
            let (remeasured, rows) = transcript.read_with(cx, |t, _| (t.remeasured_rows, t.rows().len()));
            let streaming = handle.read_with(cx, |p, cx| p.conversation.read(cx).state.is_streaming()).unwrap();
            // 第一批正文到达时「占位行 → 正文行」替换一次，之后行数应保持不变
            if rows == start_rows && !first_text_seen {
                first_text_seen = true;
            } else if first_text_seen && rows != start_rows && streaming {
                row_count_stable = false;
            }
            if remeasured > prev_remeasured {
                syncs += 1;
                prev_remeasured = remeasured;
            }
            if !streaming || Instant::now() > deadline {
                break;
            }
        }
        // 单次同步的最大重测行数由列表自己记录（按帧采样可能把相邻两次同步并成一次）
        let max_per_sync = transcript.read_with(cx, |t, _| t.max_remeasured_per_sync);
        let t12 = syncs > 5 && max_per_sync == 1 && row_count_stable;
        println!("T12: 流式期间行同步 {syncs} 次（按帧采样），单次最多重测 {max_per_sync} 行；行数稳定 {row_count_stable}");
        println!("{} S05-01 T12 流式只重测最后一行", if t12 { "PASS" } else { "FAIL" });
        let (t16, t17) = selftest_keyboard(handle, cx).await;
        let t18 = selftest_follow(handle, cx).await;
        let t19 = selftest_wheel(handle, cx).await;
        let t20 = selftest_tools(handle, cx).await;
        let t21 = selftest_actions(handle, cx).await;
        let t22 = selftest_paging(cx).await;
        let t31 = selftest_search(handle, cx).await;
        let t32 = selftest_image(handle, cx).await;
        let t33 = selftest_attachments(handle, cx).await;
        std::process::exit(if t11 && t12 && t13 && t14 && t15 && t16 && t17 && t18 && t19 && t20 && t21 && t22 && t31 && t32 && t33 { 0 } else { 1 });
    })
    .detach();
}

static LAUNCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

fn main() {
    LAUNCH.get_or_init(Instant::now);
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().collect();
    let self_test = args.iter().any(|a| a == "--selftest");
    let self_test_image = args.iter().any(|a| a == "--selftest-image");
    let self_test_attachments = args.iter().any(|a| a == "--selftest-attachments");
    let count = args
        .iter()
        .position(|a| a == "--messages")
        .and_then(|i| args.get(i + 1))
        .and_then(|n| n.parse().ok())
        .unwrap_or(1000);
    application()
        .with_assets(buddy_ui::icons::Assets)
        .run(move |cx: &mut App| {
            buddy_ui::init_theme(cx);
            Theme::install(Appearance::Light, cx);
            fonts::install_text_rendering(cx);
            markdown::init(cx);
            buddy_ui::chat::init(cx);
            buddy_ui::chat_bridge::init(cx);
            let preview_engine = ChatEngine::new(
                std::env::temp_dir().join(format!("buddy-chat-preview-{}", std::process::id())),
            );
            let bounds = Bounds::centered(None, size(px(560.0), px(760.0)), cx);
            let handle = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        ..Default::default()
                    },
                    |window, cx| {
                        let conversation = cx.new(|_| {
                            if self_test || self_test_image || self_test_attachments {
                                Conversation::new(history(count))
                            } else {
                                paged_conversation(history(count), Duration::from_millis(150), None)
                            }
                        });
                        let transcript = cx.new(|cx| Transcript::new(conversation.clone(), cx));
                        cx.new(|cx| {
                            ChatPreview::new(
                                conversation,
                                transcript,
                                preview_engine.clone(),
                                window,
                                cx,
                            )
                        })
                    },
                )
                .expect("open_window 失败");
            if self_test {
                selftest(handle, cx);
            } else if self_test_image {
                cx.spawn(async move |cx: &mut AsyncApp| {
                    let ok = selftest_image(handle, cx).await;
                    std::process::exit(if ok { 0 } else { 1 });
                })
                .detach();
            } else if self_test_attachments {
                cx.spawn(async move |cx: &mut AsyncApp| {
                    let ok = selftest_attachments(handle, cx).await;
                    std::process::exit(if ok { 0 } else { 1 });
                })
                .detach();
            }
        });
}
