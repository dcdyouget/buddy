//! 窗口入场外壳的纯状态机。
//!
//! 入场和关闭只在短时间内绘制内容淡入 / 淡出与低对比流光；原生窗口几何由 shell
//! 单独做底边锚定的短步进过渡。显示和隐藏由 runtime 触发，普通页面切换只使用
//! `DialogMotion` 的一次性流光。

use crate::{
    accessibility,
    theme_system::{
        easing,
        tokens::{metrics, motion},
    },
};
use std::time::{Duration, Instant};

/// 入场动画的阶段。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntrancePhase {
    /// 窗口已隐藏，下一次显示从紧凑起点开始。
    Hidden,
    /// 紧凑页 underlay 正在播放入场过渡。
    Entering,
    /// 窗口仍可见，正在播放关闭前的短暂流光。
    Exiting,
    /// 入场已结束，内容保持静态。
    Settled,
}

/// 一帧装饰层与内容淡入淡出的参数；不包含原生窗口尺寸。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntranceFrame {
    /// underlay 不透明度。
    pub opacity: f32,
    /// 水平缩放，中心为变换原点。
    pub scale_x: f32,
    /// 垂直缩放，中心为变换原点。
    pub scale_y: f32,
    /// underlay 圆角。
    pub radius: f32,
    /// 内容层不透明度，由透明到完全显示，关闭时反向变化。
    pub content_opacity: f32,
    /// A matching surface covers all foreground primitives together before exit.
    pub foreground_cover_opacity: f32,
    /// 流光中心的相对横坐标；允许短暂超出窗口边界。
    pub sheen_x: f32,
    /// 流光不透明度。
    pub sheen_opacity: f32,
}

/// 入场动画状态；时间只用于绘制采样，不直接触碰原生窗口 bounds。
pub(crate) struct EntranceMotion {
    phase: EntrancePhase,
    started: Instant,
    awaiting_first_frame: bool,
    start_opacity: f32,
    start_cover_opacity: f32,
}

impl Default for EntranceMotion {
    fn default() -> Self {
        Self {
            phase: EntrancePhase::Settled,
            started: Instant::now(),
            awaiting_first_frame: false,
            start_opacity: 1.0,
            start_cover_opacity: 0.0,
        }
    }
}

impl EntranceMotion {
    /// 重置为隐藏起点；隐藏操作不播放退出动画，减弱动效时直接保持稳定态。
    pub(crate) fn reset(&mut self) {
        self.reset_with_preference(accessibility::prefers_reduced_motion());
    }

    fn reset_with_preference(&mut self, reduced_motion: bool) {
        self.phase = if reduced_motion {
            EntrancePhase::Settled
        } else {
            EntrancePhase::Hidden
        };
        self.started = Instant::now();
        self.awaiting_first_frame = false;
        self.start_opacity = 0.0;
        self.start_cover_opacity = 0.0;
    }

    /// 触发一次显示入场。减弱动态效果时直接进入稳定态。
    pub(crate) fn play(&mut self) {
        self.play_with_preference(accessibility::prefers_reduced_motion());
    }

    fn play_with_preference(&mut self, reduced_motion: bool) {
        self.start_cover_opacity = if matches!(self.phase, EntrancePhase::Entering | EntrancePhase::Exiting) {
            self.current_cover_opacity(self.started.elapsed())
        } else {
            1.0
        };
        self.start_opacity = if matches!(self.phase, EntrancePhase::Entering | EntrancePhase::Exiting) {
            self.current_opacity(self.started.elapsed())
        } else {
            0.0
        };
        self.phase = if reduced_motion {
            EntrancePhase::Settled
        } else {
            EntrancePhase::Entering
        };
        self.started = Instant::now();
        self.awaiting_first_frame = !reduced_motion;
    }

    /// 开始关闭前的短暂流光。返回 `false` 表示系统要求减弱动态效果，调用方应立即隐藏。
    pub(crate) fn begin_exit(&mut self) -> bool {
        self.begin_exit_with_preference(accessibility::prefers_reduced_motion())
    }

