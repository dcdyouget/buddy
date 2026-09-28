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
//! - T14 视口上方行高变化（S05-03）：可见内容不移动（视口首行与行内偏移不变），且只重测那一行。

use buddy_ui::chat::{session::Conversation, transcript::Transcript};
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
            if i % 2 == 0 {
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
}

impl ChatPreview {
    /// 模拟一次流式回复：固定种子切片，20–120ms 间隔到达
    fn simulate_reply(&mut self, cx: &mut Context<Self>) {
        let conversation = self.conversation.clone();
        let user = message(format!("u-live-{}", buddy_ui::chat::state::unique_suffix()), MessageRole::User, "请模拟一段流式回复");
        conversation.update(cx, |c, cx| c.begin_send(user, "mock-model", cx));
        cx.spawn(async move |_, cx: &mut AsyncApp| {
            let mut seed: u64 = 7;
            let chars: Vec<char> = STREAM_REPLY.chars().collect();
            let mut i = 0;
            let _ = conversation.update(cx, |c, cx| c.apply_events(vec![StreamEvent::Start, StreamEvent::TextStart { content_index: 0 }], cx));
            while i < chars.len() {
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
                    .when(!streaming, |d| d.child(button("reply", "模拟流式回复").on_click(cx.listener(|this, _, _, cx| this.simulate_reply(cx)))))
                    .child(div().text_color(theme.colors.text_muted).child(format!("{} 行", self.transcript.read(cx).rows().len()))),
            )
            .child(div().flex_1().min_h_0().child(self.transcript.clone()))
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

        // ── T12：流式只重测一行 ──
        transcript.update(cx, |t, _| t.list_state().scroll_to_end());
        handle.update(cx, |p, _, cx| p.simulate_reply(cx)).unwrap();
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
        std::process::exit(if t11 && t12 && t13 && t14 { 0 } else { 1 });
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
        let bounds = Bounds::centered(None, size(px(560.0), px(760.0)), cx);
        let handle = cx
            .open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, |_, cx| {
                let conversation = cx.new(|_| Conversation::new(history(count)));
                let transcript = cx.new(|cx| Transcript::new(conversation.clone(), cx));
                cx.new(|_| ChatPreview { conversation, transcript })
            })
            .expect("open_window 失败");
        if self_test {
            selftest(handle, cx);
        }
    });
}
