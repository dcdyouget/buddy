//! Buddy 动效预览：使用真实 AppShell / 页面 / 会话，但不访问用户数据或模型。
//!
//! ```text
//! cargo run -p buddy-app --example motion_preview
//! ```
//!
//! 预览会自动播放：气泡入场 → 输入草稿 → 展开对话 → 模拟流式回答 → 流光收起 → 再次呼出。
//! `--loop` 可在每次播放结束后重新开始整段流程。

use buddy_engine::chat::ChatEngine;
use buddy_engine::streaming::{StopReason, StreamEvent};
use buddy_ui::chat::state::user_message;
use buddy_ui::gpui::{App, AsyncApp, WindowHandle};
use buddy_ui::gpui_platform::application;
use buddy_ui::icons::Assets;
use buddy_ui::shell::{self, AppShell};
use std::path::PathBuf;
use std::time::Duration;

const MODEL_ID: &str = "motion-preview-model";
const RESPONSE: &str =
    "这是一段离线模拟的流式回复。文字会以平稳的节奏出现，配合轻微流光，让界面保持有呼吸感。";

fn main() {
    let loop_demo = std::env::args().any(|argument| argument == "--loop");
    let once_exit = std::env::args().any(|argument| argument == "--once-exit");
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

            // open_main_window creates and shows the native panel, while the
            // normal runtime path owns focus and the entrance sheen.
            #[cfg(target_os = "macos")]
            if let Err(error) = shell::runtime::show(handle, cx).await {
                eprintln!("motion preview show failed: {error}");
                quit_app(cx);
                return;
            }

            loop {
                if let Err(error) = play_once(handle, cx).await {
                    eprintln!("motion preview failed: {error}");
                    quit_app(cx);
                    break;
                }
                if once_exit {
                    sandbox.cleanup();
                    quit_app(cx);
                    break;
                }
                if !loop_demo {
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

async fn play_once(handle: WindowHandle<AppShell>, cx: &mut AsyncApp) -> Result<(), String> {
    // Returning to Empty emits the normal compact resize target. It is a no-op
    // on the first pass and makes --loop replay the bubble choreography too.
    reset_compact(&handle, cx)?;
    sleep(cx, Duration::from_millis(420)).await;

    // Let the compact shell settle before showing the typing and dialog changes.
    sleep(cx, Duration::from_millis(650)).await;

    set_draft(&handle, "把这段界面动效做得更有呼吸感", cx)?;
    sleep(cx, Duration::from_millis(900)).await;

    // This is the same transition used by EmptyPage. Keeping it on PageRouter
    // means the shell performs its normal conversation resize and sheen pass.
    expand_conversation(&handle, cx)?;
    sleep(cx, Duration::from_millis(750)).await;

    begin_preview_stream(&handle, cx)?;
    sleep(cx, Duration::from_millis(130)).await;
    push_events(
        &handle,
        vec![
            StreamEvent::Start,
            StreamEvent::TextStart { content_index: 0 },
        ],
        cx,
    )?;

    // Small chunks make the real Conversation pacer and assistant text sheen
    // visible without involving a provider or the network.
    let characters = RESPONSE.chars().collect::<Vec<_>>();
    for chunk in characters.chunks(2) {
        let delta = chunk.iter().collect::<String>();
        push_events(
            &handle,
            vec![StreamEvent::TextDelta {
                content_index: 0,
                delta,
            }],
            cx,
        )?;
        sleep(cx, Duration::from_millis(48)).await;
    }
    push_events(
        &handle,
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
        cx,
    )?;
    sleep(cx, Duration::from_millis(2_200)).await;

    // Exercise the actual close / reopen path so the short exit sheen is visible.
    shell::runtime::hide(handle, cx).await?;
    sleep(cx, Duration::from_millis(620)).await;
    shell::runtime::show(handle, cx).await?;
    sleep(cx, Duration::from_millis(1_100)).await;
    Ok(())
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
                // The preview has shown the draft as user input; a send clears
                // it just like Composer::send does in the product path.
                router
                    .composer()
                    .update(cx, |composer, cx| composer.set_draft("", cx));
                router.conversation().update(cx, |conversation, cx| {
                    conversation.begin_send(user_message("请展示一段安静的流光回复"), MODEL_ID, cx);
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
    cx: &mut AsyncApp,
) -> Result<(), String> {
    handle
        .update(cx, |shell, _, cx| {
            shell.router().update(cx, |router, cx| {
                router
                    .conversation()
                    .update(cx, |conversation, cx| conversation.apply_events(events, cx));
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
