//! 流式显示（S04-06）—— 逐项移植 v1 的三部分行为（`v1-final`）
//!
//! 1. **节奏**（[`Pacer`]，v1 `useSmoothTextRenderer.ts`）：后端增量先入缓冲，按约 50 字/秒、
//!    每秒最多 25 次小批量放出；积压多时追赶（每批至多 16 字）；窗口隐藏时立即放出全部，
//!    重新显示后 600ms 内保持立即放出（清理补发事件），之后恢复逐字速度。
//! 2. **落定**（[`tail`] + [`settle_progress`]，v1 `createStreamingEffectPlugin` 与 `.streaming-char-settle`）：
//!    流式中「不稳定尾段」（最后一个空行之后、未闭合围栏之前）的最后 9 个可见字符（不含代码、空白）
//!    从星光白、不透明度 0.58 在 260ms 内过渡到正常样式；越旧的字符动画越靠后（每级提前 32ms）；
//!    每批新字符到达时整体重新开始。
//! 3. **星标**（[`Star`]，v1 `.streaming-next-star`）：紧跟最后一个落定字符；尾段为空时单独成行；
//!    围栏未闭合时不显示。呼吸：1.1s 循环，不透明度 0.72↔1、缩放 0.74↔1.16，每批重新开始。
//!
//! 系统开启「减弱动态效果」时，落定与星标呼吸都不播放（v1 `prefers-reduced-motion`）。

use super::normalize::EMPHASIS_GUARD;
use crate::theme_system::{easing::cubic_bezier, tokens::motion};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

// ── 1. 节奏 ──────────────────────────────────────────────────────

/// 基础速度：字/秒（v1 `STREAMING_CHARACTERS_PER_SECOND`）
pub const CHARACTERS_PER_SECOND: usize = 50;
/// 每秒最多放出次数（v1 `STREAMING_UPDATES_PER_SECOND`）
pub const UPDATES_PER_SECOND: usize = 25;
/// 放出间隔（毫秒）
pub const UPDATE_INTERVAL_MS: f64 = 1000.0 / UPDATES_PER_SECOND as f64;
/// 每批最少字数：`ceil(50 / 25)`
pub const BASE_CHARACTERS_PER_UPDATE: usize = CHARACTERS_PER_SECOND.div_ceil(UPDATES_PER_SECOND);
/// 每批最多字数（v1 `MAX_CHARACTERS_PER_UPDATE`）
pub const MAX_CHARACTERS_PER_UPDATE: usize = 16;
/// 重新显示后保持立即放出的时长（毫秒，v1 `RESUME_CATCH_UP_DURATION`）
pub const RESUME_CATCH_UP_MS: f64 = 600.0;

/// 流式文本的放出节奏。时间一律为毫秒（调用方提供单调时钟）。
#[derive(Debug, Default)]
pub struct Pacer {
    pending: String,
    next_reveal_at: Option<f64>,
    hidden: bool,
    resume_until: Option<f64>,
}

impl Pacer {
    /// 新建（窗口可见）
    pub fn new() -> Self {
        Self::default()
    }

    /// 缓冲中尚未放出的文本
    pub fn pending(&self) -> &str {
        &self.pending
    }

    fn immediate(&self, now: f64) -> bool {
        self.hidden || self.resume_until.is_some_and(|until| now < until)
    }

    /// 收到增量。隐藏或刚恢复显示时立即返回全部待放出文本，否则等 [`Self::tick`]。
    pub fn push(&mut self, delta: &str, now: f64) -> Option<String> {
        self.pending.push_str(delta);
        if self.immediate(now) { self.flush() } else { None }
    }

