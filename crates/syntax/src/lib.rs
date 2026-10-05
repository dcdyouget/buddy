// 移植自 Comet（crates/syntax，commit a4781608，原 crate 名 zeron-syntax），MIT 许可证，原文见 LICENSE-MIT-comet。
// Buddy 只内置 Python / Bash / SQL 三种 tree-sitter 语法（全部语法表约 30 MB），其余语言由 [`generic`] 通用高亮承接。
//! 语法高亮。返回按行切分、行内字节偏移的区间，无 UI 依赖。

use std::{collections::BTreeSet, ops::Range, sync::OnceLock};

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

pub mod generic;

pub const DEFAULT_MAX_SOURCE_BYTES: usize = 1024 * 1024;
const MAX_SPANS: usize = 200_000;

/// 内置 tree-sitter 语法的语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    Python,
    Bash,
    Sql,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightKind {
    Comment,
    Keyword,
    String,
    StringSpecial,
    Escape,
    Number,
    Boolean,
    Type,
    TypeBuiltin,
    Constructor,
    Function,
    FunctionBuiltin,
    Macro,
    Property,
    Constant,
    Variable,
    VariableSpecial,
    Parameter,
    Operator,
    Punctuation,
    Tag,
    Attribute,
    Label,
    MarkupHeading,
    MarkupRaw,
    MarkupLink,
    MarkupReference,
    MarkupEmphasis,
    MarkupStrong,
    Embedded,
    Invalid,
}

