//! Phase 07 窗口壳真实输入验收模块。

use super::fixture;
#[path = "capture.rs"]
mod capture;
use super::{external_target, hotkey_owner};
use super::{fullscreen_target, level_native};
#[path = "input.rs"]
mod input;
#[path = "os_input.rs"]
mod os_input;
#[path = "os_pointer.rs"]
mod os_pointer;
#[path = "t45_window.rs"]
mod t45_window;
#[path = "t46_native.rs"]
mod t46_native;
#[path = "t47_toggle.rs"]
mod t47_toggle;
#[path = "t48_dismiss.rs"]
mod t48_dismiss;
#[path = "t49_selection.rs"]
mod t49_selection;
#[path = "t50_level.rs"]
mod t50_level;
#[path = "t51_positioning.rs"]
mod t51_positioning;
#[path = "t52_drag.rs"]
mod t52_drag;
#[path = "t53_entrance.rs"]
mod t53_entrance;

use buddy_ui::gpui::{AppContext, AsyncApp};

/// 由 `shell_preview` 示例入口调用的 T45/T46 自测入口。
pub async fn run(cx: &mut AsyncApp) -> bool {
    let windows = run_windows(cx).await;
    let behavior = run_behaviors(cx).await;
    let levels = behavior && t50_level::run(cx).await;
    let positioning = levels && t51_positioning::run(cx).await;
    let dragging = positioning && t52_drag::run(cx).await;
    let entrance = dragging && t53_entrance::run(cx).await;
    let ok = windows && behavior && levels && positioning && dragging && entrance;
    println!(
        "{} S07-01–S07-08 shell 自测",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

pub async fn run_entrance(cx: &mut AsyncApp) -> bool {
    let behavior = run_behaviors(cx).await;
    behavior && t53_entrance::run(cx).await
}

pub async fn run_drag(cx: &mut AsyncApp) -> bool {
    let behavior = run_behaviors(cx).await;
    behavior && t52_drag::run(cx).await
}

pub async fn run_levels(cx: &mut AsyncApp) -> bool {
    let behavior = run_behaviors(cx).await;
    let ok = behavior && t50_level::run(cx).await;
    println!("{} S07-05 level 自测", if ok { "PASS" } else { "FAIL" });
    ok
}

pub async fn run_positioning(cx: &mut AsyncApp) -> bool {
    let behavior = run_behaviors(cx).await;
    let ok = behavior && t51_positioning::run(cx).await;
    println!(
        "{} S07-06 positioning 自测",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

/// 不投递系统键鼠，可在锁屏会话中复核既有窗口尺寸和原生属性。
pub async fn run_windows(cx: &mut AsyncApp) -> bool {
    let existing = cx.update(|app| app.windows());
    let t46 = t46_native::run(cx).await;
    let t45 = t45_window::run(cx).await;
    // T45/T46 create their own windows; do not let them steal focus/Space selection
    // from the one runtime window installed by the following behavioral tests.
    let created = cx.update(|app| app.windows());
    for window in created {
        if !existing.contains(&window) {
            let _ = cx.update_window(window, |_, window, _| window.remove_window());
        }
    }
    let ok = t45 && t46;
    println!(
        "{} S07-01/S07-02 window 自测",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

/// 系统热键 / 显隐定向自测，减少就地变异时无关窗口流程。
pub async fn run_behaviors(cx: &mut AsyncApp) -> bool {
    let clipboard = cx.update(|app| app.read_from_clipboard());
    let t47 = t47_toggle::run(cx).await;
    let t48 = t47 && t48_dismiss::run(cx).await;
    let t49 = t48 && t49_selection::run(cx).await;
    let ok = t47 && t48 && t49;
    cx.update(|app| {
        app.write_to_clipboard(
            clipboard.unwrap_or_else(|| buddy_ui::gpui::ClipboardItem::new_string(String::new())),
        )
    });
    println!(
        "{} S07-03/S07-04 behavior 自测",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}
