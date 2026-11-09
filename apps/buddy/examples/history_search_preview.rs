//! 使用隔离的示例历史检查搜索、侧滑、旧消息定位和草稿保留，不访问用户数据。
//! `cargo run -p buddy-app --example history_search_preview -- [--dark] [--standard]`
//! `--standard` 使用普通窗口承载相同页面，便于辅助功能工具操作。

use buddy_engine::{
    chat::ChatEngine,
    models::{AppConfig, MessageRole, Theme},
    storage,
};
use buddy_ui::{
    chat::{
        router::{PageRouter, preload},
        state::user_message,
    },
    chat_bridge::spawn_engine,
    gpui::{App, AppContext, AsyncApp, Bounds, Global, WindowBounds, WindowOptions},
    gpui_platform::application,
    icons::Assets,
    shell,
};
use std::path::PathBuf;

struct PreviewData(PathBuf);
impl Global for PreviewData {}
impl Drop for PreviewData {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn main() {
    let dark = std::env::args().any(|arg| arg == "--dark");
    let standard = std::env::args().any(|arg| arg == "--standard");
    let dir = std::env::temp_dir().join(format!("buddy-history-preview-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut config = AppConfig::default();
    if dark {
        config.theme = Theme::Dark;
    }
    storage::save_config(&dir, &config).unwrap();
    for ix in 0..120 {
        let question = match ix {
            0 => "Rust 的快捷键冲突怎么排查？",
            1 => {
                "Rust 的快捷键怎么设置？\n我希望能够修改默认组合键，并在保存后马上使用新的快捷键。\n如果快捷键已经被其他应用占用，希望能够看到明确的提示。"
            }
            2 => "怎样在 Windows 和 Mac 上分别处理 Rust 全局快捷键？",
            _ => "怎样让聊天窗口的阅读体验更自然？",
        };
        let mut message = user_message(question);
        message.id = format!("preview-question-{ix}");
        message.created_at = 1_793_000_000 + ix * 3600;
        storage::append_message(&dir, &message).unwrap();
        message.id = format!("preview-answer-{ix}");
        message.role = MessageRole::Assistant;
        message.content = if ix < 3 {
            "先检查当前组合键是否已经被占用，然后再修改注册设置。\n保存后可以重新唤起窗口，确认新组合键已经生效。\n\n这段较长的回答用于检查搜索摘录只展示前两行，点击后可以在原对话中继续阅读。"
        } else { "保留熟悉的输入框、圆角和文字层级，让每一步操作都能清楚地看到结果。" }.into();
        storage::append_message(&dir, &message).unwrap();
    }
    application().with_assets(Assets).run(move |cx: &mut App| {
        shell::init(cx);
        cx.set_global(PreviewData(dir.clone()));
        let engine = ChatEngine::new(dir.clone());
        cx.spawn(async move |cx: &mut AsyncApp| {
            if standard {
                let loaded = cx
                    .update(|cx| spawn_engine(cx, preload(engine.clone())))
                    .await;
                cx.update(|cx| {
                    let bounds =
                        Bounds::centered(None, shell::config::CONVERSATION_SIZE.to_gpui(), cx);
                    cx.open_window(
                        WindowOptions {
                            window_bounds: Some(WindowBounds::Windowed(bounds)),
                            ..Default::default()
                        },
                        |window, cx| {
                            window.set_window_title("Buddy 历史搜索预览");
                            cx.new(|cx| {
                                let mut router = PageRouter::new(engine, loaded, window, cx);
                                router.composer().update(cx, |composer, cx| {
                                    composer.set_draft("未发送的聊天草稿", cx)
                                });
                                router.expand_conversation(cx);
                                router
                            })
                        },
                    )
                    .unwrap();
                    cx.activate(true);
                });
                return;
            }
            let handle = shell::open_main_window(engine, shell::config::ShellConfig::default(), cx)
                .await
                .unwrap();
            shell::runtime::show(handle, cx).await.unwrap();
            handle
                .update(cx, |shell, _, cx| {
                    shell.router().update(cx, |router, cx| {
                        router.composer().update(cx, |composer, cx| {
                            composer.set_draft("未发送的聊天草稿", cx)
                        });
                        router.expand_conversation(cx);
                    })
                })
                .unwrap();
        })
        .detach();
    });
}
