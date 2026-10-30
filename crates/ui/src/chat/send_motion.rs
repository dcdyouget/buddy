//! One continuous surface: lift the draft, fold briefly, settle into its bubble.
//! The flying surface owns only paint; the real row always owns layout and input.
use crate::{
    text_area::TextSnapshot,
    theme_system::{
        BuddyTheme,
        tokens::{metrics as m, motion},
    },
};
use gpui::{
    AnyElement, App, BorderStyle, Bounds, BoxShadow, ContentMask, Corners, Hsla, Pixels, Point,
    Window, canvas, div, linear_color_stop, linear_gradient, point, prelude::*, px, quad, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

pub(super) struct SendMotion {
    message_id: String,
    source: Bounds<Pixels>,
    text: Option<Rc<TextSnapshot>>,
    started: Instant,
    cancelled: Cell<bool>,
    flight: RefCell<Option<Flight>>,
    target: Cell<Option<Bounds<Pixels>>>,
    viewport: Cell<Option<gpui::Size<Pixels>>>,
}

// Quintic edges have zero velocity AND acceleration, unlike independent ease-in/out
// segments. The launch itself uses Hermite to hand its upward velocity to Flight.
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn expansion(ms: f32) -> f32 {
    let t = ((ms - motion::SEND_UNFOLD_AT_MS) / 1000.0).max(0.0);
    let damping = motion::SEND_UNFOLD_DAMPING;
    let omega = motion::SEND_UNFOLD_FREQUENCY;
    let damped = omega * (1.0 - damping * damping).sqrt();
    let response = 1.0
        - (-damping * omega * t).exp()
            * ((damped * t).cos() + damping * omega / damped * (damped * t).sin());
    // Remove the imperceptible spring tail before handing the surface back.
    let settle = smooth((ms - motion::SEND_HANDOFF_AT_MS) / motion::SEND_HANDOFF_MS);
    response + (1.0 - response) * settle
}

fn reveal(ms: f32) -> f32 {
    smooth((ms - motion::SEND_HANDOFF_AT_MS) / motion::SEND_HANDOFF_MS)
}

/// Exact critically-damped integration. Frame-rate independent, and retargeting
/// when streaming moves the bubble preserves velocity instead of teleporting it.
struct Flight {
    position: Point<f32>,
    velocity: Point<f32>,
    sampled_ms: f32,
}

impl Flight {
    fn new(launch: Point<Pixels>) -> Self {
        Self {
            position: point(launch.x.into(), launch.y.into()),
            velocity: point(0.0, -motion::SEND_LAUNCH_SPEED),
            sampled_ms: motion::SEND_LAUNCH_AT_MS,
        }
    }

    fn advance(&mut self, ms: f32, target: Point<Pixels>) {
        let dt = ((ms - self.sampled_ms) / 1000.0).max(0.0);
        let omega = motion::SEND_FLIGHT_FREQUENCY;
        let target = point(f32::from(target.x), f32::from(target.y));
        let displacement = self.position - target;
        let momentum = self.velocity + displacement * omega;
        let decay = (-omega * dt).exp();
        self.position = target + (displacement + momentum * dt) * decay;
        self.velocity = (self.velocity - momentum * (omega * dt)) * decay;
        self.sampled_ms = ms;
    }
}

impl SendMotion {
    pub fn new(message_id: String, source: Bounds<Pixels>, text: Option<TextSnapshot>) -> Rc<Self> {
        Rc::new(Self {
            message_id,
            source,
            text: text.map(Rc::new),
            started: Instant::now(),
            cancelled: Cell::new(false),
            flight: RefCell::new(None),
            target: Cell::new(None),
            viewport: Cell::new(None),
        })
    }

    pub fn message_id(&self) -> &str {
        &self.message_id
    }

    /// A virtualized row must register its destination again each frame.
    pub fn begin_frame(&self) {
        self.target.set(None);
    }

    pub fn record_target(&self, bounds: Bounds<Pixels>, visible: Bounds<Pixels>) {
        // A partially clipped tall message cannot be represented faithfully by
        // an overlay outside the list's clip. Show the actual row immediately.
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

    /// Hide only the new status copy while the submitted text is still visible.
    /// The stop button remains fully interactive and is never faded by Composer.
    pub fn status_opacity(&self) -> f32 {
        if self.active() {
            smooth((self.elapsed() - motion::SEND_STATUS_AT_MS) / motion::SEND_STATUS_FADE_MS)
        } else {
            1.0
        }
    }

    /// Keep a stable input anchor beneath the lifted surface. Only its lighting
    /// softens; controls and their hitboxes are never faded.
    pub fn dock_opacity(&self) -> f32 {
        if !self.active() {
            return 1.0;
        }
        let ms = self.elapsed();
        let dim = smooth(ms / motion::SEND_DOCK_FADE_MS)
            * (1.0 - smooth((ms - motion::SEND_STATUS_AT_MS) / motion::SEND_STATUS_FADE_MS));
        1.0 - dim * motion::SEND_DOCK_DIM
    }

    /// Multi-line input releases its old height gradually as the light lifts out.
    pub fn composer_height(&self, natural_height: Pixels) -> Pixels {
        let progress = if self.active() {
            smooth((self.elapsed() - motion::SEND_STATUS_AT_MS) / motion::SEND_DOCK_SETTLE_MS)
        } else {
            1.0
        };
        self.source.size.height + (natural_height - self.source.size.height) * progress
    }

    fn launch(&self) -> Point<Pixels> {
        point(
            self.source.center().x,
            self.source.top() - px(motion::SEND_LIFT_PX),
        )
    }

    fn gather_center(&self, elapsed: f32) -> Point<Pixels> {
        let duration = motion::SEND_LAUNCH_AT_MS - motion::SEND_CHARGE_MS;
        let t = ((elapsed - motion::SEND_CHARGE_MS) / duration).clamp(0.0, 1.0);
        let h01 = t * t * (3.0 - 2.0 * t);
        let h11 = t * t * (t - 1.0);
        self.source.center()
            + (self.launch() - self.source.center()) * h01
            + point(
                px(0.0),
                px(-motion::SEND_LAUNCH_SPEED * duration / 1000.0 * h11),
            )
    }

    pub fn overlay(self: &Rc<Self>) -> AnyElement {
        let elapsed = self.elapsed();
        let charge = smooth(elapsed / motion::SEND_CHARGE_MS);
        let gather = smooth(
            (elapsed - motion::SEND_CHARGE_MS)
                / (motion::SEND_LAUNCH_AT_MS - motion::SEND_CHARGE_MS),
        );
        let text_opacity =
            1.0 - smooth((elapsed - motion::SEND_TEXT_FADE_AT_MS) / motion::SEND_TEXT_FADE_MS);
        let this = self.clone();
        let text_motion = self.clone();
        let text = self.text.clone();
        let offset = self.gather_center(elapsed) - self.source.center();
        let clip = self.gather_bounds(elapsed);
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(
                canvas(|_, _, _| {}, move |_, _, window, cx| this.paint(window, cx))
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
            )
            .when(self.active() && text_opacity > 0.0, |d| {
                d.child(
                    canvas(
                        |_, _, _| {},
                        move |_, _, window, cx| {
                            if text_motion.active()
                                && let Some(text) = &text
                            {
                                window.with_content_mask(
                                    Some(ContentMask { bounds: clip }),
                                    |window| text.paint(offset, window, cx),
                                );
                            }
                        },
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .opacity(text_opacity * (1.0 - charge * gather * 0.12)),
                )
            })
            .into_any_element()
    }

    fn gather_bounds(&self, elapsed: f32) -> Bounds<Pixels> {
        let u = ((elapsed - motion::SEND_CHARGE_MS)
            / (motion::SEND_LAUNCH_AT_MS - motion::SEND_CHARGE_MS))
            .clamp(0.0, 1.0);
        let width = self.source.size.width
            + (px(motion::SEND_FOLD_WIDTH) - self.source.size.width) * smooth(u);
        // Height yields a little before width, giving the surface a soft squeeze.
        let height = self.source.size.height
            + (px(motion::SEND_FOLD_HEIGHT) - self.source.size.height)
                * smooth((u * 1.12).min(1.0));
        let center = self.gather_center(elapsed);
        Bounds::new(
            center - point(width / 2.0, height / 2.0),
            size(width, height),
        )
    }

    fn paint(&self, window: &mut Window, cx: &mut App) {
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
            // No visible destination this frame: never fly toward a stale row.
            self.cancel();
            return;
        };
        let ms = self.elapsed();
        let c = cx.buddy_theme().colors;
        let charge = smooth(ms / motion::SEND_CHARGE_MS);
        let gather = smooth(
            (ms - motion::SEND_CHARGE_MS) / (motion::SEND_LAUNCH_AT_MS - motion::SEND_CHARGE_MS),
        );
        let unfold = expansion(ms);
        let handoff = reveal(ms);
        let bounds = if ms < motion::SEND_LAUNCH_AT_MS {
            self.gather_bounds(ms)
        } else {
            let mut flight = self.flight.borrow_mut();
            let flight = flight.get_or_insert_with(|| Flight::new(self.launch()));
            flight.advance(ms, target.center());
            let position = point(px(flight.position.x), px(flight.position.y));
            // Streaming and the shrinking composer can both move the endpoint.
            // Remove any residual spring lag during the crossfade, so the last
            // painted surface exactly coincides with the real message.
            let center = position + (target.center() - position) * handoff;
            let width = px(motion::SEND_FOLD_WIDTH)
                + (target.size.width - px(motion::SEND_FOLD_WIDTH)) * unfold;
            let height = px(motion::SEND_FOLD_HEIGHT)
                + (target.size.height - px(motion::SEND_FOLD_HEIGHT)) * unfold;
            Bounds::new(
                center - point(width / 2.0, height / 2.0),
                size(width, height),
            )
        };
        let dot_radius = motion::SEND_FOLD_HEIGHT / 2.0;
        let radius = (m::RADIUS_LG + (dot_radius - m::RADIUS_LG) * gather)
            + (m::RADIUS_MD - dot_radius) * unfold;
        let radii = Corners {
            top_left: px(radius),
            top_right: px(radius),
            bottom_right: px(radius),
            bottom_left: px(radius + (m::RADIUS_SM - m::RADIUS_MD) * unfold),
        };
        let glow = charge * (1.0 - unfold.clamp(0.0, 1.0)) * (1.0 - handoff);
        window.paint_drop_shadows(
            bounds,
            radii,
            &[
                BoxShadow {
                    color: Hsla::from(c.buddy_primary).opacity(motion::SEND_GLOW_OPACITY * glow),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(motion::SEND_GLOW_BLUR),
                    spread_radius: px(1.0),
                    inset: false,
                },
                BoxShadow {
                    color: Hsla::from(c.buddy_primary)
                        .opacity(motion::SEND_GLOW_OPACITY * 0.5 * glow),
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(motion::SEND_GLOW_BLUR * 0.35),
                    spread_radius: px(0.0),
                    inset: false,
                },
            ],
        );
        let clear = Hsla::transparent_black();
        let accent_mix = (charge * 0.18 + gather * 0.82)
            * motion::SEND_ACCENT_MIX
            * (1.0 - unfold.clamp(0.0, 1.0));
        let surface = Hsla::from(c.composer_surface)
            .blend(Hsla::from(c.user_bubble).opacity(unfold.clamp(0.0, 1.0)))
            .blend(Hsla::from(c.buddy_primary).opacity(accent_mix));
        // Charge is translucent over the real composer; the detached surface becomes
        // opaque. Its colour/radii settle to the exact real bubble before the handoff.
        let alpha = (charge * 0.28 + gather * 0.72) * (1.0 - handoff);
        let top = surface
            .blend(Hsla::from(c.surface_highlight).opacity(0.22 * glow))
            .opacity(alpha);
        let border = Hsla::from(c.user_bubble_border)
            .blend(Hsla::from(c.buddy_primary).opacity(glow * 0.7))
            .opacity(alpha);
        window.paint_quad(quad(
            bounds,
            radii,
            linear_gradient(
                180.0,
                linear_color_stop(top, 0.0),
                linear_color_stop(surface.opacity(alpha), 1.0),
            ),
            px(1.0),
            border,
            BorderStyle::Solid,
        ));
        // A fine rim communicates a lit surface without decorative particles.
        if glow > 0.0 {
            window.paint_quad(quad(
                bounds,
                radii,
                clear,
                px(1.0),
                Hsla::from(c.buddy_primary).opacity(glow * motion::SEND_RIM_OPACITY),
                BorderStyle::Solid,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Rc<SendMotion> {
        SendMotion::new(
            "message".into(),
            Bounds::new(point(px(20.), px(400.)), size(px(420.), px(64.))),
            None,
        )
    }

    #[test]
    fn gather_hands_its_velocity_to_flight() {
        let motion = sample();
        let launch = motion.gather_center(motion::SEND_LAUNCH_AT_MS);
        let before = motion.gather_center(motion::SEND_LAUNCH_AT_MS - 0.1);
        assert_eq!(launch, motion.launch());
        assert!((f32::from(launch.y - before.y) / 0.0001 + motion::SEND_LAUNCH_SPEED).abs() < 2.0);
        assert_eq!(motion.gather_bounds(0.0), motion.source);
        assert_eq!(
            motion.gather_bounds(motion::SEND_LAUNCH_AT_MS).size,
            size(px(motion::SEND_FOLD_WIDTH), px(motion::SEND_FOLD_HEIGHT))
        );
    }

    #[test]
    fn flight_is_frame_rate_independent_and_retargetable() {
        let target = point(px(360.), px(340.));
        let mut one_step = Flight::new(point(px(230.), px(376.)));
        let mut frames = Flight::new(point(px(230.), px(376.)));
        one_step.advance(motion::SEND_LAUNCH_AT_MS + 240.0, target);
        for i in 1..=24 {
            frames.advance(motion::SEND_LAUNCH_AT_MS + i as f32 * 10.0, target);
        }
        assert!((one_step.position.x - frames.position.x).abs() < 0.001);
        assert!((one_step.position.y - frames.position.y).abs() < 0.001);
        let before = frames.position;
        frames.advance(frames.sampled_ms, point(px(360.), px(260.)));
        assert_eq!(before, frames.position, "retargeting must not teleport");
    }

    #[test]
    fn handoff_waits_for_unfolding_and_settles_exactly() {
        assert_eq!(reveal(motion::SEND_UNFOLD_AT_MS), 0.0);
        assert_eq!(reveal(motion::SEND_HANDOFF_AT_MS), 0.0);
        assert!(expansion(motion::SEND_HANDOFF_AT_MS) > 0.98);
        assert_eq!(reveal(motion::SEND_TOTAL_MS as f32), 1.0);
        assert_eq!(expansion(motion::SEND_TOTAL_MS as f32), 1.0);
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
        assert_eq!(
            motion.target.get(),
            None,
            "an unpainted row is not a flight destination"
        );
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
