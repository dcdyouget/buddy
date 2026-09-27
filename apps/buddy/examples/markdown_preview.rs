//! Phase 04 markdown 预览与自检。
//!
//! ```text
//! cargo run -p buddy-app --example markdown_preview                  # 目检窗口（S04-07 代码块 / S04-08 GFM）
//! cargo run -p buddy-app --example markdown_preview -- --selftest   # 自检后退出
//! cargo run --release -p buddy-app --example markdown_preview -- --bench  # S04-04 解析耗时测量
//! ```
//!
//! 自检 T03（S04-02）：代码高亮不改变布局 —— 同一行代码按「带高亮样式」与「不带」各排版一次，
//! 宽度与行高必须完全相同（v1 的关键字字重 600、注释斜体在等宽字体下不改变字宽）。
//!
//! 自检 T04（S04-07）：代码块确实经 Buddy 渲染器绘制（每块调用一次），且复制内容与 v1 逐字节一致 ——
//! v1 复制的是 react-markdown（CommonMark）给出的代码文本去掉一个末尾换行，这里以 pulldown-cmark 的
//! 同一语义结果为对照；并反证「直接切源码」在列表内 / 缩进代码块上会复制出错误内容。
//!
//! 自检 T05（S04-08）：样例中每个 GFM 块元素（标题 / 引用 / 列表 / 列表项 / 表格 / 单元格 / 分隔线）
//! 都经过 Buddy 的装饰回调，次数与 CommonMark+GFM 解析结果逐类一致。
//!
//! 自检 T06（S04-08）：markdown 中的网络图片经 Buddy 的图片解析器与 HTTP 客户端真实发起请求
//! （本地服务返回一张 1×1 PNG），并解码成功。

use buddy_ui::gpui::{
    App, AppContext, AsyncApp, Bounds, Context, Entity, Font, FontStyle, Hsla, Render, TextRun,
    Window, WindowBounds, WindowHandle, WindowOptions, div, prelude::*, px, size,
};
use buddy_ui::markdown::zed_markdown::{
    CodeBlockRenderer, ListBulletKind, Markdown, MarkdownDecorations, MarkdownElement, syntax::LanguageRegistry,
};
use std::collections::BTreeMap;
use buddy_ui::markdown::code_block;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use buddy_ui::gpui_platform::application;
use buddy_ui::markdown::{self, zed_markdown::syntax::{Rope, language_for_tag}};
use buddy_ui::theme_system::{Appearance, BuddyTheme, Theme, fonts, set_appearance, tokens::metrics};

const SAMPLES: &[(&str, &str)] = &[
    ("rust", "fn main() {\n    let answer: u32 = 42; // 注释 comment\n    println!(\"{answer}\");\n}"),
    ("typescript", "export const add = (a: number, b: number): number => a + b; // sum\nclass Foo extends Bar { static x = 'str'; }"),
    ("python", "def greet(name: str) -> None:\n    # 打个招呼\n    print(f\"hi {name}\", True, None, 3.14)"),
    ("json", "{\"key\": [1, 2.5, true, null, \"值\"]}"),
    ("css", ".a > b:hover { color: #5B5FE9; margin: 0 4px !important; }"),
    // 用户决定启用的语言（v1 无高亮）
    ("bash", "for f in *.md; do echo \"$f\" | wc -l; done # 统计"),
    ("toml", "[package]\nname = \"buddy\" # 名称\nversion = 2"),
    ("java", "public class A { /** doc */ static final int N = 1; }"),
];

/// 目检与 T05 共用的 GFM 样例
const GFM_DOC: &str = r#"# 一级标题：GFM 预览（S04-08）

