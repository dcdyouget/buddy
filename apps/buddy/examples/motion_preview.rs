//! Buddy 动效预览：使用真实 AppShell / 页面 / 会话，但不访问用户数据或模型。
//!
//! ```text
//! cargo run -p buddy-app --example motion_preview
//! ```
//!
//! 预览会自动播放：气泡入场 → 输入草稿 → 展开对话 → 模拟流式回答 → 流光收起 → 再次呼出。
//! 可用 `--multiline`、`--fast`、`--dark` 调整草稿、回复节奏与外观；`--loop` 重播，`--once-exit` 播放后退出。

use buddy_engine::chat::ChatEngine;
use buddy_engine::streaming::{StopReason, StreamEvent};
use buddy_ui::chat::state::user_message;
use buddy_ui::gpui::{App, AsyncApp, WindowHandle};
use buddy_ui::gpui_platform::application;
use buddy_ui::icons::Assets;
use buddy_ui::shell::{self, AppShell};
use buddy_ui::theme_system::{self, Appearance};
use std::path::PathBuf;
use std::time::Duration;

const MODEL_ID: &str = "motion-preview-model";
const RESPONSE: &str =
    "这是一段离线模拟的流式回复。文字会以平稳的节奏出现，配合轻微流光，让界面保持有呼吸感。";
const DRAFT: &str = "把这段界面动效做得更有呼吸感";
const MULTILINE_DRAFT: &str = "把这段界面动效做得更有呼吸感，强调输入内容连续变形成光点，再落到对话气泡里。\n\n请保留自然的换行，并避免发光太强或节奏太急。\n如果需要，可以给出两种克制的方案并说明各自适合的场景。";

#[derive(Clone, Copy, Default)]
struct PreviewOptions {
    loop_demo: bool,
    once_exit: bool,
    multiline: bool,
    fast: bool,
    dark: bool,
}

impl PreviewOptions {
    fn from_args() -> Self {
        let mut options = Self::default();
        for argument in std::env::args().skip(1) {
            match argument.as_str() {
                "--loop" => options.loop_demo = true,
                "--once-exit" => options.once_exit = true,
                "--multiline" => options.multiline = true,
                "--fast" => options.fast = true,
                "--dark" => options.dark = true,
                _ => {}
            }
        }
        options
    }

    fn draft(self) -> &'static str {
        if self.multiline {
            MULTILINE_DRAFT
        } else {
            DRAFT
        }
    }
}

fn main() {
    let options = PreviewOptions::from_args();
    application().with_assets(Assets).run(move |cx: &mut App| {
        shell::init(cx);
        let mut sandbox = PreviewSandbox::new();
        let engine = ChatEngine::new(sandbox.path().to_path_buf());
        cx.spawn(async move |cx: &mut AsyncApp| {
            let handle =
                match shell::open_main_window(engine, shell::config::ShellConfig::default(), cx)
                    .await
                {
                    Ok(handle) => handle,
                    Err(error) => {
                        eprintln!("motion preview window failed: {error}");
                        quit_app(cx);
                        return;
                    }
                };

            // The router installs the saved appearance while opening the window.
            // Override it afterward so --dark affects only this preview process.
            if options.dark {
                cx.update(|cx| theme_system::set_appearance(Appearance::Dark, cx));
            }

            // open_main_window creates and shows the native panel, while the
            // normal runtime path owns focus and the entrance sheen.
            #[cfg(target_os = "macos")]
            if let Err(error) = shell::runtime::show(handle, cx).await {
                eprintln!("motion preview show failed: {error}");
                quit_app(cx);
                return;
            }

            loop {
                if let Err(error) = play_once(handle, options, cx).await {
                    eprintln!("motion preview failed: {error}");
                    quit_app(cx);
                    break;
                }
                if options.once_exit {
                    sandbox.cleanup();
                    quit_app(cx);
                    break;
                }
                if !options.loop_demo {
                    break;
                }
                sleep(cx, Duration::from_millis(1_200)).await;
            }
        })
        .detach();
    });
}

/// Isolated engine storage with RAII cleanup. The preview injects all chat
/// state in memory, but keeping a real private directory makes the engine
/// boundary behave exactly like production if a future callback writes to it.
struct PreviewSandbox {
    path: PathBuf,
}

