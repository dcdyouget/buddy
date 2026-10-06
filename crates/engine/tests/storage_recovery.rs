//! 存储恢复与损坏数据测试。

mod common;

use buddy_engine::storage;
use common::user_message;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn message(id: &str) -> buddy_engine::models::Message {
    let mut message = user_message(id);
    message.id = id.to_string();
    message
}

#[test]
fn actual_chunk_contents_override_stale_manifest_metadata() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let messages: Vec<_> = (0..3).map(|i| message(&format!("m{i}"))).collect();
    storage::append_messages(dir, &messages).unwrap();

    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "chunks": [{"file": "chunk_001.json", "count": 99}],
            "total_messages": 999
        }))
        .unwrap(),
    )
    .unwrap();

    let page = storage::load_messages(dir, 1, 1).unwrap();
    assert_eq!(page[0].id, "m1");
    assert_eq!(storage::message_count(dir).unwrap(), 3);

    storage::append_message(dir, &message("m3")).unwrap();
    let all = storage::load_messages(dir, 0, u64::MAX).unwrap();
    assert_eq!(
        all.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["m0", "m1", "m2", "m3"]
    );
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["total_messages"], 4);
    assert_eq!(manifest["chunks"][0]["count"], 4);
}

#[test]
fn chunk_written_before_manifest_is_recovered_and_not_hidden() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    storage::append_message(dir, &message("m0")).unwrap();

    // Simulate a crash after a chunk was atomically replaced but before manifest.json
    // was replaced. The extra chunk must remain visible and be used by the next append.
    let chunk = json!({
        "id": "chunk_002",
        "messages": [serde_json::to_value(message("m1")).unwrap()]
    });
    fs::write(
        dir.join("chunk_002.json"),
        serde_json::to_vec_pretty(&chunk).unwrap(),
    )
    .unwrap();

    assert_eq!(storage::message_count(dir).unwrap(), 2);
    storage::append_message(dir, &message("m2")).unwrap();
    let all = storage::load_messages(dir, 0, u64::MAX).unwrap();
    assert_eq!(
        all.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["m0", "m1", "m2"]
    );
}

#[test]
fn corrupt_chunk_is_reported_without_overwriting_existing_bytes() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    storage::append_message(dir, &message("m0")).unwrap();
    let chunk_path = dir.join("chunk_001.json");
    fs::write(&chunk_path, b"{broken").unwrap();
    let before = fs::read(&chunk_path).unwrap();

    assert!(storage::load_messages(dir, 0, 10).is_err());
    assert!(storage::message_count(dir).is_err());
    assert!(storage::append_message(dir, &message("m1")).is_err());
    assert_eq!(fs::read(&chunk_path).unwrap(), before);
}

#[test]
fn corrupt_manifest_is_reported_without_starting_from_empty_state() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    storage::append_message(dir, &message("m0")).unwrap();
    let chunk_before = fs::read(dir.join("chunk_001.json")).unwrap();
    fs::write(dir.join("manifest.json"), b"{broken").unwrap();

    assert!(storage::load_messages(dir, 0, 10).is_err());
    assert!(storage::message_count(dir).is_err());
    assert!(storage::append_message(dir, &message("m1")).is_err());
    assert_eq!(fs::read(dir.join("chunk_001.json")).unwrap(), chunk_before);
}

#[test]
fn missing_manifest_chunk_is_reported_instead_of_silently_skipped() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    storage::append_message(dir, &message("m0")).unwrap();
    fs::remove_file(dir.join("chunk_001.json")).unwrap();

    assert!(storage::load_messages(dir, 0, 10).is_err());
    assert!(storage::message_count(dir).is_err());
    assert!(storage::append_message(dir, &message("m1")).is_err());
}
