//! 选中文本的复制内容（S04-09）—— 与 v1（WebKit）选区纯文本一致
//!
//! v1 的复制即 WebKit 对选区的纯文本序列化（实测见 `docs/evidence/s04-09/`）。它与上游
//! 「每个渲染行之间一个 `\n`」有两处不同，这里按 WebKit 规则补齐：
//!
//! 1. **段落与小标题后多一个空行**：WebKit 对 `<p>` / `<h1>`–`<h6>` 在「折叠后的下外边距 × 2 ≥ 字号」时
//!    多输出一个换行（`TextIterator::shouldEmitExtraNewlineForNode`）。按 v1 CSS：
//!    段落下边距 8px、字号 14px → 空行，**引用块中最后一个段落**下边距为 0 → 无；
//!    h1 17.5px / h2 16.1px（8 × 2 < 字号）→ 无，h3 14.7px 及 h4–h6 14px → 空行。
//! 2. **表格同一行的单元格以制表符分隔**（粘贴到表格软件即成列）。
//!
//! 另有两处**有意不同**：v1 会把代码块头部的语言标签与「复制」按钮文字、任务项复选框前的空格一并复制，
//! 这些是界面控件而非消息内容，v2 不复制。加粗守卫（零宽空格，S04-05）一律去掉。

use super::normalize::strip_guards;
use super::zed_markdown::{
    ParsedMarkdown,
    parser::{MarkdownEvent, MarkdownTag, MarkdownTagEnd},
};
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// 段落；`bool` = 其后是否多一个空行
    Paragraph(bool),
    Heading(u8),
    /// 表格单元格；值为所在行的起点（同一行的单元格相同）
    Cell(usize),
    Other,
}

struct Block {
    range: Range<usize>,
    kind: Kind,
}

fn blocks(parsed: &ParsedMarkdown) -> Vec<Block> {
    let events = parsed.events();
    let mut out = Vec::new();
    let mut stack: Vec<(Range<usize>, &MarkdownTag)> = Vec::new();
    let mut row_start = 0;
    for (i, (range, event)) in events.iter().enumerate() {
        match event {
            MarkdownEvent::Start(tag) => {
                if matches!(tag, MarkdownTag::TableHead | MarkdownTag::TableRow) {
                    row_start = range.start;
                }
                stack.push((range.clone(), tag));
            }
            MarkdownEvent::End(end) => {
                let Some((range, tag)) = stack.pop() else { continue };
                let kind = match (tag, end) {
                    (MarkdownTag::Paragraph, _) => {
                        let closes_quote = matches!(events.get(i + 1), Some((_, MarkdownEvent::End(MarkdownTagEnd::BlockQuote(_)))));
                        Kind::Paragraph(!closes_quote)
                    }
                    (MarkdownTag::Heading { level, .. }, _) => Kind::Heading(*level as u8),
                    (MarkdownTag::TableCell, _) => Kind::Cell(row_start),
                    _ => Kind::Other,
                };
                out.push(Block { range, kind });
            }
            _ => {}
        }
    }
    out
}

/// 包含 `at` 的最内层块
fn innermost(blocks: &[Block], at: usize) -> Option<&Block> {
    blocks.iter().filter(|b| b.range.start <= at && at < b.range.end).min_by_key(|b| b.range.len())
}

fn extra_newline(kind: Kind) -> bool {
    match kind {
        Kind::Paragraph(extra) => extra,
        Kind::Heading(level) => level >= 3,
        _ => false,
    }
}

/// 由选中的渲染行（及其源区间）生成剪贴板文本
pub fn copy_text(lines: &[(String, Range<usize>)], parsed: &ParsedMarkdown) -> String {
    let blocks = blocks(parsed);
    let block_of = |r: &Range<usize>| innermost(&blocks, r.start).map(|b| (b.range.clone(), b.kind));
    let mut out = String::new();
    for (i, (text, range)) in lines.iter().enumerate() {
        out.push_str(text);
        let Some((_, next_range)) = lines.get(i + 1) else { break };
        let sep = match (block_of(range), block_of(next_range)) {
            (Some((a, Kind::Cell(row_a))), Some((b, Kind::Cell(row_b)))) if row_a == row_b && a != b => "\t",
            (Some((a, kind)), next) if next.as_ref().map(|n| &n.0) != Some(&a) && extra_newline(kind) => "\n\n",
            _ => "\n",
        };
        out.push_str(sep);
    }
    strip_guards(&out)
}
