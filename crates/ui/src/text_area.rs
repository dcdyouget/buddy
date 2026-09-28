//! 多行文本输入框（S05-06）—— 对应 v1 Composer 的 `<textarea>`
//!
//! 以 zed `crates/gpui/examples/input.rs` 为骨架，修正 S00-05 记录的四个缺陷
//! （`docs/evidence/s00-05/ime-input.rs`），并补上 `<textarea>` 应有的能力：
//!
//! - 软换行（`shape_text` 按宽度折行）+ 自动增高到上限后内部滚动（v1 `max-height: 120px`）；
//! - 光标 / 选区、按视觉行上下移动（保持横向位置）、按词移动与删除、行首 / 行尾（macOS 快捷键）；
//! - 鼠标点击、拖选、Shift+点击、双击选词；复制 / 剪切 / 粘贴（保留换行）；撤销 / 重做；
//! - 输入法：标记文本（下划线）、候选窗定位、`characterIndexForPoint`。
//!
//! # S00-05 四个缺陷的处理
//!
//! | # | 缺陷 | 这里 |
//! |---|------|------|
//! | 1 | 有 placeholder 时 IME 查询断言崩溃 | 布局与内容不符时按位置换算，不断言 |
//! | 2 | 组字选区 `+ range.end` | 改为相对插入点 `range.start` |
//! | 3 | 多行 `shape_line` panic | 用 `shape_text`（支持 `\n` 与折行） |
//! | 4 | 组字期间选区越界、Cmd+C panic | **根因**：`new_selected_range_utf16` 相对 `new_text`，却按整段内容做 UTF-16 换算 → 插入点之前有多字节字符时越界。改为在 `new_text` 内换算，并保留防御性 clamp（回归测试 `marked_selection_with_multibyte_prefix`） |
//!
//! # 回车
//!
//! 组字期间 Enter 由 macOS 输入法消费（gpui_macos 在 `marked_text_range()` 非空时先交给 inputContext，
//! S00-05 实测 0 次到达应用）。这里再加一道：有标记文本时忽略 [`Submit`]（v1 同样判断 `isComposing`）。

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler, Entity, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, GlobalElementId, Hsla, KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PaintQuad, Pixels, Point, ScrollWheelEvent, SharedString, Style, TextAlign, TextRun, UTF16Selection, UnderlineStyle,
    Window, WrappedLine, actions, div, fill, point, prelude::*, px, relative, size,
};
use std::ops::Range;
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;

actions!(
    text_area,
    [
        /// 发送（Enter）
        Submit,
        /// 换行（Cmd / Ctrl + Enter，v1 同）
        Newline,
        /// 删除前一字符
        Backspace,
        /// 删除后一字符
        Delete,
        /// 删除前一个词（Option+Backspace）
        DeleteWordLeft,
        /// 删除到视觉行首（Cmd+Backspace）
        DeleteToLineStart,
        /// 左移
        Left,
        /// 右移
        Right,
        /// 上移一视觉行
        Up,
        /// 下移一视觉行
        Down,
        /// 向左扩选
        SelectLeft,
        /// 向右扩选
        SelectRight,
        /// 向上扩选
        SelectUp,
        /// 向下扩选
        SelectDown,
        /// 按词左移
        WordLeft,
        /// 按词右移
        WordRight,
        /// 按词向左扩选
        SelectWordLeft,
        /// 按词向右扩选
        SelectWordRight,
        /// 视觉行首
        LineStart,
        /// 视觉行尾
        LineEnd,
        /// 扩选到视觉行首
        SelectLineStart,
        /// 扩选到视觉行尾
        SelectLineEnd,
        /// 文首
        DocStart,
        /// 文末
        DocEnd,
        /// 扩选到文首
        SelectDocStart,
        /// 扩选到文末
        SelectDocEnd,
        /// 全选
        SelectAll,
        /// 复制
        Copy,
        /// 剪切
        Cut,
        /// 粘贴
        Paste,
        /// 撤销
        Undo,
        /// 重做
        Redo,
        /// 表情与符号面板
        ShowCharacterPalette,
    ]
);

