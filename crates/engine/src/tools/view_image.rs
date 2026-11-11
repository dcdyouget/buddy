use super::{Tool, ToolContext, ToolError, ToolOutput, ToolSafety};
use crate::models::ImageAttachment;
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use parking_lot::RwLock;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncReadExt;

/// 每轮对话独立持有附件索引；模型只能用附件 ID 查看已提供的图片。
pub(crate) struct ViewImageTool {
    data_dir: PathBuf,
    attachments: RwLock<HashMap<String, ImageAttachment>>,
}

impl ViewImageTool {
    pub(crate) fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            attachments: RwLock::new(HashMap::new()),
        }
    }

    /// 由编排层登记对话附件及本轮生成图，不接受模型指定的文件路径。
    pub(crate) fn remember_images(&self, images: &[ImageAttachment]) {
        let mut attachments = self.attachments.write();
        for image in images {
            let mut image = image.clone();
            image.data_url.clear();
            attachments.insert(image.id.clone(), image);
        }
    }

    async fn read_image(
        &self,
        mut image: ImageAttachment,
        max_bytes: usize,
    ) -> Result<ToolOutput, ToolError> {
        if image.path.is_empty() {
            return Err(ToolError::Other(
                "图片未保存到本地，无法查看，请重新添加图片".into(),
            ));
        }
        let root = self.data_dir.join("attachments");
        if fs::symlink_metadata(&root).await?.file_type().is_symlink() {
            return Err(ToolError::PermissionDenied(
                "图片附件目录不能是符号链接".into(),
            ));
        }
        let root = fs::canonicalize(root).await?;
        let path = fs::canonicalize(&image.path)
            .await
            .map_err(|_| ToolError::Other(format!("图片已删除或无法读取：{}", image.name)))?;
        if !path.starts_with(&root) || path == root {
            return Err(ToolError::PermissionDenied(
                "只能查看 Buddy 对话中的图片附件".into(),
            ));
        }
        let file = fs::File::open(&path).await?;
        if !file.metadata().await?.is_file() {
            return Err(ToolError::InvalidArgs("附件不是图片文件".into()));
        }
        let mut bytes = Vec::new();
        file.take(max_bytes.saturating_add(1) as u64)
            .read_to_end(&mut bytes)
            .await?;
        if bytes.len() > max_bytes {
            return Err(ToolError::Other(format!(
                "图片文件过大：{}（上限 {} 字节）",
                image.name, max_bytes
            )));
        }
        let media_type = image_media_type(&bytes).ok_or_else(|| {
            ToolError::Other("附件不是支持的图片格式，仅支持 PNG/JPEG/GIF/WebP".into())
        })?;
        image.media_type = media_type.to_string();
        image.path = path.to_string_lossy().into_owned();
        image.data_url = format!("data:{media_type};base64,{}", BASE64_STANDARD.encode(bytes));
        Ok(ToolOutput::ok(format!("已查看图片：{}", image.name))
            .with_images(vec![image.clone()])
            .with_model_images(vec![image]))
    }
}

#[async_trait]
impl Tool for ViewImageTool {
    fn name(&self) -> &str {
        "view_image"
    }

    fn description(&self) -> &str {
        "查看当前对话中的图片附件。仅在问题需要尚未提供的图片细节时使用，从 buddy_attachments 中选择 attachment_id。不能读取任意路径、文本文件或浏览目录。图片作为下一请求的视觉输入提供。"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "attachment_id": {
                    "type": "string",
                    "description": "buddy_attachments 中图片的 id"
                }
            },
            "required": ["attachment_id"],
            "additionalProperties": false
        })
    }

    fn safety(&self) -> ToolSafety {
        ToolSafety::ReadOnly
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolOutput, ToolError> {
        let attachment_id = args
            .get("attachment_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ToolError::InvalidArgs("请提供图片的 attachment_id，不能使用文件路径".into())
            })?;
        if !ctx.supports_vision {
            return Err(ToolError::Other(
                "当前模型不支持图片输入，请改用支持视觉的模型".into(),
            ));
        }
        let image = self
            .attachments
            .read()
            .get(attachment_id)
            .cloned()
            .ok_or_else(|| {
                ToolError::InvalidArgs("找不到此图片附件，请使用当前对话中提供的图片 ID".into())
            })?;
        let read = self.read_image(image, ctx.max_image_bytes.max(1));
        let Some(mut cancel_rx) = ctx.cancel_rx else {
            return read.await;
        };
        if *cancel_rx.borrow() {
            return Err(ToolError::Other("用户已取消查看图片".into()));
        }
        tokio::pin!(read);
        loop {
            tokio::select! {
                result = &mut read => return result,
                changed = cancel_rx.changed() => {
                    if *cancel_rx.borrow() {
                        return Err(ToolError::Other("用户已取消查看图片".into()));
                    }
                    if changed.is_err() {
                        return read.await;
                    }
                }
            }
        }
    }
}

fn image_media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
