//! Move the composer's shaped text into the measured transcript bubble.
//! The real row owns layout, hit testing and selection throughout.
use crate::{
    text_area::TextSnapshot,
    theme_system::{
        BuddyTheme, box_shadows,
        tokens::{metrics as m, motion},
    },
};
use gpui::{
    AnyElement, App, BorderStyle, Bounds, ContentMask, Corners, Hsla, Pixels, Point, Window,
    canvas, div, point, prelude::*, px, quad, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

pub(super) struct SendMotion {
    message_id: String,
    dock_height: Pixels,
    text: TextSnapshot,
    started: Instant,
    cancelled: Cell<bool>,
    tracking: RefCell<Option<MovingTarget>>,
    target: Cell<Option<Bounds<Pixels>>>,
    viewport: Cell<Option<gpui::Size<Pixels>>>,
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn reveal(ms: f32) -> f32 {
    smooth(
        (ms - motion::SEND_HANDOFF_AT_MS)
            / (motion::SEND_TOTAL_MS as f32 - motion::SEND_HANDOFF_AT_MS),
    )
}

/// The composer can shrink or the first response can move the destination.
/// Track those layout changes without restarting the flight's timeline.
struct MovingTarget {
    position: Point<f32>,
    velocity: Point<f32>,
    sampled_ms: f32,
}

impl MovingTarget {
    fn new(center: Point<Pixels>, ms: f32) -> Self {
        Self {
            position: center.map(f32::from),
            velocity: point(0., 0.),
            sampled_ms: ms,
        }
    }

    fn advance(&mut self, ms: f32, center: Point<Pixels>) -> Point<Pixels> {
        let dt = ((ms - self.sampled_ms) / 1000.0).max(0.0);
        let omega = motion::SEND_TARGET_FREQUENCY;
        let target = center.map(f32::from);
        let displacement = self.position - target;
        let momentum = self.velocity + displacement * omega;
        let decay = (-omega * dt).exp();
        self.position = target + (displacement + momentum * dt) * decay;
        self.velocity = (self.velocity - momentum * (omega * dt)) * decay;
        self.sampled_ms = ms;
        self.position.map(px)
    }
}

struct MotionFrame {
    bounds: Bounds<Pixels>,
    text_offset: Point<Pixels>,
    travel: f32,
    lift: f32,
}

impl MotionFrame {
    fn sample(text: Bounds<Pixels>, target: Bounds<Pixels>, ms: f32) -> Self {
        let from_padding = point(px(m::SPACE_2), px(m::SPACE_1));
        let to_padding = point(px(m::SPACE_3 + 1.), px(m::SPACE_2 + 1.));
        let from = Bounds::from_corners(
            text.origin - from_padding,
            text.bottom_right() + from_padding,
        );
        let t = (ms / motion::SEND_TRAVEL_MS).clamp(0., 1.);
        let travel = smooth(t);
        // The normalized speed of quintic easing peaks halfway through the trip.
        let crest = (4. * t * (1. - t)).powi(2);
        let dx = f32::from(target.center().x - from.center().x);
        let lift = px((dx.abs() * 0.12).min(motion::SEND_ARC_HEIGHT) * crest);
        let stretch = (f32::from(from.size.width.min(target.size.width)) * 0.12)
            .min(dx.abs() * 0.08)
            .min(motion::SEND_STRETCH)
            * crest;
        let width = from.size.width + (target.size.width - from.size.width) * travel + px(stretch);
        let height = from.size.height + (target.size.height - from.size.height) * travel
            - px(motion::SEND_SQUASH * crest);
        let center = from.center()
            + (target.center() - from.center()) * travel
            + point(px(dx.signum() * stretch / 2.), -lift);
        let bounds = Bounds::new(center - point(width / 2., height / 2.), size(width, height));
        let padding = from_padding + (to_padding - from_padding) * travel;
        Self {
            bounds,
            text_offset: bounds.origin + padding - text.origin,
            travel,
            lift: f32::from(lift) / motion::SEND_ARC_HEIGHT,
        }
    }
}

impl SendMotion {
    pub fn new(message_id: String, dock_height: Pixels, text: TextSnapshot) -> Rc<Self> {
        Rc::new(Self {
            message_id,
            dock_height,
            text,
            started: Instant::now(),
            cancelled: Cell::new(false),
            tracking: RefCell::new(None),
            target: Cell::new(None),
            viewport: Cell::new(None),
        })
    }

    pub fn message_id(&self) -> &str {
        &self.message_id
    }

    pub fn begin_frame(&self) {
        self.target.set(None);
    }

    pub fn record_target(&self, bounds: Bounds<Pixels>, visible: Bounds<Pixels>) {
        if bounds.top() < visible.top() || bounds.bottom() > visible.bottom() {
            self.cancel();
        } else {
            self.target.set(Some(bounds));
        }
    }

    fn elapsed(&self) -> f32 {
        self.started.elapsed().as_secs_f32() * 1000.0
    }

    pub fn active(&self) -> bool {
        !self.cancelled.get()
            && self.elapsed() < motion::SEND_TOTAL_MS as f32
            && !crate::accessibility::prefers_reduced_motion()
    }

    pub fn cancel(&self) {
        self.cancelled.set(true);
    }

    pub fn bubble_opacity(&self) -> f32 {
        if self.active() {
            reveal(self.elapsed())
        } else {
            1.0
        }
    }

    pub fn status_opacity(&self) -> f32 {
        if self.active() {
            smooth((self.elapsed() - motion::SEND_STATUS_AT_MS) / motion::SEND_STATUS_FADE_MS)
        } else {
            1.0
        }
    }

    pub fn dock_opacity(&self) -> f32 {
        if !self.active() {
            return 1.0;
        }
        let ms = self.elapsed();
        let dim = smooth(ms / motion::SEND_DOCK_FADE_MS)
            * (1.0 - smooth((ms - motion::SEND_STATUS_AT_MS) / motion::SEND_STATUS_FADE_MS));
        1.0 - dim * motion::SEND_DOCK_DIM
    }

    pub fn composer_height(&self, natural_height: Pixels) -> Pixels {
        let progress = if self.active() {
            smooth((self.elapsed() - motion::SEND_STATUS_AT_MS) / motion::SEND_DOCK_SETTLE_MS)
        } else {
            1.0
        };
        self.dock_height + (natural_height - self.dock_height) * progress
    }

    pub fn overlay(self: &Rc<Self>) -> AnyElement {
        let ms = self.elapsed();
        let this = self.clone();
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .opacity(1.0 - reveal(ms))
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, cx| this.paint(ms, window, cx),
                )
                .size_full(),
            )
            .into_any_element()
    }

    fn paint(&self, ms: f32, window: &mut Window, cx: &mut App) {
        if !self.active() {
            return;
        }
        window.request_animation_frame();
        let viewport = window.viewport_size();
        if self
            .viewport
            .replace(Some(viewport))
            .is_some_and(|previous| previous != viewport)
        {
            self.cancel();
            return;
        }
        let Some(target) = self.target.get() else {
            self.cancel();
            return;
        };
        let mut tracking = self.tracking.borrow_mut();
        let tracking = tracking.get_or_insert_with(|| MovingTarget::new(target.center(), ms));
        let tracked = tracking.advance(ms, target.center());
        // Remove residual tracking lag as the bubble comes to rest.
        let lock = smooth((ms - motion::SEND_TRAVEL_MS * 0.7) / (motion::SEND_TRAVEL_MS * 0.3));
        let tracked_bounds = Bounds::new(
            target.origin + (tracked - target.center()) * (1. - lock),
            target.size,
        );
        let frame = MotionFrame::sample(self.text.visible_text, tracked_bounds, ms);
        let theme = cx.buddy_theme();
        let c = theme.colors;
        let form = smooth(ms / motion::SEND_FORM_MS);
        let radius = m::RADIUS_LG + (m::RADIUS_MD - m::RADIUS_LG) * frame.travel;
        let radii = Corners {
            top_left: px(radius),
            top_right: px(radius),
            bottom_right: px(radius),
            bottom_left: px(radius + (m::RADIUS_SM - m::RADIUS_MD) * frame.travel),
        };
        let mut shadows = box_shadows(theme.shadows.shadow_static);
        for shadow in &mut shadows {
            shadow.color = shadow.color.opacity(form);
            shadow.offset.y += px(m::SPACE_1 * frame.lift);
            shadow.blur_radius += px(m::SPACE_2 * frame.lift);
        }
        window.paint_drop_shadows(frame.bounds, radii, &shadows);
        // Cover text underneath while crossing the transcript. At rest, the
        // bubble uses the same translucent fill over the same backdrop as the row.
        let backing = form * (1. - lock);
        if backing > 0. {
            window.paint_quad(quad(
                frame.bounds,
                radii,
                Hsla::from(c.bg_surface).opacity(backing),
                px(0.),
                Hsla::transparent_black(),
                BorderStyle::Solid,
            ));
        }
        window.paint_quad(quad(
            frame.bounds,
            radii,
            Hsla::from(c.user_bubble).opacity(form),
            px(1.),
            Hsla::from(c.user_bubble_border).opacity(form),
            BorderStyle::Solid,
        ));
        window.with_content_mask(
            Some(ContentMask {
                bounds: frame.bounds,
            }),
            |window| {
                self.text.paint(frame.text_offset, window, cx);
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Rc<SendMotion> {
        SendMotion::new("message".into(), px(64.), TextSnapshot::default())
    }

    #[test]
    fn text_starts_at_the_draft_and_lands_inside_the_measured_row() {
        let text = Bounds::new(point(px(35.), px(400.)), size(px(72.), px(24.)));
        let target = Bounds::new(point(px(270.), px(310.)), size(px(98.), px(42.)));
        let start = MotionFrame::sample(text, target, 0.);
        assert_eq!(start.text_offset, point(px(0.), px(0.)));
        assert_eq!(start.bounds.size, size(px(88.), px(32.)));
        for i in 0..=30 {
            let frame = MotionFrame::sample(text, target, i as f32 * 10.);
            let moved = Bounds::new(text.origin + frame.text_offset, text.size);
            assert!(frame.bounds.contains(&moved.origin));
            assert!(frame.bounds.contains(&moved.bottom_right()));
        }
        let end = MotionFrame::sample(text, target, motion::SEND_HANDOFF_AT_MS);
        assert_eq!(end.bounds, target);
        assert_eq!(
            text.origin + end.text_offset,
            target.origin + point(px(13.), px(9.))
        );
        assert_eq!(reveal(motion::SEND_HANDOFF_AT_MS), 0.);
        assert_eq!(reveal(motion::SEND_TOTAL_MS as f32), 1.);
    }

    #[test]
    fn tracking_a_moving_row_preserves_position_at_any_refresh_rate() {
        let initial = point(px(230.), px(376.));
        let target = point(px(360.), px(340.));
        let mut step = MovingTarget::new(initial, 0.);
        let mut frames = MovingTarget::new(initial, 0.);
        step.advance(240., target);
        for i in 1..=24 {
            frames.advance(i as f32 * 10., target);
        }
        assert!((step.position.x - frames.position.x).abs() < 0.001);
        assert!((step.position.y - frames.position.y).abs() < 0.001);
        let before = frames.position;
        frames.advance(240., point(px(360.), px(260.)));
        assert_eq!(before, frames.position);
    }

    #[test]
    fn cancellation_reveals_message_and_releases_composer() {
        let motion = sample();
        motion.cancel();
        assert!(!motion.active());
        assert_eq!(motion.bubble_opacity(), 1.0);
        assert_eq!(motion.status_opacity(), 1.0);
        assert_eq!(motion.dock_opacity(), 1.0);
        assert_eq!(motion.composer_height(px(42.)), px(42.));
    }

    #[test]
    fn virtualized_destination_cannot_survive_a_new_frame() {
        let motion = sample();
        let viewport = Bounds::new(point(px(0.), px(0.)), size(px(500.), px(400.)));
        let bubble = Bounds::new(point(px(250.), px(300.)), size(px(200.), px(60.)));
        motion.record_target(bubble, viewport);
        assert_eq!(motion.target.get(), Some(bubble));
        motion.begin_frame();
        assert_eq!(motion.target.get(), None);
    }

    #[test]
    fn clipped_message_uses_the_real_row_instead_of_an_unclipped_overlay() {
        let motion = sample();
        let viewport = Bounds::new(point(px(0.), px(100.)), size(px(500.), px(300.)));
        let bubble = Bounds::new(point(px(250.), px(50.)), size(px(200.), px(350.)));
        motion.record_target(bubble, viewport);
        assert_eq!(motion.bubble_opacity(), 1.0);
        assert_eq!(motion.status_opacity(), 1.0);
        assert_eq!(motion.composer_height(px(42.)), px(42.));
    }
}
