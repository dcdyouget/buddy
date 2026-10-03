//! 全屏辅助 panel 的跨应用排序；降低层级不隐藏、不取消生成。
use super::{AppShell, visibility};
use gpui::{Context, Subscription, Window};
use std::time::Duration;

pub(super) fn observe(window: &mut Window, cx: &Context<AppShell>) -> Subscription {
    cx.observe_window_activation(window, |shell, window, cx| {
        let Ok(native) = visibility::prepare(window) else {
            return;
        };
        // ResignKey can precede application activation. Coalesce older work and reread
        // current native state for a bounded second, never replay a captured "lower" action.
        shell.focus_order_task = Some(cx.spawn(async move |_, cx| {
            for delay in [20, 80, 200, 700] {
                cx.background_executor()
                    .timer(Duration::from_millis(delay))
                    .await;
                if let Err(error) = native.sync_focus_level() {
                    log::warn!("外部应用激活后调整窗口层级失败：{error}");
                    return;
                }
            }
        }));
    })
}
