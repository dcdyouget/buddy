//! Restrained surface lighting shared by the compact and conversation surfaces.

use crate::theme_system::tokens::motion;
use gpui::{
    Animation, AnimationExt, AnyElement, Bounds, ElementId, Hsla, Rgba, canvas, div, fill,
    linear_color_stop, linear_gradient, point, prelude::*, px, size,
};
use std::time::Duration;

/// Paint-only edge lighting; it never adds hitboxes over text, controls or drag regions.
pub(crate) fn surface_sheen(id: impl Into<ElementId>, accent: Rgba, repeating: bool) -> AnyElement {
    if crate::accessibility::prefers_reduced_motion() {
        return div().absolute().into_any_element();
    }
    let duration = if repeating {
        motion::DURATION_SURFACE_FLOW
    } else {
        motion::DURATION_SURFACE_SHEEN
    };
    let animation = Animation::new(Duration::from_millis(duration)).with_max_fps(30.0);
    let animation = if repeating {
        animation.repeat()
    } else {
        animation
    };
    div()
        .absolute()
        .size_full()
        .top_0()
        .left_0()
        .with_animation(id, animation, move |d, progress| {
            d.child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let width = bounds.size.width * motion::SURFACE_SHEEN_WIDTH;
                        let x = bounds.left() - width + (bounds.size.width + width) * progress;
                        let envelope = (progress * std::f32::consts::PI).sin().max(0.0);
                        let clear = Hsla::from(accent).opacity(0.0);
                        // A fine rim plus a faint inward falloff. No wash over the transcript.
                        for (depth, strength) in [(motion::SURFACE_SHEEN_DEPTH, 0.10), (1.0, 1.0)] {
                            let hi = Hsla::from(accent)
                                .opacity(motion::SURFACE_SHEEN_OPACITY * envelope * strength);
                            for y in [bounds.top(), bounds.bottom() - px(depth)] {
                                let half = width / 2.0;
                                window.paint_quad(fill(
                                    Bounds::new(point(x, y), size(half, px(depth))),
                                    linear_gradient(
                                        90.,
                                        linear_color_stop(clear, 0.),
                                        linear_color_stop(hi, 1.),
                                    ),
                                ));
                                window.paint_quad(fill(
                                    Bounds::new(point(x + half, y), size(half, px(depth))),
                                    linear_gradient(
                                        90.,
                                        linear_color_stop(hi, 0.),
                                        linear_color_stop(clear, 1.),
                                    ),
                                ));
                            }
                        }
                    },
                )
                .absolute()
                .size_full()
                .top_0()
                .left_0(),
            )
        })
        .into_any_element()
}
