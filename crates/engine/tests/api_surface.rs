//! v1 IPC 命令 → engine 入口的编译期覆盖检查。
//!
//! `_every_v1_command_has_an_engine_entry` 从不运行，只要求能编译：
//! 任一 engine 入口改名或改签名都会让本文件编译失败。

mod common;

use buddy_engine::chat::ChatEngine;
use buddy_engine::models::{AppConfig, ImageAttachment, Message};
use buddy_engine::streaming::StreamEventEmitter;
use std::sync::Arc;
use tempfile::TempDir;

#[allow(dead_code, clippy::too_many_arguments)]
async fn _every_v1_command_has_an_engine_entry(
    e: Arc<ChatEngine>,
    emitter: StreamEventEmitter,
    messages: Vec<Message>,
    config: AppConfig,
    image: ImageAttachment,
    message: Message,
) {
    let _: Result<(), String> = e.send_message(emitter, messages, "m".into()).await; // send_message
    let _: () = e.stop_generation(); // stop_generation
    let _: Result<(), String> = e.approve_tool_call("id", true, false); // approve_tool_call
    let _: Result<(), String> = e.answer_tool_question("id", vec![0], None, None); // answer_tool_question
    let _: Result<AppConfig, String> = e.get_config().await; // get_config
    let _: Result<(), String> = e.save_config(config).await; // save_config（热键注册属于应用外壳）
    let _ = e.fetch_models("u".into(), "k".into(), None).await; // fetch_models
    let _ = e.test_latency("u".into(), "k".into(), "m".into(), None).await; // test_latency
    let _: Result<Vec<Message>, String> = e.load_messages(0, 1).await; // load_messages
    let _: Result<u64, String> = e.get_message_count().await; // get_message_count
    let _: Result<(), String> = e.save_message(message).await; // save_message
    let _: Result<ImageAttachment, String> =
        e.save_chat_image("a.png".into(), "image/png".into(), String::new()).await; // save_chat_image
    let _: Result<bool, String> = e.delete_chat_image(String::new()).await; // delete_chat_image
    let _: Result<String, String> = e.download_generated_image(image).await; // download_generated_image
    // resize_window_to_page / log_window_frontend_diagnostic → 窗口外壳，不在 engine
}

/// 原 `rust-data-models.md` 约定（移交）：保存时默认模型必须存在于模型列表
#[tokio::test]
async fn save_config_rejects_unknown_selected_model() {
    let tmp = TempDir::new().unwrap();
    let engine = ChatEngine::new(tmp.path().to_path_buf());

    let mut config = AppConfig::default();
    config.selected_model_id = "p::不存在".into();
    assert_eq!(
        engine.save_config(config).await.unwrap_err(),
        "选择的模型不在可用模型列表中"
    );
    assert!(!tmp.path().join("config.json").exists(), "校验失败不得写盘");

    let mut ok = AppConfig::default();
    ok.selected_model_id = String::new();
    engine.save_config(ok).await.unwrap();
    assert!(tmp.path().join("config.json").exists());
}
