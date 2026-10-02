//! T48 专用的外部点击目标进程。
//!
//! 主测试只启动本示例自身的 child，不触碰用户的其他应用。窗口位置固定在主屏左上
//! 的安全区域，父进程再用 CGEventPost 点击该窗口，以触发真实 AppKit 激活变化。

use buddy_ui::gpui::{
    App, AppContext, Bounds, Focusable, Hsla, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Render, Styled, WindowBounds, WindowOptions, div, point, px,
    size,
};
use buddy_ui::gpui_platform::application;
use buddy_ui::text_area::{TextArea, TextAreaStyle};
use buddy_ui::theme_system::{BuddyTheme, tokens};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub(crate) const TARGET_X: f32 = 120.0;
pub(crate) const TARGET_Y: f32 = 120.0;
pub(crate) const TARGET_WIDTH: f32 = 240.0;
pub(crate) const TARGET_HEIGHT: f32 = 120.0;
/// 外部 child 中真实可选中的文本；候选内容不会由测试进程直接写入剪贴板。
pub(crate) const SELECTION_TEXT: &str = "  T49 外部应用选区文本  \n";

struct ExternalTarget {
    input: buddy_ui::gpui::Entity<TextArea>,
    ack: Option<PathBuf>,
}

pub(crate) struct ChildTarget {
    pub(crate) child: Child,
    pub(crate) ready: PathBuf,
    pub(crate) ack: PathBuf,
    pub(crate) selected_ack: PathBuf,
}

impl Drop for ChildTarget {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.ready);
        let _ = std::fs::remove_file(&self.ack);
        let _ = std::fs::remove_file(&self.selected_ack);
    }
}

impl Render for ExternalTarget {
    fn render(
        &mut self,
        _: &mut buddy_ui::gpui::Window,
        _: &mut buddy_ui::gpui::Context<Self>,
    ) -> impl IntoElement {
        // The click ACK is emitted by the target window itself. The text is a real GPUI
        // TextArea, so Cmd+A/C goes through the same input and clipboard path as an external app.
        let ack = self.ack.clone();
        div()
            .size_full()
            .p(px(tokens::metrics::SPACE_4))
            .on_mouse_down(MouseButton::Left, move |_: &MouseDownEvent, _, _| {
                if let Some(path) = ack.as_ref() {
                    let _ = std::fs::write(path, "ACK");
                }
            })
            .child(self.input.clone())
    }
}

fn options(cx: &App) -> WindowOptions {
    let bounds = Bounds::new(
        point(px(TARGET_X), px(TARGET_Y)),
        size(px(TARGET_WIDTH), px(TARGET_HEIGHT)),
    );
    let _ = cx;
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        focus: true,
        show: true,
        ..Default::default()
    }
}

/// 由 shell_preview 主入口在 `--external-target` 下调用。
pub(crate) fn run_child() -> ! {
    application().run(|cx: &mut App| {
        buddy_ui::shell::init(cx);
        buddy_ui::theme_system::Theme::install(buddy_ui::theme_system::Appearance::Light, cx);
        let ready = std::env::var_os("BUDDY_SHELL_EXTERNAL_READY").map(PathBuf::from);
        let ack = std::env::var_os("BUDDY_SHELL_EXTERNAL_ACK").map(PathBuf::from);
        let selected_ack = std::env::var_os("BUDDY_SHELL_EXTERNAL_SELECTED_ACK").map(PathBuf::from);
        let theme = *cx.buddy_theme();
        let colors = theme.colors;
        let style = TextAreaStyle {
            font_size: px(tokens::metrics::FONT_SIZE_MD),
            line_height: px(tokens::metrics::SPACE_5),
            max_height: None,
            min_height: px(tokens::metrics::SPACE_8),
            text_color: colors.text_primary.into(),
            placeholder_color: colors.text_tertiary.into(),
            caret_color: colors.text_primary.into(),
            selection_color: Hsla::from(colors.buddy_primary).opacity(0.25),
        };
        let input = cx.new(|cx| {
            let mut input = TextArea::new("外部选区", style, cx);
            input.set_text(SELECTION_TEXT, cx);
            input
        });
        let handle = cx
            .open_window(options(cx), {
                let input = input.clone();
                let ack = ack.clone();
                move |_, cx| cx.new(|_| ExternalTarget { input, ack })
            })
            .ok();
        cx.activate(true);
        if let (Some(path), Some(selected_path), Some(handle)) = (ready, selected_ack, handle) {
            let focus = input.read(cx).focus_handle(cx);
            let _ = cx.update_window(handle.into(), |_, window, cx| {
                window.focus(&focus, cx);
                window.activate_window();
            });
            // `activate(true)` only requests activation. Poll the native window until AppKit
            // reports a live key window, so READY never lets the parent race the activation.
            cx.spawn(async move |cx| {
                let mut ready_written = false;
                for _ in 0..150 {
                    let state = cx.update_window(handle.into(), |_, window, _| {
                        let bounds = window.bounds();
                        if !window.is_window_active() {
                            window.activate_window();
                        }
                        let key = window.is_window_active();
                        (bounds, key)
                    });
                    if let Ok((bounds, true)) = state {
                        let _ = std::fs::write(
                            &path,
                            format!(
                                "READY {} {} {} {} KEY",
                                f32::from(bounds.origin.x),
                                f32::from(bounds.origin.y),
                                f32::from(bounds.size.width),
                                f32::from(bounds.size.height)
                            ),
                        );
                        ready_written = true;
                        break;
                    }
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(20))
                        .await;
                }
                if !ready_written {
                    return;
                }
                for _ in 0..150 {
                    let selected = cx.update(|app| {
                        input.read(app).selected_range_for_test() == (0..SELECTION_TEXT.len())
                    });
                    if selected {
                        let _ = std::fs::write(&selected_path, "SELECTED");
                        break;
                    }
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(20))
                        .await;
                }
            })
            .detach();
        }
    });
    std::process::exit(0)
}

/// 启动同一个示例的专用 child。父测试负责在结束时 kill/wait。
pub(crate) fn spawn() -> std::io::Result<ChildTarget> {
    let root = std::env::temp_dir().join(format!(
        "buddy-shell-preview-external-{}",
        std::process::id()
    ));
    let ready = root.with_extension("ready");
    let ack = root.with_extension("ack");
    let selected_ack = root.with_extension("selected");
    let _ = std::fs::remove_file(&ready);
    let _ = std::fs::remove_file(&ack);
    let _ = std::fs::remove_file(&selected_ack);
    let child = Command::new(std::env::current_exe()?)
        .arg("--external-target")
        .env("BUDDY_SHELL_EXTERNAL_READY", &ready)
        .env("BUDDY_SHELL_EXTERNAL_ACK", &ack)
        .env("BUDDY_SHELL_EXTERNAL_SELECTED_ACK", &selected_ack)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(ChildTarget {
        child,
        ready,
        ack,
        selected_ack,
    })
}
