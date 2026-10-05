//! GPUI 事件与每屏位置记忆；原生变更一律在退出 App 借用后执行。

use super::{AppShell, config::LogicalSize, positioning, positioning_native, visibility};
use crate::{accessibility, theme_system::easing};
use gpui::{AsyncApp, Context, Subscription, Window, WindowHandle};
use std::time::Duration;

const SAVE_DELAY: Duration = Duration::from_millis(160);
const RESIZE_STEPS: u32 = 12;
const RESIZE_STEP: Duration = Duration::from_millis(crate::theme_system::tokens::motion::DURATION_NORMAL as u64 / (RESIZE_STEPS - 1) as u64);

pub(super) fn observe(window: &mut Window, cx: &Context<AppShell>) -> [Subscription; 2] {
    let bounds = cx.observe_window_bounds(window, |shell, window, cx| {
        let handle = window.window_handle().downcast::<AppShell>().unwrap();
        // Dropping the previous task cancels its timer; only the last native position is saved.
        shell.pending_position_save = Some(cx.spawn(async move |_, cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            let _ = handle.update(cx, |shell, window, _| save_current(shell, window));
        }));
    });
    let activation = cx.observe_window_activation(window, |shell, window, _| {
        if !window.is_window_active() {
            shell.pending_position_save = None;
            save_current(shell, window);
        }
    });
    [bounds, activation]
}

fn save_current(shell: &mut AppShell, window: &Window) {
    match positioning_native::prepare(window).and_then(|native| native.snapshot()) {
        Ok(snapshot) => shell.positions.save(&snapshot.screen, snapshot.rect.origin),
        Err(error) => log::warn!("保存窗口位置失败：{error}"),
    }
}

/// 在隐藏前立即保存，避免快速移动 / 隐藏早于防抖到期。
pub(super) fn save_before_hide(shell: &mut AppShell, window: &Window) {
    shell.pending_position_save = None;
    save_current(shell, window);
}

/// 页面尺寸来源仍为 sizing；原生按底边锚定的短步进提交矩形，避免异步 resize 与移动相互覆盖。
pub(super) fn resize(
    shell: &mut AppShell,
    target: LogicalSize,
    window: &mut Window,
    cx: &mut Context<AppShell>,
) {
    #[cfg(target_os = "macos")]
    {
        // A hidden window may be resized while idle compaction is applied. Complete that
        // geometry immediately so the next hotkey does not wait for an invisible animation.
        // If the visibility probe is unavailable, keep the animated path as the safe default.
        let animate_resize = visibility::prepare(window)
            .and_then(|native| native.probe())
            .map(|snapshot| snapshot.is_visible)
            .unwrap_or(true);
        let prepared = positioning_native::prepare(window).and_then(|native| {
            let snapshot = native.snapshot()?;
            let size = positioning::Size {
                width: target.width as f64,
                height: target.height as f64,
            };
            let origin = positioning::bottom_anchored(snapshot.rect, size, &snapshot.screen);
            Ok((native, positioning::Rect { origin, size }))
        });
        match prepared {
            Ok((native, rect)) => {
                shell.pending_resize = Some(cx.spawn(async move |_, cx| {
                    if !animate_resize || accessibility::prefers_reduced_motion() {
                        if let Err(error) = native.set_rect(rect) {
                            log::error!("调整主窗口几何失败：{error}");
                        }
                        return;
                    }
                    let start = match native.snapshot() {
                        Ok(snapshot) => snapshot.rect,
                        Err(error) => {
                            log::error!("读取主窗口起始几何失败：{error}");
                            return;
                        }
                    };
                    for step in 1..=RESIZE_STEPS {
                        if step > 1 {
                            // The native frame is updated on the main executor; yielding here
                            // keeps the animation responsive and lets a newer resize replace it.
                            cx.background_executor().timer(RESIZE_STEP).await;
                        }
                        let t = step as f32 / RESIZE_STEPS as f32;
                        let t = easing::cubic_bezier(
                            crate::theme_system::tokens::motion::EASE_STANDARD,
                        )(t);
                        let current = interpolate_rect(start, rect, t);
                        if let Err(error) = native.set_rect(current) {
                            log::error!("调整主窗口几何失败：{error}");
                            return;
                        }
                    }
                }));
            }
            Err(error) => log::error!("读取主窗口几何失败：{error}"),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (shell, cx);
        window.resize(target.to_gpui());
    }
}

fn interpolate_rect(
    from: positioning::Rect,
    to: positioning::Rect,
    amount: f32,
) -> positioning::Rect {
    let amount = amount.clamp(0.0, 1.0) as f64;
    positioning::Rect {
        origin: positioning::Point {
            x: from.origin.x + (to.origin.x - from.origin.x) * amount,
            y: from.origin.y + (to.origin.y - from.origin.y) * amount,
        },
        size: positioning::Size {
            width: from.size.width + (to.size.width - from.size.width) * amount,
            height: from.size.height + (to.size.height - from.size.height) * amount,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_interpolation_keeps_bottom_anchor() {
        let from = positioning::Rect::new(100.0, 300.0, 400.0, 200.0);
        let to = positioning::Rect::new(60.0, 100.0, 480.0, 400.0);
        let middle = interpolate_rect(from, to, 0.5);
        assert_eq!(
            middle.size,
            positioning::Size {
                width: 440.0,
                height: 300.0,
            }
        );
        assert_eq!(middle.origin, positioning::Point { x: 80.0, y: 200.0 });
        assert_eq!(middle.origin.y + middle.size.height, 500.0);
        assert_eq!(to.origin.y + to.size.height, 500.0);
    }
}

/// 热键跟随焦点屏幕；托盘调用可明确选择鼠标屏幕。读取发生在激活 Buddy 之前。
pub(super) async fn restore(
    handle: WindowHandle<AppShell>,
    focused: bool,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        // Idle reset can enqueue a page resize in this same hotkey turn. Finish it first,
        // otherwise restoration would use the old size and a later resize would move it again.
        let pending = handle
            .update(cx, |shell, _, _| shell.pending_resize.take())
            .map_err(|error| error.to_string())?;
        if let Some(pending) = pending {
            pending.await;
        }
        let (native, origin) = handle
            .update(cx, |shell, window, _| {
                let native = positioning_native::prepare(window)?;
                let snapshot = native.snapshot()?;
                let screen =
                    positioning_native::target_screen(focused, Some(&snapshot.screen.key))?;
                let origin = shell.positions.restore(&screen, snapshot.rect.size);
                Ok::<_, String>((native, origin))
            })
            .map_err(|error| error.to_string())??;
        native.move_to(origin)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (handle, focused, cx);
        Ok(())
    }
}
