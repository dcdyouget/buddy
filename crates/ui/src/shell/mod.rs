//! 主窗口外壳：统一创建入口和页面尺寸策略（S07-01 / S07-02）。
//!
//! PageRouter 只负责内容，尺寸由外壳订阅页面变更决定。
//! 原生外观在首个可见帧之前应用；底边锚定与屏幕定位由 S07-06 承接。

pub mod config;
pub mod hotkey;
pub mod native;
pub mod runtime;
pub mod selection;
pub mod sizing;
mod visibility;

use crate::chat::page_state::Page;
use crate::chat::router::{PageRouter, RouterEvent, preload};
use crate::chat_bridge::spawn_engine;
use buddy_engine::chat::ChatEngine;
use config::ShellConfig;
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, KeyDownEvent, Render, Subscription, Window,
    WindowHandle, div, prelude::*,
};
use std::sync::Arc;

/// 主窗口根视图；持有同一 Router 和页面变更订阅。
pub struct AppShell {
    router: Entity<PageRouter>,
    _page_subscription: Subscription,
}

impl AppShell {
    fn new(router: Entity<PageRouter>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut settings_origin = None;
        let subscription = cx.subscribe_in(
            &router,
            window,
            move |_, _, event: &RouterEvent, window, _| {
                let RouterEvent::PageChanged { from, to } = *event;
                let target = sizing::resize_target(from, to, settings_origin);
                if to == Page::Settings {
                    settings_origin = Some(from);
                }
                if let Some(target) = target {
                    window.resize(target.to_gpui());
                }
            },
        );
        Self {
            router,
            _page_subscription: subscription,
        }
    }

    /// 主窗口共用的页面路由器。
    pub fn router(&self) -> Entity<PageRouter> {
        self.router.clone()
    }
}

impl Render for AppShell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && let Some(handle) = window.window_handle().downcast::<AppShell>()
                {
                    cx.stop_propagation();
                    runtime::request_hide(handle, cx);
                }
            }))
            .child(self.router.clone())
    }
}

/// 初始化产品和外壳预览共用的 UI 环境；须在创建任何页面前调用。
pub fn init(cx: &mut App) {
    crate::init_theme(cx);
    crate::chat_bridge::init(cx);
    crate::theme_system::fonts::install_text_rendering(cx);
    crate::http::install(cx);
    crate::markdown::init(cx);
    crate::chat::init(cx);
}

/// 经 tokio 读取配置和最新历史，在显示前应用原生外观并装配真实页面。
/// 此入口不抢焦点，不对模型菜单应用补丁；热键唤起归 S07-03。
pub async fn open_main_window(
    engine: Arc<ChatEngine>,
    config: ShellConfig,
    cx: &mut AsyncApp,
) -> anyhow::Result<WindowHandle<AppShell>> {
    config.validate().map_err(anyhow::Error::msg)?;
    let loaded = cx
        .update(|cx| spawn_engine(cx, preload(engine.clone())))
        .await;
    let handle = cx.update(|cx| {
        let options = config.window_options(cx).map_err(anyhow::Error::msg)?;
        cx.open_window(options, |window, cx| {
            let router = cx.new(|cx| PageRouter::new(engine, loaded, window, cx));
            cx.new(|cx| AppShell::new(router, window, cx))
        })
    })?;
    #[cfg(target_os = "macos")]
    {
        let prepared = cx.update_window(handle.into(), |_, window, _| {
            native::prepare_main_window(window)
        });
        // AppKit style changes synchronously call GPUI's resize callback. Release
        // AsyncApp::update's App borrow first, with the hidden window registered.
        let result = prepared
            .map_err(anyhow::Error::from)
            .and_then(|value| value.map_err(anyhow::Error::from))
            .and_then(|value| value.apply_and_show().map_err(anyhow::Error::from));
        if let Err(error) = result {
            let _ = cx.update_window(handle.into(), |_, window, _| window.remove_window());
            anyhow::bail!("主窗口外观初始化失败：{error}");
        }
    }
    cx.update_window(handle.into(), |_, window, _| window.refresh())?;
    Ok(handle)
}