    /// 每帧调用；到点则返回本批放出的文本
    pub fn tick(&mut self, now: f64) -> Option<String> {
        if self.resume_until.is_some_and(|until| now >= until) {
            self.resume_until = None;
            self.next_reveal_at = None;
        }
        if self.immediate(now) {
            return self.flush();
        }
        if self.pending.is_empty() {
            self.next_reveal_at = None;
            return None;
        }
        let next = *self.next_reveal_at.get_or_insert(now);
        if now < next {
            return None;
        }
        // v1 以 UTF-16 长度估算积压量、以 Unicode 字符为单位放出
        let backlog = self.pending.encode_utf16().count();
        let count = backlog.div_ceil(UPDATES_PER_SECOND).clamp(BASE_CHARACTERS_PER_UPDATE, MAX_CHARACTERS_PER_UPDATE);
        let split = self.pending.char_indices().nth(count).map_or(self.pending.len(), |(i, _)| i);
        let batch: String = self.pending.drain(..split).collect();
        let scheduled = next + UPDATE_INTERVAL_MS;
        self.next_reveal_at = Some(if now - scheduled > UPDATE_INTERVAL_MS { now + UPDATE_INTERVAL_MS } else { scheduled });
        Some(batch)
    }

    /// 放出全部缓冲（结束 / 出错前调用，防止丢字）
    pub fn flush(&mut self) -> Option<String> {
        self.next_reveal_at = None;
        (!self.pending.is_empty()).then(|| std::mem::take(&mut self.pending))
    }

    /// 窗口隐藏 / 失焦：立即放出，之后到达的也立即放出
    pub fn hide(&mut self) -> Option<String> {
        self.hidden = true;
        self.resume_until = None;
        self.flush()
    }

    /// 窗口重新显示：放出积压，并在 600ms 内保持立即放出
    pub fn show(&mut self, now: f64) -> Option<String> {
        self.hidden = false;
        self.resume_until = Some(now + RESUME_CATCH_UP_MS);
        self.flush()
    }
}

// ── 2 / 3. 尾段、落定与星标 ──────────────────────────────────────

/// 参与落定效果的字符数（v1 `STREAM_SETTLE_TRAIL_LENGTH`）
pub const SETTLE_TRAIL_LENGTH: usize = 9;
/// 落定起始不透明度（v1 关键帧 0%）
pub const SETTLE_START_OPACITY: f32 = 0.58;
/// 星标呼吸：不透明度与缩放的两端（v1 关键帧 0%/100% 与 50%）
pub const STAR_OPACITY: (f32, f32) = (0.72, 1.0);
/// 星标呼吸缩放
pub const STAR_SCALE: (f32, f32) = (0.74, 1.16);

/// 星标位置
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Star {
    /// 画在该源字节位置（最后一个字符之后）
    After(usize),
    /// 尾段为空：单独成行显示在消息末尾
    Standalone,
    /// 不显示（未在流式中，或围栏未闭合）
    Hidden,
}

/// 一帧的尾段效果：落定字符（源字节位置，年龄 0 = 最新）与星标
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tail {
    /// 落定字符：(字符起点的源字节位置, 年龄)
    pub settle: Vec<(usize, usize)>,
    /// 星标
    pub star: Star,
    /// 未闭合围栏的起点（v1 此时以纯文本代码块显示、不显示星标）
    pub open_fence: Option<usize>,
}

/// 未闭合围栏（v1 `openingFence` / `closesFence`）
struct Fence {
    marker: u8,
    length: usize,
    start: usize,
}

fn opening_fence(line: &str, start: usize) -> Option<Fence> {
    let indent = line.bytes().take_while(|&b| b == b' ').count();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let marker = *rest.as_bytes().first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let length = rest.bytes().take_while(|&b| b == marker).count();
    if length < 3 || (marker == b'`' && rest[length..].contains('`')) {
        return None;
    }
    Some(Fence { marker, length, start })
}

fn closes_fence(line: &str, fence: &Fence) -> bool {
    let indent = line.bytes().take_while(|&b| b == b' ').count();
    if indent > 3 {
        return false;
    }
    let rest = &line[indent..];
    let run = rest.bytes().take_while(|&b| b == fence.marker).count();
    run >= fence.length && rest[run..].bytes().all(|b| b == b' ' || b == b'\t')
}

