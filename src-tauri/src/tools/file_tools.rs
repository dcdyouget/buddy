// ============================================================================
// 目录浏览、文件搜索与局部编辑工具
// ============================================================================

mod edit_file;
mod list_directory;
mod search_files;

pub use edit_file::EditFileTool;
pub use list_directory::ListDirectoryTool;
pub use search_files::SearchFilesTool;

use super::ToolError;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn required_string(args: &Value, key: &str) -> Result<String, ToolError> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| ToolError::InvalidArgs(format!("缺少 '{}' 字段或不是字符串", key)))
}

fn required_path(args: &Value) -> Result<PathBuf, ToolError> {
    let path = required_string(args, "path")?;
    if path.trim().is_empty() {
        return Err(ToolError::InvalidArgs("'path' 不能为空".to_string()));
    }
    Ok(PathBuf::from(path))
}

fn bounded_usize(
    args: &Value,
    key: &str,
    default: usize,
    minimum: usize,
    maximum: usize,
) -> Result<usize, ToolError> {
    let Some(value) = args.get(key) else {
        return Ok(default);
    };
    let value = value
        .as_u64()
        .ok_or_else(|| ToolError::InvalidArgs(format!("'{}' 必须是正整数", key)))?;
    usize::try_from(value)
        .ok()
        .filter(|value| (*value >= minimum) && (*value <= maximum))
        .ok_or_else(|| {
            ToolError::InvalidArgs(format!("'{}' 必须在 {} 到 {} 之间", key, minimum, maximum))
        })
}

fn include_hidden(args: &Value) -> bool {
    args.get("include_hidden")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with('.'))
        .unwrap_or(false)
}

fn is_ignored_directory(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git" | ".hg" | ".svn" | ".idea" | "node_modules" | "target" | "dist" | "build")
    )
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests;
