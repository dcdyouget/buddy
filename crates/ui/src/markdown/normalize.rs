//! Markdown 源文本规范化—— 逐行为移植自 v1 `src/utils/markdownNormalizer.ts`（`v1-final`）
//!
//! v1 在交给解析器之前做两件事，v2 保持一致：
//!
//! 1. **粘连的结束围栏**：AI 偶尔把纯文本 / 无语言代码块的结束围栏粘在正文末尾
//!    （`对：程序员直接能写汇编```）。不修复则代码块一直延续，吞掉后续标题。
//!    只处理无语言或 `plain` / `plaintext` / `text` / `txt` 围栏，避免改写真实源码里的反引号字符串。
//! 2. **中文标点旁的加粗**：CommonMark 的「左 / 右侧定界符」规则会让紧邻中文标点的 `**` 失效
//!    （`**信息检索（IR）**和` 显示出星号）。在合法加粗内容两端插入零宽空格守卫，帮助解析器识别；
//!    显示与复制时须去掉守卫（[`EMPHASIS_GUARD`]）。行内代码与围栏代码块内部原样保留。
//!
//! 定界符 `*` / `` ` `` / `~` 均为 ASCII，UTF-8 多字节字符中不会出现 ASCII 字节 → 按字节查找安全。

/// 加粗守卫字符（零宽空格）。渲染 / 复制加粗文本时须移除（v1 `StreamingMarkdown.tsx:282`）。
pub const EMPHASIS_GUARD: char = '\u{200B}';
const GUARD_STR: &str = "\u{200B}";

/// 统一执行结构修复与加粗规范化（v1 `normalizeMarkdown`）
pub fn normalize_markdown(markdown: &str) -> String {
    normalize_emphasis(&repair_fences(markdown))
}

fn is_escaped(s: &[u8], index: usize) -> bool {
    let mut slashes = 0;
    let mut i = index;
    while i > 0 && s[i - 1] == b'\\' {
        slashes += 1;
        i -= 1;
    }
    slashes % 2 == 1
}

fn find_from(s: &str, pat: &str, from: usize) -> Option<usize> {
    s.get(from..)?.find(pat).map(|i| i + from)
}

fn find_strong_delimiter(s: &str, from: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut index = find_from(s, "**", from);
    while let Some(i) = index {
        let exact = (i == 0 || b[i - 1] != b'*') && b.get(i + 2) != Some(&b'*') && !is_escaped(b, i);
        if exact {
            return Some(i);
        }
        index = find_from(s, "**", i + 2);
    }
    None
}

fn normalize_strong_in_text(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut cursor = 0;
    while cursor < value.len() {
        let Some(opening) = find_strong_delimiter(value, cursor) else {
            result.push_str(&value[cursor..]);
            break;
        };
        let Some(closing) = find_strong_delimiter(value, opening + 2) else {
            result.push_str(&value[cursor..]);
            break;
        };
        let content = &value[opening + 2..closing];
        let can_normalize = !content.is_empty()
            && !content.starts_with(char::is_whitespace)
            && !content.ends_with(char::is_whitespace)
            && !content.starts_with(EMPHASIS_GUARD)
            && !content.ends_with(EMPHASIS_GUARD);
        result.push_str(&value[cursor..opening]);
        if can_normalize {
            result.push_str("**");
            result.push_str(GUARD_STR);
            result.push_str(content);
            result.push_str(GUARD_STR);
            result.push_str("**");
        } else {
            result.push_str(&value[opening..closing + 2]);
        }
        cursor = closing + 2;
    }
    result
}

fn find_matching_backtick_run(s: &str, delimiter: &str, from: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut index = find_from(s, delimiter, from);
    while let Some(i) = index {
        if (i == 0 || b[i - 1] != b'`') && b.get(i + delimiter.len()) != Some(&b'`') {
            return Some(i);
        }
        index = find_from(s, delimiter, i + delimiter.len());
    }
    None
}

fn normalize_line_outside_inline_code(value: &str) -> String {
    let b = value.as_bytes();
    let mut result = String::with_capacity(value.len());
    let mut text_start = 0;
    let mut cursor = 0;
    while cursor < b.len() {
        if b[cursor] != b'`' || is_escaped(b, cursor) {
            cursor += 1;
            continue;
        }
        let mut run_end = cursor + 1;
        while b.get(run_end) == Some(&b'`') {
            run_end += 1;
        }
        let delimiter = &value[cursor..run_end];
        let closing = find_matching_backtick_run(value, delimiter, run_end);
        result.push_str(&normalize_strong_in_text(&value[text_start..cursor]));
        let Some(closing) = closing else {
            result.push_str(&value[cursor..]);
            return result;
        };
        let code_end = closing + delimiter.len();
        result.push_str(&value[cursor..code_end]);
        cursor = code_end;
        text_start = code_end;
    }
    result.push_str(&normalize_strong_in_text(&value[text_start..]));
    result
}

