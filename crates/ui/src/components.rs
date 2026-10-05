//! 共用小组件（对应 v1 `src/components/shared/*`）

use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, box_shadows, tokens::{metrics as m, motion}};
use gpui::{AnyView, App, Context, Div, ElementId, Hsla, IntoElement, Render, RenderOnce, Rgba, SharedString, Stateful, Window, div, prelude::*, px,
};

/// 图标按钮的视觉变体（v1 `IconButton` `variant`）
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IconButtonVariant {
    /// 透明底、弱化色；悬停变正文色 + 凹陷底（v1 `.icon-button:hover`）
    #[default]
    Default,
    /// 品牌色底
    Primary,
    /// 错误色底
    Danger,
}

/// 圆形图标按钮（v1 `src/components/shared/IconButton.tsx`：默认 28px、图标 16px、禁用时 0.4 不透明）
pub fn icon_button(
    id: impl Into<ElementId>,
    name: IconName,
    size: f32,
    icon_size: f32,
    variant: IconButtonVariant,
    disabled: bool,
    cx: &App,
) -> Stateful<Div> {
    let c = cx.buddy_theme().colors;
    let (fg, bg) = match variant {
        IconButtonVariant::Default => (c.text_muted, None),
        IconButtonVariant::Primary => (c.text_on_primary, Some(c.buddy_primary)),
        IconButtonVariant::Danger => (c.text_on_primary, Some(c.state_error)),
    };
    div()
        .id(id)
        .relative()
        .overflow_hidden()
        .flex_none()
        .size(px(size))
        .rounded(px(m::RADIUS_FULL))
        .flex()
        .items_center()
        .justify_center()
        .text_color(fg)
        .when_some(bg, |d, bg| d.bg(bg))
        .when(disabled, |d| d.opacity(0.4))
        .when(!disabled, |d| {
            d.cursor_pointer().when(variant == IconButtonVariant::Default, |d| {
                    d.hover(|s| s.text_color(c.text_primary).bg(c.bg_sunken))
                })
                .when(variant == IconButtonVariant::Primary, |d| {
                    d.hover(|s| s.bg(c.buddy_primary_600))
                })
                .when(variant == IconButtonVariant::Danger, |d| {
                    d.hover(|s| s.bg(Hsla::from(c.state_error).opacity(motion::BUTTON_PRESS_OPACITY)))
                })
                // GPUI's active style is held for the mouse-down phase. A small
                // tonal step gives clicks a tactile response without shifting
                // layout or making the controls feel springy.
                .active(|s| match variant {
                    IconButtonVariant::Default => s.bg(c.bg_sunken).opacity(motion::BUTTON_PRESS_OPACITY),
                    IconButtonVariant::Primary => s.bg(c.buddy_primary_600).opacity(motion::BUTTON_PRESS_OPACITY),
                    IconButtonVariant::Danger => s.bg(Hsla::from(c.state_error).opacity(motion::BUTTON_PRESS_OPACITY)),
                })
        })
        .child(icon(name, px(icon_size)))
        .when(!disabled, |d| d.child(ButtonLight { accent: c.buddy_primary }))
}

/// Retain hover state in the button scope so rerenders do not restart the light.
#[derive(IntoElement)]
struct ButtonLight {
    accent: Rgba,
}

impl RenderOnce for ButtonLight {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let hovered = window.use_keyed_state("button-light-hover", cx, |_, _| false);
        let active = *hovered.read(cx);
        div().id("button-light").absolute().top_0().left_0().size_full()
            .on_hover(move |active, _, cx| hovered.update(cx, |state, cx| {
                *state = *active;
                cx.notify();
            }))
            .when(active, |d| d.child(crate::motion_effects::surface_sheen("button-light-pass", self.accent, false)))
    }
}

/// 悬停提示（v1 用原生 `title` 属性；GPUI 无原生提示，自绘一个最小的）
pub struct TextTooltip(SharedString);

impl TextTooltip {
    /// 构造提示视图，供 `.tooltip(|_, cx| TextTooltip::view("…", cx))` 使用
    pub fn view(text: impl Into<SharedString>, cx: &mut App) -> AnyView {
        let text = text.into();
        cx.new(|_| TextTooltip(text)).into()
    }
}

impl Render for TextTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.buddy_theme();
        let c = theme.colors;
        div()
            .px(px(m::SPACE_2))
            .py(px(m::SPACE_1))
            .rounded(px(m::RADIUS_SM))
            .border_1()
            .border_color(c.border_default)
            .bg(c.bg_surface)
            .text_color(c.text_primary)
            .text_size(px(m::FONT_SIZE_SM))
            .shadow(box_shadows(theme.shadows.shadow_static))
            .child(self.0.clone())
    }
}
