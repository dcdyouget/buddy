//! S07-11 系统唤醒恢复独立自测。
//!
//! 只模拟调用 `resume_after_wake`，不发送系统输入、不激活窗口、不触碰正式
//! Buddy 配置或托盘；热键使用进程专属 F20 组合。
//!
//! ```text
//! cargo run -p buddy-app --example wake_preview -- --selftest
//! ```

use buddy_engine::chat::ChatEngine;
use buddy_engine::models::AppConfig;
use buddy_engine::storage;
use buddy_ui::gpui::{App, AppContext, AsyncApp, WindowHandle};
use buddy_ui::gpui_platform::application;
use buddy_ui::shell::native::{NativeWindowSnapshot, probe_main_window};
use buddy_ui::shell::{self, AppShell};
use std::fs;
use std::path::PathBuf;

const HOTKEY: &str = "CmdOrCtrl+Alt+Shift+F20";

fn probe(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Option<NativeWindowSnapshot> {
    cx.update_window(handle.into(), |_, window, _| probe_main_window(window).ok())
        .ok()
        .flatten()
}

fn bounds(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<buddy_ui::gpui::Bounds<buddy_ui::gpui::Pixels>> {
    cx.update_window(handle.into(), |_, window, _| window.bounds())
        .ok()
}

fn router_id(
    handle: WindowHandle<AppShell>,
    cx: &mut AsyncApp,
) -> Option<buddy_ui::gpui::EntityId> {
    handle
        .read_with(cx, |shell, _| shell.router().entity_id())
        .ok()
}

fn sandbox() -> Result<PathBuf, String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/buddy-wake-preview")
        .join(std::process::id().to_string());
    if path.exists() {
        return Err(format!(
            "唤醒自测专用沙盒已存在，拒绝覆盖：{}",
            path.display()
        ));
    }
    fs::create_dir_all(&path).map_err(|error| format!("创建唤醒自测沙盒失败：{error}"))?;
    Ok(path)
}

async fn run(cx: &mut AsyncApp) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("S07-11 wake 自测仅在 macOS 运行".into());
    }

    let data_dir = sandbox()?;
    let mut config = AppConfig::default();
    config.hotkey = HOTKEY.into();
    if let Err(error) = storage::save_config(&data_dir, &config) {
        let cleaned = fs::remove_dir_all(&data_dir).is_ok() && !data_dir.exists();
        return Err(format!(
            "写入唤醒自测配置失败：{error}；sandbox_cleaned={cleaned}"
        ));
    }
    let engine = ChatEngine::new(data_dir.clone());
    let handle = match shell::open_main_window(engine, shell::config::ShellConfig::default(), cx).await {
        Ok(handle) => handle,
        Err(error) => {
            let cleaned = fs::remove_dir_all(&data_dir).is_ok() && !data_dir.exists();
            return Err(format!(
                "打开真实外壳失败：{error}；sandbox_cleaned={cleaned}"
            ));
        }
    };

    let operation = async {
        let initial_bounds = bounds(handle, cx).ok_or("读取初始窗口尺寸失败")?;
        let initial_router = router_id(handle, cx).ok_or("读取初始 Router 失败")?;
        let installed = shell::runtime::install(handle, cx).await;
        if let Err(error) = installed {
            return Err(format!(
                "专用热键 {HOTKEY} 注册失败；拒绝替换用户热键：{error}"
            ));
        }
        let registered_before = cx
            .update(|app| shell::runtime::registered_hotkey(app))
            .ok_or("安装后没有有效的系统热键")?;
        if !registered_before.to_ascii_lowercase().contains("f20") {
            return Err(format!(
                "安装后实际热键不是专用 F20：{registered_before}"
            ));
        }

        shell::runtime::hide(handle, cx)
            .await
            .map_err(|error| format!("隐藏主窗口失败：{error}"))?;
        let hidden_before = probe(handle, cx).ok_or("隐藏后无法读取原生窗口")?;
        if hidden_before.is_visible {
            return Err("隐藏后原生窗口仍可见".into());
        }

        // 这是对系统唤醒回调的明确模拟；不发送输入，也不调用 show / activate。
        shell::runtime::resume_after_wake(handle, cx)
            .await
            .map_err(|error| format!("模拟系统唤醒恢复失败：{error}"))?;
        let hidden_after = probe(handle, cx).ok_or("唤醒恢复后无法读取原生窗口")?;
        let router_after = router_id(handle, cx).ok_or("唤醒恢复后无法读取 Router")?;
        let registered_after = cx
            .update(|app| shell::runtime::registered_hotkey(app))
            .ok_or("唤醒恢复后没有有效的系统热键")?;
        let after_bounds = bounds(handle, cx).ok_or("唤醒恢复后无法读取窗口尺寸")?;
        let size_unchanged = initial_bounds.size == after_bounds.size;
        let same_router = initial_router == router_after;
        let key_preserved = registered_before == registered_after
            && registered_after.to_ascii_lowercase().contains("f20");
        if hidden_after.is_visible || !same_router || !key_preserved || !size_unchanged {
            return Err(format!(
                "唤醒断言失败：hidden={} same_router={} key_preserved={} size_unchanged={} before_key={} after_key={}",
                !hidden_after.is_visible,
                same_router,
                key_preserved,
                size_unchanged,
                registered_before,
                registered_after
            ));
        }
        println!(
            "PASS S07-11 wake hidden_before={} hidden_after={} same_router={} key_preserved={} size_unchanged={} key={}",
            !hidden_before.is_visible,
            !hidden_after.is_visible,
            same_router,
            key_preserved,
            size_unchanged,
            registered_after
        );
        Ok(())
    }
    .await;

    // 退出路径显式注销唯一 manager，再移除窗口；不依赖 AppKit 析构顺序。
    let released = cx.update(|app| shell::runtime::shutdown(app)).is_ok();
    let removed = cx
        .update_window(handle.into(), |_, window, _| window.remove_window())
        .is_ok();
    let cleaned = fs::remove_dir_all(&data_dir).is_ok() && !data_dir.exists();
    match (operation, released, removed, cleaned) {
        (Ok(()), true, true, true) => Ok(()),
        (result, released, removed, cleaned) => Err(format!(
            "S07-11 wake 清理失败：operation={result:?} hotkey_released={released} window_removed={removed} sandbox_cleaned={cleaned}"
        )),
    }
}

fn main() {
    application()
        .with_assets(buddy_ui::icons::Assets)
        .run(move |cx: &mut App| {
            shell::init(cx);
            cx.spawn(async move |cx: &mut AsyncApp| {
                match run(cx).await {
                    Ok(()) => println!("PASS S07-11 wake"),
                    Err(error) => {
                        eprintln!("FAIL S07-11 wake：{error}");
                        std::process::exit(1);
                    }
                }
                std::process::exit(0);
            })
            .detach();
        });
}
