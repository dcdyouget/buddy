//! 主窗口呼入 / 呼出的 Core Animation 合成动效（参照 macOS 26 聚焦搜索）。
//!
//! 动画加在 `contentView` 的图层上：它承载圆角裁切，GPUI 的 Metal 图层是其子图层，
//! 缩放与淡入淡出由窗口服务器合成，不触发 GPUI 重新布局或重绘。只加 `CAAnimation`，
//! 不改图层的模型值，AppKit 管理的几何属性不受影响。
//!
//! - 呼入：从 [`SUMMON_START_SCALE`] 以接近临界阻尼的弹簧放大到原尺寸（不回弹，避免
//!   放大超过窗口被裁切），同时快速淡入；
//! - 呼出：向中心收缩到 [`DISMISS_END_SCALE`] 并淡出，停在终帧直到原生 `orderOut`。
//!
//! 被打断时（呼出途中再次呼入，或反之）从屏幕上当前的缩放 / 不透明度继续。

use crate::theme_system::tokens::motion::{
    DISMISS_END_SCALE, DURATION_DISMISS, EASE_DISMISS, DURATION_SUMMON_FADE, SUMMON_DAMPING_RATIO,
    SUMMON_START_SCALE, SUMMON_STIFFNESS,
};
use objc::runtime::{Class, NO, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::CString;

type Id = *mut Object;

const SUMMON_KEY: &str = "buddy.summon";
const SUMMON_FADE_KEY: &str = "buddy.summon.fade";
const DISMISS_KEY: &str = "buddy.dismiss";
const DISMISS_FADE_KEY: &str = "buddy.dismiss.fade";

/// Core Animation 的 4×4 变换矩阵（行向量约定，平移在第四行）。
#[repr(C)]
#[derive(Clone, Copy)]
struct Transform3D([f64; 16]);

impl Transform3D {
    /// 以图层中心为原点缩放 `scale` 倍；`center` 为中心相对锚点的偏移。
    fn scale_about(scale: f64, center: (f64, f64)) -> Self {
        let mut m = [0.0; 16];
        m[0] = scale;
        m[5] = scale;
        m[10] = 1.0;
        m[15] = 1.0;
        // p' = s·p + (1 − s)·c：中心点保持不动
        m[12] = (1.0 - scale) * center.0;
        m[13] = (1.0 - scale) * center.1;
        Self(m)
    }
}

/// 播放呼入动效。须在主线程、窗口 alpha 置 1 之前调用；返回后动画已提交。
pub(crate) fn summon(window: Id) {
    unsafe {
        if let Some(layer) = content_layer(window) {
            summon_layer(layer);
            commit();
        }
    }
}

unsafe fn summon_layer(layer: Id) {
    unsafe {
        let (scale, opacity) = presented(layer).unwrap_or((SUMMON_START_SCALE as f64, 0.0));
        remove_all(layer);
        let center = center(layer);
        let transform = spring(
            Transform3D::scale_about(scale, center),
            Transform3D::scale_about(1.0, center),
        );
        add(layer, transform, SUMMON_KEY);
        let fade = basic("opacity");
        let _: () = msg_send![fade, setFromValue: number(opacity)];
        let _: () = msg_send![fade, setToValue: number(1.0)];
        let _: () = msg_send![fade, setDuration: DURATION_SUMMON_FADE as f64 / 1000.0];
        let _: () = msg_send![fade, setTimingFunction: timing("easeOut")];
        add(layer, fade, SUMMON_FADE_KEY);
    }
}

/// 播放呼出动效并停在透明终帧；原生隐藏后调用 [`settle`] 清除。
pub(crate) fn dismiss(window: Id) {
    unsafe {
        if let Some(layer) = content_layer(window) {
            dismiss_layer(layer);
            commit();
        }
    }
}

unsafe fn dismiss_layer(layer: Id) {
    unsafe {
        let (scale, opacity) = presented(layer).unwrap_or((1.0, 1.0));
        remove_all(layer);
        let center = center(layer);
        let duration = DURATION_DISMISS as f64 / 1000.0;
        let shrink = basic("transform");
        let _: () = msg_send![shrink, setFromValue: transform_value(Transform3D::scale_about(scale, center))];
        let _: () = msg_send![
            shrink,
            setToValue: transform_value(Transform3D::scale_about(DISMISS_END_SCALE as f64, center))
        ];
        // 减速曲线：按键当帧即明显变淡收缩，尾段柔和收住。easeIn 前三分之一几乎不动、
        // 随后骤降，体感是「卡一下再消失」。
        let _: () = msg_send![shrink, setDuration: duration];
        let _: () = msg_send![shrink, setTimingFunction: control_points(EASE_DISMISS)];
        hold_final_frame(shrink);
        add(layer, shrink, DISMISS_KEY);
        let fade = basic("opacity");
        let _: () = msg_send![fade, setFromValue: number(opacity)];
        let _: () = msg_send![fade, setToValue: number(0.0)];
        let _: () = msg_send![fade, setDuration: duration];
        let _: () = msg_send![fade, setTimingFunction: control_points(EASE_DISMISS)];
        hold_final_frame(fade);
        add(layer, fade, DISMISS_FADE_KEY);
    }
}

/// 清除所有呼入 / 呼出动画，图层回到模型值（原尺寸、不透明）。
pub(crate) fn settle(window: Id) {
    unsafe {
        if let Some(layer) = content_layer(window) {
            settle_layer(layer);
            commit();
        }
    }
}

unsafe fn settle_layer(layer: Id) {
    unsafe {
        remove_all(layer);
    }
}

/// 立即提交当前 Core Animation 事务，保证动画先于窗口 alpha 变化生效。
unsafe fn commit() {
    unsafe {
        let _: () = msg_send![class!(CATransaction), flush];
    }
}

unsafe fn content_layer(window: Id) -> Option<Id> {
    unsafe {
        let content: Id = msg_send![window, contentView];
        if content.is_null() {
            return None;
        }
        let layer: Id = msg_send![content, layer];
        (!layer.is_null()).then_some(layer)
    }
}

/// 呼入 / 呼出动画进行中时，屏幕上实际呈现的缩放与不透明度。
unsafe fn presented(layer: Id) -> Option<(f64, f64)> {
    unsafe {
        let ours = [SUMMON_KEY, SUMMON_FADE_KEY, DISMISS_KEY, DISMISS_FADE_KEY].iter().any(|key| {
            let animation: Id = msg_send![layer, animationForKey: ns_string(key)];
            !animation.is_null()
        });
        if !ours {
            return None;
        }
        let presentation: Id = msg_send![layer, presentationLayer];
        if presentation.is_null() {
            return None;
        }
        let scale = key_path_f64(presentation, "transform.scale.x")?;
        let opacity: f32 = msg_send![presentation, opacity];
        Some((scale, f64::from(opacity)))
    }
}

/// 图层中心相对锚点的偏移（AppKit 托管图层的锚点通常在左下角）。
unsafe fn center(layer: Id) -> (f64, f64) {
    unsafe {
        let width = key_path_f64(layer, "bounds.size.width").unwrap_or(0.0);
        let height = key_path_f64(layer, "bounds.size.height").unwrap_or(0.0);
        let anchor_x = key_path_f64(layer, "anchorPoint.x").unwrap_or(0.0);
        let anchor_y = key_path_f64(layer, "anchorPoint.y").unwrap_or(0.0);
        ((0.5 - anchor_x) * width, (0.5 - anchor_y) * height)
    }
}

unsafe fn spring(from: Transform3D, to: Transform3D) -> Id {
    unsafe {
        let Some(class) = Class::get("CASpringAnimation") else {
            let animation = basic("transform");
            let _: () = msg_send![animation, setFromValue: transform_value(from)];
            let _: () = msg_send![animation, setToValue: transform_value(to)];
            let _: () = msg_send![animation, setDuration: DURATION_SUMMON_FADE as f64 / 1000.0 * 2.0];
            let _: () = msg_send![animation, setTimingFunction: timing("easeOut")];
            return animation;
        };
        let animation: Id = msg_send![class, animationWithKeyPath: ns_string("transform")];
        let stiffness = SUMMON_STIFFNESS as f64;
        // 阻尼 = 2ζ√(k·m)，质量取 1
        let damping = 2.0 * SUMMON_DAMPING_RATIO as f64 * stiffness.sqrt();
        let _: () = msg_send![animation, setMass: 1.0f64];
        let _: () = msg_send![animation, setStiffness: stiffness];
        let _: () = msg_send![animation, setDamping: damping];
        let _: () = msg_send![animation, setFromValue: transform_value(from)];
        let _: () = msg_send![animation, setToValue: transform_value(to)];
        let settling: f64 = msg_send![animation, settlingDuration];
        let _: () = msg_send![animation, setDuration: settling];
        animation
    }
}

unsafe fn basic(key_path: &str) -> Id {
    unsafe { msg_send![class!(CABasicAnimation), animationWithKeyPath: ns_string(key_path)] }
}

unsafe fn hold_final_frame(animation: Id) {
    unsafe {
        let _: () = msg_send![animation, setRemovedOnCompletion: NO];
        let _: () = msg_send![animation, setFillMode: ns_string("forwards")];
    }
}

unsafe fn add(layer: Id, animation: Id, key: &str) {
    unsafe {
        let _: () = msg_send![layer, addAnimation: animation forKey: ns_string(key)];
    }
}

unsafe fn remove_all(layer: Id) {
    unsafe {
        for key in [SUMMON_KEY, SUMMON_FADE_KEY, DISMISS_KEY, DISMISS_FADE_KEY] {
            let _: () = msg_send![layer, removeAnimationForKey: ns_string(key)];
        }
    }
}

unsafe fn key_path_f64(object: Id, key_path: &str) -> Option<f64> {
    unsafe {
        let value: Id = msg_send![object, valueForKeyPath: ns_string(key_path)];
        if value.is_null() {
            return None;
        }
        let value: f64 = msg_send![value, doubleValue];
        value.is_finite().then_some(value)
    }
}

unsafe fn timing(name: &str) -> Id {
    unsafe { msg_send![class!(CAMediaTimingFunction), functionWithName: ns_string(name)] }
}

/// `+[CAMediaTimingFunction functionWithControlPoints::::]`；选择子含无名参数，
/// `msg_send!` 无法表达，按真实签名调用 `objc_msgSend`。
unsafe fn control_points([x1, y1, x2, y2]: [f32; 4]) -> Id {
    type Send = unsafe extern "C" fn(*const Class, Sel, f32, f32, f32, f32) -> Id;
    unsafe extern "C" {
        fn objc_msgSend();
    }
    unsafe {
        let send: Send = std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        send(
            class!(CAMediaTimingFunction),
            Sel::register("functionWithControlPoints::::"),
            x1,
            y1,
            x2,
            y2,
        )
    }
}

unsafe fn number(value: f64) -> Id {
    unsafe { msg_send![class!(NSNumber), numberWithDouble: value] }
}

unsafe fn transform_value(transform: Transform3D) -> Id {
    const ENCODING: &[u8] = b"{CATransform3D=dddddddddddddddd}\0";
    unsafe {
        msg_send![
            class!(NSValue),
            valueWithBytes: &transform as *const Transform3D as *const std::ffi::c_void
            objCType: ENCODING.as_ptr() as *const std::ffi::c_char
        ]
    }
}

unsafe fn ns_string(value: &str) -> Id {
    let value = CString::new(value).unwrap_or_default();
    unsafe { msg_send![class!(NSString), stringWithUTF8String: value.as_ptr()] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_about_keeps_center_fixed() {
        let center = (200.0, 120.0);
        let t = Transform3D::scale_about(0.9, center);
        let mapped = (t.0[0] * center.0 + t.0[12], t.0[5] * center.1 + t.0[13]);
        assert!((mapped.0 - center.0).abs() < 1e-9 && (mapped.1 - center.1).abs() < 1e-9);
        let corner = (t.0[0] * 0.0 + t.0[12], t.0[5] * 0.0 + t.0[13]);
        assert!(corner.0 > 0.0 && corner.1 > 0.0, "corners move inward when shrinking");
    }

    unsafe fn animation_count(layer: Id) -> usize {
        unsafe {
            let keys: Id = msg_send![layer, animationKeys];
            if keys.is_null() { 0 } else { msg_send![keys, count] }
        }
    }

    /// 在独立图层上走一遍呼入 → 呼出 → 打断呼入 → 清除，验证所有 Objective-C 消息在运行时有效。
    #[test]
    fn layer_animations_round_trip() {
        unsafe {
            let pool: Id = msg_send![class!(NSAutoreleasePool), new];
            let layer: Id = msg_send![class!(CALayer), layer];
            let _: () = msg_send![layer, setValue: number(400.0) forKeyPath: ns_string("bounds.size.width")];
            let _: () = msg_send![layer, setValue: number(240.0) forKeyPath: ns_string("bounds.size.height")];
            assert_eq!(key_path_f64(layer, "bounds.size.width"), Some(400.0));
            let (cx, cy) = center(layer);
            assert!(cx.abs() <= 200.0 && cy.abs() <= 120.0);

            let ease = control_points(EASE_DISMISS);
            assert!(!ease.is_null());
            let mut point = [0f32; 2];
            let _: () = msg_send![ease, getControlPointAtIndex: 1usize values: point.as_mut_ptr()];
            assert_eq!(point, [EASE_DISMISS[0], EASE_DISMISS[1]]);

            summon_layer(layer);
            assert_eq!(animation_count(layer), 2);
            dismiss_layer(layer);
            assert_eq!(animation_count(layer), 2, "dismiss replaces summon");
            summon_layer(layer);
            assert_eq!(animation_count(layer), 2, "summon interrupts dismiss");
            settle_layer(layer);
            assert_eq!(animation_count(layer), 0);
            commit();
            let opacity: f32 = msg_send![layer, opacity];
            assert_eq!(opacity, 1.0, "model value untouched");
            let _: () = msg_send![pool, drain];
        }
    }

    #[test]
    fn identity_at_full_scale() {
        let t = Transform3D::scale_about(1.0, (50.0, 50.0));
        assert_eq!(t.0[12], 0.0);
        assert_eq!(t.0[13], 0.0);
        assert_eq!((t.0[0], t.0[5], t.0[10], t.0[15]), (1.0, 1.0, 1.0, 1.0));
    }
}