#[derive(Clone, Copy)]
struct Fence {
    marker: u8,
    length: usize,
    can_repair_attached_close: bool,
}

/// `^ {0,3}(`{3,}|~{3,})` + 语言标签
fn opening_fence(line: &str) -> Option<Fence> {
    let b = line.as_bytes();
    let indent = b.iter().take_while(|&&c| c == b' ').count();
    if indent > 3 {
        return None;
    }
    let marker = *b.get(indent)?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let length = b[indent..].iter().take_while(|&&c| c == marker).count();
    if length < 3 {
        return None;
    }
    let language = line[indent + length..]
        .trim()
        .split(['\t', ' '])
        .next()
        .unwrap_or_default()
        .to_lowercase();
    Some(Fence {
        marker,
        length,
        can_repair_attached_close: matches!(language.as_str(), "" | "plain" | "plaintext" | "text" | "txt"),
    })
}

/// `^ {0,3}(`+|~+)[\t ]*$`，且标记相同、长度不小于开启围栏
fn is_closing_fence(line: &str, fence: Fence) -> bool {
    let b = line.as_bytes();
    let indent = b.iter().take_while(|&&c| c == b' ').count();
    if indent > 3 {
        return false;
    }
    let run = b[indent..].iter().take_while(|&&c| c == b'`' || c == b'~').count();
    if run == 0 {
        return false;
    }
    let marker_run = &b[indent..indent + run];
    let same_marker = marker_run.iter().all(|&c| c == marker_run[0]);
    let rest_blank = b[indent + run..].iter().all(|&c| c == b' ' || c == b'\t');
    same_marker && rest_blank && marker_run[0] == fence.marker && run >= fence.length
}

fn split_attached_closing_fence(line: &str, fence: Fence) -> Option<(String, String)> {
    if !fence.can_repair_attached_close {
        return None;
    }
    let trimmed = line.trim_end();
    let tb = trimmed.as_bytes();
    let mut marker_start = tb.len();
    while marker_start > 0 && tb[marker_start - 1] == fence.marker {
        marker_start -= 1;
    }
    let marker_len = tb.len() - marker_start;
    if marker_len < fence.length {
        return None;
    }
    let content = trimmed[..marker_start].trim_end();
    if content.trim().is_empty() {
        return None;
    }
    Some((content.to_string(), (fence.marker as char).to_string().repeat(marker_len)))
}

/// 修复 AI 把纯文本代码块结束围栏粘在正文末尾的情况（v1 `repairMarkdownFences`）
pub fn repair_fences(markdown: &str) -> String {
    let mut fence: Option<Fence> = None;
    let mut out: Vec<String> = Vec::new();
    for line in markdown.split('\n') {
        if let Some(f) = fence {
            if is_closing_fence(line, f) {
                out.push(line.to_string());
                fence = None;
                continue;
            }
            if let Some((content, close)) = split_attached_closing_fence(line, f) {
                out.push(content);
                out.push(close);
                fence = None;
                continue;
            }
            out.push(line.to_string());
            continue;
        }
        out.push(line.to_string());
        fence = opening_fence(line);
    }
    out.join("\n")
}

