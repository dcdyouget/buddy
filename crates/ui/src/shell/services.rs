//! 产品托盘与自启装配；预览显式安装，避免普通渲染回归留下系统托盘。
use super::{
    autostart::{AutostartBackend, SystemAutostart},
    runtime,
    tray::{MenuAction, TrayService},
};
use gpui::{App, AsyncApp, Global, KeyBinding, Menu, MenuItem, Task, WindowHandle};
use std::rc::Rc;

gpui::actions!(buddy_shell, [
    /// 退出 Buddy（应用菜单「退出 Buddy」/ ⌘Q）
    Quit
]);

struct Services {
    tray: TrayService,
    _events: Task<()>,
}
impl Global for Services {}

/// AppKit 退出前显式销毁托盘和事件任务。
pub fn shutdown(cx: &mut App) {
    if cx.has_global::<Services>() {
        drop(cx.remove_global::<Services>());
    }
}

/// 为已经安装的唯一主窗口创建托盘，保持原生对象直到进程退出。
pub fn install(cx: &mut AsyncApp) -> Result<(), String> {
    let backend =
        SystemAutostart::new().map(|service| Rc::new(service) as Rc<dyn AutostartBackend>);
    install_with_backend(backend, cx)
}

/// 探针可注入专用登录项后端，不修改正式 Buddy 的登录项。
pub fn install_with_backend(
    backend: Result<Rc<dyn AutostartBackend>, String>,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    if cx.update(|cx| cx.has_global::<Services>()) {
        return Err("托盘事件桥已经安装".into());
    }
    let handle = cx
        .update(|cx| runtime::main_window(cx))
        .ok_or_else(|| "托盘安装前必须创建唯一主窗口".to_string())?;
    let initial = backend
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|b| b.query());
    let (tray, mut events) = TrayService::new(initial)?;
    // v1（Tauri 2）在 macOS 自动提供含「退出」的应用菜单，⌘Q 由它的快捷键实现；GPUI 不建默认菜单，需显式设置。
    cx.update(|cx| {
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.on_action(move |_: &Quit, cx: &mut App| {
            log::warn!("[退出诊断] 触发 Quit 动作（⌘Q 或应用菜单「退出 Buddy」）");
            cx.spawn(async move |cx: &mut AsyncApp| quit_after_save(handle, cx).await)
                .detach();
        });
        cx.set_menus(vec![Menu::new("Buddy").items([MenuItem::action("退出 Buddy", Quit)])]);
    });
    let task = cx.update(|cx| {
        cx.spawn(async move |cx| {
            while let Some(action) = events.recv().await {
                let result = match action {
                    MenuAction::Show => runtime::show_from_tray(handle, true, cx).await,
                    MenuAction::Settings => {
                        match runtime::show_from_tray(handle, false, cx).await {
                            Ok(()) => handle
                                .update(cx, |shell, _, cx| {
                                    shell
                                        .router()
                                        .update(cx, |router, cx| router.open_settings(cx));
                                })
                                .map_err(|e| e.to_string()),
                            Err(error) => Err(error),
                        }
                    }
                    MenuAction::ToggleAutostart => {
                        let result = match &backend {
                            Ok(backend) => {
                                cx.update(|cx| {
                                    cx.global_mut::<Services>().tray.set_autostart_busy(true)
                                });
                                match handle.update(cx, |shell, _, cx| {
                                    shell.router().update(cx, |router, cx| {
                                        router.toggle_autostart(backend.clone(), cx)
                                    })
                                }) {
                                    Ok(work) => work.await.map(|_| ()),
                                    Err(error) => Err(error.to_string()),
                                }
                            }
                            Err(error) => Err(error.clone()),
                        };
                        // 成功和失败都重新查询；包括写盘失败后的真实回滚状态。
                        let actual = backend
                            .as_ref()
                            .map_err(Clone::clone)
                            .and_then(|b| b.query());
                        cx.update(|cx| {
                            let service = cx.global_mut::<Services>();
                            service.tray.set_autostart(actual);
                            service.tray.set_autostart_busy(false);
                        });
                        result
                    }
                    MenuAction::Quit => {
                        log::warn!("[退出诊断] 托盘菜单「退出」");
                        quit_after_save(handle, cx).await;
                        return;
                    }
                };
                if let Err(error) = result {
                    log::error!("托盘操作失败：{error}");
                }
            }
        })
    });
    cx.update(|cx| {
        cx.set_global(Services {
            tray,
            _events: task,
        })
    });
    Ok(())
}

/// 托盘「退出」与 ⌘Q 共用：等进行中的配置保存写盘后再退出。
async fn quit_after_save(handle: WindowHandle<super::AppShell>, cx: &mut AsyncApp) {
    if let Ok(Some(work)) = handle.update(cx, |shell, _, cx| {
        shell
            .router()
            .update(cx, |router, _| router.take_config_save())
    }) {
        work.await;
    }
    cx.update(|cx| cx.quit());
}
