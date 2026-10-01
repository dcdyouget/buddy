//! URL / API key fields.
use super::super::panel::AddProviderPanel;
use crate::{
    icons::{icon, IconName},
    settings::controls,
    theme_system::{tokens::metrics as m, BuddyTheme},
};
use gpui::{canvas, div, prelude::*, px, AnyElement, Context, KeyDownEvent};

pub(super) fn field_row(
    this: &AddProviderPanel,
    cx: &mut Context<AddProviderPanel>,
    id: &'static str,
    title: &'static str,
    field: gpui::Entity<crate::settings::controls::SettingsField>,
) -> AnyElement {
    let map = this.bounds.clone();
    div()
        .id(id)
        .relative()
        .flex()
        .flex_col()
        .gap(px(m::SPACE_1))
        .child(label(title, cx))
        .child(
            div()
                .font_family(crate::theme_system::fonts::mono_font(cx).family)
                .child(field),
        )
        .child(
            canvas(
                |_, _, _| {},
                move |rect, _, _, _| {
                    map.borrow_mut().insert(id.into(), rect);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .into_any_element()
}

pub(super) fn key_row(this: &AddProviderPanel, cx: &mut Context<AddProviderPanel>) -> AnyElement {
    let c = cx.buddy_theme().colors;
    let map = this.bounds.clone();
    let show_map = this.bounds.clone();
    let active = this.active && !this.saving;
    let show_focus = this
        .focus_handle("show-key")
        .expect("provider show-key focus");
    let show_click_focus = show_focus.clone();
    let get_focus = this
        .focus_handle("get-key")
        .expect("provider get-key focus");
    let show_button = controls::button("show-key", "", cx)
        .relative()
        .track_focus(&show_focus)
        .child(icon(
            if this.form.show_key {
                IconName::EyeOff
            } else {
                IconName::Eye
            },
            px(m::FONT_SIZE_SM),
        ))
        .when(!active, |d| d.opacity(0.45))
        .when(active, |d| {
            d.on_click(cx.listener(move |this, _, window, cx| {
                window.focus(&show_click_focus, cx);
                this.toggle_key_visibility(cx);
            }))
        })
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
            if this.active && !this.saving && super::activate(event) {
                this.toggle_key_visibility(cx);
                cx.stop_propagation();
            }
        }))
        .child(
            canvas(
                |_, _, _| {},
                move |rect, _, _, _| {
                    show_map.borrow_mut().insert("show-key".into(), rect);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
    div()
        .id("key")
        .relative()
        .flex()
        .flex_col()
        .gap(px(m::SPACE_1))
        .child(label("API Key", cx))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(m::SPACE_2))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_family(crate::theme_system::fonts::mono_font(cx).family)
                        .child(this.key_field.clone()),
                )
                .child(show_button)
                .child(
                    div()
                        .id("get-key")
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(m::SPACE_1 / 2.0))
                        .text_size(px(m::FONT_SIZE_BASE))
                        .when(active, |d| {
                            d.track_focus(&get_focus).cursor_pointer().on_click(
                                cx.listener(move |_, _, window, cx| window.focus(&get_focus, cx)),
                            )
                        })
                        .text_color(c.buddy_primary)
                        .child("获取 Key")
                        .child(icon(IconName::ExternalLink, px(m::FONT_SIZE_XS))),
                ),
        )
        .child(
            canvas(
                |_, _, _| {},
                move |rect, _, _, _| {
                    map.borrow_mut().insert("key".into(), rect);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .into_any_element()
}

pub(super) fn label(text: &'static str, cx: &Context<AddProviderPanel>) -> AnyElement {
    div()
        .text_color(cx.buddy_theme().colors.text_muted)
        .text_size(px(m::FONT_SIZE_BASE))
        .child(text)
        .into_any_element()
}