/// v1 `partitionStreamingMarkdown`：(稳定段终点, 未闭合围栏起点)。只看已完整的行（以 `\n` 结尾）。
pub fn partition(source: &str) -> (usize, Option<usize>) {
    let (mut boundary, mut fence, mut cursor) = (0, None::<Fence>, 0);
    while let Some(newline) = source[cursor..].find('\n').map(|i| cursor + i) {
        let line = source[cursor..newline].strip_suffix('\r').unwrap_or(&source[cursor..newline]);
        let next = newline + 1;
        match &fence {
            Some(f) => {
                if closes_fence(line, f) {
                    fence = None;
                }
            }
            None => {
                fence = opening_fence(line, cursor);
                if fence.is_none() && line.trim().is_empty() {
                    boundary = next;
                }
            }
        }
        cursor = next;
    }
    (boundary, fence.map(|f| f.start))
}

/// 计算一帧的尾段效果。`reveal_count` 为最近一批放出的字数（v1 `revealCount`）。
pub fn tail(source: &str, streaming: bool, reveal_count: usize) -> Tail {
    let (boundary, open_fence) = partition(source);
    let hidden = Tail { settle: Vec::new(), star: Star::Hidden, open_fence };
    if !streaming {
        return hidden;
    }
    let unstable = boundary..open_fence.unwrap_or(source.len());
    if unstable.is_empty() {
        return Tail { star: if open_fence.is_some() { Star::Hidden } else { Star::Standalone }, ..hidden };
    }
    // 尾段中可参与效果的字符（文档顺序）：文本节点中非空白、非代码的字符
    let mut chars: Vec<Range<usize>> = Vec::new();
    let text = &source[unstable.clone()];
    let opts = Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
    let mut in_code_block = false;
    for (event, range) in Parser::new_ext(text, opts).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(_)) => in_code_block = true,
            Event::End(TagEnd::CodeBlock) => in_code_block = false,
            // 只有与源文本逐字一致的文本才能按字节定位（实体 / 转义除外，渲染器同样不给它们加效果）
            Event::Text(t) if !in_code_block && *t == text[range.clone()] => {
                for (i, ch) in t.char_indices() {
                    if !ch.is_whitespace() && ch != EMPHASIS_GUARD {
                        let at = unstable.start + range.start + i;
                        chars.push(at..at + ch.len_utf8());
                    }
                }
            }
            _ => {}
        }
    }
    let Some(last) = chars.last() else {
        return Tail { star: Star::Hidden, ..hidden };
    };
    let star = Star::After(last.end);
    let settle = if reveal_count > 0 {
        chars.iter().rev().take(SETTLE_TRAIL_LENGTH).enumerate().map(|(age, r)| (r.start, age)).collect()
    } else {
        Vec::new()
    };
    Tail { settle, star, open_fence }
}

/// 落定进度（已缓动，0..=1）：批次放出后经过 `since_batch_ms`，年龄 `age` 的字符
///
/// v1：每级 `animation-delay` 提前 32ms（`--delay-streaming-char-age-N`），时长 260ms，`--ease-standard`
pub fn settle_progress(since_batch_ms: f64, age: usize) -> f32 {
    let delay = age as f64 * -f64::from(motion::DELAY_STREAMING_CHAR_AGE_1);
    let t = ((since_batch_ms + delay) / f64::from(motion::DURATION_STREAMING_CHAR_SETTLE)).clamp(0.0, 1.0) as f32;
    cubic_bezier(motion::EASE_STANDARD)(t)
}

