//! S00-05 产物：**IME 正确 + 多行可用**的文本输入实现要点（macOS）
//!
//! **为什么需要这个文件**：`spikes/` 被 `.gitignore` 排除，spike 代码会丢失。
//! 本文件固化「官方示例的缺陷与修法」+「多行渲染实现」，后续 `S05-06`（Composer）直接取用。
//!
//! 实测依据见 `docs/specs/phase-00/S00-05-ime.md` 的「证据」段。
//!
//! # 核心结论
//!
//! **`crates/gpui/examples/input.rs` 只能当骨架，不能当 IME / 多行正确性参考。**
//! 它自带 4 个缺陷，其中 2 个是「一碰输入法就崩」级别。它显然只在
//! 「无 placeholder + 英文 + 单行」场景下被验证过。
//!
//! Comet 的 `composer.rs`（8768 行）不是过度设计 —— 这些边界必须自己处理。
//!
//! # 四个缺陷（均非本项目引入）
//!
//! | # | 位置 | 缺陷 | 后果 |
//! |---|------|------|------|
//! | 1 | `character_index_for_point` | 断言 `last_layout.text == self.content` 未考虑 placeholder | **一装输入法就崩** |
//! | 2 | `replace_and_mark_text_in_range` | `+ range.end` 应为 `+ range.start` | 光标越界 → 候选窗偏移 |
//! | 3 | `TextElement::prepaint` | 整段 content 一次性 `shape_line` | **多行输入 panic** |
//! | 4 | `replace_and_mark_text_in_range` | UTF-16 区间算术产出越界 `selected_range` | **组字中 Cmd+C/X 会 panic** |
//!
//! 详见各节的说明与修法。

#![allow(dead_code)]

use gpui::{Pixels, Point, ShapedLine, SharedString, TextRun};

// ═══════════════════════════════════════════════════════════════
// 缺陷 1：IME 一激活就崩
// ═══════════════════════════════════════════════════════════════
//
// ## 现象
// ```
// thread 'main' panicked at src/main.rs:463:
// assertion `left == right` failed
//   left: "Type here..."    ← placeholder
//  right: ""                ← content
// ```
// 调用链：macOS `characterIndexForPoint:` → `gpui_macos/src/window.rs:3420`
//         → 我们的 `InputHandler::character_index_for_point`
//
// ## 根因
// `prepaint` 在 `content.is_empty()` 时把 **placeholder 作为 display_text**
// 并将该布局存进 `last_layout`；而原断言拿它和 `self.content`（空串）比。
//
// `characterIndexForPoint:` 是 **IME 定位组字位置的标准查询** —— 所以只要输入法
// 激活并查询，就必然命中该断言。
//
// ## 修法（保留断言意图，补上 placeholder 情形）
//
// ```rust
// debug_assert!(
//     last_layout.text == self.content
//         || (self.content.is_empty() && last_layout.text == self.placeholder),
//     "last_layout 与当前内容/placeholder 都不匹配（layout 可能已过期）"
// );
// ```
//
// 注意：**换成英文 placeholder 同样会崩** —— 与 placeholder 的具体文案无关。

// ═══════════════════════════════════════════════════════════════
// 缺陷 2：组字期间光标越界（候选窗偏移）
// ═══════════════════════════════════════════════════════════════
//
// ## 现象（实测日志）
// ```text
// preedit="n"     marked=Some(0..1)  cursor=1   content="n"       ✓ len=1
// preedit="ni"    marked=Some(0..2)  cursor=3   content="ni"      ✗ len=2
// preedit="ni'hao" marked=Some(0..6) cursor=11  content="ni'hao"  ✗ len=6
// ```
//
// ## 根因
// ```rust
// // 原代码
// .map(|new_range| new_range.start + range.start..new_range.end + range.end)
// //                                                        ^^^^^^^^^^ 错
// ```
// `new_selected_range_utf16` 是 **相对 `new_text`** 的偏移，两端都应加 `range.start`。
// 写成 `+ range.end` 会让每轮组字多偏移 `range.end - range.start`。
//
// ## 修法
// ```rust
// .map(|new_range| range.start + new_range.start..range.start + new_range.end)
// ```

