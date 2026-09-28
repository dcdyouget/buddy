//! Phase 05 聊天界面预览与自检。
//!
//! ```text
//! cargo run -p buddy-app --example chat_preview                          # 目检：1000 条历史 + 模拟流式回复
//! cargo run -p buddy-app --example chat_preview -- --messages 5000       # 指定历史条数
//! cargo run -p buddy-app --example chat_preview -- --selftest            # 自检后退出
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
//! - T15 用户消息保留换行（S05-08，v1 `white-space: pre-wrap`）。

use buddy_ui::chat::{
    composer::{Composer, ComposerEvent},
    session::Conversation,
    transcript::Transcript,
};
use buddy_ui::gpui::{EntityInputHandler, Focusable, KeyDownEvent, Keystroke, PlatformInput};
use std::cell::Cell;
use std::rc::Rc;
use buddy_ui::gpui::{
    App, AppContext, AsyncApp, Bounds, Context, Entity, Render, Window, WindowBounds, WindowHandle, WindowOptions, div, prelude::*, px, size,
};
use buddy_ui::gpui_platform::application;
use buddy_ui::markdown;
use buddy_ui::theme_system::{Appearance, BuddyTheme, Theme, fonts, set_appearance, tokens::metrics};
use buddy_engine::models::{Message, MessageRole};
use buddy_engine::streaming::{ContentBlock, StopReason, StreamEvent};
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
        blocks: (role == MessageRole::Assistant).then(|| vec![ContentBlock::Text { content: text.into() }]),
        model_id: None,
        created_at: 0,
        tool_calls: None,
        tool_call_id: None,
        tool_name: None,
        is_error: None,
        parent_message_id: None,
    }
}

