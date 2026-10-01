//! S06-02 T37/T38: provider workflow through real mouse, keyboard and paste
//! events.  The fixture server is loopback-only and is disposable per run.

mod provider_mock;
mod provider_test_input;
mod provider_test_t37;
mod provider_test_t38;
mod provider_test_t40;

use self::provider_mock::{MockMode, ProviderMock};
use self::provider_test_input::{click_id, draw, paste_field, settle};
use super::*;
use buddy_engine::models::AppConfig;
use buddy_ui::settings::provider::{AddProviderPanel, Busy};
use std::time::Duration;

fn open_router(cx: &mut App) -> WindowHandle<PageRouter> {
    let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
    let engine = ChatEngine::new(fixture::sandbox());
    let loaded = Loaded {
        config: AppConfig::default(),
        history: Vec::new(),
        offset: 0,
    };
    cx.open_window(fixture::options(bounds), |window, cx| {
        cx.new(|cx| PageRouter::new(engine, loaded, window, cx))
    })
    .expect("打开 provider 自测窗口")
}

fn panel(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> Option<Entity<AddProviderPanel>> {
    handle
        .read_with(cx, |router, app| {
            router.settings_view().read(app).provider_panel().clone()
        })
        .ok()
}

async fn open_panel(
    handle: WindowHandle<PageRouter>,
    cx: &mut AsyncApp,
) -> Option<Entity<AddProviderPanel>> {
    let _ = handle.update(cx, |router, _, cx| router.open_settings(cx));
    settle(handle, cx).await;
    let add = handle
        .read_with(cx, |router, app| {
            router.settings_view().read(app).add_button_bounds()
        })
        .ok()
        .flatten()?;
    input::click(
        handle,
        f32::from(add.origin.x + add.size.width / 2.0),
        f32::from(add.origin.y + add.size.height / 2.0),
        cx,
    )
    .await;
    settle(handle, cx).await;
    let open = handle
        .read_with(cx, |router, app| {
            router.settings_view().read(app).provider_open()
        })
        .unwrap_or(false);
    if !open {
        return None;
    }
    panel(handle, cx)
}

async fn close_panel(
    handle: WindowHandle<PageRouter>,
    panel: &Entity<AddProviderPanel>,
    cx: &mut AsyncApp,
) {
    let _ = click_id(handle, panel, "cancel", cx).await;
    settle(handle, cx).await;
}

async fn fill_connection(
    handle: WindowHandle<PageRouter>,
    panel: &Entity<AddProviderPanel>,
    url: &str,
    cx: &mut AsyncApp,
) -> bool {
    let custom = click_id(handle, panel, "custom", cx).await;
    let url_visible = click_id(handle, panel, "url", cx).await;
    let url_pasted = paste_field(handle, panel, false, url, cx).await;
    let key_visible = click_id(handle, panel, "key", cx).await;
    let key_pasted = paste_field(handle, panel, true, "loopback-key", cx).await;
    custom && url_visible && url_pasted && key_visible && key_pasted
}

async fn fill_preset(
    handle: WindowHandle<PageRouter>,
    panel: &Entity<AddProviderPanel>,
    id: &str,
    url: &str,
    cx: &mut AsyncApp,
) -> bool {
    let selected = click_id(handle, panel, id, cx).await;
    let default_url = panel.read_with(cx, |panel, app| {
        !panel.url_field().read(app).text(app).is_empty()
    });
    selected
        && default_url
        && click_id(handle, panel, "url", cx).await
        && paste_field(handle, panel, false, url, cx).await
        && click_id(handle, panel, "key", cx).await
        && paste_field(handle, panel, true, "loopback-key", cx).await
}

async fn wait_idle(
    handle: WindowHandle<PageRouter>,
    panel: &Entity<AddProviderPanel>,
    cx: &mut AsyncApp,
) {
    for _ in 0..100 {
        draw(handle, cx).await;
        if panel.read_with(cx, |panel, _| panel.form().busy == Busy::Idle) {
            return;
        }
        cx.background_executor()
            .timer(Duration::from_millis(20))
            .await;
    }
}

async fn fetch_case(
    handle: WindowHandle<PageRouter>,
    panel: &Entity<AddProviderPanel>,
    server: &ProviderMock,
    mode: MockMode,
    cx: &mut AsyncApp,
) -> (usize, bool) {
    server.set_mode(mode);
    let before = server.request_count();
    let clicked = click_id(handle, panel, "fetch", cx).await;
    wait_idle(handle, panel, cx).await;
    let requests = server.request_count().saturating_sub(before);
    (requests, clicked)
}

async fn wait_request(server: &ProviderMock, before: usize, cx: &mut AsyncApp) -> bool {
    for _ in 0..100 {
        if server.request_count() > before {
            return true;
        }
        cx.background_executor()
            .timer(Duration::from_millis(5))
            .await;
    }
    false
}

async fn wait_config_save(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) {
    let task = handle
        .update(cx, |router, _, _| router.take_config_save())
        .ok()
        .flatten();
    if let Some(task) = task {
        task.await;
    }
}

async fn persisted_config(cx: &mut AsyncApp) -> Option<AppConfig> {
    let engine = ChatEngine::new(fixture::sandbox());
    let task = cx.update(|app| {
        buddy_ui::chat_bridge::spawn_engine(app, async move { engine.get_config().await })
    });
    task.await.ok()
}

fn same_config(left: &AppConfig, right: &AppConfig) -> bool {
    serde_json::to_string(left).ok() == serde_json::to_string(right).ok()
}

/// Entry point called by `settings_preview --selftest-providers`.
pub(crate) async fn run(cx: &mut AsyncApp) -> bool {
    let server = ProviderMock::start();
    cx.update(|app| buddy_ui::chat_bridge::init(app));
    let handle = cx.update(|cx| open_router(cx));
    let t37_ok = provider_test_t37::run(handle, &server, cx).await;
    let t38_ok = provider_test_t38::run(handle, &server, cx).await;
    let t40_ok = provider_test_t40::run(handle, cx).await;
    let ok = t37_ok && t38_ok && t40_ok;
    println!("{} S06-02 provider 自测", if ok { "PASS" } else { "FAIL" });
    ok
}
