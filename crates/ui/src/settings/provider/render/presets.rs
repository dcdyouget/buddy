//! Preset / compat controls.
use super::super::panel::AddProviderPanel;
use super::fields::label;
use crate::settings::provider_presets::PRESETS as PROVIDER_PRESETS;
use crate::{
    icons::{icon, IconName},
    theme_system::{tokens::metrics as m, BuddyTheme},
};
use gpui::{canvas, div, prelude::*, px, AnyElement, Context, FontWeight, KeyDownEvent};

pub(super) fn preset_grid(
    this: &AddProviderPanel,
    cx: &mut Context<AddProviderPanel>,
) -> AnyElement {
    let c = cx.buddy_theme().colors;
    let active = this.active && !this.saving;
    let mut grid = div().flex().flex_col().gap(px(m::SPACE_3));
    let mut row = div().flex().gap(px(m::SPACE_3));
    for (index, preset) in PROVIDER_PRESETS.iter().copied().enumerate() {
        let id = preset.id.to_owned();
        let click_id = id.clone();
        let key_id = id.clone();
        let debug_id = id.clone();
        let bound_id = id.clone();
        let map = this.bounds.clone();
        let card_focus = this
            .focus_handle(&format!("preset-{id}"))
            .expect("provider preset focus");
        let card_click_focus = card_focus.clone();
        let selected = this.form.preset.as_deref() == Some(preset.id);
        let card = div()
            .id(format!("preset-{id}"))
            .debug_selector(move || format!("provider-preset-{debug_id}"))
            .flex_1()
            .min_h(px(m::SPACE_12 + m::SPACE_2))
            .min_w(px(0.0))
            .p(px(m::SPACE_3))
            .rounded(px(m::RADIUS_LG))
            .border_1()
            .border_color(if selected {
                c.buddy_primary
            } else {
                c.border_subtle
            })
            .bg(c.bg_elevated)
            .text_color(c.text_primary)
            .relative()
            .flex()
            .items_center()
            .gap(px(m::SPACE_2))
            .cursor_pointer()
            .when(active, |d| {
                d.track_focus(&card_focus)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        window.focus(&card_click_focus, cx);
                        this.select_preset(&click_id, cx);
                    }))
            })
            .when(active, |d| {
                d.on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    if super::activate(event) {
                        this.select_preset(&key_id, cx);
                        cx.stop_propagation();
                    }
                }))
            })
            .when(!active, |d| d.opacity(0.55))
            .child(
                div()
                    .size(px(m::SPACE_6 + m::SPACE_1))
                    .rounded(px(m::RADIUS_MD))
                    .bg(if selected {
                        c.buddy_primary
                    } else {
                        c.bg_sunken
                    })
                    .text_color(if selected {
                        c.text_on_primary
                    } else {
                        c.text_muted
                    })
                    .text_size(px(m::FONT_SIZE_BASE))
                    .font_weight(FontWeight(700.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(preset.letter),
            )
            .child(
                div()
                    .text_color(if selected {
                        c.buddy_primary
                    } else {
                        c.text_primary
                    })
                    .text_size(px(m::FONT_SIZE_BASE))
                    .font_weight(if selected {
                        FontWeight(600.0)
                    } else {
                        FontWeight(400.0)
                    })
                    .child(preset.name),
            )
            .when(preset.protocol == "anthropic", |d| {
                d.child(
                    div()
                        .absolute()
                        .top(px(m::SPACE_1))
                        .right(px(m::SPACE_1))
                        .size(px(m::SPACE_4))
                        .rounded(px(m::RADIUS_FULL))
                        .bg(if selected {
                            c.buddy_primary
                        } else {
                            c.bg_sunken
                        })
                        .text_color(if selected {
                            c.text_on_primary
                        } else {
                            c.text_muted
                        })
                        .text_size(px(m::FONT_SIZE_XS))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child("A"),
                )
            })
            .child(
                canvas(
                    |_, _, _| {},
                    move |rect, _, _, _| {
                        map.borrow_mut().insert(format!("preset-{bound_id}"), rect);
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        row = row.child(card);
        if index % 2 == 1 {
            grid = grid.child(row);
            row = div().flex().gap(px(m::SPACE_3));
        }
    }
    if PROVIDER_PRESETS.len() % 2 == 1 {
        grid = grid.child(
            row.child(
                div()
                    .flex_1()
                    .p(px(m::SPACE_3))
                    .border_1()
                    .border_color(gpui::Hsla::from(c.border_subtle).opacity(0.0)),
            ),
        );
    }
    grid.into_any_element()
}

pub(super) fn custom_link(
    this: &AddProviderPanel,
    cx: &mut Context<AddProviderPanel>,
) -> AnyElement {
    let map = this.bounds.clone();
    let active = this.active && !this.saving;
    let custom_focus = this.focus_handle("custom").expect("provider custom focus");
    let custom_click_focus = custom_focus.clone();
    div()
        .id("custom")
        .relative()
        .text_color(cx.buddy_theme().colors.buddy_primary)
        .text_size(px(m::FONT_SIZE_SM))
        .cursor_pointer()
        .when(active, |d| {
            d.track_focus(&custom_focus)
                .on_click(cx.listener(move |this, _, window, cx| {
                    window.focus(&custom_click_focus, cx);
                    this.select_preset("custom", cx);
                }))
        })
        .when(active, |d| {
            d.on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if super::activate(event) {
                    this.select_preset("custom", cx);
                    cx.stop_propagation();
                }
            }))
        })
        .child("+ 自定义兼容服务")
        .child(
            canvas(
                |_, _, _| {},
                move |rect, _, _, _| {
                    map.borrow_mut().insert("custom".into(), rect);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .into_any_element()
}

pub(super) fn protocol(this: &AddProviderPanel, cx: &mut Context<AddProviderPanel>) -> AnyElement {
    let map = this.bounds.clone();
    div()
        .id("protocol")
        .relative()
        .flex()
        .flex_col()
        .gap(px(m::SPACE_1))
        .child(label("Provider 类型", cx))
        .child(this.protocol.clone())
        .child(
            canvas(
                |_, _, _| {},
                move |rect, _, _, _| {
                    map.borrow_mut().insert("protocol".into(), rect);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .into_any_element()
}

pub(super) fn compat(
    this: &mut AddProviderPanel,
    cx: &mut Context<AddProviderPanel>,
) -> AnyElement {
    let c = cx.buddy_theme().colors;
    let map = this.bounds.clone();
    let active = this.active && !this.saving;
    let compat_focus = this
        .focus_handle("compat-toggle")
        .expect("provider compat focus");
    let compat_click_focus = compat_focus.clone();
    let chevron = if this.compat_open {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    let body = if this.compat_open && this.form.protocol == "openai_compatible" {
        div()
            .pl(px(m::SPACE_2))
            .gap(px(m::SPACE_2))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(label("Thinking 格式", cx))
                    .child(this.thinking_select.clone()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(label("Max Tokens 字段", cx))
                    .child(this.max_tokens_select.clone()),
            )
    } else {
        div()
    };
    div()
        .id("compat")
        .relative()
        .flex()
        .flex_col()
        .gap(px(m::SPACE_1))
        .child(
            div()
                .id("compat-toggle")
                .flex()
                .items_center()
                .gap(px(m::SPACE_1))
                .text_color(c.text_muted)
                .text_size(px(m::FONT_SIZE_SM))
                .cursor_pointer()
                .when(!active, |d| d.opacity(0.45))
                .when(active, |d| {
                    d.track_focus(&compat_focus).on_click(cx.listener(
                        move |this, _, window, cx| {
                            window.focus(&compat_click_focus, cx);
                            this.toggle_compat(cx);
                        },
                    ))
                })
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if this.active && !this.saving && super::activate(event) {
                        this.toggle_compat(cx);
                        cx.stop_propagation();
                    }
                }))
                .child(icon(chevron, px(m::FONT_SIZE_SM)))
                .child("兼容性配置 (Compat)"),
        )
        .child(body)
        .child(
            canvas(
                |_, _, _| {},
                move |rect, _, _, _| {
                    map.borrow_mut().insert("compat".into(), rect);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .into_any_element()
}