/// 规范化正文中的加粗语法，原样保留行内代码与围栏代码块（v1 `normalizeMarkdownEmphasis`）
pub fn normalize_emphasis(markdown: &str) -> String {
    let mut fence: Option<Fence> = None;
    markdown
        .split('\n')
        .map(|line| {
            if let Some(f) = fence {
                if is_closing_fence(line, f) {
                    fence = None;
                }
                return line.to_string();
            }
            if let Some(f) = opening_fence(line) {
                fence = Some(f);
                return line.to_string();
            }
            normalize_line_outside_inline_code(line)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 去掉守卫字符（显示 / 复制加粗文本时使用）
pub fn strip_guards(text: &str) -> String {
    text.replace(EMPHASIS_GUARD, "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

    /// 与 vendored `parser.rs` 的 `PARSE_OPTIONS` 一致
    fn opts() -> Options {
        Options::ENABLE_TABLES
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_SMART_PUNCTUATION
    }

    fn strong_texts(md: &str) -> Vec<String> {
        let (mut out, mut cur, mut depth) = (Vec::new(), String::new(), 0);
        for ev in Parser::new_ext(md, opts()) {
            match ev {
                Event::Start(Tag::Strong) => depth += 1,
                Event::End(TagEnd::Strong) => {
                    depth -= 1;
                    out.push(strip_guards(&std::mem::take(&mut cur)));
                }
                Event::Text(t) if depth > 0 => cur.push_str(&t),
                _ => {}
            }
        }
        out
    }

    fn plain_text(md: &str) -> String {
        Parser::new_ext(md, opts())
            .filter_map(|e| match e {
                Event::Text(t) | Event::Code(t) => Some(strip_guards(&t)),
                _ => None,
            })
            .collect()
    }

    /// v1 `StreamingMarkdown.test.tsx`「renders adjacent Chinese strong text without showing delimiters」
    #[test]
    fn chinese_strong_next_to_punctuation() {
        let src = "这些都是**信息检索（IR）**和**RAG（检索增强生成）**领域的核心概念";
        // 反证：不规范化时 CommonMark 解析失败
        assert!(strong_texts(src).len() < 2, "原文本应触发定界符失效");
        let n = normalize_markdown(src);
        assert_eq!(strong_texts(&n), vec!["信息检索（IR）", "RAG（检索增强生成）"]);
        assert_eq!(plain_text(&n), "这些都是信息检索（IR）和RAG（检索增强生成）领域的核心概念");
    }

    /// v1「keeps regular English strong syntax working」
    #[test]
    fn english_strong_still_works() {
        assert_eq!(strong_texts(&normalize_markdown("Use **hybrid search** here.")), vec!["hybrid search"]);
    }

    /// v1「does not normalize markers inside inline or fenced code」
    #[test]
    fn code_is_untouched() {
        let src = "行内：`const value = \"**（值）**\"`\n\n```md\n**（代码）**\n```";
        let n = normalize_markdown(src);
        assert!(!n.contains(EMPHASIS_GUARD), "代码中不得插入守卫：{n:?}");
        assert!(strong_texts(&n).is_empty());
    }

    /// v1「repairs an attached plain-text closing fence before parsing headings」
    #[test]
    fn attached_plain_fence_is_repaired() {
        let src = [
            "### 复用 ISA 的好处", "", "```", "对：编译器天然支持", "对：程序员直接能写汇编```", "",
            "### 自研 ISA 的代价", "", "```", "错：软件生态需要从头建", "错：时间长、成本高", "```",
        ]
        .join("\n");
        // 反证：不修复时代码块吞掉后续内容，第二个标题不会作为标题出现
        let raw_headings = Parser::new_ext(&src, opts()).filter(|e| matches!(e, Event::Start(Tag::Heading { .. }))).count();
        assert_eq!(raw_headings, 1, "原文本应只解析出 1 个标题");
        let n = normalize_markdown(&src);
        let (mut headings, mut blocks, mut in_h, mut in_code) = (Vec::new(), Vec::<String>::new(), false, false);
        for ev in Parser::new_ext(&n, opts()) {
            match ev {
                Event::Start(Tag::Heading { .. }) => {
                    in_h = true;
                    headings.push(String::new())
                }
                Event::End(TagEnd::Heading(_)) => in_h = false,
                Event::Start(Tag::CodeBlock(_)) => {
                    in_code = true;
                    blocks.push(String::new())
                }
                Event::End(TagEnd::CodeBlock) => in_code = false,
                Event::Text(t) if in_h => headings.last_mut().unwrap().push_str(&t),
                Event::Text(t) if in_code => blocks.last_mut().unwrap().push_str(&t),
                _ => {}
            }
        }
        assert_eq!(headings, vec!["复用 ISA 的好处", "自研 ISA 的代价"]);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].contains("对：程序员直接能写汇编") && !blocks[0].contains("自研 ISA 的代价"));
    }

    #[test]
    fn language_fences_are_not_rewritten() {
        // 带语言的代码块：源码里的 ``` 字符串不得被当作粘连围栏拆开
        let src = "```rust\nlet s = \"```\";\n```";
        assert_eq!(repair_fences(src), src);
    }

    #[test]
    fn escaped_and_triple_stars_are_left_alone() {
        assert_eq!(normalize_markdown("\\**不是加粗**"), "\\**不是加粗**");
        assert_eq!(normalize_markdown("***粗斜体***"), "***粗斜体***");
        assert_eq!(normalize_markdown("** 两侧空白 **"), "** 两侧空白 **");
    }
}
