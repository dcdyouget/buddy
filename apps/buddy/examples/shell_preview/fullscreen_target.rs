//! S07-05 专用全屏 Space child。
//!
//! child 只拥有一个普通 GPUI 窗口，通过真实 `Window::toggle_fullscreen()` 进入全屏
//! Space。READY 只在 native fullscreen、key window 和 bounds 完整覆盖主显示器后写出；
//! 退出时只发送一次 toggle，等待恢复 windowed bounds 后写 DONE。

use buddy_ui::gpui::{
    App, AppContext, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window,
    WindowBounds, WindowHandle, WindowKind, WindowOptions, div, point, px, size,
};
use buddy_ui::gpui_platform::application;
use buddy_ui::shell::workspaces;
use buddy_ui::theme_system::{Appearance, BuddyTheme, Theme, tokens};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const READY_ENV: &str = "BUDDY_SHELL_T50_READY";
const STATUS_ENV: &str = "BUDDY_SHELL_T50_STATUS";
const ACTIVATE_ENV: &str = "BUDDY_SHELL_T50_ACTIVATE";
const ACK_ENV: &str = "BUDDY_SHELL_T50_ACK";
const EXIT_ENV: &str = "BUDDY_SHELL_T50_EXIT";
const DONE_ENV: &str = "BUDDY_SHELL_T50_DONE";

struct FullscreenTarget {
    background: buddy_ui::gpui::Rgba,
    foreground: buddy_ui::gpui::Rgba,
}

impl Render for FullscreenTarget {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(self.background)
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_color(self.foreground)
                    .text_size(px(tokens::metrics::FONT_SIZE_XL))
                    .child("S07-05 全屏 Space 测试窗口"),
            )
    }
}

#[derive(Clone, Copy, Debug)]
struct ChildState {
    fullscreen: bool,
    active: bool,
    fills_screen: bool,
    is_on_active_space: bool,
    window_number: Option<i64>,
    bounds: Bounds<buddy_ui::gpui::Pixels>,
}

fn options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(80.0), px(80.0)),
            size(px(640.0), px(360.0)),
        ))),
        // GPUI only applies NSResizableWindowMask in its Some(titlebar) branch.
        titlebar: Some(Default::default()),
        kind: WindowKind::Normal,
        focus: true,
        show: true,
        ..Default::default()
    }
}

fn read_paths() -> Option<(PathBuf, PathBuf, PathBuf, PathBuf, PathBuf, PathBuf)> {
    Some((
        std::env::var_os(READY_ENV).map(PathBuf::from)?,
        std::env::var_os(STATUS_ENV).map(PathBuf::from)?,
        std::env::var_os(ACTIVATE_ENV).map(PathBuf::from)?,
        std::env::var_os(ACK_ENV).map(PathBuf::from)?,
        std::env::var_os(EXIT_ENV).map(PathBuf::from)?,
        std::env::var_os(DONE_ENV).map(PathBuf::from)?,
    ))
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= 3.0
}

fn same_bounds(
    left: Bounds<buddy_ui::gpui::Pixels>,
    right: Bounds<buddy_ui::gpui::Pixels>,
) -> bool {
    close(f32::from(left.origin.x), f32::from(right.origin.x))
        && close(f32::from(left.origin.y), f32::from(right.origin.y))
        && close(f32::from(left.size.width), f32::from(right.size.width))
        && close(f32::from(left.size.height), f32::from(right.size.height))
}

fn state(
    handle: WindowHandle<FullscreenTarget>,
    display_bounds: Bounds<buddy_ui::gpui::Pixels>,
    cx: &mut buddy_ui::gpui::AsyncApp,
) -> Option<ChildState> {
    cx.update_window(handle.into(), |_, window, _| {
        let bounds = window.bounds();
        let workspace = workspaces::probe(window).ok();
        Some(ChildState {
            fullscreen: window.is_fullscreen()
                && workspace.is_some_and(|value| value.is_fullscreen),
            active: window.is_window_active() && workspace.is_some_and(|value| value.app_is_active),
            fills_screen: same_bounds(bounds, display_bounds),
            is_on_active_space: workspace.is_some_and(|workspace| workspace.is_on_active_space),
            window_number: workspace.map(|workspace| workspace.window_number),
            bounds,
        })
    })
    .ok()
    .flatten()
}

fn encode(state: ChildState) -> String {
    format!(
        "fullscreen={} active={} active_space={} fills_screen={} window={} bounds={} {} {} {}",
        state.fullscreen,
        state.active,
        state.is_on_active_space,
        state.fills_screen,
        state
            .window_number
            .map_or_else(|| "<none>".to_owned(), |number| number.to_string()),
        f32::from(state.bounds.origin.x),
        f32::from(state.bounds.origin.y),
        f32::from(state.bounds.size.width),
        f32::from(state.bounds.size.height),
    )
}