/// 键位（上下文 `TextArea`）。macOS 用 Cmd，其余平台 Ctrl（Phase 09 验证）。
pub fn bind_keys(cx: &mut App) {
    let ctx = Some("TextArea");
    let cmd = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
    let k = |keys: String| keys;
    cx.bind_keys([
        KeyBinding::new("enter", Submit, ctx),
        // v1 只把 Cmd / Ctrl + Enter 当换行，Shift+Enter 同样发送（`InputDock.tsx` handleKeyDown）
        KeyBinding::new("shift-enter", Submit, ctx),
        KeyBinding::new(&k(format!("{cmd}-enter")), Newline, ctx),
        KeyBinding::new("ctrl-enter", Newline, ctx),
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("shift-backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("alt-backspace", DeleteWordLeft, ctx),
        KeyBinding::new(&k(format!("{cmd}-backspace")), DeleteToLineStart, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("shift-up", SelectUp, ctx),
        KeyBinding::new("shift-down", SelectDown, ctx),
        KeyBinding::new("alt-left", WordLeft, ctx),
        KeyBinding::new("alt-right", WordRight, ctx),
        KeyBinding::new("alt-shift-left", SelectWordLeft, ctx),
        KeyBinding::new("alt-shift-right", SelectWordRight, ctx),
        KeyBinding::new(&k(format!("{cmd}-left")), LineStart, ctx),
        KeyBinding::new(&k(format!("{cmd}-right")), LineEnd, ctx),
        KeyBinding::new(&k(format!("{cmd}-shift-left")), SelectLineStart, ctx),
        KeyBinding::new(&k(format!("{cmd}-shift-right")), SelectLineEnd, ctx),
        KeyBinding::new("home", LineStart, ctx),
        KeyBinding::new("end", LineEnd, ctx),
        KeyBinding::new(&k(format!("{cmd}-up")), DocStart, ctx),
        KeyBinding::new(&k(format!("{cmd}-down")), DocEnd, ctx),
        KeyBinding::new(&k(format!("{cmd}-shift-up")), SelectDocStart, ctx),
        KeyBinding::new(&k(format!("{cmd}-shift-down")), SelectDocEnd, ctx),
        KeyBinding::new(&k(format!("{cmd}-a")), SelectAll, ctx),
        KeyBinding::new(&k(format!("{cmd}-c")), Copy, ctx),
        KeyBinding::new(&k(format!("{cmd}-x")), Cut, ctx),
        KeyBinding::new(&k(format!("{cmd}-v")), Paste, ctx),
        KeyBinding::new(&k(format!("{cmd}-z")), Undo, ctx),
        KeyBinding::new(&k(format!("{cmd}-shift-z")), Redo, ctx),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, ctx),
    ]);
}

/// 输入框发出的事件
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextAreaEvent {
    /// 按下 Enter（非组字状态）
    Submit,
    /// 内容变化
    Changed,
}

/// 外观参数（由使用方按令牌设定）
#[derive(Clone, Debug)]
pub struct TextAreaStyle {
    /// 字号
    pub font_size: Pixels,
    /// 行高
    pub line_height: Pixels,
    /// 最大高度（超出后内部滚动）；`None` 不限
    pub max_height: Option<Pixels>,
    /// 最小高度
    pub min_height: Pixels,
    /// 文字色
    pub text_color: Hsla,
    /// 占位文字色
    pub placeholder_color: Hsla,
    /// 光标色
    pub caret_color: Hsla,
    /// 选区色
    pub selection_color: Hsla,
}

#[derive(Clone)]
struct Snapshot {
    content: String,
    selected_range: Range<usize>,
}

/// 连续输入合并为一次撤销的间隔
const UNDO_COALESCE: Duration = Duration::from_millis(800);
/// 光标闪烁周期的一半
const CARET_BLINK: Duration = Duration::from_millis(530);

struct LaidLine {
    /// 逻辑行在内容中的起始字节
    start: usize,
    /// 逻辑行字节长度（不含 `\n`）
    len: usize,
    line: WrappedLine,
    /// 相对内容顶部的 y
    top: Pixels,
    height: Pixels,
}

struct Layout {
    lines: Vec<LaidLine>,
    /// 文字区域（不含滚动偏移）
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    /// 布局所用的文本是否为 placeholder
    placeholder: bool,
}

/// 多行文本输入框
pub struct TextArea {
    focus_handle: FocusHandle,
    content: String,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    is_selecting: bool,
    /// 上下移动时保持的横向位置
    preferred_x: Option<Pixels>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last_edit_at: Option<Instant>,
    layout: Option<Layout>,
    scroll_y: Pixels,
    /// 光标闪烁：最近一次活动时刻
    last_activity: Instant,
    blink: Option<gpui::Task<()>>,
    style: TextAreaStyle,
}

impl EventEmitter<TextAreaEvent> for TextArea {}

impl Focusable for TextArea {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn utf16_to_utf8(text: &str, offset: usize) -> usize {
    let mut utf16 = 0;
    for (i, ch) in text.char_indices() {
        if utf16 >= offset {
            return i;
        }
        utf16 += ch.len_utf16();
    }
    text.len()
}

fn utf8_to_utf16(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].chars().map(char::len_utf16).sum()
}

/// 前一个字形边界
pub fn previous_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true).rev().find_map(|(i, _)| (i < offset).then_some(i)).unwrap_or(0)
}

