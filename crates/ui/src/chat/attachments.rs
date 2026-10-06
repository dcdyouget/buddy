//! Composer 图片附件的输入、校验与预览数据。
//!
//! 这里不负责写盘：图片先以 data URL 保存在 Composer 草稿中，再由 Composer
//! 经 `spawn_engine` 调用 engine 的 `save_chat_image`。这样切换空态 / 对话页时
//! 仍共用同一份草稿，且消息只带 engine 返回的路径附件。

use buddy_engine::models::{ImageAttachment, image_limit_message};
use gpui::{Image, ImageFormat};
use std::path::Path;
use std::sync::Arc;

/// 单条消息最多图片数（与 v1 InputDock 一致）。
pub const MAX_IMAGE_COUNT: usize = 4;

/// Composer 中尚未或正在写盘的一张图片。
#[derive(Clone)]
pub struct DraftImage {
    /// 发给 engine / 消息的附件元数据。
    pub attachment: ImageAttachment,
    /// GPUI 可直接绘制的缩略图数据；发送和切页期间保持在内存中。
    pub preview: Arc<Image>,
}

/// 将合法字节包装为未写盘的附件。
///
/// `max_bytes` 为当前服务商的单张上限（[`buddy_engine::models::ProviderConfig::max_image_bytes`]）。
pub fn draft_from_bytes(
    name: impl Into<String>,
    media_type: &str,
    bytes: Vec<u8>,
    max_bytes: usize,
) -> Result<DraftImage, String> {
    if !supported_media_type(media_type) {
        return Err("仅支持 JPEG、PNG、GIF 和 WebP 图片".into());
    }
    if bytes.len() > max_bytes {
        return Err(image_limit_message(max_bytes));
    }
    let name = name.into();
    let attachment = ImageAttachment {
        id: format!("draft-{}", unique_id()),
        name,
        media_type: media_type.to_string(),
        path: String::new(),
        data_url: format!("data:{media_type};base64,{}", encode_base64(&bytes)),
    };
    let format = ImageFormat::from_mime_type(media_type)
        .ok_or_else(|| "仅支持 JPEG、PNG、GIF 和 WebP 图片".to_string())?;
    Ok(DraftImage {
        attachment,
        preview: Arc::new(Image::from_bytes(format, bytes)),
    })
}

/// 从一个本地路径读取图片并生成草稿附件。
pub fn draft_from_path(path: &Path, max_bytes: usize) -> Result<DraftImage, String> {
    let media_type = media_type_for_path(path)
        .ok_or_else(|| "仅支持 JPEG、PNG、GIF 和 WebP 图片".to_string())?;
    let metadata =
        std::fs::metadata(path).map_err(|_| format!("无法读取图片：{}", path.display()))?;
    // 读文件前先按大小拒绝，超大图片不进内存。
    if metadata.len() > max_bytes as u64 {
        return Err(image_limit_message(max_bytes));
    }
    let bytes = std::fs::read(path).map_err(|_| format!("无法读取图片：{}", path.display()))?;
    draft_from_bytes(
        path.file_name().and_then(|n| n.to_str()).unwrap_or("图片"),
        media_type,
        bytes,
        max_bytes,
    )
}

/// 读取剪贴板中的图片。文本或不支持的条目返回 `None`，调用方继续走文本粘贴。
pub fn draft_from_clipboard_image(
    item: &gpui::ClipboardItem,
    max_bytes: usize,
) -> Result<Option<DraftImage>, String> {
    for entry in item.entries() {
        if let gpui::ClipboardEntry::Image(image) = entry {
            let media_type = image.format.mime_type();
            if !supported_media_type(media_type) {
                return Err("仅支持 JPEG、PNG、GIF 和 WebP 图片".into());
            }
            return draft_from_bytes(
                format!("粘贴图片.{}", image.format.extension()),
                media_type,
                image.bytes.clone(),
                max_bytes,
            )
            .map(Some);
        }
    }
    for entry in item.entries() {
        if let gpui::ClipboardEntry::ExternalPaths(paths) = entry {
            for path in paths.paths() {
                return draft_from_path(path, max_bytes).map(Some);
            }
        }
    }
    Ok(None)
}

/// v1 支持的图片 MIME 类型。
pub fn supported_media_type(media_type: &str) -> bool {
    matches!(
        media_type,
        "image/jpeg" | "image/png" | "image/gif" | "image/webp"
    )
}

/// 解码 engine 历史附件中兼容保留的 data URL。
pub fn decode_data_url(data_url: &str, media_type: &str) -> Option<Vec<u8>> {
    let prefix = format!("data:{media_type};base64,");
    data_url.strip_prefix(&prefix).and_then(decode_base64)
}

/// 从文件扩展名推断 MIME 类型；扩展名比较不区分大小写。
pub fn media_type_for_path(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// 发送消息时只保留已完成写盘的附件；草稿附件的 data URL 不应进入历史 JSON。
pub fn persisted_attachment(image: &DraftImage, stored: ImageAttachment) -> ImageAttachment {
    let mut stored = stored;
    stored.id = image.attachment.id.clone();
    stored.name = image.attachment.name.clone();
    stored.media_type = image.attachment.media_type.clone();
    // storage may receive a sandbox path containing `..`; canonicalize the saved
    // file before future delete / render calls, which reject parent-directory components.
    if let Ok(path) = std::fs::canonicalize(&stored.path) {
        stored.path = path.to_string_lossy().into_owned();
    }
    stored
}

fn unique_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let n = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{}-{n}", chrono::Utc::now().timestamp_millis())
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as usize;
        let b = chunk.get(1).copied().unwrap_or(0) as usize;
        let c = chunk.get(2).copied().unwrap_or(0) as usize;
        out.push(TABLE[a >> 2] as char);
        out.push(TABLE[((a & 3) << 4) | (b >> 4)] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((b & 15) << 2) | (c >> 6)] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[c & 63] as char
        } else {
            '='
        });
    }
    out
}

fn decode_base64(value: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(value.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u8;
    for byte in value.bytes() {
        let v = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\r' | b'\n' | b' ' | b'\t' => continue,
            _ => return None,
        } as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_v1_types_and_limit() {
        assert!(supported_media_type("image/jpeg"));
        assert!(supported_media_type("image/webp"));
        assert!(!supported_media_type("image/svg+xml"));
        assert_eq!(media_type_for_path(Path::new("A.JpG")), Some("image/jpeg"));
        let five_mb = 5 * 1024 * 1024;
        assert!(draft_from_bytes("x.png", "image/png", vec![0; five_mb], five_mb).is_ok());
        let error = draft_from_bytes("x.png", "image/png", vec![0; five_mb + 1], five_mb).err();
        assert_eq!(error.as_deref(), Some("单张图片不能超过 5 MB"));
        let ten_mb = 10 * 1024 * 1024;
        assert!(draft_from_bytes("x.png", "image/png", vec![0; five_mb + 1], ten_mb).is_ok());
    }

    #[test]
    fn encodes_data_url_without_external_dependency() {
        let image = draft_from_bytes("x.png", "image/png", vec![0, 1, 2, 253], 1024).unwrap();
        assert_eq!(image.attachment.data_url, "data:image/png;base64,AAEC/Q==");
    }
}