/// 星标呼吸：(不透明度, 缩放)。v1 关键帧 0%/100% 为低值、50% 为高值，每段 `--ease-standard`，无限循环
pub fn star_breath(since_batch_ms: f64) -> (f32, f32) {
    let period = f64::from(motion::DURATION_STREAMING_STAR_BREATHE);
    let phase = (since_batch_ms.max(0.0) % period / period) as f32;
    let ease = cubic_bezier(motion::EASE_STANDARD);
    let k = if phase < 0.5 { ease(phase * 2.0) } else { 1.0 - ease((phase - 0.5) * 2.0) };
    let lerp = |(a, b): (f32, f32)| a + (b - a) * k;
    (lerp(STAR_OPACITY), lerp(STAR_SCALE))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 节奏：v1 useSmoothTextRenderer.test.tsx 的 5 个用例 + 速率 ──

    #[test]
    fn visible_reveals_in_small_batches() {
        // v1「窗口可见时按低频小批量消费」：入队后等动画帧，第一帧即放出（2 字 ≤ 每批最少 2 字）
        let mut p = Pacer::new();
        assert_eq!(p.push("正常", 0.0), None);
        assert_eq!(p.pending(), "正常");
        assert_eq!(p.tick(0.0).as_deref(), Some("正常"));
        assert_eq!(p.pending(), "");
    }

    #[test]
    fn hidden_reveals_immediately() {
        // v1「Esc 隐藏后立即消费后续缓冲，不依赖动画帧」。v1 用例含 emoji 测 UTF-16 代理对；
        // 硬约束 4 禁止源码出现 emoji，改用同为代理对的 CJK 扩展 B 字「𠮷」（U+20BB7）
        let mut p = Pacer::new();
        assert_eq!(p.hide(), None);
        assert_eq!(p.push("后台继续𠮷", 0.0).as_deref(), Some("后台继续𠮷"));
        assert_eq!("𠮷".encode_utf16().count(), 2);
        assert_eq!(p.pending(), "");
    }

    #[test]
    fn hidden_handles_many_fragments() {
        // v1「后台立即消费大量碎片正文，不发生递归堆栈或遗留队列」
        let mut p = Pacer::new();
        p.hide();
        let mut out = String::new();
        for _ in 0..4000 {
            out.push_str(&p.push("字", 0.0).unwrap());
        }
        assert_eq!(out.chars().count(), 4000);
        assert_eq!(p.pending(), "");
    }

    #[test]
    fn resume_catches_up_then_returns_to_pacing() {
        // v1「重新显示时清理补发事件，随后恢复逐字速度」
        let mut p = Pacer::new();
        p.hide();
        p.show(0.0);
        assert_eq!(p.push("积压", 10.0).as_deref(), Some("积压"));
        assert_eq!(p.tick(600.0), None); // 600ms 后恢复逐字
        assert_eq!(p.push("新字", 601.0), None);
        assert_eq!(p.pending(), "新字");
    }

    #[test]
    fn show_after_hidden_start_still_paces_later() {
        // v1「隐藏窗口丢失旧动画帧后，恢复时仍能启动正文渲染」：隐藏状态启动 → 显示 → 600ms 后正文照常放出
        let mut p = Pacer::new();
        p.hide();
        p.show(0.0);
        assert_eq!(p.tick(600.0), None);
        assert_eq!(p.push("正文", 700.0), None);
        assert_eq!(p.tick(700.0).as_deref(), Some("正文"));
    }

    #[test]
    fn rate_is_50_per_second_and_catches_up() {
        let mut p = Pacer::new();
        // 少量积压：每 40ms 放 2 字 → 50 字/秒
        p.push(&"字".repeat(10), 0.0);
        let sizes: Vec<usize> = (0..5).filter_map(|i| p.tick(i as f64 * 40.0)).map(|b| b.chars().count()).collect();
        assert_eq!(sizes, vec![2; 5]);
        // 大量积压：ceil(400/25)=16（上限）
        p.push(&"字".repeat(400), 200.0);
        assert_eq!(p.tick(200.0).unwrap().chars().count(), 16);
        // 未到下一次放出时刻不放
        assert_eq!(p.tick(210.0), None);
        // 严重落后（卡顿）后不补放多批，而是从当前时刻重新排期
        assert!(p.tick(1000.0).is_some());
        assert_eq!(p.tick(1001.0), None);
        assert!(p.tick(1040.0).is_some());
    }

    // ── 尾段：v1 StreamingMarkdown.test.tsx 的流式用例 ──

    fn aged(src: &str, tail: &Tail) -> Vec<(char, usize)> {
        tail.settle.iter().map(|&(i, age)| (src[i..].chars().next().unwrap(), age)).collect()
    }

    #[test]
    fn settles_latest_characters_and_star_after_last() {
        // v1 `:151`「settles the latest characters and keeps a star at the next position」
        let src = "第一段。\n\n正在输出的第二段文字内容";
        let t = tail(src, true, 3);
        let got = aged(src, &t);
        assert_eq!(got.len(), SETTLE_TRAIL_LENGTH);
        assert_eq!(got[0], ('容', 0));
        assert_eq!(got[8], ('出', 8)); // 从末尾数第 9 个：容内字文段二第的出
        assert_eq!(t.star, Star::After(src.len()));
        // 稳定段（第一段）不参与
        assert!(t.settle.iter().all(|&(i, _)| i >= "第一段。\n\n".len()));
    }

    #[test]
    fn star_anchors_to_last_visible_list_character() {
        // v1 `:199`「anchors the star to the final visible list character」：尾部换行 / 空白不占位
        let src = "- 第一项\n- 第二项\n";
        let t = tail(src, true, 2);
        assert_eq!(t.star, Star::After(src.find("第二项").unwrap() + "第二项".len()));
        assert_eq!(aged(src, &t)[0], ('项', 0));
    }

    #[test]
    fn standalone_star_before_first_character() {
        // v1 `:214`「shows a breathing star before the first character arrives」
        assert_eq!(tail("", true, 0).star, Star::Standalone);
        // 段落刚结束（尾段为空）同样单独显示
        assert_eq!(tail("完成的一段。\n\n", true, 0).star, Star::Standalone);
        // 不在流式中：无星标
        assert_eq!(tail("文字", false, 3).star, Star::Hidden);
    }

    #[test]
    fn code_is_excluded() {
        // v1 `:230`「does not add the character transition to code content」
        let src = "说明 `inline code`";
        let t = tail(src, true, 5);
        let chars: String = aged(src, &t).iter().map(|(c, _)| *c).collect();
        assert_eq!(chars, "明说");
    }

    #[test]
    fn open_fence_hides_star() {
        // v1 `:245`「keeps an open streaming fence plain until the fence closes」：未闭合围栏 → 无星标
        let src = "开始\n```rust\nfn main() {\n";
        let t = tail(src, true, 3);
        assert_eq!(t.open_fence, Some(src.find("```").unwrap()));
        assert_eq!(t.star, Star::After(src.find("开始").unwrap() + "开始".len()));
        let closed = format!("{src}}}\n```\n");
        assert_eq!(tail(&closed, true, 3).open_fence, None);
        // 只有未闭合围栏、前面无文字：不显示星标
        assert_eq!(tail("```\ncode\n", true, 3).star, Star::Hidden);
    }

    #[test]
    fn no_settle_without_new_characters() {
        let t = tail("文字", true, 0);
        assert!(t.settle.is_empty());
        assert_eq!(t.star, Star::After("文字".len()));
    }

    #[test]
    fn guards_are_not_counted() {
        let src = super::super::normalize::normalize_markdown("**加粗（IR）**和");
        let t = tail(&src, true, 3);
        assert!(aged(&src, &t).iter().all(|(c, _)| *c != EMPHASIS_GUARD));
    }

    #[test]
    fn settle_timing_matches_v1_delays() {
        assert_eq!(settle_progress(0.0, 0), 0.0);
        assert_eq!(settle_progress(260.0, 0), 1.0);
        // 年龄 8 提前 256ms：批次刚到时已接近完成
        assert!(settle_progress(0.0, 8) > 0.99);
        // 单调
        assert!(settle_progress(100.0, 0) < settle_progress(100.0, 1));
    }

    #[test]
    fn star_breath_cycle() {
        assert_eq!(star_breath(0.0), (STAR_OPACITY.0, STAR_SCALE.0));
        let (o, s) = star_breath(550.0);
        assert!((o - STAR_OPACITY.1).abs() < 1e-4 && (s - STAR_SCALE.1).abs() < 1e-4);
        let (o, s) = star_breath(1100.0);
        assert!((o - STAR_OPACITY.0).abs() < 1e-4 && (s - STAR_SCALE.0).abs() < 1e-4);
    }
}