    pub(crate) fn is_exiting(&self) -> bool {
        self.phase == EntrancePhase::Exiting
    }

    fn begin_exit_with_preference(&mut self, reduced_motion: bool) -> bool {
        self.start_cover_opacity = self.current_cover_opacity(self.started.elapsed());
        self.start_opacity = self.current_opacity(self.started.elapsed());
        self.awaiting_first_frame = false;
        if reduced_motion {
            self.phase = EntrancePhase::Hidden;
            self.started = Instant::now();
            return false;
        }
        self.phase = EntrancePhase::Exiting;
        self.started = Instant::now();
        true
    }

    /// 关闭动画时长，供运行时等待原生 `orderOut` 的安全时机。
    pub(crate) fn exit_duration() -> Duration {
        Duration::from_millis(motion::DURATION_EXIT_FOREGROUND + motion::DURATION_EXIT_SURFACE)
    }

    /// 当前动画是否仍需请求下一帧。
    ///
    /// 终态转换由 [`Self::frame`] 独占。一次 render 可能在进入时尚未超时，
    /// 但在完成布局后才跨过动画时长；此时仍须保留一次 follow-up render，
    /// 让 `frame` 读到超时并清除 underlay。
    pub(crate) fn animating(&self) -> bool {
        if !matches!(self.phase, EntrancePhase::Entering | EntrancePhase::Exiting) {
            return false;
        }
        self.animating_with_preference(accessibility::prefers_reduced_motion())
    }

    fn animating_with_preference(&self, reduced_motion: bool) -> bool {
        if reduced_motion {
            return false;
        }
        match self.phase {
            EntrancePhase::Entering => true,
            EntrancePhase::Exiting => self.started.elapsed() < Self::exit_duration(),
            _ => false,
        }
    }

    /// 当前页面动画帧；展开页仅淡入，稳定态不绘制额外动画层。
    pub(crate) fn frame(&mut self, compact: bool) -> Option<EntranceFrame> {
        // Activation / text layout can take a frame. Do not spend the entrance
        // duration while AppKit is bringing the hidden window to the front.
        if self.awaiting_first_frame {
            self.started = Instant::now();
            self.awaiting_first_frame = false;
        }
        let reduced_motion = accessibility::prefers_reduced_motion();
        self.frame_with_preference(compact, reduced_motion, self.started.elapsed())
    }

    fn frame_with_preference(
        &mut self,
        _compact: bool,
        reduced_motion: bool,
        elapsed: Duration,
    ) -> Option<EntranceFrame> {
        if self.phase == EntrancePhase::Entering && (reduced_motion || elapsed >= duration()) {
            self.phase = EntrancePhase::Settled;
        }
        if self.phase == EntrancePhase::Exiting {
            if reduced_motion {
                return Some(exit_frame_at(1.0));
            }
            let mut frame = exit_frame_at(exit_progress(elapsed));
            frame.content_opacity *= self.start_opacity;
            frame.foreground_cover_opacity = lerp(self.start_cover_opacity, 1.0, frame.foreground_cover_opacity);
            return Some(frame);
        }
        if self.phase == EntrancePhase::Hidden {
            return Some(exit_frame_at(1.0));
        }
        if self.phase != EntrancePhase::Entering {
            return None;
        }
        let mut frame = frame_at(progress(elapsed), metrics::RADIUS_FULL);
        frame.foreground_cover_opacity *= self.start_cover_opacity;
        frame.content_opacity = lerp(self.start_opacity, 1.0, frame.content_opacity);
        Some(frame)
    }

    fn current_opacity(&self, elapsed: Duration) -> f32 {
        if self.awaiting_first_frame {
            return self.start_opacity;
        }
        match self.phase {
            EntrancePhase::Hidden => 0.0,
            EntrancePhase::Settled => 1.0,
            EntrancePhase::Entering => lerp(self.start_opacity, 1.0, frame_at(progress(elapsed), metrics::RADIUS_XL).content_opacity),
            EntrancePhase::Exiting => self.start_opacity * exit_frame_at(exit_progress(elapsed)).content_opacity,
        }
    }

