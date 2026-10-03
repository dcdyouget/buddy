//! 消息行外观（S05-08）—— 取自 v1 `MessageBubble.tsx` 的内联样式与 `global.css` 的覆盖规则
//!
//! v1 一条消息一个气泡；v2 把助手消息拆成多行（块粒度，S05-02），因此把 v1 按「整条消息」计算的留白
//! 分摊到该消息的首 / 末行：
//!
//! | v1（`global.css` 覆盖内联样式） | 数值 | 本模块 |
//! |------|------|------|
//! | `.message-row` padding | space-2 × space-4；续段顶部 space-1；有续段时底部 0 | 首行顶部 / 末行底部 |
//! | `.message-row.is-assistant .message-bubble` padding | space-1 × 0；续段顶部 0；有续段时底部 0 | 同上叠加 |
//! | `.assistant-content-flow` gap | space-1 | 非首行的顶部 |
//! | 用户气泡 | 最大宽 76%、padding space-2 × space-3、圆角 md/md/md/sm、`--user-bubble` 底与边、`--shadow-static`、14px / 1.5 | 同 |
//! | `.chat-error` | 见 [`error_banner`] | 同 |

use super::attachments;
use super::drag::{self, DragSource};
use super::rows::RowPos;
use buddy_engine::models::ImageAttachment;
use crate::components::{IconButtonVariant, icon_button};
use crate::icons::{IconName, icon};
use crate::markdown::zed_markdown::{MarkdownElement, MarkdownStyle};
use crate::theme_system::{BuddyTheme, box_shadows, fonts, tokens::metrics as m};
use gpui::{AnyElement, App, Div, FontWeight, Hsla, Image, ImageFormat, ImageSource, MouseButton, ObjectFit, Refineable, SharedString, TextStyleRefinement, div, img, prelude::*, px, relative};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

/// 用户消息的行高（v1 内联 `lineHeight: 1.5`）
const USER_LINE_HEIGHT: f32 = 1.5;
/// 用户气泡最大宽度（v1 `.message-row.is-user .message-bubble { max-width: 76% }`，覆盖内联的 80%）
const USER_MAX_WIDTH: f32 = 0.76;

/// 助手行的上下留白（见模块文档表格）
pub fn assistant_padding(pos: RowPos) -> (f32, f32) {
    let top = if !pos.first {
        m::SPACE_1 // 块间 gap
    } else if pos.continuation {
        m::SPACE_1 // 行顶 space-1 + 气泡顶 0
    } else {
        m::SPACE_2 + m::SPACE_1 // 行顶 space-2 + 气泡顶 space-1
    };
    let bottom = if !pos.last {
        0.0
    } else if pos.continues {
        0.0
    } else {
        m::SPACE_1 + m::SPACE_2 // 气泡底 space-1 + 行底 space-2
    };
    (top, bottom)
}

/// Add v1-compatible blank-body dragging without putting a full-width mouse
/// listener over Markdown text. The Markdown element itself can distinguish a
/// glyph hit from the surrounding blank area and preserves native selection.
pub fn with_blank_drag(markdown: MarkdownElement, source: DragSource) -> MarkdownElement {
    markdown.on_blank_mouse_down(move |window| drag::invoke(&source, window))
}

/// 助手行容器：左右 space-4，上下按位置
pub fn assistant_row(pos: RowPos, content: impl IntoElement) -> Div {
    let (top, bottom) = assistant_padding(pos);
    div().w_full().px(px(m::SPACE_4)).pt(px(top)).pb(px(bottom)).child(content)
}

/// Assistant row with drag regions restricted to the row's actual padding.
/// The content area is intentionally left without a structural listener;
/// Markdown owns its own blank-body hit testing and controls remain isolated.
pub fn assistant_row_with_drag(pos: RowPos, content: impl IntoElement, source: DragSource) -> Div {
    let (top, bottom) = assistant_padding(pos);
    div()
        .relative()
        .w_full()
        .px(px(m::SPACE_4))
        .pt(px(top))
        .pb(px(bottom))
        .children([
            drag::region(&source)
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(top)),
            drag::region(&source)
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                .h(px(bottom)),
            drag::region(&source)
                .absolute()
                .top(px(top))
                .bottom(px(bottom))
                .left_0()
                .w(px(m::SPACE_4)),
            drag::region(&source)
                .absolute()
                .top(px(top))
                .bottom(px(bottom))
                .right_0()
                .w(px(m::SPACE_4)),
        ])
        .child(content)
}

