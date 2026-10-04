use super::*;

/// (文本, 类别) 列表，便于断言。
fn tokens(source: &str) -> Vec<(String, HighlightKind)> {
    let lines = highlight(source).unwrap();
    let mut out = Vec::new();
    let mut offset = 0;
    for (line, spans) in source.split_inclusive('\n').zip(lines) {
        for span in spans {
            out.push((
                source[offset + span.range.start..offset + span.range.end].to_string(),
                span.kind,
            ));
        }
        offset += line.len();
    }
    out
}

fn has(tokens: &[(String, HighlightKind)], text: &str, kind: HighlightKind) -> bool {
    tokens.iter().any(|(t, k)| t == text && *k == kind)
}

#[test]
fn c_like_languages_get_core_roles() {
    let t = tokens("// note\nfn main() {\n    let s = \"hi\\n\"; let n = 0x1F + 3.5e-2;\n}\n");
    assert!(has(&t, "// note", HighlightKind::Comment), "{t:?}");
    assert!(has(&t, "fn", HighlightKind::Keyword), "{t:?}");
    assert!(has(&t, "let", HighlightKind::Keyword), "{t:?}");
    assert!(has(&t, "main", HighlightKind::Function), "{t:?}");
    assert!(has(&t, "\"hi\\n\"", HighlightKind::String), "{t:?}");
    assert!(has(&t, "0x1F", HighlightKind::Number), "{t:?}");
    assert!(has(&t, "3.5e-2", HighlightKind::Number), "{t:?}");
    assert!(has(&t, "{", HighlightKind::Punctuation), "{t:?}");
    assert!(has(&t, "=", HighlightKind::Operator), "{t:?}");
}

#[test]
fn types_literals_annotations_and_builtins() {
    let t = tokens("@Override\npublic String name(int x) { return null; }\nconst MAX_SIZE = true;\n");
    assert!(has(&t, "@Override", HighlightKind::Macro), "{t:?}");
    assert!(has(&t, "String", HighlightKind::Type), "{t:?}");
    assert!(has(&t, "int", HighlightKind::TypeBuiltin), "{t:?}");
    assert!(has(&t, "null", HighlightKind::Boolean), "{t:?}");
    assert!(has(&t, "true", HighlightKind::Boolean), "{t:?}");
    // 全大写常量不当类型
    assert!(!t.iter().any(|(s, _)| s == "MAX_SIZE"), "{t:?}");
}

#[test]
fn hash_is_comment_only_when_followed_by_space() {
    let t = tokens("# yaml comment\nkey: value # trailing\n#include <stdio.h>\n#[derive(Debug)]\ncolor: #fff\n");
    assert!(has(&t, "# yaml comment", HighlightKind::Comment), "{t:?}");
    assert!(has(&t, "# trailing", HighlightKind::Comment), "{t:?}");
    assert!(has(&t, "#include", HighlightKind::Keyword), "{t:?}");
    assert!(!t.iter().any(|(s, k)| s.starts_with("#[") && *k == HighlightKind::Comment), "{t:?}");
    assert!(!t.iter().any(|(s, k)| s.contains("#fff") && *k == HighlightKind::Comment), "{t:?}");
}

#[test]
fn multiline_comments_and_strings_span_lines() {
    let t = tokens("/* a\nb */ x = `t\nu`; y = \"\"\"p\nq\"\"\"\n<!-- h\n-->\n");
    assert!(has(&t, "/* a", HighlightKind::Comment) && has(&t, "b */", HighlightKind::Comment), "{t:?}");
    assert!(has(&t, "`t", HighlightKind::String) && has(&t, "u`", HighlightKind::String), "{t:?}");
    assert!(has(&t, "\"\"\"p", HighlightKind::String) && has(&t, "q\"\"\"", HighlightKind::String), "{t:?}");
    assert!(has(&t, "<!-- h", HighlightKind::Comment), "{t:?}");
}

#[test]
fn apostrophes_and_decrements_are_not_strings_or_comments() {
    let t = tokens("fn f<'a>(x: &'a str) -> &'static str { i--; }\n-- lua comment\nc = 'x'; s = 'it''s'\n");
    assert!(has(&t, "'x'", HighlightKind::String), "{t:?}");
    // 生命周期 'a / 'static 不当字符串
    assert!(!t.iter().any(|(s, k)| (s.contains("'a") || s.contains("static")) && *k == HighlightKind::String), "{t:?}");
    assert!(has(&t, "--", HighlightKind::Operator), "{t:?}");
    assert!(has(&t, "-- lua comment", HighlightKind::Comment), "{t:?}");
}

#[test]
fn unicode_and_unterminated_input_keep_valid_ranges() {
    for source in ["let café = \"naïve", "/* 未闭合注释", "x = '中\nprint(\"表情 \u{1F642}\")", "\"\\", "@", "0x"] {
        let lines = highlight(source).unwrap();
        let mut offset = 0;
        for (line, spans) in source.split_inclusive('\n').zip(lines) {
            let mut last = 0;
            for span in spans {
                assert!(span.range.start >= last && span.range.end <= line.len(), "{source:?}");
                assert!(line.is_char_boundary(span.range.start) && line.is_char_boundary(span.range.end));
                last = span.range.end;
            }
            offset += line.len();
        }
        assert_eq!(offset, source.len());
    }
}

#[test]
fn oversized_source_is_rejected() {
    let big = "x".repeat(crate::DEFAULT_MAX_SOURCE_BYTES + 1);
    assert_eq!(highlight(&big), Err(HighlightError::SourceTooLarge));
}