// ═══════════════════════════════════════════════════════════════
// 缺陷 3：多行输入直接 panic —— **对 Buddy 最关键**
// ═══════════════════════════════════════════════════════════════
//
// ## 现象
// ```
// panicked at gpui/src/text_system.rs:420:
// text argument should not contain newlines
// ```
//
// ## 根因
// `TextSystem::shape_line` 的文档与 `debug_assert!` 明确写着：
// > Note that this method can only shape a single line of text.
// > **It will panic if the text contains newlines.**
// > If you need to shape multiple lines of text, use `Self::shape_text` instead.
//
// 官方示例把**整段 content 一次性 `shape_line`** → **它根本无法支持多行输入**。
//
// ## 为什么对 Buddy 致命
// Buddy 的 Composer 是多行的（Shift+Enter 换行 + 自动增高）。
//
// ## 修法（本 spike 采用的方案）
//
// 按 `\n` 拆成**逻辑行**，逐行 `shape_line`，逐行 `paint`，
// 并记录每行在 content 中的起始字节偏移，供光标/选区/IME 坐标换算。
//
// 这样做的理由：`shape_text` 虽然支持软换行，但其 `WrappedLineLayout`
// 的索引 API 需要另外学习；Buddy 的 Composer 若允许水平滚动或
// 逐行渲染，拆逻辑行已够用。**若要软换行（自动折行），改用 `shape_text`。**

/// 多行布局结果 —— 本 spike 的核心新增结构
#[derive(Clone, Default)]
pub struct MultiLine {
    lines: Vec<ShapedLine>,
    /// 每行在 content 中的起始字节偏移
    starts: Vec<usize>,
    line_height: Pixels,
}

impl MultiLine {
    /// 字节偏移 → (行索引, 行内字节偏移)
    ///
    /// **内置 clamp**：越过末行时夹到末行末尾。这可以掩盖上游的越界缺陷（见缺陷 4），
    /// 但**不能因此就不修上游** —— 见缺陷 4 的说明。
    pub fn locate(&self, offset: usize) -> (usize, usize) {
        if self.lines.is_empty() {
            return (0, 0);
        }
        for (idx, &s) in self.starts.iter().enumerate() {
            let end = s + self.lines[idx].text.len();
            if offset <= end {
                return (idx, offset - s);
            }
        }
        let last = self.lines.len() - 1;
        (last, self.lines[last].text.len())
    }

    /// 字节偏移 → (行索引, 行内 x)
    pub fn x_of(&self, offset: usize) -> (usize, Pixels) {
        let (i, within) = self.locate(offset);
        (i, self.lines[i].x_for_index(within))
    }

    /// 局部坐标 → 全局字节偏移。
    ///
    /// **IME 的 `characterIndexForPoint:` 走这条路** —— 即组字位置/候选窗的定位依据。
    pub fn offset_at(&self, local_y: Pixels, local_x: Pixels) -> Option<usize> {
        if self.lines.is_empty() {
            return None;
        }
        let idx = (local_y / self.line_height).floor() as usize;
        let idx = idx.min(self.lines.len() - 1);
        let within = self.lines[idx].index_for_x(local_x)?;
        Some(self.starts[idx] + within)
    }

    pub fn total_height(&self) -> Pixels {
        self.line_height * self.lines.len() as f32
    }
}

