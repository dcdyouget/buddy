//! 图片生成卡片。
//!
//! 生图工具的结果图片由 engine 在发出 `ToolResult` 前保存为本地附件；本卡片
//! 直接交给 GPUI 的图片加载器显示本地路径或远程 URL，避免在 UI 层重复解码。

#[path = "image_gen_gallery.rs"]
mod image_gen_gallery;

use super::image_gen_state::{
    self, CopyStates, DownloadFn, DownloadStates, ImageLoadStates, RetryStates,
};
use super::state::{ToolStatus, ToolView};
use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use buddy_engine::models::ImageAttachment;
use gpui::{
    AnyElement, App, ClickEvent, Entity, FontWeight, RetainAllImageCache, SharedString, Window,
    div, prelude::*, px,
};

/// 走专用卡片的工具名（v1 `ToolSection` 的分派条件）。
pub fn is_image_gen(tool_name: &str) -> bool {
    tool_name == "generate_image"
}

/// 渲染图片生成卡片。
#[allow(clippy::too_many_arguments)]
pub fn block(
    id: SharedString,
    tool: &ToolView,
    expanded: bool,
    now_ms: f64,
    images: &[ImageAttachment],
    image_cache: Entity<RetainAllImageCache>,
    retry_states: RetryStates,
    image_load_states: ImageLoadStates,
    download_states: DownloadStates,
    copy_states: CopyStates,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    download: DownloadFn,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let c = cx.buddy_theme().colors;
    let image_max_height = px(
        (f32::from(window.viewport_size().height) * 0.5 - m::SPACE_6)
            .clamp(0.0, m::SPACE_12 * 8.0),
    );
    let result = image_gen_state::parse_result(tool.result.as_deref());
    let prompt = result
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| image_gen_state::parse_prompt(&tool.arguments));
    let active = matches!(tool.status, ToolStatus::Calling | ToolStatus::Executing);
    let reduce = crate::accessibility::prefers_reduced_motion();
    if active && !reduce {
        window.request_animation_frame();
    }
    let label = image_gen_state::label(tool.status, active);
    let heading = div()
        .id(id.clone())
        .w_full()
        .min_h(px(28.0))
        .px(px(m::SPACE_2))
        .py(px(m::SPACE_1))
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .text_color(c.text_muted)
        .cursor_pointer()
        .hover(|s| s.bg(c.bg_sunken))
        .on_click(on_toggle)
        .child(
            div()
                .flex_none()
                .text_color(c.tool_ui_accent)
                .child(icon(IconName::Image, px(14.0))),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(m::FONT_SIZE_SM))
                .font_weight(FontWeight(600.0))
                .child(label),
        )
        .children(
            active.then(|| crate::chat::think_block::loader(c.tool_ui_accent, now_ms, reduce)),
        )
        .when(!prompt.is_empty(), |d| {
            d.child(
                div()
                    .min_w_0()
                    .flex()
                    .text_size(px(m::FONT_SIZE_XS))
                    .text_color(c.text_muted)
                    .child(div().flex_none().mr(px(m::SPACE_2)).child("·"))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .child(SharedString::from(prompt.clone())),
                    ),
            )
        })
        .child(div().flex_1())
        .child(
            div()
                .flex_none()
                .size(px(m::SPACE_4))
                .flex()
                .items_center()
                .justify_center()
                .text_color(c.text_tertiary)
                .child(icon(
                    if expanded {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    },
                    px(13.0),
                )),
        );

    let grid = image_gen_gallery::gallery(
        images,
        &prompt,
        image_max_height,
        now_ms,
        image_cache,
        retry_states,
        image_load_states,
        download_states,
        download,
        window,
        cx,
    );

    let body = expanded.then(|| {
        let meta = |name: &'static str, value: String| {
            div()
                .min_w_0()
                .flex()
                .items_baseline()
                .gap(px(m::SPACE_2))
                .text_size(px(m::FONT_SIZE_XS))
                .child(
                    div()
                        .flex_none()
                        .w(px(m::SPACE_12 + m::SPACE_2))
                        .font_weight(FontWeight(600.0))
                        .text_color(c.text_tertiary)
                        .child(name),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_weight(FontWeight(500.0))
                        .text_color(c.text_primary)
                        .child(SharedString::from(value)),
                )
        };
        let copy_prompt = prompt.clone();
        let copy_key = id.to_string();
        let copied = copy_states
            .lock()
            .ok()
            .is_some_and(|states| states.contains(&copy_key));
        let copy = div()
            .id(SharedString::from("generated-image-copy"))
            .flex_none()
            .px(px(m::SPACE_2))
            .py(px(m::SPACE_1))
            .border_1()
            .border_color(c.border_subtle)
            .rounded(px(m::RADIUS_SM))
            .flex()
            .items_center()
            .gap(px(m::SPACE_1))
            .text_size(px(m::FONT_SIZE_XS))
            .text_color(if copied {
                c.state_success
            } else {
                c.text_muted
            })
            .cursor_pointer()
            .hover(|s| s.text_color(c.tool_ui_accent))
            .on_click(move |_, _, cx| {
                if !copy_prompt.is_empty() {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy_prompt.clone()));
                    if let Ok(mut states) = copy_states.lock() {
                        states.insert(copy_key.clone());
                    }
                    cx.refresh_windows();
                    let states = copy_states.clone();
                    let key = copy_key.clone();
                    cx.spawn(async move |cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(1600))
                            .await;
                        if let Ok(mut states) = states.lock() {
                            states.remove(&key);
                        }
                        cx.refresh();
                    })
                    .detach();
                }
            })
            .child(icon(
                if copied {
                    IconName::Check
                } else {
                    IconName::Copy
                },
                px(12.0),
            ))
            .child(if copied { "已复制" } else { "复制" });
        div()
            .w_full()
            .p(px(m::SPACE_3))
            .border_t_1()
            .border_color(c.border_subtle)
            .flex()
            .flex_col()
            .gap(px(m::SPACE_2))
            .bg(c.bg_sunken)
            .child(meta(
                "生成提示词",
                if prompt.is_empty() {
                    "未获得提示词".into()
                } else {
                    prompt.clone()
                },
            ))
            .when(result.model.is_some() || !prompt.is_empty(), |d| {
                d.child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(m::SPACE_2))
                        .child(meta(
                            "生成模型",
                            result.model.clone().unwrap_or_else(|| "未知模型".into()),
                        ))
                        .child(copy),
                )
            })
            .when(!result.revised_prompts.is_empty(), |d| {
                d.child(meta("优化后提示词", result.revised_prompts.join("\n")))
            })
            .when(active, |d| {
                d.child(
                    div()
                        .p(px(m::SPACE_3))
                        .border_1()
                        .border_dashed()
                        .border_color(c.border_subtle)
                        .rounded(px(m::RADIUS_MD))
                        .text_size(px(m::FONT_SIZE_XS))
                        .text_color(c.text_tertiary)
                        .text_center()
                        .child("正在等待图片生成结果…"),
                )
            })
            .when(tool.status == ToolStatus::Error, |d| {
                d.child(
                    div()
                        .p(px(m::SPACE_3))
                        .border_1()
                        .border_dashed()
                        .border_color(c.state_error)
                        .rounded(px(m::RADIUS_MD))
                        .text_size(px(m::FONT_SIZE_XS))
                        .text_color(c.state_error)
                        .text_center()
                        .child(
                            tool.result
                                .clone()
                                .unwrap_or_else(|| "图片生成接口调用失败".into()),
                        ),
                )
            })
    });

    div()
        .relative()
        .w_full()
        .my(px(m::SPACE_2))
        .overflow_hidden()
        .border_1()
        .border_color(c.border_default)
        .rounded_tr(px(m::RADIUS_MD))
        .rounded_br(px(m::RADIUS_MD))
        .bg(c.panel_surface)
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(2.0))
                .bg(c.tool_ui_accent),
        )
        .child(
            div()
                .relative()
                .child(heading)
                .children(grid)
                .children(body),
        )
        .into_any_element()
}