/// 后一个字形边界
pub fn next_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true).find_map(|(i, _)| (i > offset).then_some(i)).unwrap_or(text.len())
}

/// 前一个词首（跳过空白后到词首；CJK 按 unicode 词切分）
pub fn previous_word(text: &str, offset: usize) -> usize {
    text.split_word_bound_indices()
        .rev()
        .find(|(i, w)| *i < offset && !w.trim().is_empty())
        .map_or(0, |(i, _)| i)
}

/// 后一个词尾
pub fn next_word(text: &str, offset: usize) -> usize {
    text.split_word_bound_indices()
        .find(|(i, w)| i + w.len() > offset && !w.trim().is_empty())
        .map_or(text.len(), |(i, w)| i + w.len())
}

/// 包含 `offset` 的词（双击选词）
pub fn word_range(text: &str, offset: usize) -> Range<usize> {
    text.split_word_bound_indices()
        .find(|(i, w)| offset >= *i && offset < i + w.len())
        .map_or(offset..offset, |(i, w)| i..i + w.len())
}

impl TextArea {
    /// 新建
    pub fn new(placeholder: impl Into<SharedString>, style: TextAreaStyle, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: String::new(),
            placeholder: placeholder.into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            is_selecting: false,
            preferred_x: None,
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit_at: None,
            layout: None,
            scroll_y: px(0.),
            last_activity: Instant::now(),
            blink: None,
            style,
        }
    }

    /// 当前内容
    pub fn text(&self) -> &str {
        &self.content
    }

    /// 当前选区（字节偏移，自检用）
    pub fn selected_range_for_test(&self) -> Range<usize> {
        self.selected_range.clone()
    }

    /// 是否正在组字
    pub fn is_composing(&self) -> bool {
        self.marked_range.is_some()
    }

    /// 替换全部内容（外部设置草稿 / 发送后清空），光标置末尾；清空撤销历史
    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.content = text.to_string();
        self.selected_range = self.content.len()..self.content.len();
        self.selection_reversed = false;
        self.marked_range = None;
        self.undo.clear();
        self.redo.clear();
        self.last_edit_at = None;
        self.scroll_y = px(0.);
        cx.emit(TextAreaEvent::Changed);
        cx.notify();
    }

    /// 更新外观（主题切换时）
    pub fn set_style(&mut self, style: TextAreaStyle, cx: &mut Context<Self>) {
        self.style = style;
        cx.notify();
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed { self.selected_range.start } else { self.selected_range.end }
    }

    fn touch(&mut self, cx: &mut Context<Self>) {
        self.last_activity = Instant::now();
        cx.notify();
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = offset.min(self.content.len());
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.touch(cx);
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = offset.min(self.content.len());
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.touch(cx);
    }

    /// 记录撤销点：连续的单次输入在间隔内合并
    fn record_undo(&mut self, coalesce: bool) {
        let now = Instant::now();
        let merge = coalesce && self.last_edit_at.is_some_and(|t| now.duration_since(t) < UNDO_COALESCE);
        if !merge {
            self.undo.push(Snapshot { content: self.content.clone(), selected_range: self.selected_range.clone() });
            if self.undo.len() > 200 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_edit_at = coalesce.then_some(now);
    }

    fn edit(&mut self, range: Range<usize>, text: &str, coalesce: bool, cx: &mut Context<Self>) {
        self.record_undo(coalesce);
        self.content.replace_range(range.clone(), text);
        let end = range.start + text.len();
        self.selected_range = end..end;
        self.selection_reversed = false;
        self.marked_range = None;
        self.preferred_x = None;
        cx.emit(TextAreaEvent::Changed);
        self.touch(cx);
    }

    fn delete_selection_or(&mut self, target: usize, cx: &mut Context<Self>) {
        let range = if self.selected_range.is_empty() {
            let c = self.cursor_offset();
            c.min(target)..c.max(target)
        } else {
            self.selected_range.clone()
        };
        if !range.is_empty() {
            self.edit(range, "", false, cx);
        }
    }

    // ── 位置换算（依赖最近一次布局）──

    /// 内容偏移 → 文字区域内坐标（相对内容顶部，未减滚动）
    fn position_for(&self, offset: usize) -> Option<Point<Pixels>> {
        let layout = self.layout.as_ref().filter(|l| !l.placeholder)?;
        let lh = layout.line_height;
        let line = layout.lines.iter().rev().find(|l| offset >= l.start)?;
        let local = (offset - line.start).min(line.len);
        let p = line.line.position_for_index(local, lh)?;
        Some(point(p.x, line.top + p.y))
    }

    /// 文字区域内坐标（相对内容顶部）→ 内容偏移
    fn offset_for(&self, p: Point<Pixels>) -> usize {
        let Some(layout) = self.layout.as_ref().filter(|l| !l.placeholder) else { return 0 };
        let Some(line) = layout.lines.iter().find(|l| p.y < l.top + l.height).or(layout.lines.last()) else { return 0 };
        let y = (p.y - line.top).max(px(0.)).min(line.height - px(0.5));
        let local = match line.line.closest_index_for_position(point(p.x.max(px(0.)), y), layout.line_height) {
            Ok(i) | Err(i) => i,
        };
        line.start + local.min(line.len)
    }

    fn offset_for_window_point(&self, p: Point<Pixels>) -> usize {
        match self.layout.as_ref() {
            Some(l) => self.offset_for(point(p.x - l.bounds.left(), p.y - l.bounds.top() + self.scroll_y)),
            None => 0,
        }
    }

    fn vertical(&mut self, lines: f32, select: bool, cx: &mut Context<Self>) {
        let cursor = self.cursor_offset();
        let Some(pos) = self.position_for(cursor) else { return };
        let lh = self.layout.as_ref().map_or(px(20.), |l| l.line_height);
        let x = *self.preferred_x.get_or_insert(pos.x);
        let target_y = pos.y + lh * lines + lh / 2.;
        let target = if target_y < px(0.) {
            0
        } else if self.position_for(self.content.len()).is_some_and(|end| target_y > end.y + lh) {
            self.content.len()
        } else {
            self.offset_for(point(x, target_y))
        };
        let keep = self.preferred_x;
        if select { self.select_to(target, cx) } else { self.move_to(target, cx) }
        self.preferred_x = keep;
    }

    fn visual_line_edge(&self, end: bool) -> usize {
        let cursor = self.cursor_offset();
        let Some(pos) = self.position_for(cursor) else { return cursor };
        let lh = self.layout.as_ref().map_or(px(20.), |l| l.line_height);
        let x = if end { px(1e6) } else { px(0.) };
        self.offset_for(point(x, pos.y + lh / 2.))
    }

    // ── 动作 ──

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        if !self.is_composing() {
            cx.emit(TextAreaEvent::Submit);
        }
    }
    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.selected_range.clone();
        self.edit(range, "\n", false, cx);
    }
    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        let t = previous_boundary(&self.content, self.cursor_offset());
        self.delete_selection_or(t, cx);
    }
    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        let t = next_boundary(&self.content, self.cursor_offset());
        self.delete_selection_or(t, cx);
    }
    fn delete_word_left(&mut self, _: &DeleteWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        let t = previous_word(&self.content, self.cursor_offset());
        self.delete_selection_or(t, cx);
    }
    fn delete_to_line_start(&mut self, _: &DeleteToLineStart, _: &mut Window, cx: &mut Context<Self>) {
        let t = self.visual_line_edge(false);
        self.delete_selection_or(t, cx);
    }
    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let t = if self.selected_range.is_empty() { previous_boundary(&self.content, self.cursor_offset()) } else { self.selected_range.start };
        self.move_to(t, cx);
        self.preferred_x = None;
    }
    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let t = if self.selected_range.is_empty() { next_boundary(&self.content, self.cursor_offset()) } else { self.selected_range.end };
        self.move_to(t, cx);
        self.preferred_x = None;
    }
    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1., false, cx);
    }
    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1., false, cx);
    }
    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(previous_boundary(&self.content, self.cursor_offset()), cx);
    }
    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(next_boundary(&self.content, self.cursor_offset()), cx);
    }
    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(-1., true, cx);
    }
    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(1., true, cx);
    }
    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(previous_word(&self.content, self.cursor_offset()), cx);
    }
    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(next_word(&self.content, self.cursor_offset()), cx);
    }
    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(previous_word(&self.content, self.cursor_offset()), cx);
    }
    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(next_word(&self.content, self.cursor_offset()), cx);
    }
    fn line_start(&mut self, _: &LineStart, _: &mut Window, cx: &mut Context<Self>) {
        let t = self.visual_line_edge(false);
        self.move_to(t, cx);
    }
    fn line_end(&mut self, _: &LineEnd, _: &mut Window, cx: &mut Context<Self>) {
        let t = self.visual_line_edge(true);
        self.move_to(t, cx);
    }
    fn select_line_start(&mut self, _: &SelectLineStart, _: &mut Window, cx: &mut Context<Self>) {
        let t = self.visual_line_edge(false);
        self.select_to(t, cx);
    }
    fn select_line_end(&mut self, _: &SelectLineEnd, _: &mut Window, cx: &mut Context<Self>) {
        let t = self.visual_line_edge(true);
        self.select_to(t, cx);
    }
    fn doc_start(&mut self, _: &DocStart, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }
    fn doc_end(&mut self, _: &DocEnd, _: &mut Window, cx: &mut Context<Self>) {
        let end = self.content.len();
        self.move_to(end, cx);
    }
    fn select_doc_start(&mut self, _: &SelectDocStart, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(0, cx);
    }
    fn select_doc_end(&mut self, _: &SelectDocEnd, _: &mut Window, cx: &mut Context<Self>) {
        let end = self.content.len();
        self.select_to(end, cx);
    }
    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selected_range = 0..self.content.len();
        self.selection_reversed = false;
        self.touch(cx);
    }
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.content.get(self.selected_range.clone()).filter(|t| !t.is_empty()) {
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        }
    }
    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.content.get(self.selected_range.clone()).filter(|t| !t.is_empty()) {
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
            let range = self.selected_range.clone();
            self.edit(range, "", false, cx);
        }
    }
    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        // v1 `<textarea>` 粘贴保留换行（官方示例把 `\n` 换成空格，不适用）
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = text.replace("\r\n", "\n");
            let range = self.selected_range.clone();
            self.edit(range, &text, false, cx);
        }
    }
    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(s) = self.undo.pop() {
            self.redo.push(Snapshot { content: std::mem::replace(&mut self.content, s.content), selected_range: self.selected_range.clone() });
            self.selected_range = s.selected_range;
            self.marked_range = None;
            self.last_edit_at = None;
            cx.emit(TextAreaEvent::Changed);
            self.touch(cx);
        }
    }
    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(s) = self.redo.pop() {
            self.undo.push(Snapshot { content: std::mem::replace(&mut self.content, s.content), selected_range: self.selected_range.clone() });
            self.selected_range = s.selected_range;
            self.marked_range = None;
            self.last_edit_at = None;
            cx.emit(TextAreaEvent::Changed);
            self.touch(cx);
        }
    }
    fn show_character_palette(&mut self, _: &ShowCharacterPalette, window: &mut Window, _: &mut Context<Self>) {
        window.show_character_palette();
    }

    // ── 鼠标 ──

    fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
        let offset = self.offset_for_window_point(event.position);
        if event.click_count == 2 {
            self.selected_range = word_range(&self.content, offset);
            self.selection_reversed = false;
            self.touch(cx);
            return;
        }
        if event.click_count >= 3 {
            self.selected_range = 0..self.content.len();
            self.touch(cx);
            return;
        }
        self.is_selecting = true;
        if event.modifiers.shift { self.select_to(offset, cx) } else { self.move_to(offset, cx) }
        self.preferred_x = None;
    }
    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }
    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            let offset = self.offset_for_window_point(event.position);
            self.select_to(offset, cx);
        }
    }
    fn on_scroll(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(layout) = self.layout.as_ref() else { return };
        let content_h = layout.lines.last().map_or(px(0.), |l| l.top + l.height);
        let max = (content_h - layout.bounds.size.height).max(px(0.));
        if max <= px(0.) {
            return;
        }
        let delta = event.delta.pixel_delta(window.line_height()).y;
        self.scroll_y = (self.scroll_y - delta).clamp(px(0.), max);
        cx.stop_propagation();
        cx.notify();
    }

    fn ensure_blink(&mut self, cx: &mut Context<Self>) {
        if self.blink.is_some() {
            return;
        }
        self.blink = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(CARET_BLINK).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        }));
    }

    fn caret_visible(&self) -> bool {
        (self.last_activity.elapsed().as_millis() / CARET_BLINK.as_millis()).is_multiple_of(2)
    }
}

