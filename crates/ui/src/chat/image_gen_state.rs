//! 图片生成卡片的数据解析、图片来源与预览保存动作。

use super::state::ToolStatus;
use crate::markdown::gfm;
use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use buddy_engine::models::ImageAttachment;
use gpui::{
    AnyElement, App, Entity, Image, ImageFormat, ImageSource, ObjectFit, Pixels, Resource,
    RetainAllImageCache, SharedString, Task, Window, div, img, prelude::*, px,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// 生成图片的保存动作。生产路由可替换为 engine 的 `download_generated_image`。
pub type DownloadFn = Rc<dyn Fn(ImageAttachment, &mut App) -> Task<Result<String, String>>>;

/// 图片保存状态，由卡片和异步下载回调共享。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DownloadState {
    /// 正在调用 engine。
    Saving,
    /// 保存成功，值为系统下载路径。
    Saved(String),
    /// 保存失败，值为用户可见错误。
    Error(String),
}

/// 卡片下载状态表。
pub type DownloadStates = Arc<Mutex<HashMap<String, DownloadState>>>;

/// 卡片复制反馈状态表。
pub type CopyStates = Arc<Mutex<HashSet<String>>>;

/// 自测与卡片共享的图片重试点击状态。
pub type RetryStates = Arc<Mutex<HashSet<String>>>;

/// 图片元素最近一次通过 GPUI cache 的实际加载状态。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageLoadState {
    /// Cache request is still decoding the resource.
    Loading,
    /// Cache finished with a decode or I/O error.
    Failed,
    /// Cache decoded the image and exposed its source dimensions.
    Loaded {
        /// Decoded pixel width.
        width: i32,
        /// Decoded pixel height.
        height: i32,
    },
}

/// Shared load results used by the preview evidence.
pub type ImageLoadStates = Arc<Mutex<HashMap<String, ImageLoadState>>>;

/// 生图工具结果中的可展示元数据。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GenerateResult {
    /// 生成模型名称。
    pub model: Option<String>,
    /// 最终使用的提示词。
    pub prompt: Option<String>,
    /// 服务端返回的优化提示词。
    pub revised_prompts: Vec<String>,
    /// 服务端补充说明。
    pub note: Option<String>,
}

fn text(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

/// v1 `parsePrompt`：读取调用参数里的 prompt，非法 JSON 按空处理。
pub fn parse_prompt(arguments: &str) -> String {
    serde_json::from_str::<serde_json::Value>(arguments)
        .ok()
        .and_then(|v| v.get("prompt")?.as_str().map(|p| p.trim().to_string()))
        .unwrap_or_default()
}

/// v1 `parseResult`：解析失败或非对象按空处理。
pub fn parse_result(result: Option<&str>) -> GenerateResult {
    let Some(value) = result
        .and_then(|r| serde_json::from_str::<serde_json::Value>(r).ok())
        .filter(serde_json::Value::is_object)
    else {
        return GenerateResult::default();
    };
    let revised_prompts = value
        .get("revised_prompts")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    GenerateResult {
        model: text(&value, "model"),
        prompt: text(&value, "prompt"),
        revised_prompts,
        note: text(&value, "note"),
    }
}

/// 根据工具状态返回 v1 卡片标题。
pub fn label(status: ToolStatus, active: bool) -> &'static str {
    if active {
        "正在生成图片"
    } else if status == ToolStatus::Error {
        "图片生成失败"
    } else if status == ToolStatus::Interrupted {
        "图片生成已中断"
    } else {
        "图片生成完成"
    }
}

fn image_source(image: &ImageAttachment) -> Option<ImageSource> {
    if !image.path.trim().is_empty() {
        return Some(ImageSource::Resource(
            PathBuf::from(image.path.clone()).into(),
        ));
    }
    if let (Some(format), Some(bytes)) = (
        ImageFormat::from_mime_type(&image.media_type),
        crate::chat::attachments::decode_data_url(&image.data_url, &image.media_type),
    ) {
        return Some(ImageSource::Image(Arc::new(Image::from_bytes(
            format, bytes,
        ))));
    }
    gfm::image_source(&image.data_url)
}

