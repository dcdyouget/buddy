//! T51：真实主窗口移动、防抖保存、热键恢复与页面底边锚定。

use super::{input, os_input};
use buddy_ui::gpui::{AppContext, AsyncApp, WindowHandle};
use buddy_ui::shell::{
    self, AppShell,
    positioning::{Point, Rect, Size, WINDOW_MARGIN},
    positioning_native,
};
use std::time::Duration;

fn probe(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<positioning_native::Snapshot> {
    cx.update_window(handle.into(), |_, window, _| {
        positioning_native::prepare(window)
            .and_then(|native| native.snapshot())
            .ok()
    })
    .ok()
    .flatten()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1.0
}
fn same_point(a: Point, b: Point) -> bool {
    close(a.x, b.x) && close(a.y, b.y)
}

fn move_window(handle: WindowHandle<AppShell>, origin: Point, cx: &mut AsyncApp) -> bool {
    let native = cx.update_window(handle.into(), |_, window, _| {
        positioning_native::prepare(window)
    });
    native
        .ok()
        .and_then(Result::ok)
        .is_some_and(|native| native.move_to(origin).is_ok())
}

async fn wait_size(handle: WindowHandle<AppShell>, size: Size, cx: &mut AsyncApp) -> bool {
    for _ in 0..100 {
        if probe(handle, cx).is_some_and(|s| {
            close(s.rect.size.width, size.width) && close(s.rect.size.height, size.height)
        }) {
            return true;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
        input::draw(handle, cx).await;
    }
    false
}

async fn toggle(handle: WindowHandle<AppShell>, expected: bool, cx: &mut AsyncApp) -> bool {
    if !os_input::combo("N") {
        return false;
    }
    for _ in 0..150 {
        let state = cx
            .update_window(handle.into(), |_, window, _| {
                shell::native::probe_main_window(window).ok()
            })
            .ok()
            .flatten();
        if state.is_some_and(|s| s.is_visible == expected && (!expected || s.is_key)) {
            return true;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
    false
}

pub(crate) async fn run(cx: &mut AsyncApp) -> bool {
    let Some(handle) = cx.update(|app| shell::runtime::main_window(app)) else {
        return false;
    };
    let Ok(router) = handle.read_with(cx, |shell, _| shell.router()) else {
        return false;
    };
    if shell::runtime::show(handle, cx).await.is_err() {
        return false;
    }
    router.update(cx, |router, cx| router.invoked_after_idle(cx));
    let compact = wait_size(
        handle,
        Size {
            width: 560.0,
            height: 60.0,
        },
        cx,
    )
    .await;
    let Some(initial) = probe(handle, cx) else {
        return false;
    };
    let work = initial.screen.work_area;
    let start = Point {
        x: work.origin.x + 200.0,
        y: work.origin.y + work.size.height - 160.0,
    };
    let moved = move_window(handle, start, cx);
    cx.background_executor()
        .timer(Duration::from_millis(250))
        .await;
    let saved = handle
        .read_with(cx, |shell, _| {
            shell.saved_window_position(&initial.screen.key)
        })
        .ok()
        .flatten();
    let first_saved = saved.is_some_and(|p| same_point(p, start));

    let first = Point {
        x: start.x + 30.0,
        y: start.y - 30.0,
    };
    let last = Point {
        x: start.x + 70.0,
        y: start.y - 40.0,
    };
    let first_move = move_window(handle, first, cx);
    cx.background_executor()
        .timer(Duration::from_millis(40))
        .await;
    let second_move = move_window(handle, last, cx);
    cx.background_executor()
        .timer(Duration::from_millis(40))
        .await;
    let early = handle
        .read_with(cx, |shell, _| {
            shell.saved_window_position(&initial.screen.key)
        })
        .ok()
        .flatten();
    let debounced = early.is_some_and(|p| same_point(p, start));
    cx.background_executor()
        .timer(Duration::from_millis(250))
        .await;
    let late = handle
        .read_with(cx, |shell, _| {
            shell.saved_window_position(&initial.screen.key)
        })
        .ok()
        .flatten();
    let latest_saved = late.is_some_and(|p| same_point(p, last));
    let hidden = toggle(handle, false, cx).await;
    cx.background_executor()
        .timer(Duration::from_millis(100))
        .await;
    // Move the hidden native window without changing saved state; a no-op restore must fail.
    let displaced_hidden = move_window(
        handle,
        Point {
            x: work.origin.x - 100.0,
            y: work.origin.y - 100.0,
        },
        cx,
    );
    let shown = toggle(handle, true, cx).await;
    let restored = probe(handle, cx).is_some_and(|s| same_point(s.rect.origin, last));

    let Some(before) = probe(handle, cx) else {
        return false;
    };
    input::draw(handle, cx).await;
    let compact_capture =
        super::capture::checkpoint("BUDDY_SHELL_T51_CAPTURE_DIR", "compact-restored", cx).await;
    router.update(cx, |router, cx| router.open_settings(cx));
    let expanded = wait_size(
        handle,
        Size {
            width: 760.0,
            height: 640.0,
        },
        cx,
    )
    .await;
    input::draw(handle, cx).await;
    let expanded_capture =
        super::capture::checkpoint("BUDDY_SHELL_T51_CAPTURE_DIR", "expanded", cx).await;
    let after = probe(handle, cx);
    let anchored = after.as_ref().is_some_and(|s| {
        close(
            s.rect.origin.y + s.rect.size.height,
            before.rect.origin.y + before.rect.size.height,
        ) && close(
            s.rect.origin.x + s.rect.size.width / 2.0,
            before.rect.origin.x + before.rect.size.width / 2.0,
        )
    });
    let contained = after.as_ref().is_some_and(|s| {
        let Rect { origin, size } = s.rect;
        origin.x >= work.origin.x + WINDOW_MARGIN - 1.0
            && origin.y >= work.origin.y + WINDOW_MARGIN - 1.0
            && origin.x + size.width <= work.origin.x + work.size.width - WINDOW_MARGIN + 1.0
            && origin.y + size.height <= work.origin.y + work.size.height - WINDOW_MARGIN + 1.0
    });
    router.update(cx, |router, cx| router.close_settings(cx));
    // A user-sized conversation must still survive ordinary content/settings transitions.
    let _ = wait_size(
        handle,
        Size {
            width: 750.0,
            height: 500.0,
        },
        cx,
    )
    .await;
    let user_size = Size {
        width: 800.0,
        height: 600.0,
    };
    let prepared = cx
        .update_window(handle.into(), |_, window, _| {
            positioning_native::prepare(window)
        })
        .ok()
        .and_then(Result::ok);
    let user_resized = prepared.is_some_and(|native| {
        native
            .snapshot()
            .and_then(|s| {
                native.set_rect(Rect {
                    origin: s.rect.origin,
                    size: user_size,
                })
            })
            .is_ok()
    });
    cx.background_executor()
        .timer(Duration::from_millis(100))
        .await;
    router.update(cx, |router, cx| router.open_settings(cx));
    input::settle(handle, cx).await;
    let preserved = wait_size(handle, user_size, cx).await;
    let same_router = handle
        .read_with(cx, |shell, _| shell.router() == router)
        .unwrap_or(false);
    let ok = compact_capture
        && expanded_capture
        && compact
        && moved
        && first_saved
        && first_move
        && second_move
        && debounced
        && latest_saved
        && hidden
        && displaced_hidden
        && shown
        && restored
        && expanded
        && anchored
        && contained
        && user_resized
        && preserved
        && same_router;
    println!(
        "T51：紧凑/移动/保存 {compact}/{moved}/{first_saved}；连续移动/提前未保存/最终保存 {}/{debounced}/{latest_saved}；系统热键隐藏/重显/位置恢复 {hidden}/{shown}/{restored}；展开/底边与中心/工作区 {expanded}/{anchored}/{contained}；用户尺寸/保留/同Router {user_resized}/{preserved}/{same_router}；屏幕={} scale={} work={work:?} before={:?} after={:?}",
        first_move && second_move,
        initial.screen.key,
        initial.screen.scale,
        before.rect,
        after.map(|s| s.rect)
    );
    ok
}