impl EntityInputHandler for TextArea {
    fn text_for_range(&mut self, range_utf16: Range<usize>, actual_range: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        let start = utf16_to_utf8(&self.content, range_utf16.start);
        let end = utf16_to_utf8(&self.content, range_utf16.end).max(start);
        actual_range.replace(utf8_to_utf16(&self.content, start)..utf8_to_utf16(&self.content, end));
        Some(self.content[start..end].to_string())
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: utf8_to_utf16(&self.content, self.selected_range.start)..utf8_to_utf16(&self.content, self.selected_range.end),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range.as_ref().map(|r| utf8_to_utf16(&self.content, r.start)..utf8_to_utf16(&self.content, r.end))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(&mut self, range_utf16: Option<Range<usize>>, new_text: &str, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.resolve_range(range_utf16);
        // 普通键入合并撤销；输入法上屏（替换标记文本）同样视为键入
        self.edit(range, new_text, true, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.resolve_range(range_utf16);
        if self.marked_range.is_none() {
            self.record_undo(false);
        }
        self.content.replace_range(range.clone(), new_text);
        self.marked_range = (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        // 缺陷 2 / 4：新选区相对 new_text（UTF-16），须在 new_text 内换算再加插入点
        self.selected_range = new_selected_range_utf16
            .map(|r| range.start + utf16_to_utf8(new_text, r.start)..range.start + utf16_to_utf8(new_text, r.end))
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        let end = self.selected_range.end.min(self.content.len());
        self.selected_range = self.selected_range.start.min(end)..end;
        self.selection_reversed = false;
        self.preferred_x = None;
        cx.emit(TextAreaEvent::Changed);
        self.touch(cx);
    }

    fn bounds_for_range(&mut self, range_utf16: Range<usize>, _: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        let start = utf16_to_utf8(&self.content, range_utf16.start);
        let end = utf16_to_utf8(&self.content, range_utf16.end);
        let (a, b) = (self.position_for(start)?, self.position_for(end)?);
        let origin = layout.bounds.origin - point(px(0.), self.scroll_y);
        // 候选窗贴在区间所在行的下方：用起点所在行
        Some(Bounds::from_corners(origin + a, origin + point(if b.y == a.y { b.x } else { a.x }, a.y + layout.line_height)))
    }

    fn character_index_for_point(&mut self, p: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        // 缺陷 1：布局可能是 placeholder，不断言，按位置换算（空内容时为 0）
        let offset = self.offset_for_window_point(p);
        Some(utf8_to_utf16(&self.content, offset))
    }
}

impl TextArea {
    fn resolve_range(&self, range_utf16: Option<Range<usize>>) -> Range<usize> {
        let range = range_utf16
            .map(|r| utf16_to_utf8(&self.content, r.start)..utf16_to_utf8(&self.content, r.end))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let end = range.end.min(self.content.len());
        range.start.min(end)..end
    }
}

/// 输入框的绘制元素
struct TextAreaElement {
    input: Entity<TextArea>,
}

struct Prepaint {
    selections: Vec<PaintQuad>,
    caret: Option<PaintQuad>,
}

fn shape(input: &TextArea, width: Pixels, window: &mut Window) -> (Vec<LaidLine>, bool) {
    let placeholder = input.content.is_empty();
    let text: SharedString = if placeholder { input.placeholder.clone() } else { input.content.clone().into() };
    let style = window.text_style();
    let font = style.font();
    let color = if placeholder { input.style.placeholder_color } else { input.style.text_color };
    let base = TextRun { len: text.len(), font, color, background_color: None, underline: None, strikethrough: None };
    let runs: Vec<TextRun> = match input.marked_range.as_ref().filter(|_| !placeholder) {
        Some(m) => vec![
            TextRun { len: m.start, ..base.clone() },
            TextRun { len: m.end - m.start, underline: Some(UnderlineStyle { color: Some(color), thickness: px(1.), wavy: false }), ..base.clone() },
            TextRun { len: text.len() - m.end, ..base },
        ]
        .into_iter()
        .filter(|r| r.len > 0)
        .collect(),
        None => vec![base],
    };
    let lh = input.style.line_height;
    let shaped = window.text_system().shape_text(text.clone(), input.style.font_size, &runs, Some(width), None).unwrap_or_default();
    let mut lines = Vec::new();
    let (mut start, mut top) = (0usize, px(0.));
    for line in shaped {
        let len = line.len();
        let height = line.size(lh).height.max(lh);
        lines.push(LaidLine { start, len, line, top, height });
        start += len + 1;
        top += height;
    }
    (lines, placeholder)
}

impl IntoElement for TextAreaElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextAreaElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let input = self.input.clone();
        let id = window.request_measured_layout(style, move |known, available, window, cx| {
            let width = known.width.unwrap_or(match available.width {
                gpui::AvailableSpace::Definite(w) => w,
                _ => px(10_000.),
            });
            let area = input.read(cx);
            let (min_h, max_h, lh) = (area.style.min_height, area.style.max_height, area.style.line_height);
            let (lines, _) = shape(area, width, window);
            let content_h = lines.last().map_or(lh, |l| l.top + l.height);
            let h = content_h.max(min_h);
            size(width, max_h.map_or(h, |m| h.min(m)))
        });
        let _ = cx;
        (id, ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) -> Prepaint {
        let (lines, placeholder) = shape(self.input.read(cx), bounds.size.width, window);
        let line_height = self.input.read(cx).style.line_height;
        self.input.update(cx, |area, _| {
            area.layout = Some(Layout { lines, bounds, line_height, placeholder });
            // 保持光标可见（内容超过最大高度时）
            let content_h = area.layout.as_ref().and_then(|l| l.lines.last()).map_or(px(0.), |l| l.top + l.height);
            let max_scroll = (content_h - bounds.size.height).max(px(0.));
            if let Some(p) = area.position_for(area.cursor_offset()) {
                if p.y < area.scroll_y {
                    area.scroll_y = p.y;
                } else if p.y + line_height > area.scroll_y + bounds.size.height {
                    area.scroll_y = p.y + line_height - bounds.size.height;
                }
            }
            area.scroll_y = area.scroll_y.clamp(px(0.), max_scroll);
        });
        let area = self.input.read(cx);
        let origin = bounds.origin - point(px(0.), area.scroll_y);
        let mut selections = Vec::new();
        let range = area.selected_range.clone();
        if !range.is_empty() && !placeholder {
            // 按视觉行逐段画选区
            let lh = line_height;
            if let (Some(a), Some(b)) = (area.position_for(range.start), area.position_for(range.end)) {
                let mut y = a.y;
                while y <= b.y {
                    let x0 = if y == a.y { a.x } else { px(0.) };
                    let x1 = if y == b.y { b.x } else { bounds.size.width };
                    if x1 > x0 || y != b.y {
                        let x1 = if x1 <= x0 { x0 + px(4.) } else { x1 };
                        selections.push(fill(Bounds::from_corners(origin + point(x0, y), origin + point(x1, y + lh)), area.style.selection_color));
                    }
                    y += lh;
                }
            }
        }
        let caret = (range.is_empty() || placeholder)
            .then(|| {
                let p = if placeholder { Some(point(px(0.), px(0.))) } else { area.position_for(area.cursor_offset()) };
                p.map(|p| fill(Bounds::new(origin + p, size(px(1.5), line_height)), area.style.caret_color))
            })
            .flatten();
        Prepaint { selections, caret }
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), prepaint: &mut Prepaint, window: &mut Window, cx: &mut App) {
        let focus = self.input.read(cx).focus_handle.clone();
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.input.clone()), cx);
        let area = self.input.read(cx);
        let scroll_y = area.scroll_y;
        let caret_visible = area.caret_visible();
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            for q in prepaint.selections.drain(..) {
                window.paint_quad(q);
            }
            // 布局暂时取出再放回：绘制需要 `&mut App`，不能同时借用实体
            let layout = self.input.update(cx, |area, _| area.layout.take());
            if let Some(layout) = layout.as_ref() {
                let lh = layout.line_height;
                for l in &layout.lines {
                    let origin = bounds.origin + point(px(0.), l.top - scroll_y);
                    let _ = l.line.paint(origin, lh, TextAlign::Left, None, window, cx);
                }
            }
            self.input.update(cx, |area, _| area.layout = layout);
            if focus.is_focused(window)
                && caret_visible
                && let Some(c) = prepaint.caret.take()
            {
                window.paint_quad(c);
            }
        });
    }
}

