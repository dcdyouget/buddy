//! v1 SlideInPanel 的 200ms 侧滑 / 淡入淡出；退出立即释放输入，而绘制继续到动画结束。

use crate::{
    accessibility,
    theme_system::{easing, tokens::motion},
};
use std::time::{Duration, Instant};

/// 覆盖层的可见性与动画进度，不管理窗口尺寸或业务状态。
pub struct SlideMotion {
    shown: bool,
    from: f32,
    started: Instant,
}

impl Default for SlideMotion {
    fn default() -> Self {
        Self {
            shown: false,
            from: 0.0,
            started: Instant::now(),
        }
    }
}

impl SlideMotion {
    /// 改变可见性。快速反向时从当前进度继续，不重置到另一端。
    pub fn set_shown(&mut self, shown: bool) {
        if self.shown != shown {
            self.from = self.amount();
            self.started = Instant::now();
            self.shown = shown;
        }
    }

    /// 是否接受输入；关闭即 false，不等待动画结束。
    pub fn interactive(&self) -> bool {
        self.shown
    }

    /// 当前可见量，0 为窗口右侧且透明，1 为覆盖全窗口。
    pub fn amount(&self) -> f32 {
        if accessibility::prefers_reduced_motion() {
            return if self.shown { 1.0 } else { 0.0 };
        }
        self.amount_at(self.started.elapsed())
    }

    fn amount_at(&self, elapsed: Duration) -> f32 {
        let time = (elapsed.as_secs_f32() / (motion::DURATION_NORMAL as f32 / 1000.0)).min(1.0);
        let eased = easing::cubic_bezier(motion::EASE_STANDARD)(time);
        let to = if self.shown { 1.0 } else { 0.0 };
        self.from + (to - self.from) * eased
    }

    /// 尚有需要绘制的覆盖层（包含退出动画）。
    pub fn present(&self) -> bool {
        self.shown || self.amount() > 0.0
    }

    /// 是否需要下一帧。
    pub fn animating(&self) -> bool {
        !accessibility::prefers_reduced_motion()
            && self.started.elapsed() < Duration::from_millis(motion::DURATION_NORMAL as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exits_release_input_before_paint_finishes() {
        let mut panel = SlideMotion {
            shown: true,
            from: 0.0,
            started: Instant::now(),
        };
        let end = Duration::from_millis(motion::DURATION_NORMAL as u64);
        assert_eq!(panel.amount_at(Duration::ZERO), 0.0);
        assert_eq!(panel.amount_at(end), 1.0);
        panel.started -= end;
        panel.set_shown(false);
        assert!(!panel.interactive());
        assert!(panel.amount_at(Duration::ZERO) > 0.99);
        assert_eq!(panel.amount_at(end), 0.0);
    }
}