    fn current_cover_opacity(&self, elapsed: Duration) -> f32 {
        if self.awaiting_first_frame {
            return self.start_cover_opacity;
        }
        match self.phase {
            EntrancePhase::Exiting => lerp(self.start_cover_opacity, 1.0, exit_frame_at(exit_progress(elapsed)).foreground_cover_opacity),
            EntrancePhase::Entering => self.start_cover_opacity * frame_at(progress(elapsed), metrics::RADIUS_XL).foreground_cover_opacity,
            _ => 0.0,
        }
    }

    /// 用确定的时间采样一帧，供纯逻辑测试复用。
    #[cfg(test)]
    pub(crate) fn frame_at(elapsed: Duration) -> EntranceFrame {
        frame_at(progress(elapsed), metrics::RADIUS_FULL)
    }
}

/// 紧凑气泡展开为对话页时的一次性流光；不参与窗口几何，只给页面切换一个连续的视觉落点。
pub(crate) struct DialogMotion {
    active: bool,
    started: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DialogFrame {
    /// 流光中心的相对横坐标。
    pub sheen_x: f32,
    /// 流光不透明度。
    pub sheen_opacity: f32,
}

impl Default for DialogMotion {
    fn default() -> Self {
        Self {
            active: false,
            started: Instant::now(),
        }
    }
}

impl DialogMotion {
    pub(crate) fn play(&mut self) {
        self.active = !accessibility::prefers_reduced_motion();
        self.started = Instant::now();
    }

    pub(crate) fn frame(&mut self) -> Option<DialogFrame> {
        if !self.active {
            return None;
        }
        let elapsed = self.started.elapsed();
        if elapsed >= duration() || accessibility::prefers_reduced_motion() {
            self.active = false;
            return None;
        }
        let progress = progress(elapsed);
        let eased = easing::cubic_bezier(motion::EASE_STANDARD)(progress);
        Some(DialogFrame {
            sheen_x: lerp(-0.30, 1.08, eased),
            sheen_opacity: 0.12 * (std::f32::consts::PI * progress).sin().max(0.0),
        })
    }

