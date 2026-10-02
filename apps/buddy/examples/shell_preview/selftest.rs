//! Phase 07 窗口壳真实输入验收模块。

use super::fixture;
#[path = "input.rs"]
mod input;
#[path = "t45_window.rs"]
mod t45_window;
#[path = "t46_native.rs"]
mod t46_native;

use buddy_ui::gpui::AsyncApp;

/// 由 `shell_preview` 示例入口调用的 T45/T46 自测入口。
pub async fn run(cx: &mut AsyncApp) -> bool {
    let t46 = t46_native::run(cx).await;
    let t45 = t45_window::run(cx).await;
    let ok = t45 && t46;
    println!(
        "{} S07-01/S07-02 shell 自测",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}
