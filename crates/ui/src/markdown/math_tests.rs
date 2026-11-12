use super::zed_markdown::{
    math::{MathFormula, normalize_delimiters},
    parser::PARSE_OPTIONS,
};
use pulldown_cmark::{Event, Parser};

const KV_CACHE: &str =
    r"\text{size} = 2 \times L \times H_{kv} \times d_{head} \times S \times B \times \text{bytes}";

#[test]
fn math_typesets_kv_cache_and_common_notation_with_bundled_outlines() {
    for source in [
        KV_CACHE,
        r"\frac{-b \pm \sqrt{b^2-4ac}}{2a}",
        r"\sum_{i=1}^{n} x_i^2",
        r"\int_0^1 x^2\,dx",
        r"\begin{pmatrix}a & b \\ c & d\end{pmatrix}",
        r"f(x)=\begin{cases}x^2 & x>0 \\ 0 & x\leq 0\end{cases}",
    ] {
        for display in [false, true] {
            let formula = MathFormula::parse(source, display).unwrap_or_else(|| panic!("{source}"));
            assert!(formula.width() > 0. && formula.height() > 0.);
            assert!(formula.svg().contains("<path"));
            assert!(
                !formula.svg().contains("<text"),
                "must not depend on installed fonts"
            );
        }
    }
}

#[test]
fn math_invalid_and_excessive_input_falls_back_instead_of_disappearing() {
    for source in [
        r"\frac{1}{",
        r"\unknowncommand{x}",
        "",
        &"{".repeat(40),
        &"x".repeat(5000),
    ] {
        assert!(MathFormula::parse(source, true).is_none(), "{source}");
    }
}

#[test]
fn math_delimiters_preserve_offsets_and_protect_code_and_escapes() {
    let source = "中文 \\(x_i\\) 和 $y^2$\n\n\\[\\frac{1}{2}\\]\n\n`\\(code\\)`\n\n```tex\n\\[code\\]\n```\n\n\\\\(literal\\\\)";
    let normalized = normalize_delimiters(source);
    assert_eq!(source.len(), normalized.len());
    assert!(normalized.contains(r"`\(code\)`"));
    assert!(normalized.contains("```tex\n\\[code\\]\n```"));
    assert!(normalized.contains(r"\\(literal\\)"));
    let formulas: Vec<_> = Parser::new_ext(&normalized, PARSE_OPTIONS)
        .into_offset_iter()
        .filter_map(|(event, range)| match event {
            Event::InlineMath(latex) | Event::DisplayMath(latex) => {
                Some((latex.trim().to_owned(), source[range].to_owned()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        formulas,
        vec![
            ("{x_i}".into(), r"\(x_i\)".into()),
            ("y^2".into(), "$y^2$".into()),
            (r"\frac{1}{2}".into(), r"\[\frac{1}{2}\]".into())
        ]
    );
}

#[test]
fn math_inline_dollars_do_not_consume_prices_or_unclosed_input() {
    for source in [
        r"price $5 and $10",
        r"price $5 and $10; code `$x^2$`",
        r"escaped \$x\$",
        r"pending $x_",
        r"pending \(x_i",
    ] {
        let normalized = normalize_delimiters(source);
        assert!(
            !Parser::new_ext(&normalized, PARSE_OPTIONS)
                .any(|event| matches!(event, Event::InlineMath(_) | Event::DisplayMath(_))),
            "{source}"
        );
    }
    let source = format!("$$ {KV_CACHE} $$");
    assert!(
        Parser::new_ext(&source, PARSE_OPTIONS).any(|event| matches!(event, Event::DisplayMath(_)))
    );
}

#[test]
fn math_normalization_does_not_insert_emphasis_guards_in_tex() {
    let source = r"公式 $\text{**label**}$，以及 \[\text{**label**}\]。";
    assert_eq!(super::normalize::normalize_markdown(source), source);
}

#[test]
fn math_streaming_cursor_tracks_the_formula_end() {
    let source = "文字 $x_i^2$";
    let tail = super::streaming::tail(source, true, 1);
    assert_eq!(tail.star, super::streaming::Star::After(source.len()));
    assert!(tail.settle.is_empty());
}

#[gpui::test]
fn math_can_be_selected_and_copied_as_latex(cx: &mut gpui::TestAppContext) {
    use super::zed_markdown::{Markdown, MarkdownElement};
    use gpui::{
        AppContext, Context, Entity, IntoElement, Modifiers, MouseButton, MouseDownEvent, Render,
        Window,
    };
    struct FormulaView(Entity<Markdown>);
    impl Render for FormulaView {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            MarkdownElement::new(self.0.clone(), super::message_style(window, cx))
        }
    }
    cx.update(|cx| {
        crate::init_theme(cx);
        crate::theme_system::Theme::install(crate::theme_system::Appearance::Light, cx);
        super::init(cx);
    });
    let markdown = cx.new(|cx| Markdown::new("before $x_i^2$ after".into(), None, None, cx));
    cx.run_until_parked();
    let (_, cx) = cx.add_window_view(|_, _| FormulaView(markdown));
    let bounds = cx
        .debug_bounds("markdown-math")
        .expect("formula occupies native layout");
    assert!(bounds.size.width > gpui::px(0.));
    cx.simulate_event(MouseDownEvent {
        position: bounds.center(),
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_mouse_up(bounds.center(), MouseButton::Left, Modifiers::none());
    cx.simulate_keystrokes(super::COPY_KEYSTROKE);
    cx.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().and_then(|item| item.text()),
            Some("$x_i^2$".into())
        )
    });
    let paragraph = cx.debug_bounds("inner").unwrap();
    let start = gpui::point(paragraph.left(), bounds.center().y);
    let end = gpui::point(paragraph.right(), bounds.center().y);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.simulate_keystrokes(super::COPY_KEYSTROKE);
    cx.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().and_then(|item| item.text()),
            Some("before $x_i^2$ after".into())
        )
    });
}

#[gpui::test]
fn math_streaming_parser_preserves_source_and_closes_atomically(cx: &mut gpui::TestAppContext) {
    use super::zed_markdown::{Markdown, parser::MarkdownEvent};
    use gpui::AppContext;
    let source = format!("说明\n\n$$ {KV_CACHE}");
    let markdown = cx.new(|cx| Markdown::new(source.clone().into(), None, None, cx));
    cx.run_until_parked();
    markdown.read_with(cx, |markdown, _| {
        assert!(
            !markdown
                .parsed_markdown()
                .events()
                .iter()
                .any(|(_, event)| matches!(event, MarkdownEvent::Math { .. }))
        );
    });
    let complete = format!("{source} $$\n\n行内 \\(x_i\\)。\n\n`$code$`\n\n```tex\n$$code$$\n```");
    markdown.update(cx, |markdown, cx| markdown.replace(complete.clone(), cx));
    cx.run_until_parked();
    markdown.read_with(cx, |markdown, _| {
        let parsed = markdown.parsed_markdown();
        let formulas: Vec<_> = parsed
            .events()
            .iter()
            .filter_map(|(range, event)| match event {
                MarkdownEvent::Math { display, .. } => Some((&complete[range.clone()], *display)),
                _ => None,
            })
            .collect();
        assert_eq!(
            formulas,
            vec![
                (format!("$$ {KV_CACHE} $$").as_str(), true),
                (r"\(x_i\)", false)
            ]
        );
        assert!(parsed.root_block_starts().len() >= 4);
        assert_eq!(
            parsed.rebalanced_markdown_for_selection(0..complete.len()),
            complete
        );
    });
}