fn retry_resource(image: &ImageAttachment) -> Option<Resource> {
    if !image.path.trim().is_empty() {
        return Some(PathBuf::from(image.path.clone()).into());
    }
    match gfm::image_source(&image.data_url) {
        Some(ImageSource::Resource(resource)) => Some(resource),
        _ => None,
    }
}

fn data_url_dimensions(image: &ImageAttachment) -> Option<(f32, f32)> {
    let bytes = crate::chat::attachments::decode_data_url(&image.data_url, &image.media_type)?;
    match image.media_type.as_str() {
        "image/png" if bytes.len() >= 24 && &bytes[..8] == b"\x89PNG\r\n\x1a\n" => Some((
            u32::from_be_bytes(bytes[16..20].try_into().ok()?) as f32,
            u32::from_be_bytes(bytes[20..24].try_into().ok()?) as f32,
        )),
        "image/gif"
            if bytes.len() >= 10 && (&bytes[..6] == b"GIF87a" || &bytes[..6] == b"GIF89a") =>
        {
            Some((
                u16::from_le_bytes(bytes[6..8].try_into().ok()?) as f32,
                u16::from_le_bytes(bytes[8..10].try_into().ok()?) as f32,
            ))
        }
        _ => None,
    }
}

fn retry_button(
    cache: Entity<RetainAllImageCache>,
    resource: Resource,
    image_id: String,
    retry_states: RetryStates,
    colors: crate::theme_system::tokens::Palette,
) -> AnyElement {
    let element_id = SharedString::from(format!("generated-image-retry-{image_id}"));
    div()
        .id(element_id)
        .cursor_pointer()
        .text_color(colors.tool_ui_accent)
        .on_click(move |_, window, cx| {
            cache.update(cx, |cache, cx| cache.remove(&resource, window, cx));
            if let Ok(mut states) = retry_states.lock() {
                states.insert(image_id.clone());
            }
            window.refresh();
            cx.refresh_windows();
        })
        .child("重试")
        .into_any_element()
}

/// 使用已有图片加载链路渲染单张生成图片。
pub fn image_view(
    image: &ImageAttachment,
    _alt: &str,
    max_height: Pixels,
    cache: Entity<RetainAllImageCache>,
    retry_states: RetryStates,
    load_states: ImageLoadStates,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let c = cx.buddy_theme().colors;
    let fallback_height = max_height.min(px(m::SPACE_12 * 5.0));
    let Some(source) = image_source(image) else {
        return div()
            .size_full()
            .min_h(fallback_height)
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(m::FONT_SIZE_XS))
            .text_color(c.text_tertiary)
            .child("图片无法加载")
            .into_any_element();
    };
    let resource = retry_resource(image);
    let retry_cache = cache.clone();
    let retry_id = image.id.clone();
    let fallback_load_states = load_states.clone();
    let fallback_image_id = image.id.clone();
    // Ask the existing GPUI cache for decoded dimensions. This is asynchronous
    // for local/HTTP resources and avoids reading an entire file during render.
    // Data URLs are already in memory, so their small header can be inspected
    // directly while the decoded image follows the same image element path.
    let dimensions = if let Some(resource) = resource.as_ref() {
        let image_key = image.id.clone();
        let load = cache.update(cx, |cache, cx| cache.load(resource, window, cx));
        if let Ok(mut states) = load_states.lock() {
            match &load {
                None => {
                    states.insert(image.id.clone(), ImageLoadState::Loading);
                }
                Some(Err(_)) => {
                    states.insert(image.id.clone(), ImageLoadState::Failed);
                }
                Some(Ok(image)) => {
                    let size = image.size(0);
                    states.insert(
                        image_key,
                        ImageLoadState::Loaded {
                            width: size.width.0,
                            height: size.height.0,
                        },
                    );
                }
            }
        }
        load.and_then(|result| result.ok()).map(|image| {
            let size = image.size(0);
            (size.width.0 as f32, size.height.0 as f32)
        })
    } else {
        let dimensions = data_url_dimensions(image);
        if let Some((width, height)) = dimensions {
            if let Ok(mut states) = load_states.lock() {
                states.insert(
                    image.id.clone(),
                    ImageLoadState::Loaded {
                        width: width as i32,
                        height: height as i32,
                    },
                );
            }
        }
        dimensions
    };
    let image_element = if let Some((width, height)) = dimensions {
        let scale = ((f32::from(window.viewport_size().width) - m::SPACE_4 * 2.0 - m::SPACE_3 * 2.0 - 2.0).max(0.0).min(m::SPACE_12 * 10.0) / width)
            .min(f32::from(max_height) / height)
            .min(1.0);
        img(source)
            .w(px(width * scale))
            .h(px(height * scale))
            .object_fit(ObjectFit::Contain)
    } else if resource.is_some() {
        // A resource has no intrinsic size before decode or after failure.
        // Give its loading/error replacement a definite, visible box.
        img(source)
            .w(px((f32::from(window.viewport_size().width) - m::SPACE_4 * 2.0 - m::SPACE_3 * 2.0).min(m::SPACE_12 * 10.0)))
            .h(fallback_height)
            .object_fit(ObjectFit::Contain)
    } else {
        img(source)
            .max_w(px(m::SPACE_12 * 10.0))
            .max_h(max_height)
            .object_fit(ObjectFit::Contain)
    };
    image_element
        .image_cache(&cache)
        .id(SharedString::from(format!("generated-image-{}", image.id)))
        .with_fallback(move || {
            if let Ok(mut states) = fallback_load_states.lock() {
                states.insert(fallback_image_id.clone(), ImageLoadState::Failed);
            }
            let retry = resource.clone().map(|resource| {
                retry_button(
                    retry_cache.clone(),
                    resource,
                    retry_id.clone(),
                    retry_states.clone(),
                    *c,
                )
            });
            div()
                .size_full()
                .min_h(fallback_height)
                .flex()
                .items_center()
                .justify_center()
                .gap(px(m::SPACE_2))
                .text_size(px(m::FONT_SIZE_XS))
                .text_color(c.text_tertiary)
                .child("图片无法加载")
                .children(retry)
                .into_any_element()
        })
        .with_loading(move || {
            div()
                .size_full()
                .min_h(fallback_height)
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(m::FONT_SIZE_XS))
                .text_color(c.text_tertiary)
                .child("正在加载图片…")
                .into_any_element()
        })
        .into_any_element()
}

