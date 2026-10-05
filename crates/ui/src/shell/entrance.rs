//! 窗口呼入 / 呼出的阶段状态机。
//!
//! 动效本身由 [`super::window_motion`] 在原生图层上合成（缩放 + 淡入淡出）；这里只记录
//! 阶段，供运行时判断「是否需要动画」「呼出途中是否被再次呼入」，以及等待呼出完成再
//! `orderOut`。GPUI 内容始终按静态帧绘制。普通页面切换只使用 `DialogMotion` 的一次性流光。

use crate::{
    accessibility,
    theme_system::{easing, tokens::motion},
};
use std::time::{Duration, Instant};

/// 呼入 / 呼出的阶段。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntrancePhase {
    /// 窗口已隐藏，下一次显示播放呼入。
    Hidden,
    /// 呼入动效进行中。
    Entering,
    /// 窗口仍可见，正在播放呼出动效。
    Exiting,
    /// 动效结束，窗口静止可见。
    Settled,
}

/// 呼入 / 呼出状态；时间只用于判断阶段，不触碰原生窗口。
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
    /// 窗口已原生隐藏：下一次显示从呼入起点开始。
    pub(crate) fn reset(&mut self) {
        self.phase = EntrancePhase::Hidden;
        self.started = Instant::now();
    }

    /// 开始呼入。返回 `false` 表示系统要求减弱动态效果，调用方不播放原生动画。
    pub(crate) fn play(&mut self) -> bool {
        self.play_with_preference(accessibility::prefers_reduced_motion())
    }

    fn play_with_preference(&mut self, reduced_motion: bool) -> bool {
        self.phase = if reduced_motion {
            EntrancePhase::Settled
        } else {
            EntrancePhase::Entering
        };
        self.started = Instant::now();
        !reduced_motion
    }

    /// 开始呼出。返回 `false` 表示减弱动态效果，调用方应立即隐藏。
    pub(crate) fn begin_exit(&mut self) -> bool {
        self.begin_exit_with_preference(accessibility::prefers_reduced_motion())
    }

    fn begin_exit_with_preference(&mut self, reduced_motion: bool) -> bool {
        self.phase = EntrancePhase::Exiting;
        self.started = Instant::now();
        !reduced_motion
    }

    pub(crate) fn is_exiting(&self) -> bool {
        self.phase == EntrancePhase::Exiting
    }

    /// 当前阶段；呼入超过落定时长即视为静止。
    pub(crate) fn phase(&self) -> EntrancePhase {
        self.phase_at(self.started.elapsed())
    }

    fn phase_at(&self, elapsed: Duration) -> EntrancePhase {
        if self.phase == EntrancePhase::Entering && elapsed >= enter_duration() {
            EntrancePhase::Settled
        } else {
            self.phase
        }
    }

    /// 呼出动画时长，供运行时等待原生 `orderOut` 的安全时机。
    pub(crate) fn exit_duration() -> Duration {
        Duration::from_millis(motion::DURATION_DISMISS)
    }
}

fn enter_duration() -> Duration {
    Duration::from_millis(motion::DURATION_SUMMON)
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
        if elapsed >= dialog_duration() || accessibility::prefers_reduced_motion() {
            self.active = false;
            return None;
        }
        let progress = (elapsed.as_secs_f32() / dialog_duration().as_secs_f32()).clamp(0.0, 1.0);
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

fn dialog_duration() -> Duration {
    Duration::from_millis(motion::DURATION_NORMAL as u64)
}

fn lerp(from: f32, to: f32, amount: f32) -> f32 {
    from + (to - from) * amount.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_then_play_enters_and_settles() {
        let mut motion = EntranceMotion::default();
        assert_eq!(motion.phase, EntrancePhase::Settled);
        motion.reset();
        assert_eq!(motion.phase(), EntrancePhase::Hidden);
        assert!(motion.play_with_preference(false));
        assert_eq!(motion.phase_at(Duration::ZERO), EntrancePhase::Entering);
        assert_eq!(motion.phase_at(enter_duration()), EntrancePhase::Settled);
    }

    #[test]
    fn reduced_motion_skips_native_animation() {
        let mut motion = EntranceMotion::default();
        assert!(!motion.play_with_preference(true));
        assert_eq!(motion.phase(), EntrancePhase::Settled);
        assert!(!motion.begin_exit_with_preference(true));
        assert!(motion.is_exiting());
    }

    #[test]
    fn exit_holds_until_native_hide() {
        let mut motion = EntranceMotion::default();
        assert!(motion.begin_exit_with_preference(false));
        assert_eq!(motion.phase_at(Duration::from_secs(10)), EntrancePhase::Exiting);
        motion.reset();
        assert_eq!(motion.phase(), EntrancePhase::Hidden);
    }

    #[test]
    fn dismiss_is_quicker_than_summon() {
        assert!(EntranceMotion::exit_duration() < enter_duration());
    }
}
