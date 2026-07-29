use super::{bounded_usize, include_hidden, is_hidden, relative_display, required_path};
use crate::tools::{Tool, ToolContext, ToolError, ToolOutput, ToolSafety};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::fs;

const DEFAULT_MAX_ENTRIES: usize = 200;
const MAX_ENTRIES: usize = 1_000;
const MAX_DEPTH: usize = 50;
const MAX_SCANNED_ENTRIES: usize = 20_000;

#[derive(Serialize)]
struct DirectoryEntry {
    path: String,
    entry_type: &'static str,
    size: Option<u64>,
}

#[derive(Serialize)]
struct DirectoryListing {
    root: String,
    entries: Vec<DirectoryEntry>,
    truncated: bool,
}

pub struct ListDirectoryTool;

#[async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "列出本地目录中的文件和子目录。默认只列一层,可设置递归深度;不会跟随符号链接。"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要浏览的目录路径"
                },
                "max_depth": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_DEPTH,
                    "default": 1,
                    "description": "递归层数,1 表示只列出当前目录"
                },
                "max_entries": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_ENTRIES,
                    "default": DEFAULT_MAX_ENTRIES,
                    "description": "最多返回的条目数量"
                },
                "include_hidden": {
                    "type": "boolean",
                    "default": false,
                    "description": "是否包含名称以点开头的隐藏文件"
                }
            },
            "required": ["path"]
        })
    }

    fn safety(&self) -> ToolSafety {
        ToolSafety::ReadOnly
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let root = required_path(&args)?;
        let max_depth = bounded_usize(&args, "max_depth", 1, 1, MAX_DEPTH)?;
        let max_entries = bounded_usize(&args, "max_entries", DEFAULT_MAX_ENTRIES, 1, MAX_ENTRIES)?;
        let include_hidden = include_hidden(&args);

        let metadata = fs::metadata(&root).await?;
        if !metadata.is_dir() {
            return Err(ToolError::InvalidArgs(format!(
                "目标不是目录: {}",
                root.display()
            )));
        }

        let mut entries = Vec::new();
        let mut pending = vec![(root.clone(), 1usize)];
        let mut scanned_entries = 0usize;
        let mut truncated = false;

        while let Some((directory, depth)) = pending.pop() {
            let mut reader = fs::read_dir(&directory).await?;
            let mut children = Vec::new();

            while let Some(entry) = reader.next_entry().await? {
                scanned_entries += 1;
                if scanned_entries > MAX_SCANNED_ENTRIES {
                    truncated = true;
                    break;
                }

                let path = entry.path();
                if !include_hidden && is_hidden(&path) {
                    continue;
                }

                let file_type = entry.file_type().await?;
                let (entry_type, size) = if file_type.is_dir() {
                    ("directory", None)
                } else if file_type.is_file() {
                    let size = entry.metadata().await.ok().map(|metadata| metadata.len());
                    ("file", size)
                } else if file_type.is_symlink() {
                    ("symlink", None)
                } else {
                    ("other", None)
                };

                entries.push(DirectoryEntry {
                    path: relative_display(&root, &path),
                    entry_type,
                    size,
                });

                if entries.len() >= max_entries {
                    truncated = true;
                    break;
                }
                if file_type.is_dir() && depth < max_depth {
                    children.push(path);
                }
            }

            if truncated {
                break;
            }
            children.sort_by(|left, right| right.cmp(left));
            pending.extend(children.into_iter().map(|path| (path, depth + 1)));
        }

        entries.sort_by(|left, right| left.path.cmp(&right.path));
        let output = DirectoryListing {
            root: root.to_string_lossy().to_string(),
            entries,
            truncated,
        };
        Ok(ToolOutput::ok(serde_json::to_string_pretty(&output)?))
    }
}