impl Render for TextArea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.focus_handle.is_focused(window) {
            self.ensure_blink(cx);
        } else {
            self.blink = None;
        }
        div()
            .w_full()
            .key_context("TextArea")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .text_size(self.style.font_size)
            .line_height(self.style.line_height)
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete_to_line_start))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::line_start))
            .on_action(cx.listener(Self::line_end))
            .on_action(cx.listener(Self::select_line_start))
            .on_action(cx.listener(Self::select_line_end))
            .on_action(cx.listener(Self::doc_start))
            .on_action(cx.listener(Self::doc_end))
            .on_action(cx.listener(Self::select_doc_start))
            .on_action(cx.listener(Self::select_doc_end))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::show_character_palette))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .child(TextAreaElement { input: cx.entity() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_round_trip() {
        let s = "a你𠮷b";
        assert_eq!(utf8_to_utf16(s, s.len()), 1 + 1 + 2 + 1);
        for (i, _) in s.char_indices() {
            assert_eq!(utf16_to_utf8(s, utf8_to_utf16(s, i)), i);
        }
        assert_eq!(utf16_to_utf8(s, 999), s.len());
    }

    #[test]
    fn boundaries_respect_graphemes_and_words() {
        let s = "你好 world𠮷";
        assert_eq!(previous_boundary(s, s.len()), s.len() - "𠮷".len());
        assert_eq!(next_boundary(s, 0), "你".len());
        // 汉字按 Unicode 词边界各自成词：从末尾先到「𠮷」，再到 world
        assert_eq!(previous_word(s, s.len()), s.find("𠮷").unwrap());
        assert_eq!(previous_word(s, s.find("𠮷").unwrap()), s.find("world").unwrap());
        assert_eq!(next_word(s, 0), "你".len());
        assert_eq!(word_range("hello world", 7), 6..11);
    }
}
