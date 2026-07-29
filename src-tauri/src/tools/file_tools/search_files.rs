use super::{
    bounded_usize, include_hidden, is_hidden, is_ignored_directory, relative_display,
    required_path, required_string,
};
use crate::tools::{Tool, ToolContext, ToolError, ToolOutput, ToolSafety};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::fs;

const DEFAULT_MAX_RESULTS: usize = 50;
const MAX_RESULTS: usize = 200;
const DEFAULT_MAX_DEPTH: usize = 20;
const MAX_DEPTH: usize = 50;
const MAX_SCANNED_ENTRIES: usize = 20_000;
const MAX_SEARCH_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PREVIEW_CHARS: usize = 240;

fn truncate_preview(value: &str) -> String {
    let mut chars = value.chars();
    let preview: String = chars.by_ref().take(MAX_PREVIEW_CHARS).collect();
    if chars.next().is_some() {
        format!("{}…", preview)
    } else {
        preview
    }
}

#[derive(Clone, Copy)]
enum SearchMode {
    Name,
    Content,
    Both,
}

impl SearchMode {
    fn from_args(args: &Value) -> Result<Self, ToolError> {
        match args.get("mode").and_then(Value::as_str).unwrap_or("both") {
            "name" => Ok(Self::Name),
            "content" => Ok(Self::Content),
            "both" => Ok(Self::Both),
            value => Err(ToolError::InvalidArgs(format!(
                "'mode' 不支持 '{}',只能使用 name、content 或 both",
                value
            ))),
        }
    }

    fn searches_name(self) -> bool {
        matches!(self, Self::Name | Self::Both)
    }

    fn searches_content(self) -> bool {
        matches!(self, Self::Content | Self::Both)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Content => "content",
            Self::Both => "both",
        }
    }
}

#[derive(Serialize)]
struct FileSearchMatch {
    path: String,
    match_type: &'static str,
    line: Option<usize>,
    preview: String,
}

#[derive(Serialize)]
struct FileSearchOutput {
    root: String,
    query: String,
    mode: &'static str,
    matches: Vec<FileSearchMatch>,
    scanned_files: usize,
    skipped_files: usize,
    truncated: bool,
}

pub struct SearchFilesTool;

#[async_trait]
impl Tool for SearchFilesTool {
    fn name(&self) -> &str {
        "search_files"
    }

    fn description(&self) -> &str {
        "在本地目录中按文件名或文本内容搜索。默认跳过隐藏目录、依赖目录、构建目录、二进制文件和超过 2 MB 的文件。"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要搜索的目录或单个文件路径"
                },
                "query": {
                    "type": "string",
                    "description": "不区分大小写的搜索文本"
                },
                "mode": {
                    "type": "string",
                    "enum": ["name", "content", "both"],
                    "default": "both",
                    "description": "搜索文件名、文件内容或两者"
                },
                "max_results": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_RESULTS,
                    "default": DEFAULT_MAX_RESULTS,
                    "description": "最多返回的匹配数量"
                },
                "max_depth": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_DEPTH,
                    "default": DEFAULT_MAX_DEPTH,
                    "description": "目录递归层数"
                },
                "include_hidden": {
                    "type": "boolean",
                    "default": false,
                    "description": "是否搜索隐藏文件和常见依赖、构建目录"
                }
            },
            "required": ["path", "query"]
        })
    }

    fn safety(&self) -> ToolSafety {
        ToolSafety::ReadOnly
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let root = required_path(&args)?;
        let query = required_string(&args, "query")?;
        if query.is_empty() {
            return Err(ToolError::InvalidArgs("'query' 不能为空".to_string()));
        }

        let mode = SearchMode::from_args(&args)?;
        let max_results = bounded_usize(&args, "max_results", DEFAULT_MAX_RESULTS, 1, MAX_RESULTS)?;
        let max_depth = bounded_usize(&args, "max_depth", DEFAULT_MAX_DEPTH, 1, MAX_DEPTH)?;
        let include_hidden = include_hidden(&args);

        let metadata = fs::metadata(&root).await?;
        let mut pending = if metadata.is_dir() {
            vec![(root.clone(), 1usize)]
        } else if metadata.is_file() {
            Vec::new()
        } else {
            return Err(ToolError::InvalidArgs(format!(
                "目标不是文件或目录: {}",
                root.display()
            )));
        };
        let mut files = if metadata.is_file() {
            vec![root.clone()]
        } else {
            Vec::new()
        };
        let mut scanned_entries = files.len();
        let mut truncated = false;

        while let Some((directory, depth)) = pending.pop() {
            let mut reader = fs::read_dir(&directory).await?;
            let mut child_directories = Vec::new();
            while let Some(entry) = reader.next_entry().await? {
                scanned_entries += 1;
                if scanned_entries > MAX_SCANNED_ENTRIES {
                    truncated = true;
                    break;
                }

                let path = entry.path();
                if !include_hidden && (is_hidden(&path) || is_ignored_directory(&path)) {
                    continue;
                }
                let file_type = entry.file_type().await?;
                if file_type.is_file() {
                    files.push(path);
                } else if file_type.is_dir() && depth < max_depth {
                    child_directories.push(path);
                }
            }
            if truncated {
                break;
            }
            child_directories.sort_by(|left, right| right.cmp(left));
            pending.extend(child_directories.into_iter().map(|path| (path, depth + 1)));
        }

        files.sort();
        let query_lower = query.to_lowercase();
        let mut matches = Vec::new();
        let mut scanned_files = 0usize;
        let mut skipped_files = 0usize;

        for path in files {
            if matches.len() >= max_results {
                truncated = true;
                break;
            }
            scanned_files += 1;
            let relative_path = relative_display(&root, &path);

            if mode.searches_name() {
                let file_name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                if file_name.to_lowercase().contains(&query_lower) {
                    matches.push(FileSearchMatch {
                        path: relative_path.clone(),
                        match_type: "name",
                        line: None,
                        preview: file_name.to_string(),
                    });
                }
            }

            if !mode.searches_content() || matches.len() >= max_results {
                continue;
            }

            let metadata = match fs::metadata(&path).await {
                Ok(metadata) if metadata.len() <= MAX_SEARCH_FILE_BYTES => metadata,
                _ => {
                    skipped_files += 1;
                    continue;
                }
            };
            if metadata.len() == 0 {
                continue;
            }

            let bytes = match fs::read(&path).await {
                Ok(bytes) => bytes,
                Err(_) => {
                    skipped_files += 1;
                    continue;
                }
            };
            let content = match std::str::from_utf8(&bytes) {
                Ok(content) => content,
                Err(_) => {
                    skipped_files += 1;
                    continue;
                }
            };

            for (index, line) in content.lines().enumerate() {
                if line.to_lowercase().contains(&query_lower) {
                    matches.push(FileSearchMatch {
                        path: relative_path.clone(),
                        match_type: "content",
                        line: Some(index + 1),
                        preview: truncate_preview(line.trim()),
                    });
                    if matches.len() >= max_results {
                        truncated = true;
                        break;
                    }
                }
            }
        }

        let output = FileSearchOutput {
            root: root.to_string_lossy().to_string(),
            query,
            mode: mode.as_str(),
            matches,
            scanned_files,
            skipped_files,
            truncated,
        };
        Ok(ToolOutput::ok(serde_json::to_string_pretty(&output)?))
    }
}
