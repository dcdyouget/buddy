//! 共用小组件（对应 v1 `src/components/shared/*`）

use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use gpui::{App, ElementId, Stateful, Div, div, prelude::*, px};

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
            d.cursor_pointer().when(variant == IconButtonVariant::Default, |d| d.hover(|s| s.text_color(c.text_primary).bg(c.bg_sunken)))
        })
        .child(icon(name, px(icon_size)))
}