/// 用户消息正文的样式：14px / 1.5、正文色；链接不着色（v1 用户消息不识别链接）
pub fn user_text_style(window: &gpui::Window, cx: &App) -> MarkdownStyle {
    let c = cx.buddy_theme().colors;
    let mut base_text_style = window.text_style();
    base_text_style.refine(&TextStyleRefinement {
        font_family: Some(fonts::ui_font(cx).family),
        font_features: Some(fonts::ui_font(cx).features),
        font_size: Some(px(m::FONT_SIZE_MD).into()),
        font_weight: Some(FontWeight(m::FONT_WEIGHT_REGULAR)),
        color: Some(c.text_primary.into()),
        line_height: Some(relative(USER_LINE_HEIGHT)),
        ..Default::default()
    });
    MarkdownStyle {
        base_text_style,
        selection_background_color: Hsla::from(c.buddy_primary).opacity(crate::markdown::SELECTION_ALPHA),
        paragraph_spacing: px(0.),
        paragraph_line_height: relative(USER_LINE_HEIGHT),
        ..Default::default()
    }
}

/// 用户消息行（`text` 为 [`user_text_style`] 渲染的正文；图片附件由 S05-07 接入）
pub fn user_row(text: impl IntoElement, cx: &App) -> AnyElement {
    let theme = cx.buddy_theme();
    let c = theme.colors;
    div()
        .w_full()
        .px(px(m::SPACE_4))
        .py(px(m::SPACE_2))
        .flex()
        .justify_end()
        .child(
            div()
                .max_w(relative(USER_MAX_WIDTH))
                .min_w_0()
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_2))
                .rounded_tl(px(m::RADIUS_MD))
                .rounded_tr(px(m::RADIUS_MD))
                .rounded_br(px(m::RADIUS_MD))
                .rounded_bl(px(m::RADIUS_SM))
                .bg(c.user_bubble)
                .border_1()
                .border_color(c.user_bubble_border)
                .shadow(box_shadows(theme.shadows.shadow_static))
                .text_color(c.text_primary)
                .text_size(px(m::FONT_SIZE_MD))
                .line_height(relative(USER_LINE_HEIGHT))
                .child(text),
        )
        .into_any_element()
}

/// User row variant used by the transcript so the bubble's surrounding blank
/// area remains draggable while the bubble keeps text selection and controls.
pub fn user_row_with_drag(text: impl IntoElement, cx: &App, source: DragSource) -> AnyElement {
    let theme = cx.buddy_theme();
    let c = theme.colors;
    let bubble_bounds = Rc::new(Cell::new(None::<gpui::Bounds<gpui::Pixels>>));
    let recorded_bounds = bubble_bounds.clone();
    let root_bounds = bubble_bounds.clone();
    div()
        .relative()
        .w_full()
        .px(px(m::SPACE_4))
        .py(px(m::SPACE_2))
        .flex()
        .justify_end()
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            // Exclude the measured content box, while leaving the bubble's
            // explicit padding draggable. Markdown owns blank glyph-area
            // decisions inside this box; images and controls remain there too.
            let Some(bounds) = root_bounds.get() else {
                return;
            };
            let content_left = bounds.left() + px(m::SPACE_3 + 1.0);
            let content_right = bounds.right() - px(m::SPACE_3 + 1.0);
            let content_top = bounds.top() + px(m::SPACE_2 + 1.0);
            let content_bottom = bounds.bottom() - px(m::SPACE_2 + 1.0);
            if event.position.x >= content_left
                && event.position.x <= content_right
                && event.position.y >= content_top
                && event.position.y <= content_bottom
            {
                return;
            }
            cx.stop_propagation();
            drag::invoke(&source, window);
        })
        .child(
            div()
                .max_w(relative(USER_MAX_WIDTH))
                .min_w_0()
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_2))
                .rounded_tl(px(m::RADIUS_MD))
                .rounded_tr(px(m::RADIUS_MD))
                .rounded_br(px(m::RADIUS_MD))
                .rounded_bl(px(m::RADIUS_SM))
                .bg(c.user_bubble)
                .border_1()
                .border_color(c.user_bubble_border)
                .shadow(box_shadows(theme.shadows.shadow_static))
                .text_color(c.text_primary)
                .text_size(px(m::FONT_SIZE_MD))
                .line_height(relative(USER_LINE_HEIGHT))
                .child(text)
                // Measurement only: no hitbox and no layout contribution.
                .child(
                    gpui::canvas(|_, _, _| {}, move |bounds, _, _, _| {
                        recorded_bounds.set(Some(bounds));
                    })
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                ),
        )
        .into_any_element()
}

