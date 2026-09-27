//! S04-06 流式渐显预览与自检。
//!
//! ```text
//! cargo run -p buddy-app --example streaming_preview                  # 目检：模拟网络流式输出
//! cargo run -p buddy-app --example streaming_preview -- --selftest    # 自检后退出
//! ```
//!
//! 模拟网络：按固定种子把样例切成 1–12 字的片段，间隔 20–150ms 到达，经真实的 `Pacer`
//! （v1 节奏）放出，每批重新规范化（S04-05）并替换 markdown，尾段按 v1 规则落定、星标呼吸。
//! 窗口失焦时立即放出全部缓冲，重新聚焦后 600ms 追赶再恢复逐字（v1 同）。
//!
//! 自检：
//! - T08 渐显不改变布局：同一文档「全部字符处于落定起点 + 行内星标」与「无效果」的高度完全相同；
//!   反证：加上单独成行的星标后高度必须变化（证明探针确实测到布局）。
//! - T09 端到端：流式结束后 markdown 源文本 == 规范化后的完整样例，星标消失、无落定字符。

use buddy_ui::gpui::{
    App, AppContext, AsyncApp, Bounds, Context, Entity, Pixels, Render, ScrollHandle, Window, WindowBounds,
    WindowHandle, WindowOptions, canvas, div, prelude::*, px, size,
};
use buddy_ui::gpui_platform::application;
use buddy_ui::markdown::{
    self, code_block,
    normalize::normalize_markdown,
    streaming::{self, Pacer, Star, Tail},
    zed_markdown::{Markdown, MarkdownElement, MarkdownOverlay, MarkdownVeil, syntax::LanguageRegistry},
};
use buddy_ui::theme_system::{Appearance, BuddyTheme, Theme, fonts, set_appearance, tokens::metrics};
use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const SAMPLE: &str = r#"好的，下面分三部分说明 **信息检索（IR）**的基本流程。

## 一、索引

检索系统先对文档建立**倒排索引**：把每个词映射到包含它的文档列表。常见步骤：

1. 分词与归一化（大小写、全半角）
2. 去停用词
3. 记录词频与位置

## 二、查询

```python
def search(index, query):
    terms = tokenize(query)
    return rank(intersect(index[t] for t in terms))
```

| 方法 | 特点 |
|------|------|
| BM25 | 经典、稳定 |
| 向量检索 | 语义相近即可命中 |

## 三、排序

最后按相关性打分排序，并可结合点击反馈持续优化。需要的话我可以再展开讲 `BM25` 的公式推导。
"#;

/// 模拟网络片段：(到达时刻 ms, 文本)。线性同余生成器，结果固定可复现。
fn arrivals(text: &str) -> VecDeque<(f64, String)> {
    let mut seed: u64 = 0x5eed_b0dd;
    let mut next = |lo: u64, hi: u64| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        lo + (seed >> 33) % (hi - lo + 1)
    };
    let chars: Vec<char> = text.chars().collect();
    let (mut out, mut at, mut i) = (VecDeque::new(), 300.0, 0);
    while i < chars.len() {
        let n = (next(1, 12) as usize).min(chars.len() - i);
        out.push_back((at, chars[i..i + n].iter().collect()));
        i += n;
        at += next(20, 150) as f64;
    }
    out
}

/// T08 的测量模式
#[derive(Clone, Copy, PartialEq)]
enum Probe {
    /// 正常流式
    Off,
    /// 完整文档、无任何效果
    Plain,
    /// 完整文档、全部字符处于落定起点 + 行内星标
    Veiled,
    /// 完整文档 + 单独成行的星标（反证）
    Standalone,
}

struct StreamPreview {
    md: Entity<Markdown>,
    started: Instant,
    arrivals: VecDeque<(f64, String)>,
    pacer: Pacer,
    raw: String,
    batch_at: f64,
    reveal_count: usize,
    scroll: ScrollHandle,
    probe: Probe,
    height: Rc<Cell<Option<Pixels>>>,
    last_tail: Option<Tail>,
    /// T08：veil 查询次数、overlay 构建次数（证明效果确实作用于渲染）
    hook_calls: Arc<(AtomicUsize, AtomicUsize)>,
    _activation: Option<buddy_ui::gpui::Subscription>,
}

