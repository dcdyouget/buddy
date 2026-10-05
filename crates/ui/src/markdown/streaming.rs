//! 流式显示—— 逐项移植 v1 的三部分行为（`v1-final`）
//!
//! 1. **节奏**（[`Pacer`]）：网络文本进入 FIFO 队列，前台固定每秒约 60 个字素逐个放出，
//!    不随积压加速；后台立即同步，恢复显示后的新文本仍正常排队。
//! 2. **落定**：字符首次呈现时间由 `reveal::RevealTimeline` 维护；每个新字独立渐显，
//!    后续批次不会重播旧字的动画。此模块保留尾段定位和星标装饰。
//! 3. **星标**（[`Star`]，v1 `.streaming-next-star`）：紧跟最后一个落定字符；尾段为空时单独成行；
//!    围栏未闭合时不显示。呼吸：1.1s 循环，不透明度 0.72↔1、缩放 0.74↔1.16，每批重新开始。
//!
//! 系统开启「减弱动态效果」时，落定与星标呼吸都不播放（v1 `prefers-reduced-motion`）。

use super::normalize::EMPHASIS_GUARD;
use crate::theme_system::{easing::cubic_bezier, tokens::motion};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

// ── 1. 节奏 ──────────────────────────────────────────────────────

/// 固定显示速度：每秒 60 个 Unicode 字素（一个汉字或完整 emoji 算一个）。
pub const CHARACTERS_PER_SECOND: usize = 60;
/// 每个呈现时隙最多出队一个字素，不随积压量加速。
pub const UPDATES_PER_SECOND: usize = CHARACTERS_PER_SECOND;
/// 放出间隔（毫秒）。
pub const UPDATE_INTERVAL_MS: f64 = 1000.0 / UPDATES_PER_SECOND as f64;

/// FIFO 文本队列；消费游标避免每出一个字都搬移整段积压文本。
/// 时间由调用方提供单调毫秒时钟，空队列不积攒可突发消费的额度。
#[derive(Debug, Default)]
pub struct Pacer {
    pending: String,
    head: usize,
    next_reveal_at: Option<f64>,
    hidden: bool,
}

impl Pacer {
    /// 新建（窗口可见）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 队列中尚未呈现的文本。
    pub fn pending(&self) -> &str {
        &self.pending[self.head..]
    }

    /// 网络增量只入队；后台窗口不播放逐字动画，直接保留完整内容。
    pub fn push(&mut self, delta: &str, now: f64) -> Option<String> {
        if self.head >= 4096 && self.head >= self.pending.len() / 2 {
            self.pending.drain(..self.head);
            self.head = 0;
        }
        self.pending.push_str(delta);
        if self.hidden {
            self.flush()
        } else {
            self.next_reveal_at.get_or_insert(now);
            None
        }
    }

    /// 每帧调用；固定时隙出队一个完整字素。卡顿后不一次性补出多字。
    pub fn tick(&mut self, now: f64) -> Option<String> {
        use unicode_segmentation::UnicodeSegmentation;
        if self.hidden {
            return self.flush();
        }
        if self.pending().is_empty() {
            return None;
        }
        let next = *self.next_reveal_at.get_or_insert(now);
        if now + 1e-6 < next {
            return None;
        }
        let grapheme = self.pending().graphemes(true).next()?.to_owned();
        self.head += grapheme.len();
        if self.head == self.pending.len() {
            self.pending.clear();
            self.head = 0;
        }
        self.next_reveal_at = Some(if now - next >= UPDATE_INTERVAL_MS {
            now + UPDATE_INTERVAL_MS
        } else {
            next + UPDATE_INTERVAL_MS
        });
        Some(grapheme)
    }

    /// 立即放出全部队列（用户停止、出错或后台时）；正常 Done 等队列自然排空。
    pub fn flush(&mut self) -> Option<String> {
        self.next_reveal_at = None;
        if self.pending().is_empty() {
            return None;
        }
        let text = self.pending()[..].to_owned();
        self.pending.clear();
        self.head = 0;
        Some(text)
    }

    /// 窗口隐藏：更新完整内容，无需保持不可见的打字动画。
    pub fn hide(&mut self) -> Option<String> {
        self.hidden = true;
        self.flush()
    }

