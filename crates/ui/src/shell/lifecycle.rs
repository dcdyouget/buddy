//! 单实例与原生应用生命周期，先占有实例锁再初始化 engine / 窗口。
#[cfg(unix)]
pub mod instance;
#[cfg(target_os = "macos")]
mod termination;
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
mod macos {
    use super::instance::{InstanceGuard, Wake};
    use crate::shell::{AppShell, runtime};
    use gpui::{AsyncApp, Global, Subscription, Task, WindowHandle};
    use tokio::sync::mpsc::UnboundedReceiver;

    struct Lifecycle {
        guard: Option<InstanceGuard>,
        _requests: Task<()>,
        _wake: Subscription,
        _quit: Subscription,
        _terminate: Task<()>,
    }
    impl Global for Lifecycle {}

    /// macOS 用户专属临时目录下固定端点，产品与开发入口共用同一数据 owner。
    pub fn socket_path() -> std::path::PathBuf {
        std::env::temp_dir().join("buddy-v2").join("owner.sock")
    }

    /// 保持 owner guard；退出时先等待配置完成，再移除 socket 和释放系统注册。
    pub fn install(
        handle: WindowHandle<AppShell>,
        guard: InstanceGuard,
        mut requests: UnboundedReceiver<Wake>,
        cx: &mut AsyncApp,
    ) -> Result<(), String> {
        if cx.update(|cx| cx.has_global::<Lifecycle>()) {
            return Err("生命周期事件桥已经安装".into());
        }
        let mut termination_requests = super::termination::install().map_err(|e| e.to_string())?;
        let terminate = cx.update(|cx| {
            cx.spawn(async move |cx| {
                while termination_requests.recv().await.is_some() {
                    // Cancel then retry leaves the normal main loop running so queued
                    // settings transactions can finish, including >200ms saves.
                    loop {
                        let pending = handle
                            .update(cx, |shell, _, cx| {
                                shell
                                    .router()
                                    .update(cx, |router, _| router.take_config_save())
                            })
                            .ok()
                            .flatten();
                        match pending {
                            Some(pending) => pending.await,
                            None => break,
                        }
                    }
                    // AppKit synchronously invokes on_app_quit; no App borrow here.
                    if let Err(error) = super::termination::reply_to_application_should_terminate()
                    {
                        log::error!("回复原生退出失败：{error}");
                    }
                }
            })
        });
        let task = cx.update(|cx| {
            cx.spawn(async move |cx| {
                while requests.recv().await.is_some() {
                    if let Err(error) = runtime::show(handle, cx).await {
                        log::error!("已有实例唤回失败：{error}");
                    }
                }
            })
        });
        let wake = cx.update(|cx| {
            cx.on_system_wake(move |cx| {
                cx.spawn(async move |cx| {
                    if let Err(error) = runtime::resume_after_wake(handle, cx).await {
                        log::error!("休眠唤醒后恢复外壳失败：{error}");
                    }
                })
                .detach();
            })
        });
        let quit = cx.update(|cx| {
            cx.on_app_quit(move |cx| {
                log::warn!("[退出诊断] 应用正在退出（on_app_quit）");
                // GPUI polls quit futures while holding App's RefCell borrow.
                // Native resources must be released synchronously, never via AsyncApp.
                crate::shell::services::shutdown(cx);
                if let Err(error) = runtime::shutdown(cx) {
                    log::error!("退出时注销热键失败：{error}");
                }
                let guard = cx.global_mut::<Lifecycle>().guard.take();
                if let Some(guard) = guard {
                    guard.prepare_process_exit();
                }
                std::future::ready(())
            })
        });
        cx.update(|cx| {
            cx.set_global(Lifecycle {
                guard: Some(guard),
                _requests: task,
                _wake: wake,
                _quit: quit,
                _terminate: terminate,
            })
        });
        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub use macos::{install, socket_path};
