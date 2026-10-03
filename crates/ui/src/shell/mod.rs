//! 主窗口外壳：统一创建入口和页面尺寸策略（S07-01 / S07-02）。
//!
//! PageRouter 只负责内容，尺寸由外壳订阅页面变更决定。
//! 原生外观在首个可见帧之前应用；页面展开固定底边并裁剪到当前屏幕工作区。

pub mod config;
mod focus_order;
pub mod hotkey;
pub mod native;
pub mod positioning;
mod positioning_controller;
pub mod positioning_native;
pub mod runtime;
pub mod selection;
pub mod sizing;
mod visibility;
pub mod workspaces;

use crate::chat::page_state::Page;
use crate::chat::router::{PageRouter, RouterEvent, preload};
use crate::chat_bridge::spawn_engine;
use buddy_engine::chat::ChatEngine;
use config::ShellConfig;
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, KeyDownEvent, Render, Subscription, Task, Window,
    WindowHandle, div, prelude::*,
};
use std::sync::Arc;

/// 主窗口根视图；持有同一 Router 和页面变更订阅。
pub struct AppShell {
    router: Entity<PageRouter>,
    _page_subscription: Subscription,
    positions: positioning::PositionMemory,
    pending_position_save: Option<Task<()>>,
    pending_resize: Option<Task<()>>,
    _position_subscriptions: [Subscription; 2],
    _focus_order_subscription: Subscription,
    focus_order_task: Option<Task<()>>,
}

impl AppShell {
    fn new(router: Entity<PageRouter>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut settings_origin = None;
        let subscription = cx.subscribe_in(
            &router,
            window,
            move |shell, _, event: &RouterEvent, window, cx| {
                let RouterEvent::PageChanged { from, to } = *event;
                let target = sizing::resize_target(from, to, settings_origin);
                if to == Page::Settings {
                    settings_origin = Some(from);
                }
                if let Some(target) = target {
                    positioning_controller::resize(shell, target, window, cx);
                }
            },
        );
        let position_subscriptions = positioning_controller::observe(window, cx);
        Self {
            router,
            _page_subscription: subscription,
            positions: positioning::PositionMemory::default(),
            pending_position_save: None,
            pending_resize: None,
            _position_subscriptions: position_subscriptions,
            _focus_order_subscription: focus_order::observe(window, cx),
            focus_order_task: None,
        }
    }

    /// 主窗口共用的页面路由器。
    pub fn router(&self) -> Entity<PageRouter> {
        self.router.clone()
    }

    /// 当前进程为指定显示器记住的位置；供窗口诊断读取，不从磁盘恢复。
    pub fn saved_window_position(&self, display_key: &str) -> Option<positioning::Point> {
        self.positions.saved(display_key)
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
