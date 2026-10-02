//! Phase 07 窗口壳真实输入验收模块。

use super::external_target;
use super::fixture;
#[path = "input.rs"]
mod input;
#[path = "os_input.rs"]
mod os_input;
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

use buddy_ui::gpui::AsyncApp;

/// 由 `shell_preview` 示例入口调用的 T45/T46 自测入口。
pub async fn run(cx: &mut AsyncApp) -> bool {
    let windows = run_windows(cx).await;
    let behavior = run_behaviors(cx).await;
    let ok = windows && behavior;
    println!(
        "{} S07-01/S07-02/S07-03/S07-04 shell 自测",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

/// 不投递系统键鼠，可在锁屏会话中复核既有窗口尺寸和原生属性。
pub async fn run_windows(cx: &mut AsyncApp) -> bool {
    let t46 = t46_native::run(cx).await;
    let t45 = t45_window::run(cx).await;
    let ok = t45 && t46;
    println!(
        "{} S07-01/S07-02 window 自测",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

/// 系统热键 / 显隐定向自测，减少就地变异时无关窗口流程。
pub async fn run_behaviors(cx: &mut AsyncApp) -> bool {
    let t47 = t47_toggle::run(cx).await;
    let t48 = t47 && t48_dismiss::run(cx).await;
    let t49 = t48 && t49_selection::run(cx).await;
    let ok = t47 && t48 && t49;
    println!(
        "{} S07-03/S07-04 behavior 自测",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}
