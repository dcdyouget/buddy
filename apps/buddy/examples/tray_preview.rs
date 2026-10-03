//! S07-09 系统托盘预览与服务自检。
//!
//! ```text
//! cargo run -p buddy-app --example tray_preview
//! cargo run -p buddy-app --example tray_preview -- --selftest
//! ```
//!
//! 预览模式创建真实系统托盘，但不改写系统开机自启；菜单动作只打印并由
//! GPUI 侧接收。自检复用真实 `TrayService` 和 `CheckMenuItem` getter，覆盖
//! 勾选、忙碌禁用、查询失败禁用以及成功恢复。

use buddy_ui::gpui::{App, AppContext, AsyncApp};
use buddy_ui::gpui_platform::application;
use buddy_ui::shell::tray::{MenuAction, TrayService};

fn run_selftest(service: &mut TrayService) -> bool {
    let initial = !service.autostart_checked() && service.autostart_menu_enabled();
    println!(
        "T09-01: 初始自启 checked={} enabled={} => {}",
        service.autostart_checked(),
        service.autostart_menu_enabled(),
        initial
    );

    service.set_autostart(Ok(true));
    let set_true = service.autostart_checked() && service.autostart_menu_enabled();
    println!(
        "T09-02: 设置 true checked={} enabled={} => {}",
        service.autostart_checked(),
        service.autostart_menu_enabled(),
        set_true
    );

    service.set_autostart_busy(true);
    let busy = service.autostart_checked() && !service.autostart_menu_enabled();
    println!(
        "T09-03: busy checked={} enabled={} => {}",
        service.autostart_checked(),
        service.autostart_menu_enabled(),
        busy
    );

    service.set_autostart_busy(false);
    let resumed = service.autostart_checked() && service.autostart_menu_enabled();
    println!(
        "T09-04: busy 解除 checked={} enabled={} => {}",
        service.autostart_checked(),
        service.autostart_menu_enabled(),
        resumed
    );

    service.set_autostart(Err("预览故意模拟系统查询失败".into()));
    let query_error = !service.autostart_checked() && !service.autostart_menu_enabled();
    println!(
        "T09-05: queryErr checked={} enabled={} => {}",
        service.autostart_checked(),
        service.autostart_menu_enabled(),
        query_error
    );

    service.set_autostart(Ok(false));
    let recovered = !service.autostart_checked() && service.autostart_menu_enabled();
    println!(
        "T09-06: 恢复 Ok(false) checked={} enabled={} => {}",
        service.autostart_checked(),
        service.autostart_menu_enabled(),
        recovered
    );

    let passed = initial && set_true && busy && resumed && query_error && recovered;
    println!(
        "{} S07-09 TrayService 自启菜单状态",
        if passed { "PASS" } else { "FAIL" }
    );
    passed
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let self_test = std::env::args().any(|arg| arg == "--selftest");

    application().run(move |cx: &mut App| {
        // 预览固定从“系统未启用”开始，不调用 enable/disable，不污染真实自启配置。
        let (mut service, mut actions) = match TrayService::new(Ok(false)) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("FAIL S07-09：创建托盘失败：{error}");
                std::process::exit(1);
            }
        };

        if self_test {
            let passed = run_selftest(&mut service);
            // AppKit terminate: 固定以 0 退出，不会返回 Rust main。
            // 先同步析构探针拥有的原生托盘，再按断言结果退出；不留下状态栏图标。
            drop(service);
            drop(actions);
            std::process::exit(if passed { 0 } else { 1 });
        }

        let tray = cx.new(|_| service);
        cx.spawn(async move |cx: &mut AsyncApp| {
            // 捕获 Entity 只为保持 TrayIcon 的主线程生命周期；不把原生对象移入任务。
            let _tray = tray;
            while let Some(action) = actions.recv().await {
                println!("托盘动作：{action:?}");
                if action == MenuAction::Quit {
                    cx.update(|cx| cx.quit());
                    break;
                }
            }
        })
        .detach();
    });
}
