//! T48 专用的外部点击目标进程。
//!
//! 主测试只启动本示例自身的 child，不触碰用户的其他应用。窗口位置固定在主屏左上
//! 的安全区域，父进程再用 CGEventPost 点击该窗口，以触发真实 AppKit 激活变化。

use buddy_ui::gpui::{
    App, AppContext, Bounds, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, Render,
    Styled, WindowBounds, WindowOptions, div, point, px, size,
};
use buddy_ui::gpui_platform::application;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub(crate) const TARGET_X: f32 = 120.0;
pub(crate) const TARGET_Y: f32 = 120.0;
pub(crate) const TARGET_WIDTH: f32 = 240.0;
pub(crate) const TARGET_HEIGHT: f32 = 120.0;

struct ExternalTarget {
    ack: Option<PathBuf>,
}

pub(crate) struct ChildTarget {
    pub(crate) child: Child,
    pub(crate) ready: PathBuf,
    pub(crate) ack: PathBuf,
}

impl Render for ExternalTarget {
    fn render(
        &mut self,
        _: &mut buddy_ui::gpui::Window,
        _: &mut buddy_ui::gpui::Context<Self>,
    ) -> impl IntoElement {
        // The native window itself is the target; an empty full-size element avoids introducing
        // product UI or hard-coded product colors into the preview.
        let ack = self.ack.clone();
        div()
            .size_full()
            .on_mouse_down(MouseButton::Left, move |_: &MouseDownEvent, _, _| {
                if let Some(path) = ack.as_ref() {
                    let _ = std::fs::write(path, "ACK");
                }
            })
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
        let ready = std::env::var_os("BUDDY_SHELL_EXTERNAL_READY").map(PathBuf::from);
        let ack = std::env::var_os("BUDDY_SHELL_EXTERNAL_ACK").map(PathBuf::from);
        let handle = cx
            .open_window(options(cx), |_, cx| cx.new(|_| ExternalTarget { ack }))
            .ok();
        if let Some(path) = ready {
            if let Some(handle) = handle {
                if let Ok(bounds) = cx.update_window(handle.into(), |_, window, _| window.bounds())
                {
                    let _ = std::fs::write(
                        path,
                        format!(
                            "READY {} {} {} {}",
                            f32::from(bounds.origin.x),
                            f32::from(bounds.origin.y),
                            f32::from(bounds.size.width),
                            f32::from(bounds.size.height)
                        ),
                    );
                }
            }
        }
        cx.activate(true);
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
    let _ = std::fs::remove_file(&ready);
    let _ = std::fs::remove_file(&ack);
    let child = Command::new(std::env::current_exe()?)
        .arg("--external-target")
        .env("BUDDY_SHELL_EXTERNAL_READY", &ready)
        .env("BUDDY_SHELL_EXTERNAL_ACK", &ack)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(ChildTarget { child, ready, ack })
}
