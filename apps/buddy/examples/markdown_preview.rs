//! Phase 04 markdown 预览与自检。
//!
//! ```text
//! cargo run -p buddy-app --example markdown_preview -- --selftest   # 自检后退出
//! cargo run --release -p buddy-app --example markdown_preview -- --bench  # S04-04 解析耗时测量
//! ```
//!
//! 自检 T03（S04-02）：代码高亮不改变布局 —— 同一行代码按「带高亮样式」与「不带」各排版一次，
//! 宽度与行高必须完全相同（v1 的关键字字重 600、注释斜体在等宽字体下不改变字宽）。

use buddy_ui::gpui::{
    App, AppContext, AsyncApp, Bounds, Context, Entity, Font, FontStyle, Hsla, Render, TextRun,
    Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use buddy_ui::markdown::zed_markdown::{Markdown, syntax::LanguageRegistry};
use std::sync::Arc;
use std::time::{Duration, Instant};
use buddy_ui::gpui_platform::application;
use buddy_ui::markdown::{self, zed_markdown::syntax::{Rope, language_for_tag}};
use buddy_ui::theme_system::{Appearance, BuddyTheme, Theme, fonts, tokens::metrics};

const SAMPLES: &[(&str, &str)] = &[
    ("rust", "fn main() {\n    let answer: u32 = 42; // 注释 comment\n    println!(\"{answer}\");\n}"),
    ("typescript", "export const add = (a: number, b: number): number => a + b; // sum\nclass Foo extends Bar { static x = 'str'; }"),
    ("python", "def greet(name: str) -> None:\n    # 打个招呼\n    print(f\"hi {name}\", True, None, 3.14)"),
    ("json", "{\"key\": [1, 2.5, true, null, \"值\"]}"),
    ("css", ".a > b:hover { color: #5B5FE9; margin: 0 4px !important; }"),
];

struct Empty;
impl Render for Empty {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// 把一行代码按高亮结果切成 TextRun（与 markdown 渲染时的做法一致：高亮样式叠加在代码基础字体上）
fn runs_for(line: &str, highlighted: &[(std::ops::Range<usize>, usize)], base: &Font, st: &buddy_ui::theme::SyntaxTheme, with_style: bool) -> Vec<TextRun> {
    let color = Hsla::default();
    let mut runs = Vec::new();
    let mut at = 0;
    let mut push = |len: usize, font: Font| {
        if len > 0 {
            runs.push(TextRun { len, font, color, background_color: None, underline: None, strikethrough: None });
        }
    };
    for (r, cat) in highlighted {
        push(r.start - at, base.clone());
        let mut f = base.clone();
        if with_style && let Some(h) = st.get(*cat) {
            if let Some(w) = h.font_weight {
                f.weight = w;
            }
            if let Some(s) = h.font_style {
                f.style = s;
            }
        }
        push(r.len(), f);
        at = r.end;
    }
    push(line.len() - at, base.clone());
    runs
}

fn selftest(window: &mut Window, cx: &mut App) -> bool {
    let theme = *cx.buddy_theme();
    let st = markdown::syntax_theme(&theme);
    let mono = fonts::mono_font(cx);
    let size = px(metrics::FONT_SIZE_BASE);
    let (mut lines_checked, mut styled_runs, mut ok, mut max_dw) = (0, 0, true, 0f32);
    for (tag, src) in SAMPLES {
        let lang = language_for_tag(tag).expect("v1 语言");
        let resolved = lang.highlight_text_resolved(&Rope::from(src), 0..src.len());
        let mut line_start = 0;
        for line in src.split('\n') {
            let line_end = line_start + line.len();
            let hl: Vec<_> = resolved
                .runs
                .iter()
                .filter(|(r, _)| r.start >= line_start && r.end <= line_end)
                .map(|(r, id)| (r.start - line_start..r.end - line_start, usize::from(*id)))
                .collect();
            styled_runs += hl
                .iter()
                .filter(|(_, c)| st.get(*c).is_some_and(|h| h.font_weight.is_some() || h.font_style == Some(FontStyle::Italic)))
                .count();
            let plain = window.text_system().layout_line(line, size, &runs_for(line, &hl, &mono, &st, false), None);
            let styled = window.text_system().layout_line(line, size, &runs_for(line, &hl, &mono, &st, true), None);
            // 宽度允许浮点累加误差（实测 ≤ 6e-5 px，远小于 1 物理像素）；行高相关的 ascent / descent 须完全相等
            let dw = (f32::from(plain.width) - f32::from(styled.width)).abs();
            max_dw = max_dw.max(dw);
            let same = dw < 0.01 && plain.ascent == styled.ascent && plain.descent == styled.descent;
            if !same {
                println!("FAIL T03 [{tag}] {line:?}: 宽 {:?} vs {:?}", plain.width, styled.width);
                ok = false;
            }
            lines_checked += 1;
            line_start = line_end + 1;
        }
    }
    println!("T03: {lines_checked} 行、其中带字重/斜体的高亮片段 {styled_runs} 个；等宽字体 {}；最大宽度差 {max_dw:e} px", mono.family);
    let pass = ok && styled_runs > 0;
    println!("{} S04-02 T03 高亮不改变布局（宽度差 < 0.01px，ascent / descent 完全一致）", if pass { "PASS" } else { "FAIL" });
    pass
}


/// 生成 n 行的混合文档：段落 / 列表 / 表格 / 代码块（rust、ts）交替，接近真实长回复
fn long_doc(lines: usize) -> String {
    let blocks = [
        "## 小节标题\n\nBuddy 是一个跨平台的 AI 聊天工具，**按下热键**即可唤起，`inline code` 与 [链接](https://example.com)。\n",
        "- 第一点\n- 第二点，包含 *强调*\n  - 嵌套项\n",
        "| 列 A | 列 B |\n|------|------|\n| 1 | 2 |\n| 3 | 4 |\n",
        "```rust\nfn fib(n: u64) -> u64 {\n    match n { 0 | 1 => n, _ => fib(n - 1) + fib(n - 2) }\n}\n```\n",
        "```typescript\nexport const add = (a: number, b: number): number => a + b; // sum\n```\n",
    ];
    let mut out = String::new();
    let mut i = 0;
    while out.lines().count() < lines {
        out.push_str(blocks[i % blocks.len()]);
        out.push('\n');
        i += 1;
    }
    out
}

/// 上游在解析进行中收到新内容会置 `should_reparse`，完成后再解析一轮；
/// 因此 `!is_parsing()` 即「解析结果已追上当前全部源文本」（`parsed_markdown()` 仅 test-support 可见）
async fn wait_parsed(md: &Entity<Markdown>, want_len: usize, cx: &mut AsyncApp) {
    loop {
        let done = md.read_with(cx, |m, _| !m.is_parsing() && m.source().len() >= want_len);
        if done {
            return;
        }
        cx.background_executor().timer(Duration::from_micros(200)).await;
    }
}

/// S04-04：① 一次性解析不同长度文档的耗时；② 按流式节奏（每 16ms 追加一段）追加时，
/// 追加到解析结果可见的额外延迟与实际解析次数（验证上游后台解析 + 合并是否够用）
fn bench(cx: &mut App) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        let registry = Arc::new(LanguageRegistry::default());
        for lines in [200usize, 1000, 5000] {
            let doc = long_doc(lines);
            let md = cx.new(|cx| Markdown::new("".into(), Some(registry.clone()), None, cx));
            let mut samples = Vec::new();
            for _ in 0..5 {
                md.update(cx, |m, cx| m.replace("", cx));
                wait_parsed(&md, 0, cx).await;
                let t = Instant::now();
                md.update(cx, |m, cx| m.replace(doc.clone(), cx));
                wait_parsed(&md, doc.len(), cx).await;
                samples.push(t.elapsed().as_secs_f64() * 1000.0);
            }
            samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
            println!("bench full-parse lines={lines} bytes={} median_ms={:.2} max_ms={:.2}", doc.len(), samples[2], samples[4]);
        }
        for lines in [1000usize, 5000] {
            let doc = long_doc(lines);
            let md = cx.new(|cx| Markdown::new("".into(), Some(registry.clone()), None, cx));
            // 预先灌入大部分文档，只流式追加末尾：衡量「长回复末段」的追加延迟
            let prefill = doc.len() * 9 / 10;
            let mut pos = prefill;
            while !doc.is_char_boundary(pos) {
                pos += 1;
            }
            md.update(cx, |m, cx| m.replace(doc[..pos].to_string(), cx));
            wait_parsed(&md, pos, cx).await;
            let chunk = 48; // 约等于真实流式一次到达的字节数
            let (mut max_lag, mut appends, mut caught_up_within_frame) = (0f64, 0usize, 0usize);
            while pos < doc.len() && appends < 300 {
                let mut end = (pos + chunk).min(doc.len());
                while !doc.is_char_boundary(end) {
                    end += 1;
                }
                let piece = doc[pos..end].to_string();
                pos = end;
                let t = Instant::now();
                md.update(cx, |m, cx| m.append(&piece, cx));
                appends += 1;
                cx.background_executor().timer(Duration::from_millis(16)).await;
                if md.read_with(cx, |m, _| !m.is_parsing()) {
                    caught_up_within_frame += 1;
                } else {
                    wait_parsed(&md, pos, cx).await; // 一帧内未追上：等到追上，计入额外延迟
                }
                max_lag = max_lag.max(t.elapsed().as_secs_f64() * 1000.0 - 16.0);
            }
            println!(
                "bench streaming lines={lines} doc_bytes={} appends={appends} caught_up_within_one_frame={caught_up_within_frame} max_extra_lag_ms={:.2}",
                doc.len(),
                max_lag.max(0.0)
            );
        }
        println!("BENCH DONE");
        std::process::exit(0);
    })
    .detach();
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let self_test = std::env::args().any(|a| a == "--selftest");
    let run_bench = std::env::args().any(|a| a == "--bench");
    application().run(move |cx: &mut App| {
        buddy_ui::init_theme(cx);
        Theme::install(Appearance::Light, cx);
        fonts::install_text_rendering(cx);
        markdown::init(cx);
        if run_bench {
            bench(cx);
            return;
        }
        let bounds = Bounds::centered(None, size(px(400.0), px(200.0)), cx);
        let handle = cx
            .open_window(
                WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
                |_, cx| cx.new(|_| Empty),
            )
            .expect("open_window 失败");
        if self_test {
            let ok = handle.update(cx, |_, window, cx| selftest(window, cx)).unwrap_or(false);
            println!("RESULT: {}", if ok { "PASS" } else { "FAIL" });
            std::process::exit(if ok { 0 } else { 1 });
        }
    });
}
