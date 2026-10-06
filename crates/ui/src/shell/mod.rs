//! 主窗口外壳：统一创建入口和页面尺寸策略。
//!
//! PageRouter 只负责内容，尺寸由外壳订阅页面变更决定。
//! 原生外观在首个可见帧之前应用；页面展开固定底边并裁剪到当前屏幕工作区。

pub mod autostart;
pub mod config;
pub mod entrance;
mod focus_order;
pub mod hotkey;
pub mod lifecycle;
pub mod native;
pub mod positioning;
mod positioning_controller;
pub mod positioning_native;
pub mod runtime;
pub mod selection;
pub mod selfcheck;
pub mod services;
pub mod sizing;
pub mod tray;
pub mod visibility;
#[cfg(target_os = "macos")]
mod window_motion;
pub mod workspaces;

use crate::chat::page_state::Page;
use crate::chat::router::{PageRouter, RouterEvent, preload};
use crate::chat_bridge::spawn_engine;
use crate::theme_system::BuddyTheme;
use buddy_engine::chat::ChatEngine;
use config::ShellConfig;
use gpui::{
    App, AppContext, AsyncApp, Context, Entity, Hsla, Render, Subscription, Task, Window,
    WindowHandle, div, linear_color_stop, linear_gradient, prelude::*, px, relative,
};
use std::sync::Arc;

/// 主窗口根视图；持有同一 Router 和页面变更订阅。
pub struct AppShell {
    router: Entity<PageRouter>,
    entrance: entrance::EntranceMotion,
    dialog_motion: entrance::DialogMotion,
    visibility_generation: u64,
    _page_subscription: Subscription,
    positions: positioning::PositionMemory,
    pending_position_save: Option<Task<()>>,
    #[cfg(target_os = "windows")]
    pending_deactivation_hide: Option<Task<()>>,
    #[cfg(target_os = "macos")]
    pending_resize: Option<Task<()>>,
    _position_subscriptions: [Subscription; 2],
    _focus_order_subscription: Subscription,
    _escape_subscription: Subscription,
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
                if from.is_compact() && !to.is_compact() {
                    shell.play_dialog_transition(cx);
                }
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
        // Esc 与再按一次呼出快捷键等效。用按键观察者而不是根元素的 key_down：焦点元素
        // 不在本帧（虚拟列表滚出视口的消息、流式中被替换的 Markdown）时 GPUI 只派发给
        // 根节点，元素监听收不到。观察者只在没有子层（菜单、审批、设置页）消费时才触发。
        let escape_subscription = cx.observe_keystrokes(|_, event, window, cx| {
            let keystroke = &event.keystroke;
            if keystroke.key == "escape"
                && keystroke.modifiers.number_of_modifiers() == 0
                && let Some(handle) = window.window_handle().downcast::<AppShell>()
            {
                runtime::request_hide(handle, cx);
            }
        });
        Self {
            router,
            entrance: entrance::EntranceMotion::default(),
            dialog_motion: entrance::DialogMotion::default(),
            visibility_generation: 0,
            _page_subscription: subscription,
            positions: positioning::PositionMemory::default(),
            pending_position_save: None,
            #[cfg(target_os = "windows")]
            pending_deactivation_hide: None,
            #[cfg(target_os = "macos")]
            pending_resize: None,
            _position_subscriptions: position_subscriptions,
            _focus_order_subscription: focus_order::observe(window, cx),
            _escape_subscription: escape_subscription,
            focus_order_task: None,
        }
    }

    /// 主窗口共用的页面路由器。
    pub fn router(&self) -> Entity<PageRouter> {
        self.router.clone()
    }

    /// 当前呼入 / 呼出阶段。
    pub fn entrance_phase(&self) -> entrance::EntrancePhase {
        self.entrance.phase()
    }

    /// 隐藏时立即重置入场外壳。
    pub(crate) fn reset_entrance(&mut self, cx: &mut Context<Self>) {
        self.visibility_generation = self.visibility_generation.wrapping_add(1);
        self.entrance.reset();
        cx.notify();
    }

    /// 进入呼出阶段，返回本次可见性世代与是否播放原生呼出动画。
    pub(crate) fn begin_exit(&mut self, cx: &mut Context<Self>) -> Option<(u64, bool)> {
        if self.entrance.is_exiting() {
            return None;
        }
        self.visibility_generation = self.visibility_generation.wrapping_add(1);
        let animate = self.entrance.begin_exit();
        cx.notify();
        Some((self.visibility_generation, animate))
    }

    pub(crate) fn is_current_visibility_generation(&self, generation: u64) -> bool {
        self.visibility_generation == generation
    }

    /// Invalidate any delayed hide before awaiting positioning or first-frame work.
    pub(crate) fn prepare_show(&mut self, visible: bool, cx: &mut Context<Self>) -> (u64, bool) {
        let resume_exit = self.entrance.is_exiting();
        self.visibility_generation = self.visibility_generation.wrapping_add(1);
        if !visible {
            self.entrance.reset();
        }
        cx.notify();
        (self.visibility_generation, resume_exit)
    }

    /// 显示完成后进入呼入阶段，返回是否播放原生呼入动画。
    pub(crate) fn play_entrance(&mut self, cx: &mut Context<Self>) -> bool {
        self.visibility_generation = self.visibility_generation.wrapping_add(1);
        let animate = self.entrance.play();
        cx.notify();
        animate
    }

    fn play_dialog_transition(&mut self, cx: &mut Context<Self>) {
        self.dialog_motion.play();
        cx.notify();
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 呼入 / 呼出由原生图层合成（见 `window_motion`），内容始终按静态帧绘制。
        let dialog_frame = self.dialog_motion.frame();
        if self.dialog_motion.animating() {
            window.request_animation_frame();
        }
        let c = cx.buddy_theme().colors;
        div()
            .size_full()
            .relative()
            .overflow_hidden()
            .child(div().size_full().child(self.router.clone()))
            .children(
                dialog_frame
                    .filter(|_| !self.entrance.is_exiting())
                    .map(|frame| sheen(frame.sheen_x, frame.sheen_opacity, c.buddy_primary)),
            )
    }
}

/// 低对比度的品牌色流光；只覆盖绘制层，不创建交互 hitbox，也不改变窗口布局。
fn sheen(x: f32, opacity: f32, color: gpui::Rgba) -> gpui::Div {
    let transparent = Hsla::from(color).opacity(0.0);
    div()
        .absolute()
        .top_0()
        .left(relative(x - 0.18))
        .w(relative(0.36))
        .h_full()
        .rounded(px(crate::theme_system::tokens::metrics::RADIUS_XL))
        .bg(linear_gradient(
            90.,
            linear_color_stop(transparent, 0.0),
            linear_color_stop(Hsla::from(color).opacity(0.28), 1.0),
        ))
        .opacity(opacity)
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
/// 此入口不抢焦点，不对模型菜单应用补丁；热键唤起归 。
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