/// 用户消息行，附带 v1 同样的图片缩略图带。
pub fn user_row_with_images(
    text: impl IntoElement,
    images: &[ImageAttachment],
    has_text: bool,
    window: &gpui::Window,
    cx: &App,
) -> AnyElement {
    if images.is_empty() {
        return user_row(text, cx);
    }
    let columns = if images.len() > 1 { 2 } else { 1 };
    // Match v1's two responsive grid columns inside the 76% user bubble.
    let available = (f32::from(window.viewport_size().width) - m::SPACE_4 * 2.0) * USER_MAX_WIDTH
        - m::SPACE_3 * 2.0 - 2.0;
    let max_width = ((available - m::SPACE_1 * (columns - 1) as f32) / columns as f32)
        .clamp(m::SPACE_12, 260.0);
    let image_strip = div()
        .grid()
        .grid_cols(columns)
        .max_w(px(max_width * columns as f32 + m::SPACE_1 * (columns - 1) as f32))
        .gap(px(m::SPACE_1))
        .when(has_text, |d| d.mb(px(m::SPACE_2)))
        .children(images.iter().map(|image| attachment_image_sized(image, cx, max_width)));
    user_row(
        div().flex().flex_col().items_end().child(image_strip).child(text),
        cx,
    )
}

/// User row with attachments and a structural blank drag region.
pub fn user_row_with_images_drag(
    text: impl IntoElement,
    images: &[ImageAttachment],
    has_text: bool,
    window: &gpui::Window,
    cx: &App,
    source: DragSource,
) -> AnyElement {
    if images.is_empty() {
        return user_row_with_drag(text, cx, source);
    }
    let columns = if images.len() > 1 { 2 } else { 1 };
    let available = (f32::from(window.viewport_size().width) - m::SPACE_4 * 2.0) * USER_MAX_WIDTH
        - m::SPACE_3 * 2.0
        - 2.0;
    let max_width = ((available - m::SPACE_1 * (columns - 1) as f32) / columns as f32)
        .clamp(m::SPACE_12, 260.0);
    let image_strip = div()
        .grid()
        .grid_cols(columns)
        .max_w(px(max_width * columns as f32 + m::SPACE_1 * (columns - 1) as f32))
        .gap(px(m::SPACE_1))
        .when(has_text, |d| d.mb(px(m::SPACE_2)))
        .children(images.iter().map(|image| attachment_image_sized(image, cx, max_width)));
    user_row_with_drag(
        div().flex().flex_col().items_end().child(image_strip).child(text),
        cx,
        source,
    )
}

/// 渲染消息附件；本地路径、HTTP/data URL 均走现有图片来源链路。
pub fn attachment_image(image: &ImageAttachment, cx: &App) -> AnyElement {
    attachment_image_sized(image, cx, 260.0)
}