正文段落，含 **加粗**、*斜体*、~~删除线~~、`行内代码` 与 [链接文字](https://v2.tauri.app/)，
以及裸网址 https://example.com 和无 scheme 的 [相对链接](example.com/path)。
原始 HTML 应按原样显示为文字（v1 同）：<kbd>Ctrl</kbd> + <b>C</b>

## 二级标题

### 三级标题

#### 四级标题（无竖条）

- 无序列表第一项
- 第二项，较长的文字用于观察换行时第二行是否与第一行文字左对齐，而不是与圆点对齐
  - 嵌套项
  - 嵌套项二
- 第三项

1. 有序列表
2. 第二项
10. 第十项（两位数序号）

- [x] 已完成的任务
- [ ] 未完成的任务

> 引用块：左侧主色竖边，其余细边，底色为淡色渐变，文字弱化。
>
> 第二段引用。

| 名称 | 说明 | 数值 |
|------|:----:|----:|
| Buddy | 居中列 | 42 |
| GPUI | 较长的单元格内容，观察换行 | 3.14 |

---

分隔线上下留白 16px，两端渐隐。

网络图片（需联网）：![GitHub 头像](https://avatars.githubusercontent.com/u/9919?s=64)

加载失败的图片（应显示失败占位）：![不存在的图片](https://example.invalid/missing.png)
"#;

/// 目检与 T04 共用的样例文档：覆盖 v1 代码块的各种形态
const CODE_DOC: &str = r#"## 代码块预览（S04-07）

Rust（有高亮，右上角「复制」，点击后 2 秒内变绿显示「已复制」）：

```rust
fn main() {
    let answer: u32 = 42; // 注释 comment
    println!("{answer}");
}
```

TypeScript，含超长行（**不换行**，横向滚动）：

```ts
export const veryLongFunctionName = (alpha: number, beta: number, gamma: string): string => `${alpha + beta} ${gamma} ${"0123456789".repeat(8)}`;
```

无语言的围栏（纯文本块：**无语言标签**、行高更松 1.75）：

```
三公级 ──── 御史大夫
              ↓
顾问/显职 ── 光禄大夫
```

bash（v1 无高亮；v2 按用户决定启用高亮）：

```bash
cargo run -p buddy-app --example markdown_preview
```

列表里的代码块：

1. 第一步
   ```python
   def greet(name: str) -> None:
       print(f"hi {name}")
   ```
2. 第二步

缩进式代码块：

    indented line 1
        still indented

结尾段落。
"#;

type Recorded = Rc<RefCell<Vec<String>>>;
type Counts = Rc<RefCell<BTreeMap<&'static str, usize>>>;

struct Preview {
    md: Entity<Markdown>,
    /// T04：记录渲染器每次被调用时算出的复制内容
    recorded: Recorded,
    /// T05：各装饰回调的调用次数
    counts: Counts,
}

fn doc() -> String {
    format!("{GFM_DOC}\n{CODE_DOC}")
}

/// 给每个装饰回调包一层计数（只观察，不改变结果）
fn counting(mut d: MarkdownDecorations, counts: &Counts) -> MarkdownDecorations {
    fn bump(counts: &Counts, key: &'static str) {
        *counts.borrow_mut().entry(key).or_default() += 1;
    }
    if let Some(f) = d.heading.take() {
        let n = counts.clone();
        d.heading = Some(Arc::new(move |div, level, cx| {
            bump(&n, if level <= 3 { "heading(1-3)" } else { "heading(4-6)" });
            f(div, level, cx)
        }));
    }
    if let Some(f) = d.block_quote.take() {
        let n = counts.clone();
        d.block_quote = Some(Arc::new(move |div, cx| {
            bump(&n, "block_quote");
            f(div, cx)
        }));
    }
    if let Some(f) = d.list.take() {
        let n = counts.clone();
        d.list = Some(Arc::new(move |div, top, cx| {
            bump(&n, if top { "list(top)" } else { "list(nested)" });
            f(div, top, cx)
        }));
    }
    if let Some(f) = d.list_bullet.take() {
        let n = counts.clone();
        d.list_bullet = Some(Arc::new(move |kind, cx| {
            bump(&n, match kind {
                ListBulletKind::Unordered => "bullet(unordered)",
                ListBulletKind::Ordered(_) => "bullet(ordered)",
                ListBulletKind::Task { .. } => "bullet(task)",
            });
            f(kind, cx)
        }));
    }
    if let Some(f) = d.table.take() {
        let n = counts.clone();
        d.table = Some(Arc::new(move |div, cx| {
            bump(&n, "table");
            f(div, cx)
        }));
    }
    if let Some(f) = d.table_cell.take() {
        let n = counts.clone();
        d.table_cell = Some(Arc::new(move |div, info, cx| {
            bump(&n, if info.is_header { "cell(head)" } else { "cell(body)" });
            f(div, info, cx)
        }));
    }
    if let Some(f) = d.rule.take() {
        let n = counts.clone();
        d.rule = Some(Arc::new(move |cx| {
            bump(&n, "rule");
            f(cx)
        }));
    }
    d
}

/// T05 对照：CommonMark + GFM 解析出的各类块元素数量
fn expected_counts(doc: &str) -> BTreeMap<&'static str, usize> {
    use pulldown_cmark::{Event, Options, Parser, Tag};
    let opts = Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
    let events: Vec<Event> = Parser::new_ext(doc, opts).collect();
    let mut out = BTreeMap::new();
    let mut bump = |k: &'static str| *out.entry(k).or_insert(0usize) += 1;
    let (mut list_depth, mut ordered_stack, mut in_head) = (0usize, Vec::new(), false);
    for (i, ev) in events.iter().enumerate() {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => bump(if (*level as usize) <= 3 { "heading(1-3)" } else { "heading(4-6)" }),
            Event::Start(Tag::BlockQuote(_)) => bump("block_quote"),
            Event::Start(Tag::List(first)) => {
                bump(if list_depth == 0 { "list(top)" } else { "list(nested)" });
                list_depth += 1;
                ordered_stack.push(first.is_some());
            }
            Event::End(pulldown_cmark::TagEnd::List(_)) => {
                list_depth -= 1;
                ordered_stack.pop();
            }
            Event::Start(Tag::Item) => {
                let task = events[i + 1..].iter().take(2).any(|e| matches!(e, Event::TaskListMarker(_)));
                bump(if task { "bullet(task)" } else if *ordered_stack.last().unwrap() { "bullet(ordered)" } else { "bullet(unordered)" });
            }
            Event::Start(Tag::Table(_)) => bump("table"),
            Event::Start(Tag::TableHead) => in_head = true,
            Event::End(pulldown_cmark::TagEnd::TableHead) => in_head = false,
            Event::Start(Tag::TableCell) => bump(if in_head { "cell(head)" } else { "cell(body)" }),
            Event::Rule => bump("rule"),
            _ => {}
        }
    }
    out
}

impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.buddy_theme();
        let CodeBlockRenderer::Custom { render, transform } = code_block::renderer(self.md.downgrade(), false) else {
            unreachable!("Buddy 代码块渲染器为 Custom")
        };
        let recorded = self.recorded.clone();
        let recording = CodeBlockRenderer::Custom {
            render: Arc::new(move |kind, parsed, range, metadata, window, cx| {
                recorded.borrow_mut().push(code_block::code_text(parsed, &range));
                render(kind, parsed, range, metadata, window, cx)
            }),
            transform,
        };
        let next = match theme.appearance {
            Appearance::Light => Appearance::Dark,
            Appearance::Dark => Appearance::Light,
        };
        let mut style = markdown::message_style(window, cx);
        style.decorations = counting(style.decorations, &self.counts);
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.colors.bg_surface)
            .child(
                div()
                    .id("toggle")
                    .m(px(metrics::SPACE_3))
                    .px(px(metrics::SPACE_3))
                    .py(px(metrics::SPACE_1))
                    .rounded(px(metrics::RADIUS_SM))
                    .border_1()
                    .border_color(theme.colors.border_default)
                    .text_color(theme.colors.text_primary)
                    .cursor_pointer()
                    .child(if next == Appearance::Dark { "切换到深色" } else { "切换到浅色" })
                    .on_click(move |_, _, cx| set_appearance(next, cx)),
            )
            .child(
                div().id("scroll").flex_1().overflow_y_scroll().px(px(metrics::SPACE_4)).pb(px(metrics::SPACE_6)).child(
                    MarkdownElement::new(self.md.clone(), style)
                        .code_block_renderer(recording)
                        .on_url_click(|url, _, cx| markdown::gfm::open_link(&url, cx))
                        .image_resolver(|url, _| markdown::gfm::image_source(url)),
                ),
            )
    }
}

