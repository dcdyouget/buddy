use super::super::ModelListView;
use crate::{
    icons::{IconName, icon},
    settings::model_config::ModelEdit,
    theme_system::{
        BuddyTheme,
        tokens::{Palette, metrics as m},
    },
};
use buddy_engine::models::ModelInfo;
use gpui::{Context, FontWeight, KeyDownEvent, Window, div, prelude::*, px};

pub(super) fn model_info(
    model: &ModelInfo,
    provider_name: Option<String>,
    is_default: bool,
    context: Option<gpui::Stateful<gpui::Div>>,
    c: Palette,
) -> gpui::Div {
    let mut title = div()
        .flex()
        .items_center()
        .gap(px(m::SPACE_1))
        .min_w_0()
        .child(
            div()
                .text_size(px(m::FONT_SIZE_BASE))
                .font_weight(FontWeight(500.))
                .text_color(c.text_primary)
                .child(model.display_name.clone()),
        );
    if let Some(provider) = provider_name {
        title = title.child(
            div()
                .text_size(px(m::FONT_SIZE_XS))
                .text_color(c.text_muted)
                .child(provider),
        );
    }
    if is_default {
        title = title.child(
            div()
                .px(px(m::SPACE_2))
                .py(px(m::SPACE_1 / 4.0))
                .rounded(px(m::RADIUS_FULL))
                .bg(c.primary_tint_soft)
                .text_color(c.buddy_primary)
                .text_size(px(10.))
                .font_weight(FontWeight(600.))
                .child("默认"),
        );
    }
    div().min_w_0().flex_1().child(title).children(context)
}

pub(super) fn latency(value: Option<u32>, c: Palette) -> gpui::Div {
    let Some(value) = value else {
        return div();
    };
    let color = latency_color(value, c);
    div()
        .flex()
        .items_center()
        .gap(px(m::SPACE_1))
        .flex_none()
        .text_size(px(m::FONT_SIZE_XS))
        .text_color(c.text_muted)
        .child(div().size(px(6.)).rounded(px(m::RADIUS_FULL)).bg(color))
        .child(format!("{value}ms"))
}

fn latency_color(value: u32, c: Palette) -> gpui::Rgba {
    if value < 500 {
        c.state_success
    } else if value < 1500 {
        c.state_warning
    } else {
        c.state_error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_boundaries_use_each_theme_state_tokens() {
        for c in [
            crate::theme_system::tokens::LIGHT,
            crate::theme_system::tokens::DARK,
        ] {
            assert_eq!(latency_color(499, c), c.state_success);
            assert_eq!(latency_color(500, c), c.state_warning);
            assert_eq!(latency_color(1499, c), c.state_warning);
            assert_eq!(latency_color(1500, c), c.state_error);
        }
    }
}

pub(super) fn check_control<F, K>(
    id: &str,
    checked: bool,
    active: bool,
    size: f32,
    focus: gpui::FocusHandle,
    click: F,
    key: K,
    bounds: std::rc::Rc<
        std::cell::RefCell<std::collections::BTreeMap<String, gpui::Bounds<gpui::Pixels>>>,
    >,
    cx: &mut Context<ModelListView>,
) -> gpui::Stateful<gpui::Div>
where
    F: Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    K: Fn(&KeyDownEvent, &mut Window, &mut gpui::App) + 'static,
{
    let c = cx.buddy_theme().colors;
    let id_owned = id.to_owned();
    let mut control = div()
        .id(id.to_owned())
        .relative()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(m::SPACE_1))
        .text_size(px(m::FONT_SIZE_XS))
        .text_color(c.text_muted)
        .when(active, |d| d.track_focus(&focus).cursor_pointer())
        .when(active, |d| d.on_click(click))
        .on_key_down(key)
        .child(
            div()
                .size(px(size))
                .rounded(px(m::RADIUS_SM))
                .border_1()
                .border_color(if checked {
                    c.buddy_primary
                } else {
                    c.border_default
                })
                .bg(if checked {
                    c.buddy_primary
                } else {
                    c.bg_sunken
                })
                .flex()
                .items_center()
                .justify_center()
                .when(checked, |d| {
                    d.text_color(c.text_on_primary)
                        .child(icon(IconName::Check, px(m::FONT_SIZE_XS)))
                }),
        );
    control = control.child(
        gpui::canvas(
            |_, _, _| {},
            move |rect, _, _, _| {
                bounds.borrow_mut().insert(id_owned.clone(), rect);
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full(),
    );
    control
}

pub(super) fn default_button(
    id: &str,
    active: bool,
    focus: gpui::FocusHandle,
    bounds: std::rc::Rc<
        std::cell::RefCell<std::collections::BTreeMap<String, gpui::Bounds<gpui::Pixels>>>,
    >,
    cx: &mut Context<ModelListView>,
) -> gpui::Stateful<gpui::Div> {
    let model_id = id.to_owned();
    let click_focus = focus.clone();
    let key_model_id = id.to_owned();
    let bounds_id = format!("default-{id}");
    let mut button = div()
        .id(bounds_id.clone())
        .relative()
        .flex_none()
        .px(px(m::SPACE_2))
        .py(px(2.))
        .rounded(px(m::RADIUS_SM))
        .border_1()
        .border_color(cx.buddy_theme().colors.border_subtle)
        .text_color(cx.buddy_theme().colors.text_muted)
        .text_size(px(m::FONT_SIZE_XS))
        .when(active, |d| {
            d.track_focus(&focus).cursor_pointer().on_click(cx.listener(
                move |this, _, window, cx| {
                    window.focus(&click_focus, cx);
                    if this.active && !this.saving {
                        cx.emit(ModelEdit::SetDefault(model_id.clone()));
                    }
                },
            ))
        })
        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
            if this.active
                && !this.saving
                && matches!(event.keystroke.key.as_str(), "enter" | "space")
            {
                cx.emit(ModelEdit::SetDefault(key_model_id.clone()));
                cx.stop_propagation();
            }
        }))
        .child("设为默认");
    button = button.child(
        gpui::canvas(
            |_, _, _| {},
            move |rect, _, _, _| {
                bounds.borrow_mut().insert(bounds_id.clone(), rect);
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full(),
    );
    button
}
