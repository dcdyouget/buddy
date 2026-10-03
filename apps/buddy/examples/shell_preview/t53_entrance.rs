//! T53：按真实 render 采样入场时序，原生尺寸与同一 Router 保持不变。
use super::input;
use buddy_ui::gpui::{AppContext, AsyncApp, WindowHandle};
use buddy_ui::shell::{
    self, AppShell, entrance::EntrancePhase, positioning::Rect, positioning_native,
};
use std::time::{Duration, Instant};

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
    let mut samples = Vec::new();
    let mut fixed_bounds = true;
    for _ in 0..16 {
        input::draw(handle, cx).await;
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
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    let finished = phase(handle, cx) == Some(EntrancePhase::Settled);
    let painted: Vec<_> = samples
        .iter()
        .filter_map(|(time, _, frame)| frame.map(|f| (*time, f)))
        .collect();
    let frame_timing = if reduced {
        painted.is_empty()
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
    println!(
        "T53 reduced={reduced} reset={reset} duration={}ms bounds_fixed={fixed_bounds} samples={samples:?}",
        started.elapsed().as_millis()
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
