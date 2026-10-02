//! S06-05/S06-06：通过真实 GPUI 输入验收外观与快捷键配置。

mod preferences_test_input;
mod preferences_test_t43;
mod preferences_test_t44;

use super::*;
use buddy_engine::models::AppConfig;
use buddy_ui::settings::SettingsView;
use buddy_ui::settings::hotkey::HotkeyRecorder;
use buddy_ui::settings::theme_control::ThemeControl;

pub(crate) fn view(
    handle: WindowHandle<PageRouter>,
    cx: &mut AsyncApp,
) -> Option<Entity<SettingsView>> {
    handle
        .read_with(cx, |router, _| router.settings_view().clone())
        .ok()
}

pub(crate) fn hotkey(
    handle: WindowHandle<PageRouter>,
    cx: &mut AsyncApp,
) -> Option<Entity<HotkeyRecorder>> {
    Some(view(handle, cx)?.read_with(cx, |view, _| view.hotkey_recorder().clone()))
}

pub(crate) fn theme(
    handle: WindowHandle<PageRouter>,
    cx: &mut AsyncApp,
) -> Option<Entity<ThemeControl>> {
    Some(view(handle, cx)?.read_with(cx, |view, _| view.theme_control().clone()))
}

pub(crate) async fn open_settings(
    handle: WindowHandle<PageRouter>,
    cx: &mut AsyncApp,
) -> Option<Entity<SettingsView>> {
    let _ = handle.update(cx, |router, _, cx| router.open_settings(cx));
    preferences_test_input::settle(handle, cx).await;
    view(handle, cx)
}

pub(crate) async fn wait_save(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) {
    let task = handle
        .update(cx, |router, _, _| router.take_config_save())
        .ok()
        .flatten();
    if let Some(task) = task {
        task.await;
    }
    preferences_test_input::settle(handle, cx).await;
}

pub(crate) async fn persisted_config(
    cx: &mut AsyncApp,
    data_dir: std::path::PathBuf,
) -> Option<AppConfig> {
    let engine = ChatEngine::new(data_dir);
    let task = cx.update(|app| {
        buddy_ui::chat_bridge::spawn_engine(app, async move { engine.get_config().await })
    });
    task.await.ok()
}

pub(crate) fn config(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> Option<AppConfig> {
    handle
        .read_with(cx, |router, _| router.config().clone())
        .ok()
}

pub(crate) async fn run(cx: &mut AsyncApp) -> bool {
    cx.update(|app| buddy_ui::chat_bridge::init(app));
    let handle = cx.update(|app| fixture::open_router(app));
    let Some(_) = open_settings(handle, cx).await else {
        println!("FAIL S06-05/S06-06：无法打开设置");
        return false;
    };
    let t43 = preferences_test_t43::run(handle, cx).await;
    let t44 = preferences_test_t44::run(handle, cx).await;
    let ok = t43 && t44;
    println!(
        "{} S06-05/S06-06 偏好设置自测",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}
