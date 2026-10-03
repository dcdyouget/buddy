use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

use buddy_engine::{models::AppConfig, storage};
use buddy_ui::chat::page_state::Page;
use buddy_ui::gpui::{App, AppContext, AsyncApp, WindowHandle};
use buddy_ui::gpui_platform::application;
use buddy_ui::shell::autostart::{AutostartBackend, SystemAutostart};
use buddy_ui::shell::{self, AppShell};

use super::protocol::{
    Fixture, PHASE_TIMEOUT, POLL, STATE_TIMEOUT, clear_marker, fixture_from_args, marker,
};

async fn wait_for_marker(path: &std::path::Path, cx: &mut AsyncApp) -> Result<(), String> {
    let deadline = Instant::now() + PHASE_TIMEOUT;
    loop {
        if path.is_file() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!("等待 {} 超时", path.display()));
        }
        cx.background_executor().timer(POLL).await;
    }
}

#[derive(Clone, Copy)]
struct NativeState {
    visible: bool,
    key: bool,
    active: bool,
}

fn native_state(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<NativeState> {
    let native = cx
        .update_window(handle.into(), |_, window, _| {
            buddy_ui::shell::native::probe_main_window(window).ok()
        })
        .ok()
        .flatten()?;
    let workspace = cx
        .update_window(handle.into(), |_, window, _| {
            buddy_ui::shell::workspaces::probe(window).ok()
        })
        .ok()
        .flatten()?;
    Some(NativeState {
        visible: native.is_visible,
        key: native.is_key,
        active: workspace.app_is_active,
    })
}

async fn wait_visible_key_active(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Result<NativeState, String> {
    let deadline = Instant::now() + STATE_TIMEOUT;
    let mut latest = NativeState {
        visible: false,
        key: false,
        active: false,
    };
    loop {
        if let Some(state) = native_state(handle, cx) {
            latest = state;
            if state.visible && state.key && state.active {
                return Ok(state);
            }
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "窗口未在限定时间激活：visible={} key={} active={}",
                latest.visible, latest.key, latest.active
            ));
        }
        cx.background_executor().timer(POLL).await;
    }
}

async fn wait_settings_active(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let deadline = Instant::now() + STATE_TIMEOUT;
    loop {
        let active = handle
            .read_with(cx, |shell, app| {
                let router = shell.router();
                let page = router.read(app).page();
                let settings = router.read(app).settings_view().clone();
                page == Page::Settings && settings.read(app).active()
            })
            .map_err(|error| error.to_string())?;
        if active {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("Settings 菜单点击后未读到 active=true".into());
        }
        cx.background_executor().timer(POLL).await;
    }
}

async fn wait_autostart_state(
    service: &SystemAutostart,
    fixture: &Fixture,
    desired: bool,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let plist = fixture.plist()?;
    let deadline = Instant::now() + STATE_TIMEOUT;
    loop {
        let entry = service.query()?;
        let disk = storage::get_config(&fixture.data)?.auto_start;
        if entry == desired && disk == desired && plist.exists() == desired {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "自启读回异常：desired={} launch_agent_entry={} disk={} plist_exists={}",
                desired,
                entry,
                disk,
                plist.exists()
            ));
        }
        cx.background_executor().timer(POLL).await;
    }
}

