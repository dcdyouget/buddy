//! Buddy 产品入口：装配引擎、真实页面与统一主窗口外壳。
use buddy_engine::{chat::ChatEngine, storage};
use buddy_ui::gpui::{App, AsyncApp};
use buddy_ui::gpui_platform::application;
use buddy_ui::shell::{self, config::ShellConfig};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    install_exit_trace();
    if std::env::args().any(|argument| argument == "--selfcheck-window") {
        // 诊断隔离于真实用户数据和唯一产品实例；内部关闭临时窗口并清理后返回。
        application()
            .with_assets(buddy_ui::icons::Assets)
            .run(|cx: &mut App| {
                shell::init(cx);
                cx.spawn(async move |cx: &mut AsyncApp| {
                    let passed = shell::selfcheck::run(cx).await;
                    std::process::exit(if passed { 0 } else { 1 });
                })
                .detach();
            });
        return;
    }
    #[cfg(target_os = "macos")]
    let instance = {
        use shell::lifecycle::instance::{Acquire, acquire};
        match acquire(shell::lifecycle::socket_path()) {
            Ok(Acquire::Owner { guard, receiver }) => (guard, receiver),
            Ok(Acquire::Forwarded) => {
                println!("已向运行中的 Buddy 发送唤起请求");
                return;
            }
            Err(error) => {
                log::error!("单实例启动失败：{error}");
                std::process::exit(1);
            }
        }
    };
    // 上次自更新留下的 `.Buddy.app.update-old` 与下载缓存；只涉及文件删除，放到后台线程
    std::thread::spawn(buddy_update::cleanup_after_launch);
    // 与 v1 相同的应用数据目录；不进行历史迁移。
    let data_dir = match storage::default_data_dir() {
        Ok(path) => path,
        Err(error) => {
            log::error!("无法定位应用数据目录：{error}");
            #[cfg(target_os = "macos")]
            drop(instance);
            std::process::exit(1);
        }
    };
    application()
        .with_assets(buddy_ui::icons::Assets)
        .run(move |cx: &mut App| {
            shell::init(cx);
            cx.spawn(async move |cx: &mut AsyncApp| {
                match shell::open_main_window(ChatEngine::new(data_dir), ShellConfig::default(), cx)
                    .await
                {
                    Ok(handle) => {
                        if let Err(error) = shell::runtime::install(handle, cx).await {
                            log::error!("安装主窗口事件失败：{error}");
                        }
                        if let Err(error) = shell::services::install(cx) {
                            log::error!("安装系统托盘失败：{error}");
                        }
                        #[cfg(target_os = "macos")]
                        if let Err(error) =
                            shell::lifecycle::install(handle, instance.0, instance.1, cx)
                        {
                            log::error!("安装生命周期事件失败：{error}");
                            cx.update(|cx| {
                                shell::services::shutdown(cx);
                                if let Err(error) = shell::runtime::shutdown(cx) {
                                    log::error!("清理启动失败的运行时：{error}");
                                }
                                cx.quit();
                            });
                        }
                    }
                    Err(error) => {
                        log::error!("创建主窗口失败：{error}");
                        #[cfg(target_os = "macos")]
                        drop(instance);
                        cx.update(|cx| cx.quit());
                    }
                }
            })
            .detach();
        });
}

/// 进程以任何方式调用 `exit()` 时打印调用栈（排查退出码为 0 的不明退出）。
fn install_exit_trace() {
    unsafe extern "C" {
        fn atexit(callback: extern "C" fn()) -> std::ffi::c_int;
    }
    extern "C" fn trace() {
        eprintln!(
            "[退出诊断] 进程 exit()，调用栈：\n{}",
            std::backtrace::Backtrace::force_capture()
        );
    }
    unsafe {
        atexit(trace);
    }
}
