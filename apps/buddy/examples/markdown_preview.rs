//! Phase 04 markdown 预览与自检。
//!
//! ```text
//! cargo run -p buddy-app --example markdown_preview -- --selftest   # 自检后退出
//! ```
//!
//! 自检 T03（S04-02）：代码高亮不改变布局 —— 同一行代码按「带高亮样式」与「不带」各排版一次，
//! 宽度与行高必须完全相同（v1 的关键字字重 600、注释斜体在等宽字体下不改变字宽）。

use buddy_ui::gpui::{
    App, Bounds, Context, Font, FontStyle, Hsla, Render, TextRun, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, size,
};
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

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let self_test = std::env::args().any(|a| a == "--selftest");
    application().run(move |cx: &mut App| {
        buddy_ui::init_theme(cx);
        Theme::install(Appearance::Light, cx);
        fonts::install_text_rendering(cx);
        markdown::init(cx);
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
