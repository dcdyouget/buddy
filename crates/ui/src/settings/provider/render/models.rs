//! 获取到的模型列表与能力开关。

use super::super::panel::AddProviderPanel;
use crate::{
    icons::{icon, IconName},
    theme_system::{tokens::metrics as m, BuddyTheme},
};
use gpui::{canvas, div, prelude::*, px, AnyElement, Context, FontWeight};

pub(super) fn render_models(
    this: &AddProviderPanel,
    cx: &mut Context<AddProviderPanel>,
) -> AnyElement {
    let c = cx.buddy_theme().colors;
    if this.form.models.is_empty() {
        return div().id("models").into_any_element();
    }
    let mut list = div()
        .id("models")
        .flex()
        .flex_col()
        .gap(px(m::SPACE_2))
        .child(
            div()
                .text_size(px(m::FONT_SIZE_LG))
                .font_weight(FontWeight(600.0))
                .child(format!("可用模型（{}）", this.form.models.len())),
        );
    for model in this.form.models.iter().cloned() {
        let id = model.id.clone();
        let selected = this.form.selected.contains(&id);
        let context = this
            .model_context
            .iter()
            .find(|(known, _)| known == &id)
            .map(|(_, select)| select.clone());
        let context_map = this.bounds.clone();
        let model_map = this.bounds.clone();
        let vision_map = this.bounds.clone();
        let image_map = this.bounds.clone();
        let model_bound_id = id.clone();
        let context_id = id.clone();
        let vision_id = format!("vision-{id}");
        let image_id = format!("image-{id}");
        let model_focus = this
            .model_focus(&id, "model")
            .expect("provider model focus");
        let model_click_focus = model_focus.clone();
        let model_key_id = id.clone();
        let model_enabled = this.active && !this.saving;
        let row = div()
            .id(format!("model-{id}"))
            .relative()
            .flex()
            .items_center()
            .gap(px(m::SPACE_2))
            .py(px(m::SPACE_1))
            .child(
                div()
                    .id(format!("select-{id}"))
                    .relative()
                    .size(px(m::SPACE_4))
                    .rounded(px(m::RADIUS_SM))
                    .border_1()
                    .border_color(if selected {
                        c.buddy_primary
                    } else {
                        c.border_default
                    })
                    .bg(if selected {
                        c.buddy_primary
                    } else {
                        c.bg_sunken
                    })
                    .text_color(c.text_on_primary)
                    .cursor_pointer()
                    .when(model_enabled, |d| d.track_focus(&model_focus))
                    .when(model_enabled, |d| {
                        d.on_click(cx.listener({
                            let id = id.clone();
                            move |this, _, window, cx| {
                                window.focus(&model_click_focus, cx);
                                this.form.toggle_model(&id);
                                cx.notify();
                            }
                        }))
                    })
                    .on_key_down(cx.listener(move |this, event, _, cx| {
                        if this.active && !this.saving && super::activate(event) {
                            this.form.toggle_model(&model_key_id);
                            cx.stop_propagation();
                            cx.notify();
                        }
                    }))
                    .when(selected, |d| {
                        d.child(icon(IconName::Check, px(m::FONT_SIZE_SM)))
                    })
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |rect, _, _, _| {
                                model_map
                                    .borrow_mut()
                                    .insert(format!("model-{model_bound_id}"), rect);
                            },
                        )
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(m::FONT_SIZE_MD))
                    .child(model.display_name.clone()),
            )
            .when_some(model.latency_ms, |d, latency| {
                d.child(
                    div()
                        .text_size(px(m::FONT_SIZE_XS))
                        .text_color(c.text_muted)
                        .child(format!("{latency}ms")),
                )
            })
            .child(toggle(
                this,
                &vision_id,
                "支持图片",
                model.supports_vision,
                vision_map,
                cx,
                id.clone(),
                true,
            ))
            .child(toggle(
                this,
                &image_id,
                "支持生图",
                model.supports_image_generation,
                image_map,
                cx,
                id,
                false,
            ))
            .when_some(context, |d, select| {
                d.child(
                    div()
                        .id(format!("context-{context_id}"))
                        .w(px(m::SPACE_8 * 2.0))
                        .flex_none()
                        .relative()
                        .child(select)
                        .child(
                            canvas(
                                |_, _, _| {},
                                move |rect, _, _, _| {
                                    context_map
                                        .borrow_mut()
                                        .insert(format!("context-{context_id}"), rect);
                                },
                            )
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full(),
                        ),
                )
            });
        list = list.child(row);
    }
    list.into_any_element()
}

fn toggle(
    this: &AddProviderPanel,
    id: &str,
    label: &'static str,
    checked: bool,
    map: std::rc::Rc<
        std::cell::RefCell<std::collections::BTreeMap<String, gpui::Bounds<gpui::Pixels>>>,
    >,
    cx: &mut Context<AddProviderPanel>,
    model_id: String,
    vision: bool,
) -> gpui::Stateful<gpui::Div> {
    let c = cx.buddy_theme().colors;
    let stable = id.to_owned();
    let focus = this
        .model_focus(model_id.as_str(), if vision { "vision" } else { "image" })
        .expect("provider model toggle focus");
    let click_focus = focus.clone();
    let key_model_id = model_id.clone();
    let enabled = this.active && !this.saving && (vision || this.form.protocol != "anthropic");
    let checkbox = div()
        .size(px(m::FONT_SIZE_MD))
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
        });
    div()
        .id(id.to_owned())
        .debug_selector(move || format!("settings-model-capability-{id}"))
        .relative()
        .flex()
        .items_center()
        .gap(px(m::SPACE_1))
        .text_color(c.text_muted)
        .text_size(px(m::FONT_SIZE_XS))
        .cursor_pointer()
        .when(enabled, |d| d.track_focus(&focus))
        .when(!enabled, |d| d.opacity(0.45))
        .when(enabled, |d| {
            d.on_click(cx.listener(move |this, _, window, cx| {
                window.focus(&click_focus, cx);
                if vision {
                    this.form.set_vision_support(&model_id, !checked);
                } else {
                    this.form.set_image_generation_support(&model_id, !checked);
                }
                cx.stop_propagation();
                cx.notify();
            }))
        })
        .on_key_down(cx.listener(move |this, event, _, cx| {
            if enabled && super::activate(event) {
                if vision {
                    this.form.set_vision_support(&key_model_id, !checked);
                } else {
                    this.form
                        .set_image_generation_support(&key_model_id, !checked);
                }
                cx.stop_propagation();
                cx.notify();
            }
        }))
        .child(checkbox)
        .child(label)
        .child(
            canvas(
                |_, _, _| {},
                move |rect, _, _, _| {
                    map.borrow_mut().insert(stable.clone(), rect);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
}
