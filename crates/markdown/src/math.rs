//! Native math typesetting. All parsing/layout runs on the Markdown background task;
//! the resulting font-outline SVG is independent of theme, font size and display scale.

use latex_rust::{Dim, MathFont, MathStyle, SvgOptions, layout, parse, render_svg};
use pulldown_cmark::{Event, Options, Parser, Tag};
use std::{
    borrow::Cow,
    hash::{Hash, Hasher},
    sync::LazyLock,
};

/// A typeset formula, measured in ems, with bundled STIX Two Math outlines.
#[derive(Debug)]
pub struct MathFormula {
    svg: String,
    width: f32,
    height: f32,
    pub(crate) cache_key: gpui::SharedString,
}

impl MathFormula {
    /// Returns `None` for incomplete, unsupported, or excessively large input.
    /// Callers must preserve the original source when typesetting fails.
    pub fn parse(source: &str, display: bool) -> Option<Self> {
        // Bound recursive parser/layout work on model-generated input.
        if source.len() > 4096
            || source.bytes().filter(|&b| b == b'\\').count() > 128
            || source.trim().is_empty()
        {
            return None;
        }
        let mut depth = 0usize;
        for ch in source.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    if depth > 32 {
                        return None;
                    }
                }
                '}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        static FONT: LazyLock<Option<MathFont>> = LazyLock::new(|| MathFont::stix_two_math().ok());
        let font = FONT.as_ref()?;
        let ast = parse(source).ok()?;
        let tree = layout(
            &ast,
            font,
            if display {
                MathStyle::Display
            } else {
                MathStyle::Text
            },
        )
        .ok()?;
        let width = f32::from_bits(tree.width.to_ieee32_bits());
        let height = f32::from_bits((&tree.height + &tree.depth).to_ieee32_bits());
        if !width.is_finite()
            || !height.is_finite()
            || width <= 0.
            || height <= 0.
            || width > 256.
            || height > 64.
        {
            return None;
        }
        let options = SvgOptions {
            font_size_pt: Dim::one(),
            display,
            ..SvgOptions::default()
        };
        let svg = render_svg(&tree, font, &options).ok()?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        svg.hash(&mut hasher);
        let cache_key = format!("buddy-math-{}", hasher.finish()).into();
        Some(Self {
            svg,
            width,
            height,
            cache_key,
        })
    }

    pub fn svg(&self) -> &str {
        &self.svg
    }
    pub fn width(&self) -> f32 {
        self.width
    }
    pub fn height(&self) -> f32 {
        self.height
    }
}

/// Prepare TeX delimiters without changing byte offsets. Dollar signs in inline
/// code are masked so a preceding price cannot consume the code's opening `$`.
/// The Markdown renderer always reads code from the original source.
pub fn normalize_delimiters(source: &str) -> Cow<'_, str> {
    if !source.contains('$') && !source.contains(r"\(") && !source.contains(r"\[") {
        return Cow::Borrowed(source);
    }
    let mut out = None;
    let mut protected: Vec<_> = Parser::new_ext(source, Options::empty())
        .into_offset_iter()
        .filter_map(|(event, range)| {
            if matches!(event, Event::Code(_)) && source[range.clone()].contains('$') {
                let bytes = out.get_or_insert_with(|| source.as_bytes().to_vec());
                for byte in &mut bytes[range.clone()] {
                    if *byte == b'$' {
                        *byte = b' ';
                    }
                }
            }
            matches!(
                event,
                Event::Code(_)
                    | Event::Html(_)
                    | Event::InlineHtml(_)
                    | Event::Start(Tag::CodeBlock(_) | Tag::HtmlBlock)
            )
            .then_some(range)
        })
        .collect();
    protected.extend(
        Parser::new_ext(source, Options::ENABLE_MATH)
            .into_offset_iter()
            .filter_map(|(event, range)| {
                matches!(event, Event::InlineMath(_) | Event::DisplayMath(_)).then_some(range)
            }),
    );
    let bytes = source.as_bytes();
    let escaped = |at: usize| {
        bytes[..at]
            .iter()
            .rev()
            .take_while(|&&b| b == b'\\')
            .count()
            % 2
            == 1
    };
    let mut cursor = 0;
    while cursor + 1 < bytes.len() {
        if bytes[cursor] != b'\\'
            || escaped(cursor)
            || !matches!(bytes[cursor + 1], b'(' | b'[')
            || protected.iter().any(|range| range.contains(&cursor))
        {
            cursor += 1;
            continue;
        }
        let display = bytes[cursor + 1] == b'[';
        let close = if display { r"\]" } else { r"\)" };
        let end = source[cursor + 2..]
            .match_indices(close)
            .map(|(at, _)| cursor + 2 + at)
            .find(|&at| !escaped(at));
        let Some(end) = end else {
            cursor += 2;
            continue;
        };
        if protected
            .iter()
            .any(|range| range.start < end + 2 && range.end > cursor)
            || (!display && source[cursor..end].contains("\n\n"))
        {
            cursor += 2;
            continue;
        }
        let out = out.get_or_insert_with(|| bytes.to_vec());
        // A harmless TeX group keeps inline delimiters two bytes wide without
        // introducing whitespace (which would make `$ ... $` ordinary prose).
        out[cursor..cursor + 2].copy_from_slice(if display { b"$$" } else { b"${" });
        out[end..end + 2].copy_from_slice(if display { b"$$" } else { b"}$" });
        cursor = end + 2;
    }
    out.map(|bytes| Cow::Owned(String::from_utf8(bytes).expect("only ASCII delimiters replaced")))
        .unwrap_or(Cow::Borrowed(source))
}