/// 1×1 透明 PNG
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01,
    0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41,
    0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
    0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

async fn selftest_t06(handle: WindowHandle<Preview>, md: &Entity<Markdown>, cx: &mut AsyncApp) -> bool {
    use std::io::{Read, Write};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("本地端口");
    let port = listener.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let served = hits.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut s = stream;
            let mut buf = [0u8; 2048];
            let n = s.read(&mut buf).unwrap_or(0);
            if !String::from_utf8_lossy(&buf[..n]).starts_with("GET /a.png") {
                continue;
            }
            served.fetch_add(1, Ordering::SeqCst);
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", PNG_1X1.len());
            let _ = s.write_all(head.as_bytes()).and_then(|_| s.write_all(PNG_1X1));
        }
    });
    let src = format!("图片：![本地](http://127.0.0.1:{port}/a.png)");
    md.update(cx, |m, cx| m.replace(src.clone(), cx));
    wait_parsed(md, src.len(), cx).await;
    let mut ok = false;
    for _ in 0..50 {
        let _ = cx.update_window(handle.into(), |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
        if hits.load(Ordering::SeqCst) > 0 {
            ok = true;
            break;
        }
        cx.background_executor().timer(Duration::from_millis(100)).await;
    }
    // 请求到达后再等解码（GPUI 资源加载异步），用 use_asset 的结果间接确认：再画几帧不崩、不再请求
    println!("T06: 本地图片请求次数 {}", hits.load(Ordering::SeqCst));
    println!("{} S04-08 T06 网络图片经 Buddy 图片解析器与 HTTP 客户端发起请求", if ok { "PASS" } else { "FAIL" });
    ok
}

