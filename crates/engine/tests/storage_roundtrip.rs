//! S02-04：storage 在临时目录内的往返测试（无 Tauri）。

mod common;

use buddy_engine::models::{AppConfig, Message};
use buddy_engine::storage;
use tempfile::TempDir;

fn message(i: usize) -> Message {
    let mut m = common::user_message(&format!("第 {i} 条"));
    m.id = format!("m{i:04}");
    m.created_at = i as u64;
    m
}

#[test]
fn config_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    // 不存在时回退默认配置
    let default = storage::get_config(dir).unwrap();
    assert_eq!(
        serde_json::to_value(&default).unwrap(),
        serde_json::to_value(AppConfig::default()).unwrap()
    );

    let mut config = AppConfig::default();
    config.hotkey = "CmdOrCtrl+K".into();
    config.auto_start = !config.auto_start;
    storage::save_config(dir, &config).unwrap();
    assert!(dir.join("config.json").exists());

    let loaded = storage::get_config(dir).unwrap();
    assert_eq!(serde_json::to_value(&loaded).unwrap(), serde_json::to_value(&config).unwrap());
}

#[test]
fn messages_roundtrip_across_chunks() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    assert_eq!(storage::message_count(dir).unwrap(), 0);

    // 250 条：单条追加 1 条 + 批量 249 条，跨 3 个分块（100/100/50）
    storage::append_message(dir, &message(0)).unwrap();
    let batch: Vec<Message> = (1..250).map(message).collect();
    storage::append_messages(dir, &batch).unwrap();

    assert_eq!(storage::message_count(dir).unwrap(), 250);
    for f in ["chunk_001.json", "chunk_002.json", "chunk_003.json", "manifest.json"] {
        assert!(dir.join(f).exists(), "{f} 应存在");
    }
    assert!(!dir.join("chunk_004.json").exists());

    // 跨分块读取：offset=150, limit=30 → m0150..m0179
    let page = storage::load_messages(dir, 150, 30).unwrap();
    let ids: Vec<&str> = page.iter().map(|m| m.id.as_str()).collect();
    let expected: Vec<String> = (150..180).map(|i| format!("m{i:04}")).collect();
    assert_eq!(ids, expected);

    // 全量读回与写入逐条一致
    let all = storage::load_messages(dir, 0, 1000).unwrap();
    assert_eq!(all.len(), 250);
    for (i, m) in all.iter().enumerate() {
        assert_eq!(
            serde_json::to_value(m).unwrap(),
            serde_json::to_value(message(i)).unwrap()
        );
    }
}

#[test]
fn attachments_stay_inside_data_dir() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();

    let img = storage::store_image_bytes(dir, "a.png", "image/png", b"\x89PNG", "test").unwrap();
    let path = std::path::PathBuf::from(&img.path);
    assert!(path.starts_with(dir.join("attachments")), "{path:?}");
    assert_eq!(std::fs::read(&path).unwrap(), b"\x89PNG");
    assert!(img.data_url.is_empty(), "持久化附件不应携带 data_url");

    assert!(storage::delete_attachment_file(dir, &img.path).unwrap());
    assert!(!path.exists());

    // 目录外路径不得被删除（v1 的穿越防护原样保留）
    let outside = tmp.path().join("outside.txt");
    std::fs::write(&outside, "keep").unwrap();
    assert!(storage::delete_attachment_file(dir, outside.to_str().unwrap()).is_err());
    assert!(outside.exists());
}

#[cfg(target_os = "macos")]
#[test]
fn default_data_dir_matches_v1() {
    let dir = storage::default_data_dir().unwrap();
    let home = std::env::var("HOME").unwrap();
    assert_eq!(
        dir,
        std::path::Path::new(&home).join("Library/Application Support/com.buddy.chat")
    );
    println!("default_data_dir = {}", dir.display());
}

/// S01-06-10：长会话样本（`scripts/v1-baseline/gen_long_session.py` 生成）可被 v1 格式的 storage 完整读回。
/// 手动运行：先生成样本，再 `cargo test -p buddy-engine --test storage_roundtrip -- --ignored --nocapture`
#[test]
#[ignore = "需先运行 scripts/v1-baseline/gen_long_session.py"]
fn long_session_sample_loads() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/v1-baseline/long-session");
    let count = storage::message_count(&dir).unwrap();
    let all = storage::load_messages(&dir, 0, u64::MAX).unwrap();
    println!("long-session: manifest total={count} loaded={}", all.len());
    assert!(count >= 1000);
    assert_eq!(all.len() as u64, count, "每条消息都应能按 v1 格式反序列化");
    assert!(all.iter().any(|m| m.tool_calls.is_some()) && all.iter().any(|m| m.blocks.is_some()));
}