/// 由 shell_preview 主入口在 `--fullscreen-target` 下调用。
pub(crate) fn run_child() -> ! {
    application().run(|cx: &mut App| {
        buddy_ui::shell::init(cx);
        Theme::install(Appearance::Light, cx);
        let Some((ready, status, activate, ack, exit, done)) = read_paths() else {
            return;
        };
        let Some(display_bounds) = cx.primary_display().map(|display| display.bounds()) else {
            eprintln!("fullscreen target: no primary display");
            return;
        };
        let theme = *cx.buddy_theme();
        let handle = match cx.open_window(options(), move |_, cx| {
            cx.new(|_| FullscreenTarget {
                background: theme.colors.bg_surface,
                foreground: theme.colors.text_primary,
            })
        }) {
            Ok(handle) => handle,
            Err(error) => {
                eprintln!("fullscreen target: open window failed: {error}");
                return;
            }
        };
        cx.activate(true);
        let restore_bounds = cx
            .update_window(handle.into(), |_, window, _| {
                let bounds = window.bounds();
                window.toggle_fullscreen();
                bounds
            })
            .ok();
        let Some(restore_bounds) = restore_bounds else {
            return;
        };
        cx.spawn(async move |cx| {
            let mut ready_written = false;
            let mut activate_pending = false;
            let mut exit_pending = false;
            let mut sample = String::new();
            let query = status.with_extension("query");
            let next_status = status.with_extension("status-next");
            loop {
                if activate.is_file() {
                    let _ = std::fs::remove_file(&activate);
                    activate_pending = true;
                }
                if exit.is_file() {
                    let _ = std::fs::remove_file(&exit);
                    if !exit_pending {
                        exit_pending = true;
                        let _ = cx.update_window(handle.into(), |_, window, _| {
                            if window.is_fullscreen() {
                                window.toggle_fullscreen();
                            }
                        });
                    }
                }
                if activate_pending {
                    cx.update(|app| app.activate(true));
                    let _ =
                        cx.update_window(handle.into(), |_, window, _| window.activate_window());
                }
                if let Some(current) = state(handle, display_bounds, cx) {
                    if let Ok(request) = std::fs::read_to_string(&query) {
                        sample = request;
                        let _ = std::fs::remove_file(&query);
                    }
                    if std::fs::write(&next_status, format!("{} sample={sample}", encode(current)))
                        .is_ok()
                    {
                        let _ = std::fs::rename(&next_status, &status);
                    }
                    if current.fullscreen
                        && current.active
                        && current.is_on_active_space
                        && current.fills_screen
                        && current.window_number.is_some()
                        && !ready_written
                    {
                        let _ = std::fs::write(&ready, format!("READY {}", encode(current)));
                        ready_written = true;
                    }
                    if activate_pending && current.active {
                        let _ = std::fs::write(&ack, format!("ACK {}", encode(current)));
                        activate_pending = false;
                    }
                    if exit_pending
                        && !current.fullscreen
                        && same_bounds(current.bounds, restore_bounds)
                    {
                        let _ = std::fs::write(&done, format!("DONE {}", encode(current)));
                        cx.update(|app| app.quit());
                        return;
                    }
                }
                cx.background_executor()
                    .timer(Duration::from_millis(20))
                    .await;
            }
        })
        .detach();
    });
    std::process::exit(0)
}

pub(crate) struct ChildTarget {
    pub(crate) child: Child,
    pub(crate) ready: PathBuf,
    pub(crate) status: PathBuf,
    pub(crate) activate: PathBuf,
    pub(crate) ack: PathBuf,
    pub(crate) exit: PathBuf,
    pub(crate) done: PathBuf,
}

impl Drop for ChildTarget {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.exit, "EXIT");
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(self.status.with_extension("query"));
        let _ = std::fs::remove_file(self.status.with_extension("status-next"));
        for path in [
            &self.ready,
            &self.status,
            &self.activate,
            &self.ack,
            &self.exit,
            &self.done,
        ] {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub(crate) fn spawn() -> std::io::Result<ChildTarget> {
    let root = std::env::temp_dir().join(format!(
        "buddy-shell-preview-fullscreen-{}",
        std::process::id()
    ));
    let ready = root.with_extension("ready");
    let status = root.with_extension("status");
    let activate = root.with_extension("activate");
    let ack = root.with_extension("ack");
    let exit = root.with_extension("exit");
    let done = root.with_extension("done");
    for path in [&ready, &status, &activate, &ack, &exit, &done] {
        let _ = std::fs::remove_file(path);
    }
    let child = Command::new(std::env::current_exe()?)
        .arg("--fullscreen-target")
        .env(READY_ENV, &ready)
        .env(STATUS_ENV, &status)
        .env(ACTIVATE_ENV, &activate)
        .env(ACK_ENV, &ack)
        .env(EXIT_ENV, &exit)
        .env(DONE_ENV, &done)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(ChildTarget {
        child,
        ready,
        status,
        activate,
        ack,
        exit,
        done,
    })
}