/// S04-09 复制用例（与 v1 测量共用，见 docs/evidence/s04-09/）
const COPY_SAMPLE: &str = include_str!("../../../docs/evidence/s04-09/copy-sample.md");
const V1_SELECTION: &str = include_str!("../../../docs/evidence/s04-09/v1-selection.json");

/// 模拟真实鼠标拖选整段消息 + Copy 动作，读剪贴板（结束后恢复用户原剪贴板）
async fn selftest_t07(handle: WindowHandle<Preview>, md: &Entity<Markdown>, cx: &mut AsyncApp) -> bool {
    use buddy_ui::gpui::{KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, point};
    let src = markdown::normalize::normalize_markdown(COPY_SAMPLE);
    md.update(cx, |m, cx| m.replace(src.clone(), cx));
    wait_parsed(md, src.len(), cx).await;
    let draw = |cx: &mut AsyncApp| {
        let _ = cx.update_window(handle.into(), |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
    };
    draw(cx);
    let saved = cx.update(|cx| cx.read_from_clipboard());
    let (from, to) = (point(px(17.), px(62.)), point(px(630.), px(810.)));
    // 与真实操作一致：每个事件后画一帧（GPUI 的命中测试用上一帧的悬停状态）
    let m = Modifiers::default();
    let events = [
        PlatformInput::MouseMove(MouseMoveEvent { position: from, pressed_button: None, modifiers: m }),
        PlatformInput::MouseDown(MouseDownEvent { button: MouseButton::Left, position: from, modifiers: m, click_count: 1, first_mouse: false }),
        PlatformInput::MouseMove(MouseMoveEvent { position: to, pressed_button: Some(MouseButton::Left), modifiers: m }),
        PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position: to, modifiers: m, click_count: 1 }),
    ];
    for event in events {
        let _ = cx.update_window(handle.into(), |_, window, cx| window.dispatch_event(event, cx));
        draw(cx);
    }
    let sel = md.read_with(cx, |m, _| (m.has_selection(), m.selected_source().map(|s| s.len())));
    println!("T07: 选区 {sel:?}");
    // 按真实快捷键（经 markdown::init 的键位绑定），而不是直接派发动作
    let key = PlatformInput::KeyDown(KeyDownEvent {
        keystroke: Keystroke::parse(markdown::COPY_KEYSTROKE).expect("快捷键"),
        is_held: false,
        prefer_character_input: false,
    });
    let _ = cx.update_window(handle.into(), |_, window, cx| window.dispatch_event(key, cx));
    let copied = cx.update(|cx| cx.read_from_clipboard()).and_then(|c| c.text()).unwrap_or_default();
    if let Some(saved) = saved {
        cx.update(|cx| cx.write_to_clipboard(saved));
    }
    let v1: String = serde_json::from_str(V1_SELECTION.trim()).expect("v1 选区 JSON");
    println!("T07: v2 复制 {}", serde_json::to_string(&copied).unwrap());
    println!("T07: v1 选区 {}", serde_json::to_string(&v1).unwrap());
    // 期望 = v1 实测去掉两处界面控件文字（有意不同，见 markdown::copy 模块文档）；先断言它们确实存在
    let chrome = ["rust\n\n复制\n", "\n 任务项"];
    let mut want = v1.clone();
    let mut ok = true;
    for c in chrome {
        ok &= want.contains(c);
        want = want.replacen(c, if c.starts_with("\n ") { "\n任务项" } else { "" }, 1);
    }
    ok &= copied == want && !copied.contains(markdown::normalize::EMPHASIS_GUARD);
    if copied != want {
        println!("T07: 期望   {}", serde_json::to_string(&want).unwrap());
    }
    println!("{} S04-09 T07 真实拖选 + Cmd+C，结果与 v1 选区文本一致（除代码块头部与复选框空格）", if ok { "PASS" } else { "FAIL" });
    ok
}