fn attachment_image_sized(image: &ImageAttachment, cx: &App, max_width: f32) -> AnyElement {
    let c = *cx.buddy_theme().colors;
    let source = if !image.path.is_empty() {
        Some(ImageSource::Resource(PathBuf::from(&image.path).into()))
    } else {
        attachments::decode_data_url(&image.data_url, &image.media_type)
            .and_then(|bytes| ImageFormat::from_mime_type(&image.media_type)
                .map(|format| ImageSource::Image(Arc::new(Image::from_bytes(format, bytes)))))
            .or_else(|| crate::markdown::gfm::image_source(&image.data_url))
    };
    let label = SharedString::from(if image.path.is_empty() { image.name.clone() } else { image.path.clone() });
    let Some(source) = source else {
        return div().w(px(max_width)).h(px(64.0))
            .rounded(px(m::RADIUS_SM))
            .border_1().border_color(c.border_subtle).bg(c.bg_sunken)
            .flex().flex_col().items_center().justify_center()
            .text_color(c.text_muted).text_size(px(m::FONT_SIZE_XS))
            .child(icon(IconName::ImageOff, px(18.0)))
            .child("图片已删除").child(label).into_any_element();
    };
    let retry_source = source.clone();
    let retry_id = SharedString::from(format!("attachment-retry-{}", image.id));
    div().w(px(max_width)).max_h(px(240.0))
        .rounded(px(m::RADIUS_SM))
        .overflow_hidden().border_1().border_color(c.border_subtle).bg(c.bg_sunken)
        .child(img(source).w(px(max_width)).max_h(px(240.0)).object_fit(ObjectFit::Contain)
            .with_fallback(move || {
                let source = retry_source.clone();
                div().w(px(max_width)).min_h(px(64.0))
                    .flex().flex_col().items_center().justify_center()
                    .text_color(c.text_muted).text_size(px(m::FONT_SIZE_XS))
                    .child(icon(IconName::ImageOff, px(18.0)))
                    .child("图片加载失败")
                    .child(div().id(retry_id.clone()).cursor_pointer().text_color(c.buddy_primary)
                        .on_click(move |_, window, cx| {
                            source.remove_asset(cx);
                            window.refresh();
                            cx.refresh_windows();
                        }).child("重试"))
                    .child(label.clone()).into_any_element()
            })).into_any_element()
}

/// 错误提示条（v1 `ChatPage.tsx` 的 `.chat-error`：图标 15px、关闭按钮 24 / 13）
pub fn error_banner(message: &str, on_close: impl Fn(&mut gpui::Window, &mut App) + 'static, cx: &App) -> AnyElement {
    let c = cx.buddy_theme().colors;
    let tint = |alpha: f32| Hsla::from(c.state_error).opacity(alpha);
    div()
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .mx(px(m::SPACE_3))
        .mb(px(m::SPACE_2))
        .px(px(m::SPACE_3))
        .py(px(m::SPACE_2))
        .rounded(px(m::RADIUS_MD))
        .text_color(c.state_error)
        .bg(tint(0.09))
        .border_1()
        .border_color(tint(0.22))
        .text_size(px(m::FONT_SIZE_SM))
        .child(icon(IconName::CircleAlert, px(15.0)))
        .child(div().flex_1().min_w_0().child(SharedString::from(message.to_string())))
        .child(
            icon_button("chat-error-close", IconName::Close, 24.0, 13.0, IconButtonVariant::Default, false, cx)
                .on_click(move |_, window, cx| on_close(window, cx)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(first: bool, last: bool, continuation: bool, continues: bool) -> RowPos {
        RowPos { first, last, continuation, continues }
    }

    #[test]
    fn single_block_message_matches_v1_total_spacing() {
        // 普通单块助手消息：上 8+4、下 4+8（v1 行 padding space-2 + 气泡 padding space-1）
        assert_eq!(assistant_padding(pos(true, true, false, false)), (12.0, 12.0));
        // 续段：上 4 + 0；有后续续段：下 0 + 0
        assert_eq!(assistant_padding(pos(true, true, true, true)), (4.0, 0.0));
        // 中间块：gap space-1
        assert_eq!(assistant_padding(pos(false, false, false, false)), (4.0, 0.0));
    }
}
