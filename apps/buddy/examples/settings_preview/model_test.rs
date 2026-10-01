//! S06-03 T41/T42: saved model editing through real GPUI input events.

mod model_test_capabilities;
mod model_test_fixture;
mod model_test_input;
mod model_test_t41;
mod model_test_t42;

use self::model_test_fixture as fixture;
use self::model_test_input::settle;
use super::*;
use buddy_engine::models::AppConfig;
use buddy_ui::settings::model_list::ModelListView;

pub(crate) fn open_router(cx: &mut App) -> WindowHandle<PageRouter> {
    let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
    let engine = ChatEngine::new(fixture::sandbox());
    let mut config = fixture::config();
    config.theme = match cx.buddy_theme().appearance {
        Appearance::Light => Theme::Light,
        Appearance::Dark => Theme::Dark,
    };
    let loaded = Loaded {
        config,
        history: Vec::new(),
        offset: 0,
    };
    cx.open_window(fixture::options(bounds), |window, cx| {
        cx.new(|cx| PageRouter::new(engine, loaded, window, cx))
    })
    .expect("打开 model 自测窗口")
}

fn view(
    handle: WindowHandle<PageRouter>,
    cx: &mut AsyncApp,
) -> Option<Entity<buddy_ui::settings::SettingsView>> {
    handle
        .read_with(cx, |router, _| router.settings_view().clone())
        .ok()
}

fn list(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> Option<Entity<ModelListView>> {
    handle
        .read_with(cx, |router, app| {
            router.settings_view().read(app).model_list().clone()
        })
        .ok()
}

async fn open_settings(
    handle: WindowHandle<PageRouter>,
    cx: &mut AsyncApp,
) -> Option<(
    Entity<buddy_ui::settings::SettingsView>,
    Entity<ModelListView>,
)> {
    let _ = handle.update(cx, |router, _, cx| router.open_settings(cx));
    settle(handle, cx).await;
    Some((view(handle, cx)?, list(handle, cx)?))
}

async fn close_settings(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) {
    let _ = handle.update(cx, |router, _, cx| router.close_settings(cx));
    settle(handle, cx).await;
}

pub(crate) async fn config(
    handle: WindowHandle<PageRouter>,
    cx: &mut AsyncApp,
) -> Option<AppConfig> {
    handle
        .read_with(cx, |router, _| router.config().clone())
        .ok()
}

/// Entry point called by `settings_preview --selftest-models`.
pub(crate) async fn run(cx: &mut AsyncApp) -> bool {
    cx.update(|app| buddy_ui::chat_bridge::init(app));
    let handle = cx.update(|app| open_router(app));
    let Some((view, list)) = open_settings(handle, cx).await else {
        println!("FAIL S06-03 model 自测：无法打开设置模型列表");
        return false;
    };
    let t41 = model_test_t41::run(handle, &view, &list, cx).await;
    close_settings(handle, cx).await;
    let t42 = model_test_t42::run(handle, &view, &list, cx).await;
    let ok = t41 && t42;
    println!("{} S06-03 model 自测", if ok { "PASS" } else { "FAIL" });
    ok
}

pub(crate) use config as router_config;