// ── 接入渲染 ─────────────────────────────────────────────────────

use super::zed_markdown::{MarkdownOverlay, MarkdownStyle, MarkdownVeil};
use crate::icons::{IconName, icon};
use crate::theme_system::{Theme, tokens::metrics as m};
use gpui::{AnyElement, BoxShadow, Transformation, div, point, prelude::*, px, size};
use std::collections::HashMap;
use std::sync::Arc;

/// 星标元素（12px 四角星 + 光晕）。`reduce_motion` 时不呼吸（v1 `animation: none`，即缩放 1、不透明度 1）。
///
/// v1 星标为白 → 蓝 → 白的 135° 渐变并带三层 drop-shadow；GPUI 的 SVG 为单色蒙版，
/// 这里以蓝色星形 + 其下的圆形光晕近似（目检项）。
pub fn star_element(theme: &Theme, since_batch_ms: f64, reduce_motion: bool) -> AnyElement {
    let c = theme.colors;
    let (opacity, scale) = if reduce_motion { (1.0, 1.0) } else { star_breath(since_batch_ms) };
    let glow = div()
        .absolute()
        .top(px(m::SPACE_3 / 2.0 - 1.0))
        .left(px(m::SPACE_3 / 2.0 - 1.0))
        .size(px(2.0))
        .rounded(px(m::RADIUS_FULL))
        .shadow(vec![
            BoxShadow { color: c.streaming_star_white.into(), offset: point(px(0.), px(0.)), blur_radius: px(2.), spread_radius: px(0.), inset: false },
            BoxShadow { color: c.streaming_star_blue.into(), offset: point(px(0.), px(0.)), blur_radius: px(5.), spread_radius: px(0.), inset: false },
            BoxShadow { color: c.streaming_star_blue_soft.into(), offset: point(px(0.), px(0.)), blur_radius: px(9.), spread_radius: px(0.), inset: false },
        ]);
    div()
        .relative()
        .flex_none()
        .size(px(m::SPACE_3))
        .opacity(opacity)
        .child(glow)
        .child(
            icon(IconName::StreamingStar, px(m::SPACE_3))
                .text_color(c.streaming_star_blue)
                .with_transformation(Transformation::scale(size(scale, scale))),
        )
        .into_any_element()
}

