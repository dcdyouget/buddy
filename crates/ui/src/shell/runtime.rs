//! 进程唯一的全局热键 / 外部点击运行时；隐藏仅操作原生窗口。

use super::{AppShell, hotkey::GlobalHotkey, selection, visibility};
use gpui::{App, AppContext, AsyncApp, Global, Task, WindowHandle};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

/// 外壳持有系统注册及无轮询监听任务；只在 GPUI 主线程使用。
pub struct Runtime {
    window: WindowHandle<AppShell>,
    hotkey: Rc<RefCell<GlobalHotkey>>,
    last_invoked: Instant,
    _click_monitor: visibility::OutsideClickMonitor,
    _hotkey_task: Task<()>,
    _click_task: Task<()>,
}

impl Global for Runtime {}

/// 安装单一主窗口的系统事件桥。启动冲突保留 manager，设置中仍可重试。
pub async fn install(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Result<(), String> {
    if cx.update(|cx| cx.has_global::<Runtime>()) {
        return Err("主窗口外壳事件桥已经安装".into());
    }
    let router = handle
        .read_with(cx, |shell, _| shell.router())
        .map_err(|e| format!("读取主窗口热键失败：{e}"))?;
    let config = router.read_with(cx, |router, _| router.config().hotkey.clone());
    // 所有可能失败的窗口 / monitor 检查先行，避免消耗唯一事件桥后失去重试机会。
    let (clicks, mut click_events) = tokio::sync::mpsc::unbounded_channel();
    let monitor = visibility::OutsideClickMonitor::new(clicks).map_err(|e| e.to_string())?;
    let (manager, mut events) = GlobalHotkey::unregistered().map_err(|e| e.to_string())?;
    let manager = Rc::new(RefCell::new(manager));
    let registration = manager
        .borrow_mut()
        .update(&config)
        .map_err(|e| e.to_string());
    let update_manager = manager.clone();
    router.update(cx, |router, _| {
        router.set_hotkey_updater(Rc::new(move |value| {
            update_manager
                .borrow_mut()
                .update(value)
                .map_err(|e| e.to_string())
        }));
    });
    let event_manager = manager.clone();
    let hotkey_task = cx.update(|cx| {
        cx.spawn(async move |cx| {
            while let Some(event) = events.recv().await {
                let accepted = event_manager.borrow_mut().accept(event);
                if accepted {
                    if let Err(error) = toggle(handle, cx).await {
                        log::error!("全局热键切换失败：{error}");
                    }
                }
            }
        })
    });
    let click_task = cx.update(|cx| {
        cx.spawn(async move |cx| {
            while let Some(visibility::VisibilityEvent::ExternalMouseDown) =
                click_events.recv().await
            {
                if let Err(error) = hide(handle, cx).await {
                    log::error!("点击外部隐藏失败：{error}");
                }
            }
        })
    });
    cx.update(|cx| {
        cx.set_global(Runtime {
            window: handle,
            hotkey: manager,
            last_invoked: Instant::now(),
            _click_monitor: monitor,
            _hotkey_task: hotkey_task,
            _click_task: click_task,
        })
    });
    if let Err(error) = &registration {
        let settings = router.read_with(cx, |router, _| router.settings_view().clone());
        settings.update(cx, |view, cx| {
            view.preference_failed(true, error.clone(), cx)
        });
    }
    registration
}

/// 当前进程的主窗口；供原生行为自检复用同一 Router。
pub fn main_window(cx: &App) -> Option<WindowHandle<AppShell>> {
    cx.try_global::<Runtime>().map(|runtime| runtime.window)
}

/// 实际有效热键，不把配置写盘当作系统注册成功。
pub fn registered_hotkey(cx: &App) -> Option<String> {
    cx.try_global::<Runtime>()
        .and_then(|runtime| runtime.hotkey.borrow().current())
        .map(|key| key.to_string())
}

/// v1 三态 toggle：可见且聚焦时隐藏，其余情况唤回前台。
pub async fn toggle(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Result<(), String> {
    let prepared = prepare(handle, cx)?;
    let snapshot = prepared.probe().map_err(|e| e.to_string())?;
    drop(prepared);
    if snapshot.is_visible && snapshot.is_key {
        hide(handle, cx).await
    } else {
        // CGEventPost enqueues Cmd+C; activating now can deliver it to Buddy instead of
        // the external app. Finish the bounded capture (and restore clipboard) first.
        let selected = if !snapshot.is_visible {
            let capture = selection::begin_before_show(cx);
            selection::finish_after_copy(cx, capture).await
        } else {
            None
        };
        let idle = cx.update(|cx| {
            if !cx.has_global::<Runtime>() {
                return false;
            }
            let runtime = cx.global_mut::<Runtime>();
            let now = Instant::now();
            let idle = requires_compact(now.duration_since(runtime.last_invoked));
            runtime.last_invoked = now;
            idle
        });
        if idle {
            handle
                .update(cx, |shell, _, cx| {
                    shell
                        .router()
                        .update(cx, |router, cx| router.invoked_after_idle(cx))
                })
                .map_err(|e| e.to_string())?;
        }
        let shown = show(handle, cx).await;
        if shown.is_ok()
            && let Some(text) = selected
        {
            let _ = handle.update(cx, |shell, _, cx| {
                shell
                    .router()
                    .update(cx, |router, cx| router.accept_selected_text(&text, cx))
            });
        }
        shown
    }
}

/// 原生隐藏，同一 Router / engine 任务仍存活；失败时恢复前台缓冲模式。
pub async fn hide(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Result<(), String> {
    let prepared = prepare(handle, cx)?;
    if !prepared.probe().map_err(|e| e.to_string())?.is_visible {
        return Ok(());
    }
    handle
        .update(cx, |shell, _, cx| {
            shell
                .router()
                .update(cx, |router, cx| router.prepare_window_hide(cx))
        })
        .map_err(|e| e.to_string())?;
    let result = prepared.hide().map_err(|e| e.to_string());
    if result.is_err() {
        let _ = handle.update(cx, |shell, _, cx| {
            shell
                .router()
                .update(cx, |router, cx| router.window_visibility_changed(true, cx))
        });
    }
    result.map(|_| ())
}

/// 唤回当前主窗口；原生调用在 GPUI App 借用已结束后同步执行。
pub async fn show(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Result<(), String> {
    let prepared = prepare(handle, cx)?;
    prepared.show_and_focus().map_err(|e| e.to_string())?;
    handle
        .update(cx, |shell, window, cx| {
            shell
                .router()
                .update(cx, |router, cx| router.window_visibility_changed(true, cx));
            window.refresh();
        })
        .map_err(|e| e.to_string())
}

fn prepare(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Result<visibility::PreparedVisibility, String> {
    cx.update_window(handle.into(), |_, window, _| visibility::prepare(window))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Esc 已由子层优先处理；剩余事件只请求异步原生隐藏。
pub(crate) fn request_hide(handle: WindowHandle<AppShell>, cx: &mut App) {
    cx.spawn(async move |cx| {
        if let Err(error) = hide(handle, cx).await {
            log::error!("隐藏主窗口失败：{error}");
        }
    })
    .detach();
}

fn requires_compact(idle: Duration) -> bool {
    idle >= Duration::from_secs(600)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_only_after_ten_minutes() {
        assert!(!requires_compact(Duration::from_secs(599)));
        assert!(requires_compact(Duration::from_secs(600)));
        assert!(requires_compact(Duration::from_secs(601)));
    }
}