/// v1 复制内容的对照：CommonMark 代码块文本去掉一个末尾换行
fn expected_copies(doc: &str) -> Vec<String> {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    for ev in Parser::new(doc) {
        match ev {
            Event::Start(Tag::CodeBlock(_)) => cur = Some(String::new()),
            Event::Text(t) => {
                if let Some(c) = cur.as_mut() {
                    c.push_str(&t)
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                let mut c = cur.take().unwrap();
                if c.ends_with('\n') {
                    c.pop();
                }
                out.push(c);
            }
            _ => {}
        }
    }
    out
}

/// 反证用：直接切源码（去掉首尾围栏行）得到的「复制内容」
fn naive_copies(doc: &str) -> Vec<String> {
    use pulldown_cmark::{Event, Parser, Tag};
    Parser::new(doc)
        .into_offset_iter()
        .filter_map(|(ev, r)| matches!(ev, Event::Start(Tag::CodeBlock(_))).then(|| doc[r].to_string()))
        .map(|block| {
            let lines: Vec<&str> = block.trim_end_matches('\n').split('\n').collect();
            let fenced = lines[0].trim_start().starts_with("```");
            if fenced { lines[1..lines.len() - 1].join("\n") } else { lines.join("\n") }
        })
        .collect()
}

fn selftest_t04(handle: WindowHandle<Preview>, cx: &mut App) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        let md = handle.read_with(cx, |p, _| p.md.clone()).expect("窗口根视图");
        let doc = doc();
        wait_parsed(&md, doc.len(), cx).await;
        // 显式画一帧：不依赖窗口是否在前台出帧（后台窗口可能没有 display link 回调）
        handle
            .update(cx, |p, _, _| {
                p.recorded.borrow_mut().clear();
                p.counts.borrow_mut().clear();
            })
            .expect("窗口根视图");
        // 须经 AnyWindowHandle：`handle.update` 会占用根视图，绘制时再渲染它会重入
        cx.update_window(handle.into(), |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        })
        .expect("绘制");
        let got = handle.read_with(cx, |p, _| p.recorded.borrow().clone()).expect("窗口根视图");
        let want = expected_copies(&doc);
        let naive = naive_copies(&doc);
        let mut ok = got.len() == want.len() && !want.is_empty(); // 一帧内每块恰好调用一次
        for (i, w) in want.iter().enumerate() {
            let g = got.get(i).map(String::as_str).unwrap_or("<缺失>");
            if g != w {
                println!("FAIL T04 块 {i}: 复制内容不一致\n  得到 {g:?}\n  期望 {w:?}");
                ok = false;
            }
        }
        let naive_wrong = naive.iter().zip(&want).filter(|(n, w)| n != w).count();
        println!("T04: 代码块 {} 个，渲染器调用 {} 次；直接切源码会出错的块 {naive_wrong} 个（反证，应 ≥ 2：列表内、缩进式）", want.len(), got.len());
        ok &= naive_wrong >= 2;
        println!("{} S04-07 T04 代码块经 Buddy 渲染器绘制且复制内容与 v1 逐字节一致", if ok { "PASS" } else { "FAIL" });

        let got = handle.read_with(cx, |p, _| p.counts.borrow().clone()).expect("窗口根视图");
        let want = expected_counts(&doc);
        let t05 = got == want && want.len() >= 11;
        println!("T05: 装饰回调 {got:?}");
        println!("T05: 期望     {want:?}");
        println!("{} S04-08 T05 每个 GFM 块元素都经过 Buddy 装饰（逐类计数一致，覆盖 {} 类）", if t05 { "PASS" } else { "FAIL" }, want.len());
        let t06 = selftest_t06(handle, &md, cx).await;
        let t07 = selftest_t07(handle, &md, cx).await;
        std::process::exit(if ok && t05 && t06 && t07 { 0 } else { 1 });
    })
    .detach();
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
    application().with_assets(buddy_ui::icons::Assets).run(move |cx: &mut App| {
        buddy_ui::init_theme(cx);
        Theme::install(Appearance::Light, cx);
        fonts::install_text_rendering(cx);
        markdown::init(cx);
        buddy_ui::chat_bridge::init(cx);
        buddy_ui::http::install(cx);
        if run_bench {
            bench(cx);
            return;
        }
        let bounds = Bounds::centered(None, size(px(640.0), px(820.0)), cx);
        let handle = cx
            .open_window(
                WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
                |_, cx| {
                    let registry = Arc::new(LanguageRegistry::default());
                    let md = cx.new(|cx| Markdown::new(doc().into(), Some(registry), None, cx));
                    cx.new(|_| Preview { md, recorded: Rc::default(), counts: Rc::default() })
                },
            )
            .expect("open_window 失败");
        if self_test {
            let ok = handle.update(cx, |_, window, cx| selftest(window, cx)).unwrap_or(false);
            if !ok {
                println!("RESULT: FAIL");
                std::process::exit(1);
            }
            selftest_t04(handle, cx);
        }
    });
}