fn run_child(fixture: Fixture) -> Result<(), String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("读取 child 可执行文件失败：{error}"))?;
    let service = Rc::new(SystemAutostart::with_identity(
        &fixture.app_name,
        &executable,
    )?);
    if service.query()? || fixture.plist()?.exists() {
        return Err("child 检测到专用 LaunchAgent entry 初始状态非空".into());
    }
    let mut config = AppConfig::default();
    config.hotkey = "CmdOrCtrl+Alt+Shift+F19".into();
    storage::save_config(&fixture.data, &config)?;

    let failed = Arc::new(AtomicBool::new(false));
    let failed_for_app = failed.clone();
    application()
        .with_assets(buddy_ui::icons::Assets)
        .run(move |cx: &mut App| {
            shell::init(cx);
            let fixture = fixture;
            let service = service;
            let failed = failed_for_app.clone();
            cx.spawn(async move |cx: &mut AsyncApp| {
                let result = async {
                    let handle = shell::open_main_window(
                        buddy_engine::chat::ChatEngine::new(fixture.data.clone()),
                        shell::config::ShellConfig::default(),
                        cx,
                    )
                    .await
                    .map_err(|error| format!("创建真实主窗口失败：{error}"))?;
                    shell::runtime::install(handle, cx)
                        .await
                        .map_err(|error| format!("安装 runtime 失败：{error}"))?;
                    shell::runtime::hide(handle, cx)
                        .await
                        .map_err(|error| format!("隐藏初始主窗口失败：{error}"))?;
                    let backend: Rc<dyn AutostartBackend> = service.clone();
                    shell::services::install_with_backend(Ok(backend), cx)
                        .map_err(|error| format!("安装真实 tray services 失败：{error}"))?;
                    clear_marker(&fixture.marker("ready.continue"))?;
                    marker(&fixture.marker("ready"), "READY\n")?;
                    println!("[S07-09/13 child] READY：真实 tray 已安装，主窗口 hidden");

                    clear_marker(&fixture.marker("show.continue"))?;
                    marker(&fixture.marker("show.ready"), "SHOW_READY\n")?;
                    wait_for_marker(&fixture.marker("show.continue"), cx).await?;
                    let state = wait_visible_key_active(handle, cx).await?;
                    marker(
                        &fixture.marker("show.ack"),
                        format!(
                            "visible={} key={} active={}\n",
                            state.visible, state.key, state.active
                        ),
                    )?;
                    println!(
                        "[S07-09/13 child] SHOW_ACK visible={} key={} active={}",
                        state.visible, state.key, state.active
                    );
                    shell::runtime::hide(handle, cx)
                        .await
                        .map_err(|error| format!("准备 Settings 阶段隐藏失败：{error}"))?;

                    clear_marker(&fixture.marker("settings.continue"))?;
                    marker(&fixture.marker("settings.ready"), "SETTINGS_READY\n")?;
                    wait_for_marker(&fixture.marker("settings.continue"), cx).await?;
                    wait_settings_active(handle, cx).await?;
                    marker(
                        &fixture.marker("settings.ack"),
                        "page=settings active=true\n",
                    )?;
                    println!("[S07-09/13 child] SETTINGS_ACK page=settings active=true");

                    clear_marker(&fixture.marker("autostart-enable.continue"))?;
                    marker(
                        &fixture.marker("autostart-enable.ready"),
                        "AUTOSTART_ENABLE_READY\n",
                    )?;
                    wait_for_marker(&fixture.marker("autostart-enable.continue"), cx).await?;
                    wait_autostart_state(&service, &fixture, true, cx).await?;
                    marker(
                        &fixture.marker("autostart-enable.ack"),
                        "launch_agent_entry=true disk=true plist=true\n",
                    )?;
                    println!(
                        "[S07-09/13 child] AUTOSTART_ENABLE_ACK launch_agent_entry=true disk=true plist=true"
                    );

                    clear_marker(&fixture.marker("autostart-disable.continue"))?;
                    marker(
                        &fixture.marker("autostart-disable.ready"),
                        "AUTOSTART_DISABLE_READY\n",
                    )?;
                    wait_for_marker(&fixture.marker("autostart-disable.continue"), cx).await?;
                    wait_autostart_state(&service, &fixture, false, cx).await?;
                    marker(
                        &fixture.marker("autostart-disable.ack"),
                        "launch_agent_entry=false disk=false plist=false\n",
                    )?;
                    println!(
                        "[S07-09/13 child] AUTOSTART_DISABLE_ACK launch_agent_entry=false disk=false plist=false"
                    );

                    clear_marker(&fixture.marker("quit.continue"))?;
                    marker(&fixture.marker("quit.ready"), "QUIT_READY\n")?;
                    println!("[S07-09/13 child] QUIT_READY：实际点击‘退出’");
                    Ok::<(), String>(())
                }
                .await;
                if let Err(error) = result {
                    failed.store(true, Ordering::Release);
                    let _ = marker(&fixture.marker("fail"), error.as_bytes());
                    eprintln!("[S07-09/13 child] FAIL：{error}");
                    cx.update(|cx| cx.quit());
                }
            })
            .detach();
        });
    if failed.load(Ordering::Acquire) {
        Err("真实 tray child 阶段失败，详见 fixture/fail".into())
    } else {
        Ok(())
    }
}

pub(crate) fn run(args: &[String]) -> Result<(), String> {
    run_child(fixture_from_args(args)?)
}
