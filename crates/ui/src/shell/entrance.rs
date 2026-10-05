//! 窗口入场外壳的纯状态机。
//!
//! 入场只影响紧凑页的装饰 underlay；Composer 与对话内容保持静态，原生窗口也不逐帧
//! resize。显示时由 runtime 触发，隐藏时立即 reset；普通页面切换不会调用本模块。

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
    /// 入场已结束，内容保持静态。
    Settled,
}

/// 一帧紧凑 underlay 的装饰参数；不包含窗口尺寸或内容变换。
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
}

/// 入场动画状态；时间只用于绘制采样，不触碰原生窗口 bounds。
pub(crate) struct EntranceMotion {
    phase: EntrancePhase,
    started: Instant,
}

impl Default for EntranceMotion {
    fn default() -> Self {
        Self {
            phase: EntrancePhase::Settled,
            started: Instant::now(),
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
    }

    /// 触发一次显示入场。减弱动态效果时直接进入稳定态。
    pub(crate) fn play(&mut self) {
        self.play_with_preference(accessibility::prefers_reduced_motion());
    }

    fn play_with_preference(&mut self, reduced_motion: bool) {
        self.phase = if reduced_motion {
            EntrancePhase::Settled
        } else {
            EntrancePhase::Entering
        };
        self.started = Instant::now();
    }

    /// 直接进入稳定态；展开页没有 underlay 入场。
    pub(crate) fn settle(&mut self) {
        self.phase = EntrancePhase::Settled;
        self.started = Instant::now();
    }

    /// 当前动画是否仍需请求下一帧。
    ///
    /// 终态转换由 [`Self::frame`] 独占。一次 render 可能在进入时尚未超时，
    /// 但在完成布局后才跨过动画时长；此时仍须保留一次 follow-up render，
    /// 让 `frame` 读到超时并清除 underlay。
    pub(crate) fn animating(&self) -> bool {
        if self.phase != EntrancePhase::Entering {
            return false;
        }
        self.animating_with_preference(accessibility::prefers_reduced_motion())
    }

    fn animating_with_preference(&self, reduced_motion: bool) -> bool {
        self.phase == EntrancePhase::Entering && !reduced_motion
    }

    /// 当前紧凑 underlay 帧；展开页和稳定态不渲染额外 underlay。
    pub(crate) fn frame(&mut self, compact: bool) -> Option<EntranceFrame> {
        let reduced_motion =
            self.phase == EntrancePhase::Entering && accessibility::prefers_reduced_motion();
        self.frame_with_preference(compact, reduced_motion, self.started.elapsed())
    }

    fn frame_with_preference(
        &mut self,
        compact: bool,
        reduced_motion: bool,
        elapsed: Duration,
    ) -> Option<EntranceFrame> {
        if self.phase == EntrancePhase::Entering && (reduced_motion || elapsed >= duration()) {
            self.phase = EntrancePhase::Settled;
        }
        if !compact || self.phase != EntrancePhase::Entering {
            return None;
        }
        Some(frame_at(progress(elapsed), metrics::RADIUS_FULL))
    }

    /// 用确定的时间采样一帧，供纯逻辑测试复用。
    #[cfg(test)]
    pub(crate) fn frame_at(elapsed: Duration) -> EntranceFrame {
        frame_at(progress(elapsed), metrics::RADIUS_FULL)
    }

}

fn duration() -> Duration {
    Duration::from_millis(motion::DURATION_NORMAL as u64)
}

fn progress(elapsed: Duration) -> f32 {
    let seconds = duration().as_secs_f32();
    (elapsed.as_secs_f32() / seconds).clamp(0.0, 1.0)
}

fn frame_at(progress: f32, compact_radius: f32) -> EntranceFrame {
    let progress = progress.clamp(0.0, 1.0);
    let opacity = segment(progress, 0.0, 0.48, 0.18, 0.58)
        .or_else(|| segment(progress, 0.48, 0.76, 0.58, 0.34))
        .or_else(|| segment(progress, 0.76, 1.0, 0.34, 0.0))
        .unwrap_or(0.0);
    let scale_x = segment(progress, 0.0, 0.76, 0.94, 1.006)
        .or_else(|| segment(progress, 0.76, 1.0, 1.006, 1.0))
        .unwrap_or(1.0);
    let scale_y = segment(progress, 0.0, 0.76, 0.88, 1.008)
        .or_else(|| segment(progress, 0.76, 1.0, 1.008, 1.0))
        .unwrap_or(1.0);
    let radius = segment(progress, 0.0, 0.76, compact_radius, metrics::RADIUS_XL)
        .or_else(|| segment(progress, 0.76, 1.0, metrics::RADIUS_XL, metrics::RADIUS_XL))
        .unwrap_or(metrics::RADIUS_XL);
    EntranceFrame {
        opacity,
        scale_x,
        scale_y,
        radius,
    }
}

fn segment(progress: f32, start: f32, end: f32, from: f32, to: f32) -> Option<f32> {
    if progress < start || progress > end {
        return None;
    }
    let local = (progress - start) / (end - start);
    let eased = easing::cubic_bezier(motion::EASE_STANDARD)(local);
    Some(lerp(from, to, eased))
}

fn lerp(from: f32, to: f32, amount: f32) -> f32 {
    from + (to - from) * amount.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_matches_v1_endpoints_without_window_resize() {
        assert_eq!(duration(), Duration::from_millis(200));
        let start = EntranceMotion::frame_at(Duration::ZERO);
        let end = EntranceMotion::frame_at(duration());
        assert!((start.opacity - 0.18).abs() < 1e-6);
        assert!((start.scale_x - 0.94).abs() < 1e-6);
        assert!((start.scale_y - 0.88).abs() < 1e-6);
        assert!((start.radius - metrics::RADIUS_FULL).abs() < 1e-6);
        assert!(end.opacity.abs() < 1e-6);
        assert!((end.scale_x - 1.0).abs() < 1e-6);
        assert!((end.scale_y - 1.0).abs() < 1e-6);
        assert!((end.radius - metrics::RADIUS_XL).abs() < 1e-6);
    }

    #[test]
    fn frame_uses_standard_curve_and_midpoint_keyframe() {
        let middle = EntranceMotion::frame_at(Duration::from_millis(100));
        assert!(middle.opacity > 0.5);
        assert!(middle.scale_x > 1.0);
        assert!(middle.scale_y > 1.0);
    }

    #[test]
    fn frame_honors_v1_keyframe_boundaries() {
        let first = EntranceMotion::frame_at(Duration::from_millis(96));
        let second = EntranceMotion::frame_at(Duration::from_millis(152));
        assert!((first.opacity - 0.58).abs() < 1e-6);
        assert!((second.opacity - 0.34).abs() < 1e-6);
        assert!((second.scale_x - 1.006).abs() < 1e-6);
        assert!((second.scale_y - 1.008).abs() < 1e-6);
        assert!((second.radius - metrics::RADIUS_XL).abs() < 1e-6);
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
}
