use buddy_engine::chat::ChatEngine;
use buddy_ui::gpui::{App, AsyncApp};
use buddy_ui::gpui_platform::application;
use buddy_ui::shell::{
    self,
    config::ShellConfig,
    lifecycle::instance::{Acquire, InstanceError, acquire},
};
#[path = "os_input.rs"]
mod os_input;
mod protocol;
#[path = "state.rs"]
mod state;
use protocol::{Fixture, OwnedChild, wait_marker_blocking};
use state::{registered_hotkey, router_id, wait_active_same_window, wait_marker, window_state};
use std::fs;
use std::path::PathBuf;
const HOTKEY: &str = "CmdOrCtrl+Alt+Shift+F19";

pub fn run() -> Result<(), String> {
    match os_input::preflight() {
        os_input::Preflight::Ready => {}
        os_input::Preflight::SessionLocked => {
            return Err("BLOCKED：macOS 会话已锁定，未启动生命周期 GUI 探针".into());
        }
        os_input::Preflight::EventAccessDenied => {
            return Err("BLOCKED：未获得 CGEventPost 权限，未启动生命周期 GUI 探针".into());
        }
    }
    let fixture = Fixture::create()?;
    let mut child = OwnedChild::spawn_owner(&fixture)?;
    child.wait_ready(&fixture.ready)?;
    println!("[S07-11] owner READY：生命周期已安装，窗口已隐藏");
    let mut secondary = OwnedChild::spawn_secondary(&fixture.socket, &fixture.forwarded)?;
    secondary.wait_success()?;
    if !fixture.forwarded.exists() {
        return Err("secondary 退出但没有 Forwarded 标记".into());
    }
    wait_marker_blocking(&mut child, &fixture.ipc_ack, "IPC 唤回 ACK")?;
    println!("[S07-11] secondary Forwarded -> owner 同一窗口/Router visible/key/active PASS");
    fs::write(&fixture.resume, "DIRECT_RESUME_NOT_SYSTEM_SLEEP\n")
        .map_err(|error| format!("写直接唤醒测试标记失败：{error}"))?;
    wait_marker_blocking(&mut child, &fixture.resume_ready, "直接唤醒恢复")?;
    println!("[S07-11] hide -> resume_after_wake 通过（仅模拟恢复，不等同实际睡眠）");
    match os_input::preflight() {
        os_input::Preflight::Ready => {}
        os_input::Preflight::SessionLocked => {
            return Err("BLOCKED：macOS 会话在发送 F19 前锁定，未计入产品断言".into());
        }
        os_input::Preflight::EventAccessDenied => {
            return Err("BLOCKED：发送 F19 前 CGEventPost 权限不可用，未计入产品断言".into());
        }
    }
    let mut sender = OwnedChild::spawn_f19_sender()?;
    if let Err(error) = sender.wait_success() {
        match os_input::preflight() {
            os_input::Preflight::SessionLocked => {
                return Err("BLOCKED：F19 sender 执行期间 macOS 会话已锁定，未计入产品断言".into());
            }
            os_input::Preflight::EventAccessDenied => {
                return Err(
                    "BLOCKED：F19 sender 执行期间 CGEventPost 权限不可用，未计入产品断言".into(),
                );
            }
            os_input::Preflight::Ready => return Err(error),
        }
    }
    match os_input::preflight() {
        os_input::Preflight::Ready => {}
        os_input::Preflight::SessionLocked => {
            return Err("BLOCKED：F19 sender 执行期间 macOS 会话已锁定，未计入产品断言".into());
        }
        os_input::Preflight::EventAccessDenied => {
            return Err(
                "BLOCKED：F19 sender 执行期间 CGEventPost 权限不可用，未计入产品断言".into(),
            );
        }
    }
    wait_marker_blocking(&mut child, &fixture.hotkey_ack, "独立 F19 唤回")?;
    println!("[S07-11] 独立进程 Cmd+Alt+Shift+F19 -> 同一窗口/Router visible/key PASS");
    fs::write(&fixture.quit, "QUIT\n")
        .map_err(|error| format!("写 owner 退出标记失败：{error}"))?;
    child.wait_success()?;
    if fixture.socket.exists() {
        return Err("真实 on_app_quit 后 socket 未清理".into());
    }
    if !fixture.lock().exists() {
        return Err("真实 on_app_quit 后 lock 文件被删除".into());
    }
    match acquire(&fixture.socket) {
        Ok(Acquire::Owner { guard, receiver }) => {
            drop(receiver);
            drop(guard);
        }
        Ok(Acquire::Forwarded) => {
            return Err("真实 child 退出后仍收到 Forwarded".into());
        }
        Err(error) => {
            return Err(format!("真实 child 退出后重新 acquire 失败：{error}"));
        }
    }
    let config = fs::read_to_string(fixture.config_path())
        .map_err(|error| format!("读取 child 最终配置失败：{error}"))?;
    let config = serde_json::from_str::<serde_json::Value>(&config)
        .map_err(|error| format!("解析 child 最终配置失败：{error}"))?;
    if config.get("theme").and_then(serde_json::Value::as_str) != Some("dark") {
        return Err(format!(
            "native quit 后主题未落盘：实际 {:?}，期望 dark",
            config.get("theme")
        ));
    }
    println!(
        "[S07-11] PASS：二进程 IPC、直接 wake rearm、独立 F19 唤回、真实 GPUI cx.quit 与主题落盘均通过"
    );
    Ok(())
}

