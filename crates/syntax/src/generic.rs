//! 通用词法高亮（Buddy）：没有内置语法的语言用一套跨语言规则着色。
//!
//! 思路同 prism 的 `clike` 语法：只识别注释、字符串、数字、关键字、函数调用、类型名、运算符与标点，
//! 不做结构解析，因此对 C / Java / JS / Go / Rust / Kotlin / Swift / C# / PHP / Ruby / YAML 等都能给出
//! 「大体正确」的颜色；个别语言的少见写法可能着色不准，但不会破坏文本。
//!
//! 规则：
//! - 注释：`//`、`/* */`、`<!-- -->`；`#` 与 `--` 仅在行首（缩进后）或空白之后且后跟空白时视为注释，
//!   以免误伤 `#include`、`#[derive]`、CSS `#id`、`i--`。
//! - 字符串：`"…"`、`'…'`（须在同一行闭合，避免把撇号 / 生命周期当字符串）、`` `…` ``、`"""…"""`、`'''…'''`（可跨行）。
//! - 标识符：关键字表 → 关键字；`true/false/null/nil/None/undefined` → 布尔；后跟 `(` → 函数；
//!   首字母大写且非全大写 → 类型；`@name` → 宏（注解 / 装饰器）；行首 `#word` → 关键字（预处理指令）。

use crate::{HighlightError, HighlightKind, HighlightSpan, split_lines};

/// 高亮任意语言的源码，返回按行、行内字节偏移的区间（与 [`crate::HighlightedDocument::lines`] 相同）。
pub fn highlight(source: &str) -> Result<Vec<Vec<HighlightSpan>>, HighlightError> {
    if source.len() > crate::DEFAULT_MAX_SOURCE_BYTES {
        return Err(HighlightError::SourceTooLarge);
    }
    split_lines(source, Lexer::new(source).run())
}

const KEYWORDS: &[&str] = &[
    "abstract", "and", "as", "async", "await", "begin", "break", "case", "catch", "class", "const",
    "continue", "def", "default", "defer", "del", "delete", "do", "done", "elif", "else", "elsif",
    "end", "enum", "esac", "except", "export", "extends", "extern", "fi", "final", "finally", "fn",
    "for", "foreach", "from", "fun", "func", "function", "go", "goto", "guard", "if", "impl",
    "implements", "import", "in", "instanceof", "interface", "is", "lambda", "let", "loop", "match",
    "mod", "module", "mut", "namespace", "new", "not", "or", "override", "package", "pass",
    "private", "protected", "pub", "public", "raise", "readonly", "ref", "return", "sealed",
    "select", "self", "static", "struct", "super", "switch", "then", "this", "throw", "throws",
    "trait", "try", "type", "typeof", "unless", "unsafe", "until", "use", "using", "val", "var",
    "virtual", "void", "when", "where", "while", "with", "yield",
];

const BUILTIN_TYPES: &[&str] = &[
    "bool", "boolean", "byte", "char", "double", "float", "f32", "f64", "i8", "i16", "i32", "i64",
    "i128", "int", "isize", "long", "short", "str", "string", "u8", "u16", "u32", "u64", "u128",
    "uint", "usize",
];

const LITERALS: &[&str] = &["true", "false", "null", "nil", "None", "True", "False", "undefined"];

const DIRECTIVES: &[&str] = &[
    "include", "define", "undef", "if", "ifdef", "ifndef", "elif", "else", "endif", "pragma",
    "import", "region", "endregion",
];

struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
    spans: Vec<HighlightSpan>,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
            spans: Vec::new(),
        }
    }

    fn run(mut self) -> Vec<HighlightSpan> {
        while self.pos < self.bytes.len() {
            let start = self.pos;
            let b = self.bytes[start];
            if b >= 0x80 {
                // 非 ASCII 字符不着色，按字符宽度前进，保证 UTF-8 边界
                self.pos += self.src[start..].chars().next().map_or(1, char::len_utf8);
            } else if self.starts_with("/*") {
                self.until_after(start + 2, "*/", HighlightKind::Comment);
            } else if self.starts_with("<!--") {
                self.until_after(start + 4, "-->", HighlightKind::Comment);
            } else if self.starts_with("//") || self.line_comment_marker() {
                self.to_line_end(start, HighlightKind::Comment);
            } else if self.starts_with("\"\"\"") || self.starts_with("'''") {
                let quote = &self.src[start..start + 3];
                self.until_after(start + 3, quote, HighlightKind::String);
            } else if b == b'"' || b == b'`' {
                self.string(start, b, b == b'`');
            } else if b == b'\'' {
                self.char_or_quote(start);
            } else if b == b'#' && self.at_line_start(start) {
                self.directive(start);
            } else if b == b'@' && self.ident_start_at(start + 1) {
                let end = self.ident_end(start + 1);
                self.push(start, end, HighlightKind::Macro);
            } else if b.is_ascii_digit() && !self.ident_before(start) {
                self.number(start);
            } else if self.ident_start_at(start) {
                self.identifier(start);
            } else if b"{}[]();,.:".contains(&b) {
                self.push(start, start + 1, HighlightKind::Punctuation);
            } else if b"+-*/%=!<>&|^~?".contains(&b) {
                let mut end = start + 1;
                while end < self.bytes.len() && b"+-*/%=!<>&|^~?".contains(&self.bytes[end]) {
                    end += 1;
                }
                self.push(start, end, HighlightKind::Operator);
            } else {
                self.pos += 1;
            }
        }
        self.spans
    }

    fn push(&mut self, start: usize, end: usize, kind: HighlightKind) {
        if start < end {
            self.spans.push(HighlightSpan {
                range: start..end,
                kind,
            });
        }
        self.pos = end;
    }

    fn starts_with(&self, prefix: &str) -> bool {
        self.src[self.pos..].starts_with(prefix)
    }

    fn line_end(&self, from: usize) -> usize {
        self.src[from..].find('\n').map_or(self.src.len(), |i| from + i)
    }

    fn to_line_end(&mut self, start: usize, kind: HighlightKind) {
        let end = self.line_end(start);
        self.push(start, end, kind);
    }

    /// 从 `from` 起找 `close`，区间包含 `close`；未闭合则到文末。
    fn until_after(&mut self, from: usize, close: &str, kind: HighlightKind) {
        let start = self.pos;
        let end = self.src[from..]
            .find(close)
            .map_or(self.src.len(), |i| from + i + close.len());
        self.push(start, end, kind);
    }

    fn at_line_start(&self, at: usize) -> bool {
        self.bytes[..at]
            .iter()
            .rev()
            .take_while(|&&c| c != b'\n')
            .all(|c| *c == b' ' || *c == b'\t')
    }

    /// `#` / `--` 注释：位于行首或空白之后，且后面是空白、行尾或同一符号。
    fn line_comment_marker(&self) -> bool {
        let at = self.pos;
        let marker = if self.bytes[at] == b'#' {
            1
        } else if self.starts_with("--") {
            2
        } else {
            return false;
        };
        let before_ok = at == 0 || matches!(self.bytes[at - 1], b' ' | b'\t' | b'\n');
        let after = self.bytes.get(at + marker).copied();
        let after_ok = matches!(after, None | Some(b' ' | b'\t' | b'\n' | b'\r'))
            || (marker == 1 && after == Some(b'#'));
        before_ok && after_ok && (marker == 1 || self.at_line_start(at))
    }

    fn string(&mut self, start: usize, quote: u8, multiline: bool) {
        let mut i = start + 1;
        while i < self.bytes.len() {
            match self.bytes[i] {
                b'\\' => i += 2,
                b'\n' if !multiline => break,
                c if c == quote => {
                    i += 1;
                    break;
                }
                _ => i += 1,
            }
        }
        self.push(start, i.min(self.bytes.len()), HighlightKind::String);
    }

    /// 单引号只在同一行闭合时作为字符串，否则（撇号）当普通字符；
    /// `<'a>`、`&'a`、`'static` 是 Rust 生命周期，不当字符串。
    fn char_or_quote(&mut self, start: usize) {
        if self.ident_start_at(start + 1) {
            let end = self.ident_end(start + 1);
            let prev = self.bytes[..start].iter().rev().find(|c| !c.is_ascii_whitespace());
            let lifetime = self.bytes.get(end) != Some(&b'\'')
                && (matches!(prev, Some(b'&' | b'<')) || &self.src[start + 1..end] == "static");
            if lifetime {
                self.pos = end;
                return;
            }
        }
        let mut i = start + 1;
        while i < self.bytes.len() && self.bytes[i] != b'\n' {
            match self.bytes[i] {
                b'\\' => i += 2,
                b'\'' => return self.push(start, i + 1, HighlightKind::String),
                _ => i += 1,
            }
        }
        self.pos = start + 1;
    }

    /// 行首 `#`：预处理指令（`#include`）为关键字；`#[` / `#!` 属性只标标点。
    fn directive(&mut self, start: usize) {
        if self.ident_start_at(start + 1) {
            let end = self.ident_end(start + 1);
            if DIRECTIVES.contains(&&self.src[start + 1..end]) {
                return self.push(start, end, HighlightKind::Keyword);
            }
        }
        self.push(start, start + 1, HighlightKind::Punctuation);
    }

    fn number(&mut self, start: usize) {
        let mut end = start;
        while end < self.bytes.len() {
            let c = self.bytes[end];
            let exponent_sign = matches!(c, b'+' | b'-')
                && matches!(self.bytes[end - 1], b'e' | b'E')
                && !self.src[start..end].starts_with("0x");
            let fraction = c == b'.' && self.bytes.get(end + 1).is_some_and(u8::is_ascii_digit);
            if c.is_ascii_alphanumeric() || c == b'_' || fraction || exponent_sign {
                end += 1;
            } else {
                break;
            }
        }
        self.push(start, end, HighlightKind::Number);
    }

    fn identifier(&mut self, start: usize) {
        let end = self.ident_end(start);
        let word = &self.src[start..end];
        let kind = if LITERALS.contains(&word) {
            Some(HighlightKind::Boolean)
        } else if KEYWORDS.contains(&word) {
            Some(HighlightKind::Keyword)
        } else if BUILTIN_TYPES.contains(&word) {
            Some(HighlightKind::TypeBuiltin)
        } else if self.src[end..].trim_start_matches([' ', '\t']).starts_with('(') {
            Some(HighlightKind::Function)
        } else if is_type_name(word) {
            Some(HighlightKind::Type)
        } else {
            None
        };
        match kind {
            Some(kind) => self.push(start, end, kind),
            None => self.pos = end,
        }
    }

    fn ident_start_at(&self, at: usize) -> bool {
        self.bytes
            .get(at)
            .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_' || *c == b'$')
    }

    fn ident_end(&self, from: usize) -> usize {
        let mut end = from;
        while end < self.bytes.len()
            && (self.bytes[end].is_ascii_alphanumeric() || matches!(self.bytes[end], b'_' | b'$'))
        {
            end += 1;
        }
        end
    }

    fn ident_before(&self, at: usize) -> bool {
        at > 0 && (self.bytes[at - 1].is_ascii_alphanumeric() || self.bytes[at - 1] == b'_')
    }
}

/// `HashMap`、`String` 是类型；`MAX_SIZE`、`A` 不是。
fn is_type_name(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && word.len() > 1
        && word.chars().any(|c| c.is_ascii_lowercase())
}

#[cfg(test)]
#[path = "generic_tests.rs"]
mod tests;