    pub(crate) fn animating(&self) -> bool {
        self.active && !accessibility::prefers_reduced_motion()
    }
}

fn duration() -> Duration {
    Duration::from_millis(motion::DURATION_ENTER_SURFACE + motion::DURATION_ENTER_FOREGROUND)
}

fn progress(elapsed: Duration) -> f32 {
    let seconds = duration().as_secs_f32();
    (elapsed.as_secs_f32() / seconds).clamp(0.0, 1.0)
}

fn frame_at(progress: f32, _compact_radius: f32) -> EntranceFrame {
    let progress = progress.clamp(0.0, 1.0);
    let elapsed = progress * (motion::DURATION_ENTER_SURFACE + motion::DURATION_ENTER_FOREGROUND) as f32;
    let ease = easing::cubic_bezier(motion::EASE_STANDARD);
    let surface = ease((elapsed / motion::DURATION_ENTER_SURFACE as f32).clamp(0.0, 1.0));
    let foreground = ease(((elapsed - motion::DURATION_ENTER_SURFACE as f32) / motion::DURATION_ENTER_FOREGROUND as f32).clamp(0.0, 1.0));
    EntranceFrame {
        // The content owns the silhouette. A second bouncing silhouette behind
        // a fading composer made the entrance look like two separate steps.
        opacity: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        radius: metrics::RADIUS_XL,
        content_opacity: surface,
        foreground_cover_opacity: 1.0 - foreground,
        sheen_x: lerp(-0.34, 1.08, ease(progress)),
        sheen_opacity: 0.16 * (std::f32::consts::PI * progress).sin().max(0.0),
    }
}

fn exit_progress(elapsed: Duration) -> f32 {
    let seconds = EntranceMotion::exit_duration().as_secs_f32();
    (elapsed.as_secs_f32() / seconds).clamp(0.0, 1.0)
}

fn exit_frame_at(progress: f32) -> EntranceFrame {
    let progress = progress.clamp(0.0, 1.0);
    let elapsed = progress * (motion::DURATION_EXIT_FOREGROUND + motion::DURATION_EXIT_SURFACE) as f32;
    let ease = easing::cubic_bezier(motion::EASE_STANDARD);
    let foreground = ease((elapsed / motion::DURATION_EXIT_FOREGROUND as f32).clamp(0.0, 1.0));
    let surface = ease(((elapsed - motion::DURATION_EXIT_FOREGROUND as f32) / motion::DURATION_EXIT_SURFACE as f32).clamp(0.0, 1.0));
    EntranceFrame {
        opacity: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        radius: metrics::RADIUS_XL,
        content_opacity: 1.0 - surface,
        foreground_cover_opacity: foreground,
        sheen_x: 0.0,
        sheen_opacity: 0.0,
    }
}


fn lerp(from: f32, to: f32, amount: f32) -> f32 {
    from + (to - from) * amount.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entrance_has_one_monotonic_curve_without_a_second_silhouette() {
        let mut previous = 0.0;
        for tick in 0..=200 {
            let frame = EntranceMotion::frame_at(Duration::from_millis(tick));
            assert!(frame.content_opacity >= previous);
            assert_eq!(frame.opacity, 0.0);
            assert_eq!((frame.scale_x, frame.scale_y), (1.0, 1.0));
            assert_eq!(frame.radius, metrics::RADIUS_XL);
            previous = frame.content_opacity;
        }
        assert_eq!(EntranceMotion::frame_at(Duration::ZERO).content_opacity, 0.0);
        assert_eq!(previous, 1.0);
    }

    #[test]
    fn entrance_establishes_surface_before_revealing_foreground() {
        let mut motion = EntranceMotion::default();
        motion.play_with_preference(false);
        let mut previous_cover = 1.0;
        for ms in 0..duration().as_millis() as u64 {
            let frame = motion.frame_with_preference(true, false, Duration::from_millis(ms)).unwrap();
            if ms <= super::motion::DURATION_ENTER_SURFACE {
                assert_eq!(frame.foreground_cover_opacity, 1.0, "foreground visible at {ms}ms");
            } else {
                assert_eq!(frame.content_opacity, 1.0, "surface not ready at {ms}ms");
            }
            assert!(frame.foreground_cover_opacity <= previous_cover);
            previous_cover = frame.foreground_cover_opacity;
        }
        assert_eq!(frame_at(1.0, metrics::RADIUS_XL).foreground_cover_opacity, 0.0);
    }

    #[test]
    fn hidden_frame_remains_transparent_until_show() {
        let mut motion = EntranceMotion::default();
        motion.reset_with_preference(false);
        let frame = motion.frame_with_preference(true, false, Duration::from_secs(10)).unwrap();
        assert_eq!(frame.content_opacity, 0.0);
        assert_eq!(frame.sheen_opacity, 0.0);
        assert!(!motion.animating_with_preference(false));
    }

    #[test]
    fn entrance_clock_waits_for_first_render() {
        let mut motion = EntranceMotion::default();
        motion.play_with_preference(false);
        assert!(motion.awaiting_first_frame);
        assert_eq!(motion.current_opacity(Duration::from_secs(1)), 0.0);
    }

    #[test]
    fn interrupted_entrance_does_not_jump_to_opaque_on_exit() {
        let mut motion = EntranceMotion::default();
        motion.play_with_preference(false);
        motion.begin_exit_with_preference(false);
        assert_eq!(motion.start_opacity, 0.0);
        assert_eq!(motion.frame_with_preference(true, false, Duration::ZERO).unwrap().content_opacity, 0.0);
    }

    #[test]
    fn frame_is_clamped_after_duration() {
        let before = EntranceMotion::frame_at(duration() - Duration::from_millis(1));
        let after = EntranceMotion::frame_at(duration() + Duration::from_secs(1));
        assert!(before.opacity >= 0.0);
        assert_eq!(after, EntranceMotion::frame_at(duration()));
    }

    #[test]
    fn reset_and_play_are_the_only_triggers() {
        let mut motion = EntranceMotion::default();
        assert_eq!(motion.phase, EntrancePhase::Settled);
        motion.reset_with_preference(false);
        assert_eq!(motion.phase, EntrancePhase::Hidden);
        motion.play_with_preference(false);
        assert_eq!(motion.phase, EntrancePhase::Entering);
    }

    #[test]
    fn animating_requests_followup_when_render_crosses_duration() {
        let mut motion = EntranceMotion::default();
        motion.play_with_preference(false);

        // 模拟 render 在时长结束前进入；布局完成时已经跨过 duration。
        assert!(
            motion
                .frame_with_preference(true, false, Duration::ZERO)
                .is_some()
        );
        motion.started = Instant::now() - duration() - Duration::from_millis(1);
        assert!(motion.animating_with_preference(false));
        assert_eq!(motion.phase, EntrancePhase::Entering);
        assert!(
            motion
                .frame_with_preference(true, false, duration() + Duration::from_millis(1))
                .is_none()
        );
        assert_eq!(motion.phase, EntrancePhase::Settled);
        assert!(!motion.animating_with_preference(false));
    }

    #[test]
    fn reduced_motion_resets_and_plays_without_animation() {
        let mut motion = EntranceMotion::default();
        motion.reset_with_preference(true);
        assert_eq!(motion.phase, EntrancePhase::Settled);
        assert!(!motion.animating());
        motion.play_with_preference(true);
        assert_eq!(motion.phase, EntrancePhase::Settled);
        assert!(!motion.animating());
    }

    #[test]
    fn expanded_page_fades_in_without_compact_underlay() {
        let mut motion = EntranceMotion::default();
        motion.play_with_preference(false);
        let start = motion.frame_with_preference(false, false, Duration::ZERO).unwrap();
        assert_eq!(start.content_opacity, 0.0);
        assert_eq!(start.opacity, 0.0);
        let middle = motion.frame_with_preference(false, false, duration() / 2).unwrap();
        assert!(middle.content_opacity > 0.0 && middle.content_opacity <= 1.0);
        assert!(motion.frame_with_preference(false, false, duration()).is_none());
    }

    #[test]
    fn exit_keeps_transparent_final_frame_until_native_hide() {
        let mut motion = EntranceMotion::default();
        assert!(motion.begin_exit_with_preference(false));
        assert_eq!(motion.phase, EntrancePhase::Exiting);
        assert!(
            motion
                .frame_with_preference(false, false, Duration::ZERO)
                .is_some()
        );
        let final_frame = motion
            .frame_with_preference(false, false, EntranceMotion::exit_duration())
            .expect("exit keeps a final transparent frame until native hide");
        assert!(final_frame.content_opacity.abs() < 1e-6);
        assert_eq!(motion.phase, EntrancePhase::Exiting);
    }

    #[test]
    fn exit_removes_foreground_before_surface_and_never_reveals_it_again() {
        let foreground_end = Duration::from_millis(motion::DURATION_EXIT_FOREGROUND);
        for ms in 0..=motion::DURATION_EXIT_FOREGROUND {
            let frame = exit_frame_at(exit_progress(Duration::from_millis(ms)));
            assert_eq!(frame.content_opacity, 1.0, "surface faded before foreground at {ms}ms");
            assert_eq!(frame.sheen_opacity, 0.0);
        }
        let boundary = exit_frame_at(exit_progress(foreground_end));
        assert_eq!(boundary.foreground_cover_opacity, 1.0);
        for ms in motion::DURATION_EXIT_FOREGROUND..=EntranceMotion::exit_duration().as_millis() as u64 {
            let frame = exit_frame_at(exit_progress(Duration::from_millis(ms)));
            assert_eq!(frame.foreground_cover_opacity, 1.0, "foreground reappeared at {ms}ms");
        }
        assert_eq!(exit_frame_at(1.0).content_opacity, 0.0);
    }
}