/// 把一帧的尾段效果装进消息样式：落定字符的颜色渐变（veil）与行内星标（overlay）。
/// [`Star::Standalone`] 由调用方在消息末尾单独放置 [`star_element`]。
///
/// 返回是否仍有动画在进行（调用方据此 `window.request_animation_frame()`）。
pub fn decorate(style: &mut MarkdownStyle, tail: &Tail, since_batch_ms: f64, theme: &Theme, reduce_motion: bool) -> bool {
    let theme = *theme;
    let settling = !reduce_motion && tail.settle.iter().any(|&(_, age)| settle_progress(since_batch_ms, age) < 1.0);
    if settling {
        let ages: HashMap<usize, usize> = tail.settle.iter().copied().collect();
        style.decorations.veil = Some(MarkdownVeil {
            start_color: theme.colors.streaming_star_white.into(),
            start_opacity: SETTLE_START_OPACITY,
            progress: Arc::new(move |at| ages.get(&at).map(|&age| settle_progress(since_batch_ms, age))),
        });
    }
    if let Star::After(at) = tail.star {
        style.decorations.overlay = Some(MarkdownOverlay {
            source_index: at,
            build: Arc::new(move |line_height, _, _| {
                // v1 `vertical-align: middle`：在行框内垂直居中
                div().h(line_height).flex().items_center().child(star_element(&theme, since_batch_ms, reduce_motion)).into_any_element()
            }),
        });
    }
    settling || (!reduce_motion && tail.star != Star::Hidden)
}