fn history(count: usize) -> Vec<Message> {
    (0..count)
        .map(|i| {
            if i == 0 {
                // T15：多行用户消息（v1 `white-space: pre-wrap` 保留换行）
                message("u0".into(), MessageRole::User, "第一行\n第二行\n第三行")
            } else if i % 2 == 0 {
                message(format!("u{i}"), MessageRole::User, &format!("第 {} 个问题：请解释一下这个概念？", i / 2 + 1))
            } else {
                message(format!("a{i}"), MessageRole::Assistant, ANSWERS[(i / 2) % ANSWERS.len()])
            }
        })
        .collect()
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
    fn new(conversation: Entity<Conversation>, transcript: Entity<Transcript>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![
            // v1：窗口失焦 / 隐藏时立即放出缓冲，重新聚焦后追赶再恢复逐字
            cx.observe_window_activation(window, |this: &mut Self, window, cx| {
                let active = window.is_window_active();
                this.conversation.update(cx, |c, cx| if active { c.window_shown(cx) } else { c.window_hidden(cx) });
            }),
            // 流式状态同步到输入区（流式中显示「生成中」+ 停止按钮）
            cx.observe_in(&conversation, window, |this: &mut Self, conversation, window, cx| {
                let streaming = conversation.read(cx).state.is_streaming();
                this.composer.update(cx, |c, cx| c.set_streaming(streaming, Some("mock-model".into()), window, cx));
                cx.notify();
            }),
        ];
        let composer = cx.new(|cx| Composer::new(window, cx));
        let mut subscriptions = subscriptions;
        subscriptions.push(cx.subscribe(&composer, |this: &mut Self, composer, event: &ComposerEvent, cx| match event {
            ComposerEvent::Send(text) => {
                this.sent.push(text.clone());
                composer.update(cx, |c, cx| c.set_draft("", cx));
                let text = text.clone();
                this.simulate_reply(&text, cx);
            }
            // v1：停止 → engine 发出 Error(Aborted) → 按正常结束处理
            ComposerEvent::Stop => this.stop.set(true),
            ComposerEvent::OpenSettings | ComposerEvent::PickModel => {}
        }));
        window.focus(&composer.focus_handle(cx), cx);
        Self { conversation, transcript, composer, sent: Vec::new(), stop: Rc::default(), _subscriptions: subscriptions }
    }

    /// 模拟一次出错：发送后先到一段正文，再收到网络错误（v1：已显示正文保留、提示条显示错误）
    fn simulate_error(&mut self, cx: &mut Context<Self>) {
        let user = message(format!("u-err-{}", buddy_ui::chat::state::unique_suffix()), MessageRole::User, "请模拟一次出错");
        self.conversation.update(cx, |c, cx| {
            c.begin_send(user, "mock-model", cx);
            c.apply_events(
                vec![
                    StreamEvent::TextStart { content_index: 0 },
                    StreamEvent::TextDelta { content_index: 0, delta: "这是出错前已经生成的一段内容。".into() },
                    StreamEvent::Error { reason: StopReason::Error, message: "网络错误：连接被重置（模拟）".into(), partial_text: String::new() },
                ],
                cx,
            );
        });
    }

    /// 模拟一次流式回复：固定种子切片，20–120ms 间隔到达
    fn simulate_reply(&mut self, question: &str, cx: &mut Context<Self>) {
        let conversation = self.conversation.clone();
        let user = message(format!("u-live-{}", buddy_ui::chat::state::unique_suffix()), MessageRole::User, question);
        conversation.update(cx, |c, cx| c.begin_send(user, "mock-model", cx));
        let stop = self.stop.clone();
        stop.set(false);
        cx.spawn(async move |_, cx: &mut AsyncApp| {
            let mut seed: u64 = 7;
            let chars: Vec<char> = STREAM_REPLY.chars().collect();
            let mut i = 0;
            let _ = conversation.update(cx, |c, cx| c.apply_events(vec![StreamEvent::Start, StreamEvent::TextStart { content_index: 0 }], cx));
            while i < chars.len() {
                if stop.get() {
                    let _ = conversation.update(cx, |c, cx| {
                        c.apply_events(vec![StreamEvent::Error { reason: StopReason::Aborted, message: "用户取消".into(), partial_text: String::new() }], cx)
                    });
                    return;
                }
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let n = (1 + (seed >> 33) % 8) as usize;
                let piece: String = chars[i..(i + n).min(chars.len())].iter().collect();
                i += n;
                let _ = conversation.update(cx, |c, cx| c.apply_events(vec![StreamEvent::TextDelta { content_index: 0, delta: piece }], cx));
                cx.background_executor().timer(Duration::from_millis(20 + (seed >> 40) % 100)).await;
            }
            let _ = conversation.update(cx, |c, cx| {
                c.apply_events(
                    vec![
                        StreamEvent::TextEnd { content_index: 0, content: STREAM_REPLY.into() },
                        StreamEvent::TurnEnd { tool_calls_pending: 0 },
                        StreamEvent::Done { reason: StopReason::Stop, full_text: STREAM_REPLY.into() },
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
                    .child(button("toggle", if next == Appearance::Dark { "切换到深色" } else { "切换到浅色" }).on_click(move |_, _, cx| set_appearance(next, cx)))
                    .when(!streaming, |d| {
                        d.child(button("reply", "模拟流式回复").on_click(cx.listener(|this, _, _, cx| this.simulate_reply("请模拟一段流式回复", cx))))
                            .child(button("error", "模拟出错").on_click(cx.listener(|this, _, _, cx| this.simulate_error(cx))))
                    })
                    .child(div().text_color(theme.colors.text_muted).child(format!("{} 行", self.transcript.read(cx).rows().len()))),
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
    let key = PlatformInput::KeyDown(KeyDownEvent { keystroke: Keystroke::parse(keys).expect("按键"), is_held: false, prefer_character_input: false });
    let _ = cx.update_window(handle.into(), |_, window, cx| window.dispatch_event(key, cx));
    draw(handle, cx).await;
}

/// T16 回车三态 / T17 组字选区（缺陷 4 回归）
async fn selftest_keyboard(handle: WindowHandle<ChatPreview>, cx: &mut AsyncApp) -> (bool, bool) {
    // 等上一段流式结束，输入区回到可发送状态
    while handle.read_with(cx, |p, cx| p.conversation.read(cx).state.is_streaming()).unwrap() {
        cx.background_executor().timer(Duration::from_millis(50)).await;
    }
    let area = handle.read_with(cx, |p, cx| p.composer.read(cx).text_area().clone()).unwrap();
    let focus = area.read_with(cx, |a, cx| a.focus_handle(cx));
    let _ = cx.update_window(handle.into(), |_, window, cx| window.focus(&focus, cx));
    draw(handle, cx).await;
    let sent = |cx: &mut AsyncApp| handle.read_with(cx, |p, _| p.sent.len()).unwrap();
    let text = |cx: &mut AsyncApp| area.read_with(cx, |a, _| a.text().to_string());
    let with_input = |f: Box<dyn FnOnce(&mut buddy_ui::text_area::TextArea, &mut Window, &mut Context<buddy_ui::text_area::TextArea>)>, cx: &mut AsyncApp| {
        let area = area.clone();
        let _ = cx.update_window(handle.into(), move |_, window, cx| area.update(cx, |a, cx| f(a, window, cx)));
    };
    let base = sent(cx);

    // 1) 组字中（拼音 "ni" 为标记文本）按 Enter：不发送
    with_input(Box::new(|a, w, cx| a.replace_and_mark_text_in_range(None, "ni", Some(2..2), w, cx)), cx);
    press(handle, "enter", cx).await;
    let composing_blocked = sent(cx) == base && text(cx) == "ni";
    // 2) 上屏「你」，再按 Enter：发送 "你"
    with_input(Box::new(|a, w, cx| a.replace_text_in_range(None, "你", w, cx)), cx);
    press(handle, "enter", cx).await;
    let enter_sends = sent(cx) == base + 1 && handle.read_with(cx, |p, _| p.sent.last().cloned()).unwrap().as_deref() == Some("你") && text(cx).is_empty();
    // 等模拟回复结束
    while handle.read_with(cx, |p, cx| p.conversation.read(cx).state.is_streaming()).unwrap() {
        cx.background_executor().timer(Duration::from_millis(50)).await;
    }
    // 3) Cmd+Enter 换行、不发送
    with_input(Box::new(|a, w, cx| a.replace_text_in_range(None, "第一行", w, cx)), cx);
    press(handle, &format!("{}-enter", if cfg!(target_os = "macos") { "cmd" } else { "ctrl" }), cx).await;
    let newline = sent(cx) == base + 1 && text(cx) == "第一行\n";
    // 4) Shift+Enter：v1 同样发送
    with_input(Box::new(|a, w, cx| a.replace_text_in_range(None, "第二行", w, cx)), cx);
    press(handle, "shift-enter", cx).await;
    let shift_sends = sent(cx) == base + 2;
    println!("T16: 组字中 Enter 不发送 {composing_blocked}；Enter 发送 {enter_sends}；Cmd+Enter 换行 {newline}；Shift+Enter 发送 {shift_sends}（v1 同）");
    let t16 = composing_blocked && enter_sends && newline && shift_sends;
    println!("{} S05-06 T16 回车三态与 v1 一致", if t16 { "PASS" } else { "FAIL" });
    while handle.read_with(cx, |p, cx| p.conversation.read(cx).state.is_streaming()).unwrap() {
        cx.background_executor().timer(Duration::from_millis(50)).await;
    }

    // T17：「你好」之后组字 "dian"，输入法给出相对标记文本的光标 1..1（在 d 之后）→ 正确为字节 6+1=7；
    // 按整段内容换算（官方示例 / S00-05 缺陷 4 根因）会得到 6+3=9 —— 仍在长度内，防御性 clamp 掩盖不了
    with_input(Box::new(|a, w, cx| {
        a.set_text("你好", cx);
        a.replace_and_mark_text_in_range(None, "dian", Some(1..1), w, cx);
    }), cx);
    let (content, selection) = area.read_with(cx, |a, _| (a.text().to_string(), a.selected_range_for_test()));
    press(handle, &format!("{}-c", if cfg!(target_os = "macos") { "cmd" } else { "ctrl" }), cx).await;
    let expected = "你好d".len();
    let t17 = content == "你好dian" && selection == (expected..expected);
    println!("T17: 内容 {content:?}，选区 {selection:?}（期望 {expected}..{expected}）；组字中 Cmd+C 未崩溃");
    println!("{} S05-06 T17 组字选区按标记文本换算（S00-05 缺陷 4 回归）", if t17 { "PASS" } else { "FAIL" });
    (t16, t17)
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
                        buddy_ui::chat::rows::RowKind::Block { msg, .. } | buddy_ui::chat::rows::RowKind::User { msg } => msg,
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
        handle.update(cx, |p, _, cx| p.simulate_reply("请模拟一段流式回复", cx)).unwrap();
        draw(handle, cx).await;
        let start_rows = transcript.read_with(cx, |t, _| t.rows().len());
        let (mut max_per_sync, mut syncs, mut prev_remeasured) = (0usize, 0usize, transcript.read_with(cx, |t, _| t.remeasured_rows));
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
                max_per_sync = max_per_sync.max(remeasured - prev_remeasured);
                prev_remeasured = remeasured;
            }
            if !streaming || Instant::now() > deadline {
                break;
            }
        }
        let t12 = syncs > 5 && max_per_sync == 1 && row_count_stable;
        println!("T12: 流式期间行同步 {syncs} 次（按帧采样），单次最多重测 {max_per_sync} 行；行数稳定 {row_count_stable}");
        println!("{} S05-01 T12 流式只重测最后一行", if t12 { "PASS" } else { "FAIL" });
        let (t16, t17) = selftest_keyboard(handle, cx).await;
        std::process::exit(if t11 && t12 && t13 && t14 && t15 && t16 && t17 { 0 } else { 1 });
    })
    .detach();
}

static LAUNCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

fn main() {
    LAUNCH.get_or_init(Instant::now);
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().collect();
    let self_test = args.iter().any(|a| a == "--selftest");
    let count = args.iter().position(|a| a == "--messages").and_then(|i| args.get(i + 1)).and_then(|n| n.parse().ok()).unwrap_or(1000);
    application().with_assets(buddy_ui::icons::Assets).run(move |cx: &mut App| {
        buddy_ui::init_theme(cx);
        Theme::install(Appearance::Light, cx);
        fonts::install_text_rendering(cx);
        markdown::init(cx);
        buddy_ui::chat::init(cx);
        let bounds = Bounds::centered(None, size(px(560.0), px(760.0)), cx);
        let handle = cx
            .open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, |window, cx| {
                let conversation = cx.new(|_| Conversation::new(history(count)));
                let transcript = cx.new(|cx| Transcript::new(conversation.clone(), cx));
                cx.new(|cx| ChatPreview::new(conversation, transcript, window, cx))
            })
            .expect("open_window 失败");
        if self_test {
            selftest(handle, cx);
        }
    });
}