impl StreamPreview {
    fn new(md: Entity<Markdown>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // v1：窗口失焦 / 隐藏时立即放出缓冲，重新聚焦后 600ms 追赶
        let activation = cx.observe_window_activation(window, |this: &mut Self, window, cx| {
            let now = this.now();
            let batch = if window.is_window_active() { this.pacer.show(now) } else { this.pacer.hide() };
            if let Some(batch) = batch {
                this.apply(batch, now, cx);
            }
        });
        Self {
            md,
            started: Instant::now(),
            arrivals: arrivals(SAMPLE),
            pacer: Pacer::new(),
            raw: String::new(),
            batch_at: 0.0,
            reveal_count: 0,
            scroll: ScrollHandle::new(),
            probe: Probe::Off,
            height: Rc::default(),
            last_tail: None,
            hook_calls: Arc::default(),
            _activation: Some(activation),
        }
    }

    fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1000.0
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
        self.started = Instant::now();
        self.arrivals = arrivals(SAMPLE);
        self.pacer = Pacer::new();
        self.raw.clear();
        self.reveal_count = 0;
        self.md.update(cx, |m, cx| m.replace("", cx));
    }

    fn apply(&mut self, batch: String, now: f64, cx: &mut Context<Self>) {
        self.reveal_count = batch.chars().count();
        self.raw.push_str(&batch);
        self.batch_at = now;
        let source = normalize_markdown(&self.raw);
        self.md.update(cx, |m, cx| m.replace(source, cx));
    }

    fn streaming(&self) -> bool {
        !self.arrivals.is_empty() || !self.pacer.pending().is_empty()
    }
}

impl Render for StreamPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.buddy_theme();
        let reduce = buddy_ui::accessibility::prefers_reduced_motion();
        let now = self.now();
        let mut style = markdown::message_style(window, cx);
        let standalone;
        let streaming;

        if self.probe == Probe::Off {
            while self.arrivals.front().is_some_and(|(at, _)| *at <= now) {
                let (_, chunk) = self.arrivals.pop_front().unwrap();
                if let Some(batch) = self.pacer.push(&chunk, now) {
                    self.apply(batch, now, cx);
                }
            }
            if let Some(batch) = self.pacer.tick(now) {
                self.apply(batch, now, cx);
            }
            streaming = self.streaming();
            let tail = streaming::tail(&normalize_markdown(&self.raw), streaming, self.reveal_count);
            let animating = streaming::decorate(&mut style, &tail, now - self.batch_at, &theme, reduce);
            standalone = tail.star == Star::Standalone;
            self.last_tail = Some(tail);
            if streaming || animating {
                window.request_animation_frame();
                self.scroll.scroll_to_bottom();
            }
        } else {
            streaming = false;
            let len = self.md.read(cx).source().len();
            if self.probe == Probe::Veiled {
                let (calls_a, calls_b) = (self.hook_calls.clone(), self.hook_calls.clone());
                style.decorations.veil = Some(MarkdownVeil {
                    start_color: theme.colors.streaming_star_white.into(),
                    start_opacity: streaming::SETTLE_START_OPACITY,
                    progress: Arc::new(move |_| {
                        calls_a.0.fetch_add(1, Ordering::Relaxed);
                        Some(0.0)
                    }),
                });
                style.decorations.overlay = Some(MarkdownOverlay {
                    source_index: len.saturating_sub(1),
                    build: Arc::new(move |h, _, _| {
                        calls_b.1.fetch_add(1, Ordering::Relaxed);
                        div().h(h).flex().items_center().child(streaming::star_element(&theme, 0.0, false)).into_any_element()
                    }),
                });
            }
            standalone = self.probe == Probe::Standalone;
        }

        let md = self.md.clone();
        let height = self.height.clone();
        let next = match theme.appearance {
            Appearance::Light => Appearance::Dark,
            Appearance::Dark => Appearance::Light,
        };
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
                    .m(px(metrics::SPACE_3))
                    .child(button("toggle", if next == Appearance::Dark { "切换到深色" } else { "切换到浅色" }).on_click(move |_, _, cx| set_appearance(next, cx)))
                    .child(button("replay", "重新播放").on_click(cx.listener(|this, _, _, cx| this.restart(cx))))
                    .child(div().text_color(theme.colors.text_muted).child(if streaming { "输出中…" } else { "已结束" })),
            )
            .child(
                div().id("scroll").flex_1().overflow_y_scroll().track_scroll(&self.scroll).px(px(metrics::SPACE_4)).pb(px(metrics::SPACE_6)).child(
                    div()
                        .relative()
                        .child(
                            MarkdownElement::new(md.clone(), style)
                                .code_block_renderer(code_block::renderer(md.downgrade(), streaming))
                                .on_url_click(|url, _, cx| markdown::gfm::open_link(&url, cx)),
                        )
                        .when(standalone, |d| d.child(streaming::star_element(&theme, now - self.batch_at, reduce)))
                        // T08 探针：记录消息内容的实际高度
                        .child(canvas(move |bounds: Bounds<Pixels>, _, _| height.set(Some(bounds.size.height)), |_, _, _, _| {}).absolute().size_full()),
                ),
            )
    }
}