/// 把全局 `runs` 切到某一行的字节区间 `[start, start+len)`
///
/// 必须这样做：`runs` 是相对**整段 content** 的，而行文本只占其中一段。
/// 否则下划线（组字标记）会错位到别的行。
pub fn slice_runs(runs: &[TextRun], start: usize, len: usize) -> Vec<TextRun> {
    let end = start + len;
    let mut out = Vec::new();
    let mut pos = 0usize;
    for r in runs {
        let r_start = pos;
        let r_end = pos + r.len;
        pos = r_end;
        let s = r_start.max(start);
        let e = r_end.min(end);
        if s < e {
            let mut nr = r.clone();
            nr.len = e - s;
            out.push(nr);
        }
    }
    out
}

/// 拆行 + 逐行 shape（替代官方示例的整段 `shape_line`）
///
/// ## 关键点
/// - `byte_pos += seg_len + 1` —— 那个 `+1` 是被 `split('\n')` 吃掉的换行符
/// - 每行用 `slice_runs` 取自己的 run 切片
pub fn shape_multiline(
    text_system: &gpui::TextSystem,
    display_text: &str,
    runs: &[TextRun],
    font_size: Pixels,
    line_height: Pixels,
) -> MultiLine {
    let mut lines: Vec<ShapedLine> = Vec::new();
    let mut starts: Vec<usize> = Vec::new();
    let mut byte_pos = 0usize;
    for seg in display_text.split('\n') {
        starts.push(byte_pos);
        let seg_len = seg.len();
        let seg_runs = slice_runs(runs, byte_pos, seg_len);
        let line = text_system.shape_line(
            SharedString::from(seg.to_string()),
            font_size,
            &seg_runs,
            None,
        );
        lines.push(line);
        byte_pos += seg_len + 1; // +1 为被 split 吃掉的 '\n'
    }
    MultiLine { lines, starts, line_height }
}

/// 多行版 `bounds_for_range` —— **返回 range 的屏幕矩形，即 IME 候选窗的锚点**
///
/// 官方示例的单行版用 `last_layout.x_for_index()` 并假定 y 固定；
/// 多行必须同时算出行索引。
pub fn bounds_for_range_multiline(
    ml: &MultiLine,
    bounds: gpui::Bounds<Pixels>,
    range: core::ops::Range<usize>,
) -> gpui::Bounds<Pixels> {
    let (ls, lx) = ml.x_of(range.start);
    let (le, ex) = ml.x_of(range.end);
    let lh = ml.line_height;
    gpui::Bounds::from_corners(
        gpui::point(bounds.left() + lx, bounds.top() + lh * ls as f32),
        gpui::point(bounds.left() + ex, bounds.top() + lh * (le + 1) as f32),
    )
}

/// 多行版 `character_index_for_point` —— IME 的定位查询入口
pub fn character_index_for_point_multiline(
    ml: &MultiLine,
    bounds: gpui::Bounds<Pixels>,
    position: gpui::Point<Pixels>,
) -> Option<usize> {
    let local = bounds.localize(&position)?;
    ml.offset_at(local.y, local.x)
}

// ═══════════════════════════════════════════════════════════════
// 缺陷 4：组字期间 Cmd+C / Cmd+X 会 panic
// ═══════════════════════════════════════════════════════════════
//
// ## 现象（实测日志）
// ```text
// preedit="d" marked=Some(9..10) cursor=12/len=10  content="你好\n\n\nd"
// ```
// `content` 长 10 字节，而 `selected_range.end = 12`。
//
// ## 危害
// `copy()` / `cut()` 会执行 `self.content[self.selected_range.clone()]` →
// **切片越界 panic**。（`copy` 只用 `is_empty()` 守卫，而组字期间 selected_range 非空。）
//
// ## 为什么视觉上看不出来
// 渲染层（`MultiLine::locate`）与 `bounds_for_range` 都做了 clamp，
// 所以光标画在行尾、候选窗位置也正常 —— **用户不会察觉**，但语义层已经错了。
//
// ## 修法（防御性 clamp，必须放在 `selected_range` 赋值之后）
//
// ```rust
// // 在 replace_and_mark_text_in_range 里，算完 selected_range 后立即夹住：
// let clamped_end = self.selected_range.end.min(self.content.len());
// let clamped_start = self.selected_range.start.min(clamped_end);
// self.selected_range = clamped_start..clamped_end;
// ```
//
// ## ⚠️ 上游根因未完全查清
// 官方示例的 UTF-16 ↔ UTF-8 区间换算（`range_from_utf16` / `text_for_range` /
// `new_selected_range_utf16` 的语义）存在不一致。本 spike 只做了**防御性 clamp**，
// **未定位到上游的精确错误点**。
//
// **交给 `S05-06` 的任务**：`text_for_range` 返回的 `actual_range` 与
// `new_selected_range_utf16` 的坐标系语义需要重新核对，并补一个
// 「组字期间 Cmd+C」的回归测试。