impl HighlightKind {
    /// Stable precedence used to resolve overlapping parser captures.
    pub const fn precedence(self) -> u8 {
        match self {
            Self::Invalid => 100,
            Self::Escape => 95,
            Self::Macro => 90,
            Self::Property | Self::Attribute => 85,
            Self::FunctionBuiltin | Self::TypeBuiltin | Self::VariableSpecial => 80,
            Self::StringSpecial | Self::Constructor | Self::Parameter => 75,
            Self::Function | Self::Type | Self::Constant | Self::Tag | Self::Label => 70,
            Self::Comment | Self::Keyword | Self::String | Self::Number | Self::Boolean => 60,
            Self::Variable | Self::Operator => 50,
            Self::Punctuation | Self::Embedded => 40,
            // Markdown block captures can wrap more specific inline captures,
            // and fenced-code captures can wrap an injected language. Keep
            // markup below programming-language tokens while preserving the
            // nesting order among Markdown roles.
            Self::MarkupHeading => 30,
            Self::MarkupEmphasis => 31,
            Self::MarkupStrong => 32,
            Self::MarkupLink | Self::MarkupReference => 33,
            Self::MarkupRaw => 34,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub range: Range<usize>,
    pub kind: HighlightKind,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum HighlightError {
    #[error("highlight range {start}..{end} is invalid for a {len}-byte source")]
    InvalidRange {
        start: usize,
        end: usize,
        len: usize,
    },
    #[error("highlight range {start}..{end} is not on UTF-8 boundaries")]
    InvalidUtf8Boundary { start: usize, end: usize },
    #[error("source exceeds the configured highlighting limit")]
    SourceTooLarge,
    #[error("highlight output exceeds the configured span limit")]
    TooManySpans,
    #[error("parser failed: {0}")]
    Parser(String),
}

/// 代码块围栏标签 → 内置语法；其他标签返回 `None`（调用方改用 [`generic`]）。
pub fn language_for_alias(alias: &str) -> Option<LanguageId> {
    let alias = alias.trim().split_ascii_whitespace().next()?.to_ascii_lowercase();
    Some(match alias.as_str() {
        "python" | "py" | "python3" => LanguageId::Python,
        "bash" | "sh" | "shell" | "zsh" | "console" => LanguageId::Bash,
        "sql" => LanguageId::Sql,
        _ => return None,
    })
}

/// 用内置语法高亮整段源码，返回每行的区间。
pub fn highlight(language: LanguageId, source: &str) -> Result<Vec<Vec<HighlightSpan>>, HighlightError> {
    if source.len() > DEFAULT_MAX_SOURCE_BYTES {
        return Err(HighlightError::SourceTooLarge);
    }
    let configuration = cached_configuration(language)?;
    let mut highlighter = Highlighter::new();
    let events = highlighter
        .highlight(configuration, source.as_bytes(), None, |_| None)
        .map_err(|error| HighlightError::Parser(error.to_string()))?;
    let mut active = Vec::new();
    let mut spans = Vec::new();
    for event in events {
        match event.map_err(|error| HighlightError::Parser(error.to_string()))? {
            HighlightEvent::HighlightStart(highlight) => active.push(CAPTURE_KINDS[highlight.0]),
            HighlightEvent::HighlightEnd => {
                active.pop();
            }
            HighlightEvent::Source { start, end } => {
                if let Some(kind) = active.iter().copied().max_by_key(|kind| kind.precedence()) {
                    spans.push(HighlightSpan {
                        range: start..end,
                        kind,
                    });
                    if spans.len() > MAX_SPANS {
                        return Err(HighlightError::TooManySpans);
                    }
                }
            }
        }
    }
    split_lines(source, spans)
}

/// 查询编译后不含文档状态，每种语法只编译一次。
fn cached_configuration(language: LanguageId) -> Result<&'static HighlightConfiguration, HighlightError> {
    static PYTHON: OnceLock<Result<HighlightConfiguration, HighlightError>> = OnceLock::new();
    static BASH: OnceLock<Result<HighlightConfiguration, HighlightError>> = OnceLock::new();
    static SQL: OnceLock<Result<HighlightConfiguration, HighlightError>> = OnceLock::new();
    let (cell, build): (_, fn() -> Result<HighlightConfiguration, HighlightError>) = match language {
        LanguageId::Python => (&PYTHON, || {
            make_configuration(tree_sitter_python::LANGUAGE.into(), "python", tree_sitter_python::HIGHLIGHTS_QUERY, "", "")
        }),
        LanguageId::Bash => (&BASH, || {
            make_configuration(tree_sitter_bash::LANGUAGE.into(), "bash", tree_sitter_bash::HIGHLIGHT_QUERY, "", "")
        }),
        LanguageId::Sql => (&SQL, || {
            make_configuration(tree_sitter_sequel::LANGUAGE.into(), "sql", tree_sitter_sequel::HIGHLIGHTS_QUERY, "", "")
        }),
    };
    cell.get_or_init(|| {
        let mut config = build()?;
        config.configure(CAPTURE_NAMES);
        Ok(config)
    })
    .as_ref()
    .map_err(Clone::clone)
}

fn make_configuration(
    language: tree_sitter::Language,
    name: &str,
    highlights: &str,
    injections: &str,
    locals: &str,
) -> Result<HighlightConfiguration, HighlightError> {
    HighlightConfiguration::new(language, name, highlights, injections, locals)
        .map_err(|error| HighlightError::Parser(error.to_string()))
}

/// Validate, split, and normalize absolute source spans into line-relative spans.
pub(crate) fn split_lines(
    source: &str,
    spans: impl IntoIterator<Item = HighlightSpan>,
) -> Result<Vec<Vec<HighlightSpan>>, HighlightError> {
    {
        let starts = line_starts(source);
        let mut lines = vec![Vec::new(); starts.len()];
        for span in spans {
            validate_span(source, &span.range)?;
            if span.range.is_empty() {
                continue;
            }
            let first_line = starts.partition_point(|&start| start <= span.range.start) - 1;
            for (line_ix, &start) in starts.iter().enumerate().skip(first_line) {
                let raw_end = starts.get(line_ix + 1).copied().unwrap_or(source.len());
                let mut end = raw_end;
                if source.as_bytes().get(end.wrapping_sub(1)) == Some(&b'\n') {
                    end -= 1;
                    if source.as_bytes().get(end.wrapping_sub(1)) == Some(&b'\r') {
                        end -= 1;
                    }
                }
                let segment_start = span.range.start.max(start);
                let segment_end = span.range.end.min(end);
                if segment_start < segment_end {
                    lines[line_ix].push(HighlightSpan {
                        range: segment_start - start..segment_end - start,
                        kind: span.kind,
                    });
                }
                if raw_end >= span.range.end {
                    break;
                }
            }
        }
        for line in &mut lines {
            *line = normalize_line(std::mem::take(line));
        }
        Ok(lines)
    }
}

fn validate_span(source: &str, range: &Range<usize>) -> Result<(), HighlightError> {
    if range.start > range.end || range.end > source.len() {
        return Err(HighlightError::InvalidRange {
            start: range.start,
            end: range.end,
            len: source.len(),
        });
    }
    if !source.is_char_boundary(range.start) || !source.is_char_boundary(range.end) {
        return Err(HighlightError::InvalidUtf8Boundary {
            start: range.start,
            end: range.end,
        });
    }
    Ok(())
}

fn normalize_line(spans: Vec<HighlightSpan>) -> Vec<HighlightSpan> {
    #[derive(Clone, Copy)]
    enum Edge {
        Start(usize),
        End(usize),
    }

    let mut edges = spans
        .iter()
        .enumerate()
        .flat_map(|(index, span)| {
            [
                (span.range.start, Edge::Start(index)),
                (span.range.end, Edge::End(index)),
            ]
        })
        .collect::<Vec<_>>();
    edges.sort_unstable_by_key(|(offset, _)| *offset);

    // The span index is the tie-breaker so equal-precedence overlaps retain
    // the old `Iterator::max_by_key` behavior (the later span wins).
    let mut active = BTreeSet::new();
    let mut normalized: Vec<HighlightSpan> = Vec::new();
    let mut cursor = 0;
    while cursor < edges.len() {
        let offset = edges[cursor].0;
        let group_start = cursor;
        while cursor < edges.len() && edges[cursor].0 == offset {
            if let Edge::End(index) = edges[cursor].1 {
                active.remove(&(spans[index].kind.precedence(), index));
            }
            cursor += 1;
        }
        for (_, edge) in &edges[group_start..cursor] {
            if let Edge::Start(index) = *edge {
                active.insert((spans[index].kind.precedence(), index));
            }
        }

        let Some(next_offset) = edges.get(cursor).map(|(next, _)| *next) else {
            break;
        };
        if offset == next_offset {
            continue;
        }
        if let Some((_, index)) = active.last().copied() {
            let kind = spans[index].kind;
            if let Some(previous) = normalized.last_mut()
                && previous.kind == kind
                && previous.range.end == offset
            {
                previous.range.end = next_offset;
            } else {
                normalized.push(HighlightSpan {
                    range: offset..next_offset,
                    kind,
                });
            }
        }
    }
    normalized
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        source
            .match_indices('\n')
            .map(|(index, _)| index + 1)
            .filter(|start| *start < source.len()),
    );
    starts
}

// Ordered from generic to specific. `HighlightConfiguration::configure`
// resolves dotted captures to the best recognized name in this table.
const CAPTURE_NAMES: &[&str] = &[
    "comment",
    "keyword",
    "string",
    "string.special",
    "string.escape",
    "number",
    "boolean",
    "type",
    "type.builtin",
    "constructor",
    "function",
    "function.builtin",
    "function.macro",
    "property",
    "constant",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "operator",
    "punctuation",
    "tag",
    "attribute",
    "label",
    "text.title",
    "text.literal",
    "text.uri",
    "text.reference",
    "text.emphasis",
    "text.strong",
    "embedded",
    "error",
];

const CAPTURE_KINDS: &[HighlightKind] = &[
    HighlightKind::Comment,
    HighlightKind::Keyword,
    HighlightKind::String,
    HighlightKind::StringSpecial,
    HighlightKind::Escape,
    HighlightKind::Number,
    HighlightKind::Boolean,
    HighlightKind::Type,
    HighlightKind::TypeBuiltin,
    HighlightKind::Constructor,
    HighlightKind::Function,
    HighlightKind::FunctionBuiltin,
    HighlightKind::Macro,
    HighlightKind::Property,
    HighlightKind::Constant,
    HighlightKind::Variable,
    HighlightKind::VariableSpecial,
    HighlightKind::Parameter,
    HighlightKind::Operator,
    HighlightKind::Punctuation,
    HighlightKind::Tag,
    HighlightKind::Attribute,
    HighlightKind::Label,
    HighlightKind::MarkupHeading,
    HighlightKind::MarkupRaw,
    HighlightKind::MarkupLink,
    HighlightKind::MarkupReference,
    HighlightKind::MarkupEmphasis,
    HighlightKind::MarkupStrong,
    HighlightKind::Embedded,
    HighlightKind::Invalid,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn fragments(language: LanguageId, source: &str) -> Vec<(String, HighlightKind)> {
        let mut out = Vec::new();
        let mut offset = 0;
        for (line, spans) in source.split_inclusive('\n').zip(highlight(language, source).unwrap()) {
            for span in spans {
                out.push((source[offset + span.range.start..offset + span.range.end].to_string(), span.kind));
            }
            offset += line.len();
        }
        out
    }

    #[test]
    fn aliases_map_only_bundled_languages() {
        assert_eq!(language_for_alias("py"), Some(LanguageId::Python));
        assert_eq!(language_for_alias("Shell"), Some(LanguageId::Bash));
        assert_eq!(language_for_alias("sql title=x"), Some(LanguageId::Sql));
        assert_eq!(language_for_alias("rust"), None);
        assert_eq!(language_for_alias(""), None);
    }

    #[test]
    fn bundled_grammars_highlight_core_roles() {
        let py = fragments(LanguageId::Python, "def f(x):\n    return 'a'  # n\n");
        assert!(py.contains(&("def".into(), HighlightKind::Keyword)), "{py:?}");
        assert!(py.contains(&("# n".into(), HighlightKind::Comment)), "{py:?}");
        let sh = fragments(LanguageId::Bash, "echo \"hi\" # c\n");
        assert!(sh.contains(&("# c".into(), HighlightKind::Comment)), "{sh:?}");
        let sql = fragments(LanguageId::Sql, "SELECT id FROM t;\n");
        assert!(sql.iter().any(|(t, k)| t == "SELECT" && *k == HighlightKind::Keyword), "{sql:?}");
    }

    #[test]
    fn spans_are_line_relative_sorted_and_utf8_safe() {
        let source = "x = 'café'\n# 注释\nprint(x)";
        let lines = highlight(LanguageId::Python, source).unwrap();
        assert_eq!(lines.len(), 3);
        for (line, spans) in source.split_inclusive('\n').zip(lines) {
            assert!(spans.windows(2).all(|p| p[0].range.end <= p[1].range.start));
            for span in spans {
                assert!(line.is_char_boundary(span.range.start) && line.is_char_boundary(span.range.end));
            }
        }
        assert_eq!(
            split_lines("café", [HighlightSpan { range: 2..4, kind: HighlightKind::Type }]),
            Err(HighlightError::InvalidUtf8Boundary { start: 2, end: 4 })
        );
    }

    #[test]
    fn oversized_source_is_rejected() {
        let big = "x".repeat(DEFAULT_MAX_SOURCE_BYTES + 1);
        assert_eq!(highlight(LanguageId::Bash, &big), Err(HighlightError::SourceTooLarge));
    }

    #[test]
    fn normalization_preserves_overlap_precedence_and_tie_order() {
        let normalized = normalize_line(vec![
            HighlightSpan {
                range: 0..10,
                kind: HighlightKind::Variable,
            },
            HighlightSpan {
                range: 2..8,
                kind: HighlightKind::Keyword,
            },
            HighlightSpan {
                range: 4..6,
                kind: HighlightKind::String,
            },
        ]);
        assert_eq!(
            normalized,
            vec![
                HighlightSpan {
                    range: 0..2,
                    kind: HighlightKind::Variable,
                },
                HighlightSpan {
                    range: 2..4,
                    kind: HighlightKind::Keyword,
                },
                HighlightSpan {
                    range: 4..6,
                    kind: HighlightKind::String,
                },
                HighlightSpan {
                    range: 6..8,
                    kind: HighlightKind::Keyword,
                },
                HighlightSpan {
                    range: 8..10,
                    kind: HighlightKind::Variable,
                },
            ]
        );
    }

}