async fn draw_frames(handle: WindowHandle<StreamPreview>, n: usize, cx: &mut AsyncApp) {
    for _ in 0..n {
        let _ = cx.update_window(handle.into(), |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
        cx.background_executor().timer(Duration::from_millis(16)).await;
    }
}

async fn wait_parsed(md: &Entity<Markdown>, cx: &mut AsyncApp) {
    while md.read_with(cx, |m, _| m.is_parsing()) {
        cx.background_executor().timer(Duration::from_millis(2)).await;
    }
}

fn selftest(handle: WindowHandle<StreamPreview>, cx: &mut App) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        // ── T09：端到端流式 ──
        let deadline = Instant::now() + Duration::from_secs(60);
        let (mut max_settle, mut saw_inline_star, mut saw_standalone) = (0, false, false);
        loop {
            draw_frames(handle, 1, cx).await;
            if let Some(t) = handle.read_with(cx, |p, _| p.last_tail.clone()).unwrap() {
                max_settle = max_settle.max(t.settle.len());
                saw_inline_star |= matches!(t.star, Star::After(_));
                saw_standalone |= t.star == Star::Standalone;
            }
            let done = handle.read_with(cx, |p, _| !p.streaming()).unwrap();
            if done || Instant::now() > deadline {
                break;
            }
        }
        let md = handle.read_with(cx, |p, _| p.md.clone()).unwrap();
        wait_parsed(&md, cx).await;
        draw_frames(handle, 3, cx).await;
        let want = normalize_markdown(SAMPLE);
        let got = md.read_with(cx, |m, _| m.source().to_string());
        let tail = handle.read_with(cx, |p, _| p.last_tail.clone()).unwrap();
        println!("T09: 流式中最多落定字符 {max_settle}，出现过行内星标 {saw_inline_star}、单独星标 {saw_standalone}");
        let t09 = got == want
            && tail.as_ref().is_some_and(|t| t.star == Star::Hidden && t.settle.is_empty())
            && max_settle == streaming::SETTLE_TRAIL_LENGTH
            && saw_inline_star
            && saw_standalone;
        println!("T09: 源文本 {} 字节（期望 {}），结束时星标 {:?}", got.len(), want.len(), tail.map(|t| t.star));
        println!("{} S04-06 T09 流式结束后文本完整、星标与落定效果消失", if t09 { "PASS" } else { "FAIL" });

        // ── T08：布局不变 ──
        let mut heights = Vec::new();
        for probe in [Probe::Plain, Probe::Veiled, Probe::Standalone] {
            handle.update(cx, |p, _, _| p.probe = probe).unwrap();
            draw_frames(handle, 3, cx).await;
            heights.push(handle.read_with(cx, |p, _| p.height.get()).unwrap());
        }
        let (veiled, overlays) = handle
            .read_with(cx, |p, _| (p.hook_calls.0.load(Ordering::Relaxed), p.hook_calls.1.load(Ordering::Relaxed)))
            .unwrap();
        println!("T08: 高度 无效果 {:?} / 落定起点+行内星标 {:?} / 单独星标（反证） {:?}", heights[0], heights[1], heights[2]);
        println!("T08: 落定查询 {veiled} 次、星标构建 {overlays} 次");
        let t08 = heights[0].is_some() && heights[0] == heights[1] && heights[2] != heights[0] && veiled > 0 && overlays > 0;
        println!("{} S04-06 T08 渐显与行内星标不改变布局（反证：单独成行的星标改变高度）", if t08 { "PASS" } else { "FAIL" });
        std::process::exit(if t08 && t09 { 0 } else { 1 });
    })
    .detach();
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let self_test = std::env::args().any(|a| a == "--selftest");
    application().with_assets(buddy_ui::icons::Assets).run(move |cx: &mut App| {
        buddy_ui::init_theme(cx);
        Theme::install(Appearance::Light, cx);
        fonts::install_text_rendering(cx);
        markdown::init(cx);
        let bounds = Bounds::centered(None, size(px(560.0), px(760.0)), cx);
        let handle = cx
            .open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, |window, cx| {
                let md = cx.new(|cx| Markdown::new("".into(), Some(Arc::new(LanguageRegistry::default())), None, cx));
                cx.new(|cx| StreamPreview::new(md, window, cx))
            })
            .expect("open_window 失败");
        if self_test {
            selftest(handle, cx);
        }
    });
}
