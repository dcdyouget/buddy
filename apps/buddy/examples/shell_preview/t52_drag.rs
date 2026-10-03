//! T52：实际 OS 鼠标拖动、字形选择与控件隔离，复用唯一 runtime。
use super::{input, os_input, os_pointer};
use buddy_engine::models::MessageRole;
use buddy_ui::chat::state::{ChatState, user_message};
use buddy_ui::gpui::{AppContext, AsyncApp, WindowHandle};
use buddy_ui::shell::{
    self, AppShell,
    positioning::{Point, Rect},
    positioning_native,
};

#[path = "t52_native.rs"]
mod native;
#[path = "t52_regions.rs"]
mod regions;

fn rect(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<Rect> {
    cx.update_window(handle.into(), |_, window, _| {
        positioning_native::prepare(window)
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
        && (a.size.width - b.size.width).abs() < 1.0
        && (a.size.height - b.size.height).abs() < 1.0
}

// Keep each OS-input fixture clear of desktop-center system prompts, regardless
// of the dimensions left by an earlier test. Product positioning is unchanged.
fn place_for_os_input(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> bool {
    let prepared = cx
        .update_window(handle.into(), |_, window, _| {
            positioning_native::prepare(window)
        })
        .ok()
        .and_then(Result::ok);
    let placed = prepared
        .and_then(|position| position.move_to(Point { x: 40.0, y: 100.0 }).ok())
        .is_some();
    if !placed {
        println!("FAIL T52：无法将测试主窗移至受控输入区域");
    }
    placed
}

async fn drag(
    handle: WindowHandle<AppShell>,
    from: (f64, f64),
    delta: (f64, f64),
    cx: &mut AsyncApp,
) -> bool {
    let Some(bounds) = rect(handle, cx) else {
        return false;
    };
    let start = (bounds.origin.x + from.0, bounds.origin.y + from.1);
    let end = (start.0 + delta.0, start.1 + delta.1);
    let sent = cx
        .background_spawn(async move { os_pointer::drag(start, end) })
        .await;
    input::settle(handle, cx).await;
    sent
}
async fn moved(handle: WindowHandle<AppShell>, point: (f64, f64), cx: &mut AsyncApp) -> bool {
    let Some(before) = rect(handle, cx) else {
        return false;
    };
    let state = cx
        .update_window(handle.into(), |_, w, _| {
            (
                shell::native::probe_main_window(w).ok(),
                shell::workspaces::probe(w).ok(),
            )
        })
        .ok();
    let sent = drag(handle, point, (45.0, 35.0), cx).await;
    let after = rect(handle, cx);
    let ok = sent
        && after.is_some_and(|a| {
            (a.origin.x - before.origin.x - 45.0).abs() < 2.0
                && (a.origin.y - before.origin.y - 35.0).abs() < 2.0
                && a.size == before.size
        });
    if !ok {
        println!("T52 failed drag initial_native={state:?}");
    }
    println!("T52 native drag point={point:?} before={before:?} after={after:?} moved={ok}");
    ok
}

pub(crate) async fn run(cx: &mut AsyncApp) -> bool {
    let saved = cx.update(|app| app.read_from_clipboard());
    let ok = run_inner(cx).await;
    cx.update(|app| {
        app.write_to_clipboard(
            saved.unwrap_or_else(|| buddy_ui::gpui::ClipboardItem::new_string(String::new())),
        )
    });
    ok
}

async fn run_inner(cx: &mut AsyncApp) -> bool {
    let preflight = os_input::preflight();
    if preflight != os_input::Preflight::Ready {
        println!("FAIL T52：{}", os_input::preflight_message(preflight));
        return false;
    }
    let Some(handle) = cx.update(|app| shell::runtime::main_window(app)) else {
        return false;
    };
    let Ok(router) = handle.read_with(cx, |s, _| s.router()) else {
        return false;
    };
    if let Err(error) = shell::runtime::show(handle, cx).await {
        println!("FAIL T52：初始显示主窗口失败：{error}");
        return false;
    }
    if !native::wait_native_input_ready("初始 OS 输入", handle, cx).await {
        return false;
    }
    if !place_for_os_input(handle, cx) {
        return false;
    }
    router.update(cx, |r, cx| r.open_settings(cx));
    input::settle(handle, cx).await;
    router.update(cx, |r, cx| r.close_settings(cx));
    input::settle(handle, cx).await;
    let sample = "拖动选择验收文本";
    let mut message = user_message(sample);
    message.role = MessageRole::Assistant;
    message.id = "t52-assistant".into();
    let conversation = router.read_with(cx, |r, _| r.conversation().clone());
    conversation.update(cx, |c, cx| {
        let revision = c.state.revision + 1;
        c.state = ChatState::from_history(vec![message]);
        c.state.revision = revision;
        cx.notify();
    });
    let transcript = router.read_with(cx, |r, app| r.transcript(app));
    transcript.update(cx, |t, cx| t.scroll_to_bottom(cx));
    input::settle(handle, cx).await;
    let row = transcript.read_with(cx, |t, _| {
        t.rows()
            .iter()
            .find(|r| matches!(r.kind, buddy_ui::chat::rows::RowKind::Block { .. }))
            .map(|r| r.id.clone())
    });
    let Some(row) = row else {
        println!("FAIL T52：无正文行");
        return false;
    };
    let row_bounds = transcript.read_with(cx, |t, _| t.painted_row_bounds(&row));
    let Some(row_bounds) = row_bounds else {
        println!("FAIL T52：正文未绘制");
        return false;
    };
    let y = f64::from(f32::from(row_bounds.origin.y)) + 22.0;
    let empty_space = moved(handle, (300.0, 90.0), cx).await;
    let edge = moved(handle, (3.0, 80.0), cx).await;
    let blank = moved(handle, (500.0, y), cx).await;
    let Some(before) = rect(handle, cx) else {
        return false;
    };
    let selection_sent = drag(handle, (17.0, y), (190.0, 0.0), cx).await;
    let copy_sent = os_input::copy();
    input::settle(handle, cx).await;
    let copied = cx
        .update(|app| app.read_from_clipboard())
        .and_then(|c| c.text())
        .unwrap_or_default();
    let selection = selection_sent
        && copy_sent
        && copied == sample
        && rect(handle, cx).is_some_and(|r| same(before, r));
    println!(
        "T52 selection copied={copied:?} expected={sample:?} unchanged={:?}",
        rect(handle, cx).map(|r| same(before, r))
    );
    let captured = super::capture::checkpoint("BUDDY_SHELL_T52_CAPTURE_DIR", "selected", cx).await;
    let Some(button) = router.read_with(cx, |r, app| r.composer().read(app).model_button_bounds())
    else {
        println!("FAIL T52：模型按钮未绘制");
        return false;
    };
    let current = rect(handle, cx).unwrap_or(before);
    let clicked = os_input::click_screen(
        current.origin.x + f64::from(f32::from(button.center().x)),
        current.origin.y + f64::from(f32::from(button.center().y)),
    );
    input::settle(handle, cx).await;
    let menu = router.read_with(cx, |r, _| r.model_menu());
    let control = clicked && menu.is_some() && rect(handle, cx).is_some_and(|r| same(current, r));
    if let Some(menu) = menu {
        let _ = cx.update_window(menu.into(), |_, w, cx| {
            w.dispatch_event(
                buddy_ui::gpui::PlatformInput::KeyDown(buddy_ui::gpui::KeyDownEvent {
                    keystroke: buddy_ui::gpui::Keystroke::parse("escape").unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                }),
                cx,
            );
        });
    }
    if let Err(error) = shell::runtime::show(handle, cx).await {
        println!("FAIL T52：滚动前重新显示主窗口失败：{error}");
        return false;
    }
    if !native::wait_native_input_ready("滚动", handle, cx).await {
        return false;
    }
    conversation.update(cx, |c, cx| {
        let revision = c.state.revision + 1;
        c.state = ChatState::from_history(
            (0..60)
                .map(|i| user_message(&format!("滚动验收第 {i} 行")))
                .collect(),
        );
        c.state.revision = revision;
        cx.notify();
    });
    input::settle(handle, cx).await;
    transcript.update(cx, |t, cx| t.scroll_to_bottom(cx));
    input::settle(handle, cx).await;
    let scroll_before = transcript.read_with(cx, |t, _| t.list_state().logical_scroll_top());
    let scroll_rect = rect(handle, cx).unwrap_or(current);
    let sent = cx
        .background_spawn(async move {
            os_pointer::scroll(
                scroll_rect.origin.x + 300.0,
                scroll_rect.origin.y + 150.0,
                240,
            )
        })
        .await;
    input::settle(handle, cx).await;
    let scroll_after = transcript.read_with(cx, |t, _| t.list_state().logical_scroll_top());
    let scroll = sent
        && (scroll_before.item_ix != scroll_after.item_ix
            || scroll_before.offset_in_item != scroll_after.offset_in_item)
        && rect(handle, cx).is_some_and(|r| same(scroll_rect, r));
    println!("T52 scroll before={scroll_before:?} after={scroll_after:?} scroll={scroll}");
    let regions = regions::run(handle, &router, cx).await;
    let identity = handle
        .read_with(cx, |s, _| s.router().entity_id() == router.entity_id())
        .unwrap_or(false);
    let ok = empty_space
        && edge
        && blank
        && selection
        && control
        && scroll
        && regions
        && identity
        && captured;
    println!(
        "{} S07-07 T52 empty_space={empty_space} regions={regions} edge={edge} blank={blank} selection={selection} control={control} scroll={scroll} router={identity}",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}
