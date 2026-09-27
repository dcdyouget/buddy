//! CSS `cubic-bezier()` 缓动（令牌 `tokens::motion::EASE_*` 为四参数）
//!
//! 与浏览器相同的解法：给定时间进度 x，先解出曲线参数 t（牛顿迭代，失败则二分），再求 y。
//! y 可超出 [0, 1]（`EASE_SPRING` 的回弹即如此）。

/// 由四参数构造缓动函数：输入 / 输出均为进度
pub fn cubic_bezier([x1, y1, x2, y2]: [f32; 4]) -> impl Fn(f32) -> f32 + Clone {
    move |x: f32| {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        // B(t) = 3(1-t)²t·p1 + 3(1-t)t²·p2 + t³
        let bez = |t: f32, p1: f32, p2: f32| {
            let u = 1.0 - t;
            3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
        };
        let dbez = |t: f32, p1: f32, p2: f32| {
            let u = 1.0 - t;
            3.0 * u * u * p1 + 6.0 * u * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
        };
        let mut t = x;
        let mut solved = false;
        for _ in 0..8 {
            let err = bez(t, x1, x2) - x;
            if err.abs() < 1e-6 {
                solved = true;
                break;
            }
            let d = dbez(t, x1, x2);
            if d.abs() < 1e-6 {
                break;
            }
            t -= err / d;
        }
        if !solved || !(0.0..=1.0).contains(&t) {
            let (mut lo, mut hi) = (0.0f32, 1.0f32);
            t = x;
            for _ in 0..40 {
                let v = bez(t, x1, x2);
                if (v - x).abs() < 1e-6 {
                    break;
                }
                if v < x { lo = t } else { hi = t }
                t = (lo + hi) / 2.0;
            }
        }
        bez(t, y1, y2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme_system::tokens::motion;

    #[test]
    fn endpoints_and_linear() {
        let linear = cubic_bezier([0.0, 0.0, 1.0, 1.0]);
        for x in [0.0, 0.1, 0.5, 0.9, 1.0] {
            assert!((linear(x) - x).abs() < 1e-4, "{x}");
        }
        let std = cubic_bezier(motion::EASE_STANDARD);
        assert_eq!(std(0.0), 0.0);
        assert_eq!(std(1.0), 1.0);
    }

    #[test]
    fn standard_is_monotonic_and_spring_overshoots() {
        let std = cubic_bezier(motion::EASE_STANDARD);
        let mut prev = 0.0;
        for i in 1..=100 {
            let y = std(i as f32 / 100.0);
            assert!(y >= prev - 1e-5);
            prev = y;
        }
        // 对照值由独立的高精度二分求得（python，100 次迭代）：0.8778336
        assert!((std(0.5) - 0.877_833_6).abs() < 1e-4, "{}", std(0.5));
        let spring = cubic_bezier(motion::EASE_SPRING);
        let peak = (1..100).map(|i| spring(i as f32 / 100.0)).fold(0.0f32, f32::max);
        assert!(peak > 1.05, "ease-spring 应回弹超过 1，实得 {peak}");
    }
}
