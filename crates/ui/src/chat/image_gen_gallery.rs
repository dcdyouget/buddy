//! 图片生成结果画廊：图片展示、下载状态和错误重试。

use super::image_gen_state::{
    self, DownloadFn, DownloadState, DownloadStates, ImageLoadStates, RetryStates,
};
use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, box_shadows, tokens::metrics as m};
use buddy_engine::models::ImageAttachment;
use gpui::{
    AnyElement, App, Entity, Hsla, Pixels, RetainAllImageCache, Window, div, prelude::*, px,
};

/// 渲染结果图片画廊；结果图片常驻卡片，详情区由父卡片单独控制。
#[allow(clippy::too_many_arguments)]
pub fn gallery(
    images: &[ImageAttachment],
    prompt: &str,
    image_max_height: Pixels,
    now_ms: f64,
    image_cache: Entity<RetainAllImageCache>,
    retry_states: RetryStates,
    load_states: ImageLoadStates,
    download_states: DownloadStates,
    download: DownloadFn,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    (!images.is_empty()).then(|| {
        div()
            .w_full()
            .p(px(m::SPACE_3))
            .border_t_1()
            .border_color(cx.buddy_theme().colors.border_subtle)
            .flex()
            .flex_wrap()
            .items_start()
            .justify_center()
            .gap(px(m::SPACE_3))
            .bg(cx.buddy_theme().colors.bg_sunken)
            .children(images.iter().map(|image| {
                let image = image.clone();
                let download = download.clone();
                let download_states = download_states.clone();
                let image_id = image.id.clone();
                let download_state = download_states
                    .lock()
                    .ok()
                    .and_then(|states| states.get(&image_id).cloned());
                let alt = if prompt.is_empty() {
                    image.name.clone()
                } else {
                    prompt.to_string()
                };
                let c = cx.buddy_theme().colors;
                let saving = matches!(download_state, Some(DownloadState::Saving));
                let mut save_icon = icon(match download_state {
                    Some(DownloadState::Saved(_)) => IconName::Check,
                    Some(DownloadState::Saving) => IconName::LoaderCircle,
                    _ => IconName::Download,
                }, px(14.0));
                if saving && !crate::accessibility::prefers_reduced_motion() {
                    save_icon = save_icon.with_transformation(gpui::Transformation::rotate(gpui::radians((now_ms % 1000.0 / 1000.0 * std::f64::consts::TAU) as f32)));
                    window.request_animation_frame();
                }
                div()
                    .relative()
                    .max_w(px(m::SPACE_12 * 10.0))
                    .overflow_hidden()
                    .border_1()
                    .border_color(c.border_subtle)
                    .rounded(px(m::RADIUS_LG))
                    .bg(c.bg_elevated)
                    .shadow(box_shadows(cx.buddy_theme().shadows.shadow_floating_sm))
                    .child(image_gen_state::image_view(
                        &image,
                        &alt,
                        image_max_height,
                        image_cache.clone(),
                        retry_states.clone(),
                        load_states.clone(),
                        window,
                        cx,
                    ))
                    .child(
                        div()
                            .id(gpui::SharedString::from(format!(
                                "generated-image-save-{}",
                                image.id
                            )))
                            .absolute()
                            .top(px(m::SPACE_2))
                            .right(px(m::SPACE_2))
                            .px(px(m::SPACE_2))
                            .py(px(m::SPACE_1))
                            .min_h(px(m::SPACE_8))
                            .rounded(px(m::RADIUS_FULL))
                            .flex()
                            .items_center()
                            .gap(px(m::SPACE_1))
                            .text_size(px(m::FONT_SIZE_XS))
                            .font_weight(gpui::FontWeight(600.0))
                            .text_color(c.neutral_0)
                            .bg(match download_state.clone() {
                                Some(DownloadState::Saved(_)) => {
                                    Hsla::from(c.state_success).opacity(0.88)
                                }
                                Some(DownloadState::Error(_)) => {
                                    Hsla::from(c.state_error).opacity(0.88)
                                }
                                _ => Hsla::from(c.neutral_900).opacity(0.72),
                            })
                            .cursor_pointer()
                            .on_click(move |_, _, cx| {
                                if matches!(
                                    download_states
                                        .lock()
                                        .ok()
                                        .and_then(|states| states.get(&image_id).cloned()),
                                    Some(DownloadState::Saving)
                                ) {
                                    return;
                                }
                                if let Ok(mut states) = download_states.lock() {
                                    states.insert(image_id.clone(), DownloadState::Saving);
                                }
                                cx.refresh_windows();
                                let task = download(image.clone(), cx);
                                let states = download_states.clone();
                                let id = image_id.clone();
                                cx.spawn(async move |cx| {
                                    let result = task.await;
                                    if let Ok(mut states) = states.lock() {
                                        states.insert(
                                            id,
                                            match result {
                                                Ok(path) => DownloadState::Saved(path),
                                                Err(error) => DownloadState::Error(error),
                                            },
                                        );
                                    }
                                    cx.refresh();
                                })
                                .detach();
                            })
                            .child(save_icon)
                            .child(match download_state.clone() {
                                Some(DownloadState::Saving) => "保存中".to_string(),
                                Some(DownloadState::Saved(_)) => "已下载".to_string(),
                                Some(DownloadState::Error(_)) => "重试".to_string(),
                                None => "下载".to_string(),
                            }),
                    )
                    .when_some(
                        download_state.and_then(|state| match state {
                            DownloadState::Error(error) => Some(error),
                            _ => None,
                        }),
                        |d, error| {
                            d.child(
                                div()
                                    .w_full()
                                    .px(px(m::SPACE_3))
                                    .py(px(m::SPACE_2))
                                    .text_size(px(m::FONT_SIZE_XS))
                                    .text_color(c.state_error)
                                    .bg(Hsla::from(c.state_error).opacity(0.08))
                                    .child(gpui::SharedString::from(error)),
                            )
                        },
                    )
            }))
            .into_any_element()
    })
}