    /// 恢复显示时只同步已有积压；此后新到达的字立即恢复固定速率。
    pub fn show(&mut self, _now: f64) -> Option<String> {
        self.hidden = false;
        self.flush()
    }
}

// ── 2 / 3. 尾段、落定与星标 ──────────────────────────────────────

/// 参与落定效果的字符数（v1 `STREAM_SETTLE_TRAIL_LENGTH`）
pub const SETTLE_TRAIL_LENGTH: usize = 9;
/// 落定起始不透明度（v1 关键帧 0%）
pub const SETTLE_START_OPACITY: f32 = motion::STREAMING_SETTLE_START_OPACITY;
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

    #[test]
    fn fixed_rate_does_not_accelerate_with_backlog() {
        for count in [2, 60, 4000] {
            let mut p = Pacer::new();
            assert_eq!(p.push(&"字".repeat(count), 0.0), None);
            assert_eq!(p.tick(0.0).as_deref(), Some("字"));
            assert_eq!(p.tick(1.0), None);
            assert_eq!(p.tick(UPDATE_INTERVAL_MS).as_deref(), Some("字"));
            assert_eq!(p.pending().chars().count(), count - 2);
        }
    }

    #[test]
    fn steady_clock_consumes_sixty_graphemes_in_one_second() {
        let mut p = Pacer::new();
        p.push(&"文".repeat(120), 0.0);
        let output: String = (0..1000).filter_map(|ms| p.tick(ms as f64)).collect();
        assert_eq!(output.chars().count(), CHARACTERS_PER_SECOND);
    }

    #[test]
    fn packet_boundaries_do_not_change_rate_or_order() {
        let mut whole = Pacer::new();
        let mut fragments = Pacer::new();
        whole.push("流畅输出中文abcdef", 0.0);
        for fragment in ["流", "畅输", "出中文", "a", "bcdef"] {
            fragments.push(fragment, 0.0);
        }
        for tick in 0..20 {
            let now = tick as f64 * UPDATE_INTERVAL_MS;
            assert_eq!(whole.tick(now), fragments.tick(now));
        }
    }

    #[test]
    fn empty_queue_and_delayed_frames_do_not_create_bursts() {
        let mut p = Pacer::new();
        p.push("甲", 0.0);
        assert_eq!(p.tick(0.0).as_deref(), Some("甲"));
        assert_eq!(p.tick(1.0), None);
        p.push("乙丙丁", 2.0);
        assert_eq!(p.tick(2.0), None);
        assert_eq!(p.tick(1000.0).as_deref(), Some("乙"));
        assert_eq!(p.tick(1001.0), None);
        assert_eq!(p.tick(1000.0 + UPDATE_INTERVAL_MS).as_deref(), Some("丙"));
    }

    #[test]
    fn complex_graphemes_and_fifo_survive_compaction() {
        let mut p = Pacer::new();
        let prefix = "字".repeat(1500);
        p.push(&prefix, 0.0);
        let mut rendered = String::new();
        for i in 0..1400 {
            rendered.push_str(&p.tick(i as f64 * UPDATE_INTERVAL_MS).unwrap());
        }
        p.push("👩‍💻e\u{301}🇨🇳", 1400.0 * UPDATE_INTERVAL_MS);
        for i in 1400..1500 {
            rendered.push_str(&p.tick(i as f64 * UPDATE_INTERVAL_MS).unwrap());
        }
        assert_eq!(rendered, prefix);
        for (i, expected) in ["👩‍💻", "e\u{301}", "🇨🇳"].iter().enumerate() {
            assert_eq!(p.tick((1500 + i) as f64 * UPDATE_INTERVAL_MS).as_deref(), Some(*expected));
        }
        assert!(p.pending().is_empty());
    }

    #[test]
    fn hiding_flushes_but_showing_does_not_bypass_pacing() {
        let mut p = Pacer::new();
        p.push("已有积压", 0.0);
        assert_eq!(p.hide().as_deref(), Some("已有积压"));
        assert_eq!(p.push("后台继续𠮷", 1.0).as_deref(), Some("后台继续𠮷"));
        p.show(2.0);
        assert_eq!(p.push("新字", 3.0), None);
        assert_eq!(p.tick(3.0).as_deref(), Some("新"));
        assert_eq!(p.pending(), "字");
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