impl PreviewSandbox {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "buddy-motion-preview-{}-{nonce}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&path);
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn cleanup(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

impl Drop for PreviewSandbox {
    fn drop(&mut self) {
        self.cleanup();
    }
}

async fn play_once(
    handle: WindowHandle<AppShell>,
    options: PreviewOptions,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    // Returning to Empty emits the normal compact resize target. It is a no-op
    // on the first pass and makes --loop replay the bubble choreography too.
    reset_compact(&handle, cx)?;
    sleep(cx, Duration::from_millis(420)).await;

    // Let the compact shell settle before showing the typing and dialog changes.
    sleep(cx, Duration::from_millis(650)).await;

    set_draft(&handle, options.draft(), cx)?;
    sleep(cx, Duration::from_millis(900)).await;

    // This is the same transition used by EmptyPage. Keeping it on PageRouter
    // means the shell performs its normal conversation resize and sheen pass.
    expand_conversation(&handle, cx)?;
    sleep(cx, Duration::from_millis(750)).await;

    begin_preview_stream(&handle, cx)?;
    play_response(&handle, options.fast, cx).await?;
    sleep(cx, Duration::from_millis(2_200)).await;

    // Exercise the actual close / reopen path so the short exit sheen is visible.
    shell::runtime::hide(handle, cx).await?;
    sleep(cx, Duration::from_millis(620)).await;
    shell::runtime::show(handle, cx).await?;
    sleep(cx, Duration::from_millis(1_100)).await;
    Ok(())
}

async fn play_response(
    handle: &WindowHandle<AppShell>,
    fast: bool,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    if fast {
        push_events(
            handle,
            vec![
                StreamEvent::Start,
                StreamEvent::TextStart { content_index: 0 },
                StreamEvent::TextDelta {
                    content_index: 0,
                    delta: RESPONSE.to_string(),
                },
                StreamEvent::TextEnd {
                    content_index: 0,
                    content: RESPONSE.to_string(),
                },
                StreamEvent::Done {
                    reason: StopReason::Stop,
                    full_text: RESPONSE.to_string(),
                },
            ],
            true,
            cx,
        )?;
        return Ok(());
    }

    sleep(cx, Duration::from_millis(130)).await;
    push_events(
        handle,
        vec![
            StreamEvent::Start,
            StreamEvent::TextStart { content_index: 0 },
        ],
        false,
        cx,
    )?;

    // Small chunks expose the real Conversation pacer and assistant text sheen.
    let characters = RESPONSE.chars().collect::<Vec<_>>();
    for chunk in characters.chunks(2) {
        push_events(
            handle,
            vec![StreamEvent::TextDelta {
                content_index: 0,
                delta: chunk.iter().collect(),
            }],
            false,
            cx,
        )?;
        sleep(cx, Duration::from_millis(48)).await;
    }
    push_events(
        handle,
        vec![
            StreamEvent::TextEnd {
                content_index: 0,
                content: RESPONSE.to_string(),
            },
            StreamEvent::Done {
                reason: StopReason::Stop,
                full_text: RESPONSE.to_string(),
            },
        ],
        false,
        cx,
    )
}

fn set_draft(handle: &WindowHandle<AppShell>, text: &str, cx: &mut AsyncApp) -> Result<(), String> {
    handle
        .update(cx, |shell, _, cx| {
            shell.router().update(cx, |router, cx| {
                router
                    .composer()
                    .update(cx, |composer, cx| composer.set_draft(text, cx));
            })
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn expand_conversation(handle: &WindowHandle<AppShell>, cx: &mut AsyncApp) -> Result<(), String> {
    handle
        .update(cx, |shell, _, cx| {
            shell
                .router()
                .update(cx, |router, cx| router.expand_conversation(cx));
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn begin_preview_stream(handle: &WindowHandle<AppShell>, cx: &mut AsyncApp) -> Result<(), String> {
    handle
        .update(cx, |shell, _, cx| {
            shell.router().update(cx, |router, cx| {
                let draft = router.composer().read(cx).draft(cx);
                let user = user_message(&draft);
                router
                    .chat_page()
                    .update(cx, |chat, cx| chat.animate_send(user.id.clone(), cx));
                // The preview has shown the draft as user input; a send clears
                // it just like Composer::send does in the product path.
                router
                    .composer()
                    .update(cx, |composer, cx| composer.set_draft("", cx));
                router.conversation().update(cx, |conversation, cx| {
                    conversation.begin_send(user, MODEL_ID, cx);
                });
            })
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn reset_compact(handle: &WindowHandle<AppShell>, cx: &mut AsyncApp) -> Result<(), String> {
    handle
        .update(cx, |shell, _, cx| {
            shell
                .router()
                .update(cx, |router, cx| router.invoked_after_idle(cx));
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn push_events(
    handle: &WindowHandle<AppShell>,
    events: Vec<StreamEvent>,
    flush: bool,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    handle
        .update(cx, |shell, _, cx| {
            shell.router().update(cx, |router, cx| {
                router.conversation().update(cx, |conversation, cx| {
                    conversation.apply_events(events, cx);
                    if flush {
                        let now = conversation.now_ms();
                        conversation.state.flush(now);
                        cx.notify();
                    }
                });
            });
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

async fn sleep(cx: &mut AsyncApp, duration: Duration) {
    cx.background_executor().timer(duration).await;
}

fn quit_app(cx: &mut AsyncApp) {
    cx.update(|cx| cx.quit());
}
