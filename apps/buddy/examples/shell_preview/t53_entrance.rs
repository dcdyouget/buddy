//! T53：按真实 render 采样入场时序，原生尺寸与同一 Router 保持不变。
use super::input;
use buddy_ui::gpui::{AppContext, AsyncApp, WindowHandle};
use buddy_ui::shell::{
    self, AppShell, entrance::EntrancePhase, positioning::Rect, positioning_native,
};
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

const FRAME_SAMPLE_DEADLINE: Duration = Duration::from_millis(350);
const FRAME_POLL: Duration = Duration::from_millis(5);

fn rect(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<Rect> {
    cx.update_window(handle.into(), |_, w, _| {
        positioning_native::prepare(w)
            .and_then(|n| n.snapshot())
            .map(|s| s.rect)
            .ok()
    })
    .ok()
    .flatten()
}
fn same(a: Rect, b: Rect) -> bool {
    (a.origin.x - b.origin.x).abs() < 1.0
        && (a.origin.y - b.origin.y).abs() < 1.0
        && a.size == b.size
}
fn phase(handle: WindowHandle<AppShell>, cx: &AsyncApp) -> Option<EntrancePhase> {
    handle.read_with(cx, |s, _| s.entrance_phase()).ok()
}

#[derive(Clone, Copy, Debug)]
struct FrameTiming {
    callback_ms: u128,
    resumed_ms: u128,
    resumed_before_deadline: bool,
}

/// Wait for a real platform frame. `Window::on_next_frame` callbacks run before
/// that frame's draw, so the task observes the rendered state on the next
/// foreground turn instead of sampling an arbitrary 20 ms timer boundary.
async fn next_frame(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
    started: Instant,
    deadline: Instant,
) -> Option<FrameTiming> {
    let callback_at = Rc::new(Cell::new(None));
    let callback_at_by_frame = callback_at.clone();
    if cx
        .update_window(handle.into(), |_, window, _| {
            window.on_next_frame(move |_, _| callback_at_by_frame.set(Some(Instant::now())));
        })
        .is_err()
    {
        return None;
    }
    while callback_at.get().is_none() && Instant::now() < deadline {
        cx.background_executor().timer(FRAME_POLL).await;
    }
    let resumed_at = Instant::now();
    callback_at.get().map(|callback_at| FrameTiming {
        callback_ms: callback_at.saturating_duration_since(started).as_millis(),
        resumed_ms: resumed_at.saturating_duration_since(started).as_millis(),
        resumed_before_deadline: resumed_at <= deadline,
    })
}

pub(crate) async fn run(cx: &mut AsyncApp) -> bool {
    let Some(handle) = cx.update(|app| shell::runtime::main_window(app)) else {
        return false;
    };
    let Ok(router) = handle.read_with(cx, |s, _| s.router()) else {
        return false;
    };
    let reduced = buddy_ui::accessibility::prefers_reduced_motion();
    if shell::runtime::hide(handle, cx).await.is_err() {
        return false;
    }
    let reset = phase(handle, cx)
        == Some(if reduced {
            EntrancePhase::Settled
        } else {
            EntrancePhase::Hidden
        });
    router.update(cx, |r, cx| r.invoked_after_idle(cx));
    if shell::runtime::show(handle, cx).await.is_err() {
        return false;
    }
    let Some(before) = rect(handle, cx) else {
        return false;
    };
    let started = Instant::now();
    let deadline = started + FRAME_SAMPLE_DEADLINE;
    println!(
        "T53 sampling_started deadline={}ms reduced={reduced}",
        FRAME_SAMPLE_DEADLINE.as_millis()
    );
    let mut samples = Vec::new();
    let mut frame_timings = Vec::new();
    let mut fixed_bounds = true;
    while Instant::now() < deadline {
        let Some(timing) = next_frame(handle, cx, started, deadline).await else {
            break;
        };
        frame_timings.push(timing);
        if !timing.resumed_before_deadline {
            break;
        }
        if let Ok(sample) = handle.read_with(cx, |s, _| {
            (
                s.entrance_elapsed_ms(),
                s.entrance_phase(),
                s.last_entrance_frame(),
            )
        }) {
            samples.push(sample);
        }
        fixed_bounds &= rect(handle, cx).is_some_and(|r| same(before, r));
        if samples.last().is_some_and(|(time, phase, frame)| {
            *time == 200 && *phase == EntrancePhase::Settled && frame.is_none()
        }) {
            break;
        }
    }
    let finished = phase(handle, cx) == Some(EntrancePhase::Settled);
    let painted: Vec<_> = samples
        .iter()
        .filter_map(|(time, _, frame)| frame.map(|f| (*time, f)))
        .collect();
    let frame_timing = if reduced {
        !samples.is_empty()
            && painted.is_empty()
            && samples
                .iter()
                .all(|(_, phase, _)| *phase == EntrancePhase::Settled)
    } else {
        painted.len() >= 3
            && painted
                .first()
                .is_some_and(|(time, f)| *time < 60 && f.opacity > 0.0 && f.scale_x < 1.006)
            && painted
                .iter()
                .all(|(time, f)| *time <= 200 && f.opacity >= 0.0 && f.opacity <= 0.59)
            && samples.iter().any(|(time, phase, frame)| {
                *time == 200 && *phase == EntrancePhase::Settled && frame.is_none()
            })
    };
    let frame_timings: Vec<_> = frame_timings
        .iter()
        .map(|t| (t.callback_ms, t.resumed_ms))
        .collect();
    println!(
        "T53 reduced={reduced} reset={reset} duration={}ms bounds_fixed={fixed_bounds} samples={samples:?} frame_timings={frame_timings:?}",
        started.elapsed().as_millis(),
    );
    router.update(cx, |r, cx| r.open_settings(cx));
    input::settle(handle, cx).await;
    router.update(cx, |r, cx| r.close_settings(cx));
    input::settle(handle, cx).await;
    router.update(cx, |r, cx| r.invoked_after_idle(cx));
    input::settle(handle, cx).await;
    let no_page_replay = handle
        .read_with(cx, |s, _| {
            s.entrance_phase() == EntrancePhase::Settled && s.last_entrance_frame().is_none()
        })
        .unwrap_or(false);
    let hidden = shell::runtime::hide(handle, cx).await.is_ok();
    let reshown = shell::runtime::show(handle, cx).await.is_ok();
    input::draw(handle, cx).await;
    let replay = handle
        .read_with(cx, |s, _| {
            if reduced {
                s.last_entrance_frame().is_none()
            } else {
                s.entrance_phase() == EntrancePhase::Entering && s.last_entrance_frame().is_some()
            }
        })
        .unwrap_or(false);
    input::settle(handle, cx).await;
    router.update(cx, |r, cx| r.open_settings(cx));
    input::settle(handle, cx).await;
    let expanded_hidden = shell::runtime::hide(handle, cx).await.is_ok();
    let expanded_shown = shell::runtime::show(handle, cx).await.is_ok();
    input::draw(handle, cx).await;
    let expanded_static = handle
        .read_with(cx, |s, _| {
            s.entrance_phase() == EntrancePhase::Settled && s.last_entrance_frame().is_none()
        })
        .unwrap_or(false);
    let identity = handle
        .read_with(cx, |s, _| s.router().entity_id() == router.entity_id())
        .unwrap_or(false);
    let ok = reset
        && fixed_bounds
        && frame_timing
        && finished
        && no_page_replay
        && hidden
        && reshown
        && replay
        && expanded_hidden
        && expanded_shown
        && expanded_static
        && identity;
    println!(
        "{} S07-08 T53 timing={frame_timing} settled={finished} no_page_replay={no_page_replay} replay={replay} expanded_static={expanded_static} router={identity}",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}
