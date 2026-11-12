//! Native math smoke preview: cargo run -p buddy-ui --example markdown_math -- [dark] [narrow]
//! Uses no model, credentials or conversation storage.
use buddy_ui::{
    gpui::*,
    gpui_platform::application,
    markdown::{
        self,
        zed_markdown::{Markdown, MarkdownElement},
    },
    theme_system::{Appearance, BuddyTheme, Theme},
};

const SOURCE: &str = r#"### KV cache 显存占用

存的是每一层、每个 KV head、每个历史 token 的 K 和 V 向量。

$$ \text{size} = 2 \times L \times H_{kv} \times d_{head} \times S \times B \times \text{bytes} $$

行内公式 $H_{kv}$ 和 $d_{head}$ 应与正文自然连接，复杂度为 $O(S^2)$。这是一段更长的中文说明，用来检查公式在窄窗口中的换行，以及后续文字是否仍然可以选择复制。

\[ x = \frac{-b \pm \sqrt{b^2-4ac}}{2a} \]

行内分数 \(\frac{1}{2}\) 与普通文字，English words and $x_i^2$ continue here.

$$\begin{pmatrix}a & b \\ c & d\end{pmatrix} \qquad \sum_{i=1}^n x_i^2$$

普通价格 $5 and $10；代码 `$x^2$` 保持原文。

```latex
$$ \frac{1}{2} $$
```

不支持的公式保留原文：$\unknowncommand{x}$。

$$ \frac{1}{
"#;

struct Preview {
    markdown: Entity<Markdown>,
    _subscription: Subscription,
}
impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("math-preview")
            .size_full()
            .overflow_y_scroll()
            .p_6()
            .bg(cx.buddy_theme().colors.bg_canvas)
            .child(MarkdownElement::new(
                self.markdown.clone(),
                markdown::message_style(window, cx),
            ))
    }
}
fn main() {
    let dark = std::env::args().any(|arg| arg == "dark");
    let narrow = std::env::args().any(|arg| arg == "narrow");
    let snapshot =
        std::env::args().find_map(|arg| arg.strip_prefix("--snapshot=").map(str::to_owned));
    application()
        .with_assets(buddy_ui::icons::Assets)
        .run(move |cx| {
            buddy_ui::init_theme(cx);
            Theme::install(
                if dark {
                    Appearance::Dark
                } else {
                    Appearance::Light
                },
                cx,
            );
            markdown::init(cx);
            let handle = cx
                .open_window(
                    WindowOptions {
                        titlebar: Some(TitlebarOptions {
                            title: Some("Buddy · Math preview".into()),
                            ..Default::default()
                        }),
                        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                            None,
                            size(px(if narrow { 420. } else { 760. }), px(820.)),
                            cx,
                        ))),
                        ..Default::default()
                    },
                    |_, cx| {
                        cx.new(|cx| {
                            let markdown =
                                cx.new(|cx| Markdown::new(SOURCE.into(), None, None, cx));
                            let subscription = cx.observe(&markdown, |_, _, cx| cx.notify());
                            Preview {
                                markdown,
                                _subscription: subscription,
                            }
                        })
                    },
                )
                .unwrap();
            cx.activate(true);
            if let Some(path) = snapshot {
                cx.spawn(async move |cx: &mut AsyncApp| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(500))
                        .await;
                    cx.update_window(handle.into(), |root, window, cx| {
                        let view = root.downcast::<Preview>().unwrap();
                        let events = view.read(cx).markdown.read(cx).parsed_markdown().events();
                        let math_count = events
                            .iter()
                            .filter(|(_, event)| {
                                matches!(
                                    event,
                                    markdown::zed_markdown::parser::MarkdownEvent::Math { .. }
                                )
                            })
                            .count();
                        assert_eq!(
                            math_count,
                            9,
                            "complete formulas, including the unsupported fallback: {:?}",
                            events
                                .iter()
                                .filter(|(_, event)| matches!(
                                    event,
                                    markdown::zed_markdown::parser::MarkdownEvent::Math { .. }
                                ))
                                .collect::<Vec<_>>()
                        );
                    window.refresh();
                    let clear = window.draw(cx);
                        window
                            .render_to_image()
                            .expect("render frame")
                            .save(&path)
                            .expect("save frame");
                    clear.clear(cx);
                        eprintln!("Verified {math_count} math events; saved {path}");
                        cx.quit();
                    })
                    .unwrap();
                })
                .detach();
            }
        });
}
