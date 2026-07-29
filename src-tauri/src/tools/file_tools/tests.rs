use super::*;
use crate::tools::{Tool, ToolContext, ToolError};
use serde_json::{json, Value};
use std::path::Path;
use tempfile::TempDir;
use tokio::fs;

fn tool_args(path: &Path, extra: Value) -> Value {
    let mut args = json!({ "path": path.to_string_lossy() });
    if let (Some(target), Some(extra)) = (args.as_object_mut(), extra.as_object()) {
        target.extend(extra.clone());
    }
    args
}

#[tokio::test]
async fn list_directory_respects_depth_and_hidden_default() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("src/nested"))
        .await
        .unwrap();
    fs::write(temp.path().join("src/main.rs"), "fn main() {}")
        .await
        .unwrap();
    fs::write(temp.path().join("src/nested/deep.rs"), "deep")
        .await
        .unwrap();
    fs::write(temp.path().join(".secret"), "hidden")
        .await
        .unwrap();

    let output = ListDirectoryTool
        .execute(
            tool_args(temp.path(), json!({ "max_depth": 2 })),
            ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(output.content.contains("\"path\": \"src\""));
    assert!(output.content.contains("\"path\": \"src/main.rs\""));
    assert!(output.content.contains("\"path\": \"src/nested\""));
    assert!(!output.content.contains("deep.rs"));
    assert!(!output.content.contains(".secret"));
}

#[tokio::test]
async fn search_files_finds_names_and_content() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("src")).await.unwrap();
    fs::write(
        temp.path().join("src/BuddyService.rs"),
        "first line\nBuddy search content\n",
    )
    .await
    .unwrap();
    fs::write(temp.path().join("other.txt"), "nothing")
        .await
        .unwrap();

    let output = SearchFilesTool
        .execute(
            tool_args(temp.path(), json!({ "query": "buddy" })),
            ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(output.content.contains("\"match_type\": \"name\""));
    assert!(output.content.contains("\"match_type\": \"content\""));
    assert!(output.content.contains("\"line\": 2"));
}

#[tokio::test]
async fn search_files_skips_ignored_directories() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join("node_modules/pkg"))
        .await
        .unwrap();
    fs::write(
        temp.path().join("node_modules/pkg/index.js"),
        "hidden dependency match",
    )
    .await
    .unwrap();

    let output = SearchFilesTool
        .execute(
            tool_args(temp.path(), json!({ "query": "dependency" })),
            ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!output.content.contains("index.js"));
}

#[tokio::test]
async fn edit_file_replaces_unique_text() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("file.txt");
    fs::write(&path, "before old after").await.unwrap();
    let tool = EditFileTool {
        allowed_paths: vec![temp.path().to_string_lossy().to_string()],
    };

    let output = tool
        .execute(
            tool_args(&path, json!({ "old_text": "old", "new_text": "new" })),
            ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(output.content.contains("替换 1 处"));
    assert_eq!(fs::read_to_string(&path).await.unwrap(), "before new after");
}

#[tokio::test]
async fn edit_file_rejects_ambiguous_match() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("file.txt");
    fs::write(&path, "same same").await.unwrap();
    let tool = EditFileTool {
        allowed_paths: vec![temp.path().to_string_lossy().to_string()],
    };

    let result = tool
        .execute(
            tool_args(&path, json!({ "old_text": "same", "new_text": "changed" })),
            ToolContext::default(),
        )
        .await;

    assert!(matches!(result, Err(ToolError::InvalidArgs(_))));
    assert_eq!(fs::read_to_string(&path).await.unwrap(), "same same");
}

#[tokio::test]
async fn edit_file_replace_all_and_path_guard() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("file.txt");
    fs::write(&path, "same same").await.unwrap();
    let tool = EditFileTool {
        allowed_paths: vec![temp.path().to_string_lossy().to_string()],
    };

    tool.execute(
        tool_args(
            &path,
            json!({
                "old_text": "same",
                "new_text": "changed",
                "replace_all": true
            }),
        ),
        ToolContext::default(),
    )
    .await
    .unwrap();
    assert_eq!(fs::read_to_string(&path).await.unwrap(), "changed changed");

    let outside = temp.path().parent().unwrap().join("outside.txt");
    let denied = tool
        .execute(
            tool_args(&outside, json!({ "old_text": "x", "new_text": "y" })),
            ToolContext::default(),
        )
        .await;
    assert!(matches!(denied, Err(ToolError::PermissionDenied(_))));
}
