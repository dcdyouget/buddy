//! Phase 06 设置页预览与自检。
//!
//! ```text
//! cargo run -p buddy-app --example settings_preview
//! cargo run -p buddy-app --example settings_preview -- --selftest
//! cargo run -p buddy-app --example settings_preview -- --controls
//! ```
//!
//! 预览直接使用 `PageRouter` 和真实 `SettingsView`。示例只构造沙盒配置，
//! 不连接网络；设置层退出时底层 Composer / Conversation 仍由同一个 Router 持有。

use buddy_engine::chat::ChatEngine;
use buddy_engine::models::{AppConfig, Message, MessageRole, ModelInfo, ProviderConfig, Theme};
use buddy_engine::streaming::StreamEvent;
use buddy_ui::chat::page_state::Page;
use buddy_ui::chat::router::{Loaded, PageRouter};
use buddy_ui::chat::state::text_of;
use buddy_ui::gpui::{
    canvas, div, point, prelude::*, px, size, App, AppContext, AsyncApp, Bounds, ClipboardItem,
    Context, Entity, Focusable, IntoElement, KeyDownEvent, Keystroke, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, PlatformInput, Render, ScrollDelta,
    ScrollWheelEvent, Subscription, TouchPhase, Window, WindowBackgroundAppearance, WindowBounds,
    WindowHandle, WindowOptions,
};
use buddy_ui::gpui_platform::application;
use buddy_ui::settings::controls::{self as settings_controls, SettingsField, SettingsFieldEvent};
use buddy_ui::settings::select::{SettingsSelect, SettingsSelectChanged};
use buddy_ui::theme_system::tokens::metrics as m;
use buddy_ui::theme_system::{fonts, Appearance, BuddyTheme, Theme as UiTheme};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

const WIDTH: f32 = 760.0;
const HEIGHT: f32 = 640.0;

#[path = "settings_preview/controls.rs"]
mod controls;
#[path = "settings_preview/fixture.rs"]
mod fixture;
#[path = "settings_preview/input.rs"]
mod input;
#[path = "settings_preview/provider_test.rs"]
mod provider_test;
#[path = "settings_preview/selftest.rs"]
mod selftest;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().collect();
    let self_test = args.iter().any(|arg| arg == "--selftest");
    let self_test_controls = args.iter().any(|arg| arg == "--selftest-controls");
    let self_test_providers = args.iter().any(|arg| arg == "--selftest-providers");
    let provider = args.iter().any(|arg| arg == "--provider");
    let controls = args.iter().any(|arg| arg == "--controls");
    let dark = args.iter().any(|arg| arg == "--dark");
    application()
        .with_assets(buddy_ui::icons::Assets)
        .run(move |cx: &mut App| {
            buddy_ui::init_theme(cx);
            UiTheme::install(
                if dark {
                    Appearance::Dark
                } else {
                    Appearance::Light
                },
                cx,
            );
            fonts::install_text_rendering(cx);
            buddy_ui::markdown::init(cx);
            buddy_ui::chat::init(cx);
            if !self_test && !self_test_controls && !self_test_providers {
                cx.observe_keystrokes(|event, _, cx| {
                    if event.keystroke.key == "escape" || event.keystroke.unparse() == "cmd-q" {
                        cx.quit();
                    }
                })
                .detach();
            }
            if self_test_providers {
                cx.spawn(async move |cx: &mut AsyncApp| {
                    let ok = provider_test::run(cx).await;
                    std::process::exit(if ok { 0 } else { 1 });
                })
                .detach();
                return;
            }
            let handle = fixture::open_router(cx);
            let control_handle =
                (controls || self_test_controls).then(|| controls::open_controls(cx));
            if !self_test && !self_test_controls && !self_test_providers {
                let _ = handle.update(cx, |router, _, cx| {
                    router.open_settings(cx);
                    if provider {
                        router
                            .settings_view()
                            .clone()
                            .update(cx, |view, cx| view.open_provider(cx));
                    }
                });
            }
            if !self_test && !self_test_controls && !self_test_providers {
                cx.activate(true);
            }
            if self_test {
                cx.spawn(async move |cx: &mut AsyncApp| {
                    let ok = selftest::selftest(handle, cx).await;
                    std::process::exit(if ok { 0 } else { 1 });
                })
                .detach();
            } else if self_test_controls {
                let control_handle = control_handle.expect("控件自测窗口");
                cx.spawn(async move |cx: &mut AsyncApp| {
                    let ok = controls::testing::selftest_controls(control_handle, cx).await;
                    std::process::exit(if ok { 0 } else { 1 });
                })
                .detach();
            }
        });
}