/// 预览默认动作：未接入 engine 时明确返回错误，由卡片显示“重试”。
/// 生产路由通过 `Transcript::set_download_handler` 注入 engine 保存动作。
pub fn default_download(
    _image: ImageAttachment,
    cx: &mut App,
) -> gpui::Task<Result<String, String>> {
    cx.background_spawn(async { Err("预览未接入图片下载引擎".to_string()) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(path: &str, data_url: &str) -> ImageAttachment {
        ImageAttachment {
            id: "retry-test".into(),
            name: "retry.png".into(),
            media_type: "image/png".into(),
            path: path.into(),
            data_url: data_url.into(),
        }
    }

    #[test]
    fn parses_prompt_and_result_like_v1() {
        assert_eq!(parse_prompt(r#"{"prompt":"  一只猫  "}"#), "一只猫");
        assert_eq!(parse_prompt("{半截"), "");
        assert_eq!(
            parse_result(Some(
                r#"{"model":"gpt-image-2","prompt":"猫","revised_prompts":["猫和月亮"],"note":"部分"}"#
            )),
            GenerateResult {
                model: Some("gpt-image-2".into()),
                prompt: Some("猫".into()),
                revised_prompts: vec!["猫和月亮".into()],
                note: Some("部分".into())
            }
        );
        assert_eq!(parse_result(Some("不是 JSON")), GenerateResult::default());
    }

    #[test]
    fn labels_match_v1() {
        assert_eq!(label(ToolStatus::Executing, true), "正在生成图片");
        assert_eq!(label(ToolStatus::Error, false), "图片生成失败");
        assert_eq!(label(ToolStatus::Interrupted, false), "图片生成已中断");
        assert_eq!(label(ToolStatus::Done, false), "图片生成完成");
    }

    #[test]
    fn retry_resource_only_caches_reloadable_resources() {
        assert!(retry_resource(&image("/tmp/retry.png", "")).is_some());
        assert!(retry_resource(&image("", "https://example.com/retry.png")).is_some());
        assert!(retry_resource(&image("", "data:image/png;base64,AA==")).is_none());
        assert!(retry_resource(&image("", "")).is_none());
    }
}