// ═══════════════════════════════════════════════════════════════
// 结论：Enter 三态**由 gpui 在平台层保证**，无需应用层特殊处理
// ═══════════════════════════════════════════════════════════════
//
// `gpui_macos/src/window.rs:2635-2695` 的逻辑：
//
// ```rust
// // is_composing 直接来自我们实现的 marked_text_range()
// let is_composing = input_handler.marked_text_range().flatten().is_some();
//
// if is_composing || is_ime_printable_key || (…) {
//     let handled: BOOL = msg_send![input_context, handleEvent: native_event];
//     if let Some(h) = do_command_handled.take() { return h as BOOL; }
//     else if handled == YES { return YES; }   // ← IME 已消费，不再走 keyDown 回调
//     let handled = run_callback(PlatformInput::KeyDown(key_down_event));
//     return handled;
// }
// ```
//
// 即：**组字期间 Enter 被 `inputContext` 消费，不会到达应用的 action 处理器。**
//
// ## 推论（重要）
// 三态正确性的**前提**是应用正确实现 `marked_text_range()`。
// 如果它返回 `None` 而实际正在组字，`is_composing` 就是 false，
// Enter 可能被当成普通按键 → **误发送**。
//
// 所以：**`marked_text_range()` 的实现必须与 `marked_range` 字段严格同步。**
//
// 实测证据：组字期间按 Enter **0 次**触发应用的 `Send` action。

// ═══════════════════════════════════════════════════════════════
// 自动增高
// ═══════════════════════════════════════════════════════════════
//
// 官方示例的容器高度是固定的（`px(30. + 4. * 2.)`），不支持增高。
// 多行版按行数计算：
//
// ```rust
// let lines = self.content.matches('\n').count() + 1;
// let visible_lines = lines.clamp(1, 6);
// let box_h = 30. * visible_lines as f32 + 4. * 2.;
// ```
//
// 注意：**这是「逻辑行数」，不是「视觉行数」**。若一行很长需要软换行，
// 必须改用 `shape_text` 拿 `WrappedLineLayout.wrap_boundaries` 才能算准高度。

// ═══════════════════════════════════════════════════════════════
// 实测通过的清单（macOS 26.4 / zed rev 290cbcb）
// ═══════════════════════════════════════════════════════════════
//
// | 项 | 结果 |
// |----|------|
// | 拼音组字 + preedit 下划线 | ✅ 34 次组字更新，`marked=Some(..)` 正确 |
// | 中文提交 | ✅ `"你好"` / `"多少啊"` / `"就恢复的师傅几点开始阿富汗"` 等 |
// | 候选窗跟随 | ✅ `bounds_for_range` 多行版；`characterIndexForPoint:` 实测被调用 |
// | Enter 三态 | ✅ 组字中 Enter **0 次**到达应用；普通 Enter 发送；Shift+Enter 换行 |
// | 多行 + 自动增高 | ✅ Shift+Enter 5 次，行数 2→6，零 panic |
// | 组字中 Cmd+C | ✅ clamp 后不崩 |
// | 光标越界 | ✅ clamp 后 0 次 |
//
// 残留未验证：输入法候选窗的**绝对像素位置**未做像素级校验（仅验证了 API 链路与 clamp）。