pub fn run_child(args: &[String]) -> Result<(), String> {
    if args.first().map(String::as_str) == Some("--send-f19") {
        return os_input::send_f19();
    }
    if args.first().map(String::as_str) == Some("--secondary") {
        return secondary_child(args);
    }
    let socket = required_path(args, 1, "socket")?;
    let data_dir = required_path(args, 2, "data")?;
    let ready = required_path(args, 3, "ready")?;
    let ipc_ack = required_path(args, 4, "ipc_ack")?;
    let resume = required_path(args, 5, "resume")?;
    let resume_ready = required_path(args, 6, "resume_ready")?;
    let hotkey_ack = required_path(args, 7, "hotkey_ack")?;
    let quit = required_path(args, 8, "quit")?;
    let acquired = match acquire(&socket).map_err(render_error)? {
        Acquire::Owner { guard, receiver } => (guard, receiver),
        Acquire::Forwarded => return Err("应用 child 意外成为 secondary".into()),
    };
    let (guard, receiver) = acquired;
    let child_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        application()
                .with_assets(buddy_ui::icons::Assets)
                .run(move |cx: &mut App| {
                    shell::init(cx);
                    cx.spawn(async move |cx: &mut AsyncApp| {
                        let engine = ChatEngine::new(data_dir);
                        let handle =
                            match shell::open_main_window(engine, ShellConfig::default(), cx).await
                            {
                                Ok(handle) => handle,
                                Err(error) => fail_child(format!("创建专用主窗口失败：{error}")),
                            };
                        if let Err(error) = shell::runtime::install(handle, cx).await {
                            fail_child(format!("安装专用 F19 runtime 失败：{error}"));
                        }
                        if let Err(error) = shell::runtime::hide(handle, cx).await {
                            fail_child(format!("隐藏专用主窗口失败：{error}"));
                        }
                        if let Err(error) = shell::lifecycle::install(handle, guard, receiver, cx) {
                            fail_child(format!("安装真实生命周期 hook 失败：{error}"));
                        }
                        let initial_router = router_id(handle, cx)
                            .unwrap_or_else(|| fail_child("读取初始 Router 失败".into()));
                        let initial_window = window_state(handle, cx)
                            .unwrap_or_else(|| fail_child("读取初始原生窗口失败".into()));
                        if initial_window.visible {
                            fail_child("READY 前主窗口未隐藏".into());
                        }
                        // `HotKey::to_string()` uses its canonical display form, which may not
                        // equal the config spelling `CmdOrCtrl+Alt+Shift+F19`. Keep the actual
                        // registered value as the rearm baseline and still require F19.
                        let initial_hotkey = registered_hotkey(cx)
                            .filter(|hotkey| hotkey.to_ascii_uppercase().contains("F19"))
                            .unwrap_or_else(|| fail_child("初始实际注册热键不是 F19".into()));
                        fs::write(
                            &ready,
                            format!(
                                "READY pid={} hotkey={HOTKEY} actual={initial_hotkey} window={} router={initial_router:?} hidden=true\n",
                                std::process::id(),
                                initial_window.window_number,
                            ),
                        )
                        .unwrap_or_else(|error| {
                            fail_child(format!("写应用 child READY 失败：{error}"))
                        });
                        if wait_active_same_window(handle, initial_router, initial_window.window_number, cx)
                            .await
                            .is_some()
                        {
                            // The IPC receiver installed by lifecycle::install owns this show path.
                            fs::write(&ipc_ack, "IPC_ACK\n")
                                .unwrap_or_else(|error| fail_child(format!("写 IPC ACK 失败：{error}")));
                        } else {
                            fail_child("IPC 唤回未达到 visible/key/appactive 或替换了窗口/Router".into());
                        }
                        if !wait_marker(&resume, cx).await {
                            fail_child("等待直接 wake 标记超时".into());
                        }
                        if let Err(error) = shell::runtime::hide(handle, cx).await {
                            fail_child(format!("直接 wake 前隐藏窗口失败：{error}"));
                        }
                        if let Err(error) = shell::runtime::resume_after_wake(handle, cx).await {
                            fail_child(format!("直接 resume_after_wake 失败：{error}"));
                        }
                        let after_resume = window_state(handle, cx)
                            .unwrap_or_else(|| fail_child("直接 wake 后无法读取原生窗口".into()));
                        let rearmed = registered_hotkey(cx)
                            .is_some_and(|hotkey| hotkey == initial_hotkey);
                        if after_resume.visible
                            || after_resume.window_number != initial_window.window_number
                            || router_id(handle, cx) != Some(initial_router)
                            || !rearmed
                        {
                            fail_child(format!(
                                "直接 wake 断言失败：visible={} same_window={} same_router={} rearmed={} state={after_resume:?}",
                                after_resume.visible,
                                after_resume.window_number == initial_window.window_number,
                                router_id(handle, cx) == Some(initial_router),
                                rearmed,
                            ));
                        }
                        fs::write(&resume_ready, "DIRECT_RESUME_READY\n")
                            .unwrap_or_else(|error| fail_child(format!("写直接 wake ACK 失败：{error}")));
                        if !wait_active_same_window(handle, initial_router, initial_window.window_number, cx)
                            .await
                            .is_some()
                        {
                            fail_child("独立 F19 后未达到同一窗口/Router visible/key/appactive".into());
                        }
                        let rearmed_hotkey = registered_hotkey(cx)
                            .is_some_and(|hotkey| hotkey == initial_hotkey);
                        if !rearmed_hotkey {
                            fail_child("独立 F19 唤回后实际注册热键不是 F19".into());
                        }
                        fs::write(&hotkey_ack, "F19_ACK\n")
                            .unwrap_or_else(|error| fail_child(format!("写 F19 ACK 失败：{error}")));
                        if !wait_marker(&quit, cx).await {
                            fail_child("等待 owner 退出标记超时".into());
                        }
                        // Queue the real SettingsEvent immediately before cx.quit so the
                        // termination hook must drain the pending save instead of saving long
                        // before the 200ms native termination window.
                        if let Err(error) = handle.update(cx, |shell, _, cx| {
                            shell.router().update(cx, |router, cx| {
                                router.open_settings(cx);
                                let settings = router.settings_view().clone();
                                settings.update(cx, |_, cx| {
                                    cx.emit(buddy_ui::settings::SettingsEvent::ThemeChanged(
                                        buddy_engine::models::Theme::Dark,
                                    ));
                                });
                            });
                        }) {
                            fail_child(format!("退出前排队主题配置失败：{error}"));
                        }
                        cx.update(|cx| cx.quit());
                    })
                    .detach();
                });
    }));
    if child_result.is_err() {
        return Err("应用 child GPUI 执行 panic".into());
    }
    Ok(())
}

fn fail_child(message: String) -> ! {
    eprintln!("[S07-11 app child] FAIL：{message}");
    std::process::exit(1);
}

fn required_path(args: &[String], index: usize, label: &str) -> Result<PathBuf, String> {
    args.get(index)
        .map(PathBuf::from)
        .ok_or_else(|| format!("缺少 child {label} 路径"))
}

fn render_error(error: InstanceError) -> String {
    error.to_string()
}

fn secondary_child(args: &[String]) -> Result<(), String> {
    let socket = required_path(args, 1, "socket")?;
    let forwarded = required_path(args, 2, "forwarded")?;
    match acquire(&socket).map_err(render_error)? {
        Acquire::Forwarded => {
            fs::write(forwarded, "FORWARDED\n")
                .map_err(|error| format!("写 Forwarded 标记失败：{error}"))?;
            Ok(())
        }
        Acquire::Owner { guard, receiver } => {
            drop(receiver);
            drop(guard);
            Err("secondary 越过 owner 锁成为新 owner".into())
        }
    }
}
