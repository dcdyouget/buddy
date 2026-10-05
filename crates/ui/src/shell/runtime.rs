//! 进程唯一的全局热键 / 外部点击运行时；隐藏仅操作原生窗口。

use super::{AppShell, hotkey::GlobalHotkey, positioning_controller, selection, visibility};
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
            view.preference_failed(crate::settings::PreferenceKind::Hotkey, error.clone(), cx)
        });
    }
    registration
}

/// 当前进程的主窗口；供原生行为自检复用同一 Router。
pub fn main_window(cx: &App) -> Option<WindowHandle<AppShell>> {
    cx.try_global::<Runtime>().map(|runtime| runtime.window)
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
        let idle = record_invocation(cx);
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
    let visible = prepared.probe().map_err(|e| e.to_string())?.is_visible;
    drop(prepared);
    if !visible {
        handle
            .update(cx, |shell, _, cx| shell.reset_entrance(cx))
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    handle
        .update(cx, |shell, window, cx| {
            positioning_controller::save_before_hide(shell, window);
            shell
                .router()
                .update(cx, |router, cx| router.prepare_window_hide(cx))
        })
        .map_err(|e| e.to_string())?;
    let Some((generation, animate)) = handle
        .update(cx, |shell, _, cx| shell.begin_exit(cx))
        .map_err(|e| e.to_string())?
    else {
        // Another hide is already animating; let its generation own the native hide.
        return Ok(());
    };
    if animate {
        prepare(handle, cx)?.dismiss().map_err(|e| e.to_string())?;
        cx.background_executor()
            .timer(super::entrance::EntranceMotion::exit_duration())
            .await;
    }
    let still_current = handle
        .update(cx, |shell, _, _| {
            shell.is_current_visibility_generation(generation)
        })
        .map_err(|e| e.to_string())?;
    if !still_current {
        return Ok(());
    }
    let prepared = prepare(handle, cx)?;
    if !prepared.probe().map_err(|e| e.to_string())?.is_visible {
        let _ = prepared.settle();
        handle
            .update(cx, |shell, _, cx| shell.reset_entrance(cx))
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    let result = prepared.hide().map_err(|e| e.to_string());
    // 透明终帧已随 orderOut 收起；清掉动画，下次呼入从干净的图层开始。
    let _ = prepared.settle();
    if result.is_ok() {
        let _ = handle.update(cx, |shell, _, cx| {
            if shell.is_current_visibility_generation(generation) {
                shell.router().update(cx, |router, cx| router.window_visibility_changed(false, cx));
                shell.reset_entrance(cx);
            }
        });
    } else {
        let _ = handle.update(cx, |shell, _, cx| {
            if shell.is_current_visibility_generation(generation) {
                shell
                    .router()
                    .update(cx, |router, cx| router.window_visibility_changed(true, cx));
                shell.reset_entrance(cx);
            }
        });
    }
    result.map(|_| ())
}

/// 唤回当前主窗口；原生调用在 GPUI App 借用已结束后同步执行。
pub async fn show(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Result<(), String> {
    show_on_screen(handle, true, cx).await
}

/// 托盘左键始终呼出；按鼠标所在屏定位，不捕获外部选区。
pub async fn show_from_tray(
    handle: WindowHandle<AppShell>,
    allow_idle_compact: bool,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let idle = record_invocation(cx);
    if allow_idle_compact && idle {
        handle
            .update(cx, |shell, _, cx| {
                shell
                    .router()
                    .update(cx, |router, cx| router.invoked_after_idle(cx))
            })
            .map_err(|error| error.to_string())?;
    }
    show_on_screen(handle, false, cx).await
}

async fn show_on_screen(
    handle: WindowHandle<AppShell>,
    focused_screen: bool,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let was_visible = prepare(handle, cx)?.probe().map_err(|e| e.to_string())?.is_visible;
    let (generation, resume_exit) = handle
        .update(cx, |shell, _, cx| shell.prepare_show(was_visible, cx))
        .map_err(|e| e.to_string())?;
    positioning_controller::restore(handle, focused_screen, cx).await?;
    if !handle.read_with(cx, |shell, _| shell.is_current_visibility_generation(generation)).map_err(|e| e.to_string())? {
        return Ok(());
    }
    let prepared = prepare(handle, cx)?;
    if !was_visible {
        prepared.set_alpha(0.0).map_err(|e| e.to_string())?;
    }
    handle
        .update(cx, |shell, window, cx| {
            shell
                .router()
                .update(cx, |router, cx| router.window_visibility_changed(true, cx));
            window.refresh();
        })
        .map_err(|e| e.to_string())?;
    if let Err(error) = prepared.show_and_focus() {
        let _ = prepared.set_alpha(1.0);
        return Err(error.to_string());
    }
    if !was_visible {
        // First callback requests a fresh frame; the second runs after that
        // frame has been presented. Never expose the old cached drawable.
        let (ready, rendered) = tokio::sync::oneshot::channel();
        handle.update(cx, |_, window, _| {
            window.on_next_frame(move |window, _| {
                window.refresh();
                window.on_next_frame(move |_, _| { let _ = ready.send(()); });
            });
        }).map_err(|e| e.to_string())?;
        rendered.await.map_err(|e| e.to_string())?;
    }
    if !handle.read_with(cx, |shell, _| shell.is_current_visibility_generation(generation)).map_err(|e| e.to_string())? {
        return Ok(());
    }
    let animate = handle
        .update(cx, |shell, _, cx| (!was_visible || resume_exit).then(|| shell.play_entrance(cx)))
        .map_err(|e| e.to_string())?;
    // 动画须在窗口变为可见之前提交，否则会先闪出一帧完整窗口。
    match animate {
        Some(true) => prepared.summon().map_err(|e| e.to_string())?,
        Some(false) => prepared.settle().map_err(|e| e.to_string())?,
        None => {}
    }
    prepared.set_alpha(1.0).map_err(|e| e.to_string())?;
    handle.update(cx, |_, window, _| window.refresh()).map_err(|e| e.to_string())
}

fn record_invocation(cx: &mut AsyncApp) -> bool {
    cx.update(|cx| {
        if !cx.has_global::<Runtime>() {
            return false;
        }
        let runtime = cx.global_mut::<Runtime>();
        let now = Instant::now();
        let idle = requires_compact(now.duration_since(runtime.last_invoked));
        runtime.last_invoked = now;
        idle
    })
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

/// 真正系统唤醒后重装注册并按当前屏幕裁剪位置；不改变原显隐状态或重播入场。
pub async fn resume_after_wake(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let registration = cx.update(|cx| {
        let runtime = cx
            .try_global::<Runtime>()
            .ok_or_else(|| "主窗口运行时尚未安装".to_string())?;
        runtime
            .hotkey
            .borrow_mut()
            .rearm()
            .map_err(|e| e.to_string())
    });
    if let Err(error) = &registration {
        let _ = handle.update(cx, |shell, _, cx| {
            let settings = shell.router().read(cx).settings_view().clone();
            settings.update(cx, |view, cx| {
                view.preference_failed(crate::settings::PreferenceKind::Hotkey, error.clone(), cx)
            });
        });
    }
    let native = cx
        .update_window(handle.into(), |_, window, _| {
            super::positioning_native::prepare(window)
        })
        .map_err(|error| error.to_string())??;
    let snapshot = native.snapshot()?;
    let target = super::positioning::clamp_to_work_area(
        snapshot.rect.origin,
        snapshot.rect.size,
        snapshot.screen.work_area,
        super::positioning::WINDOW_MARGIN,
    );
    if target != snapshot.rect.origin {
        native.move_to(target)?;
    }
    registration
}

/// 退出前注销系统键并释放外部鼠标监听；不依赖进程退出自动回收。
/// 唯一事件桥不可再次安装，此入口仅用于终止产品运行时或独立诊断进程。
pub fn shutdown(cx: &mut App) -> Result<(), String> {
    if cx.has_global::<Runtime>() {
        let runtime = cx.remove_global::<Runtime>();
        let result = runtime
            .hotkey
            .borrow_mut()
            .unregister_all()
            .map_err(|error| error.to_string());
        drop(runtime);
        return result;
    }
    Ok(())
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
