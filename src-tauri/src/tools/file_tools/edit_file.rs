use super::{required_path, required_string};
use crate::tools::builtin::check_write_allowed;
use crate::tools::{Tool, ToolContext, ToolError, ToolOutput, ToolSafety};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::io::AsyncWriteExt;

pub struct EditFileTool {
    pub allowed_paths: Vec<String>,
}

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "通过精确匹配 old_text 对现有 UTF-8 文本文件进行局部替换。默认要求 old_text 只出现一次,避免误改。"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要编辑的现有文本文件路径"
                },
                "old_text": {
                    "type": "string",
                    "description": "必须与文件内容完全一致的原文本"
                },
                "new_text": {
                    "type": "string",
                    "description": "替换后的文本;允许为空字符串以删除原文本"
                },
                "replace_all": {
                    "type": "boolean",
                    "default": false,
                    "description": "是否替换全部匹配;默认 false,且多处匹配时拒绝执行"
                }
            },
            "required": ["path", "old_text", "new_text"]
        })
    }

    fn safety(&self) -> ToolSafety {
        ToolSafety::Write
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let path = required_path(&args)?;
        check_write_allowed(&path, &self.allowed_paths)?;
        let old_text = required_string(&args, "old_text")?;
        let new_text = required_string(&args, "new_text")?;
        let replace_all = args
            .get("replace_all")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        if old_text.is_empty() {
            return Err(ToolError::InvalidArgs("'old_text' 不能为空".to_string()));
        }

        let metadata = fs::metadata(&path).await.map_err(|error| {
            ToolError::Io(match error.kind() {
                std::io::ErrorKind::NotFound => std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("文件不存在: {}", path.display()),
                ),
                _ => error,
            })
        })?;
        if !metadata.is_file() {
            return Err(ToolError::InvalidArgs(format!(
                "目标不是文件: {}",
                path.display()
            )));
        }

        let content = fs::read_to_string(&path).await?;
        let match_count = content.matches(&old_text).count();
        if match_count == 0 {
            return Err(ToolError::InvalidArgs(
                "未找到与 old_text 完全一致的内容,文件未修改".to_string(),
            ));
        }
        if match_count > 1 && !replace_all {
            return Err(ToolError::InvalidArgs(format!(
                "old_text 在文件中出现 {} 次;请提供更完整的上下文,或明确设置 replace_all=true",
                match_count
            )));
        }

        let updated = if replace_all {
            content.replace(&old_text, &new_text)
        } else {
            content.replacen(&old_text, &new_text, 1)
        };
        write_replacement(&path, updated.as_bytes(), metadata.permissions()).await?;

        Ok(ToolOutput::ok(format!(
            "已编辑文件: {} (替换 {} 处)",
            path.display(),
            if replace_all { match_count } else { 1 }
        )))
    }
}

async fn write_replacement(
    path: &Path,
    content: &[u8],
    permissions: std::fs::Permissions,
) -> Result<(), ToolError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = parent.join(format!(".{}.buddy-edit-{}.tmp", file_name, nonce));

    let result = async {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await?;
        file.write_all(content).await?;
        file.sync_all().await?;
        fs::set_permissions(&temporary, permissions).await?;
        fs::rename(&temporary, path).await?;
        Ok::<(), std::io::Error>(())
    }
    .await;

    if let Err(error) = result {
        let _ = fs::remove_file(&temporary).await;
        return Err(ToolError::Io(error));
    }
    Ok(())
}
