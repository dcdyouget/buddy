//! Provider 面板头部、预设和表单布局。

use super::super::panel::AddProviderPanel;
use super::{fields, models::render_models, presets};
use crate::{
    icons::{icon, IconName},
    settings::controls,
    theme_system::{tokens::metrics as m, BuddyTheme},
};
use gpui::{div, prelude::*, px, AnyElement, Context, FontWeight, KeyDownEvent};

pub(super) fn header(this: &AddProviderPanel, cx: &mut Context<AddProviderPanel>) -> AnyElement {
    let c = cx.buddy_theme().colors;
    let active = this.active && !this.saving;
    let back_map = this.bounds.clone();
    let back_focus = this.focus_handle("back").expect("provider back focus");
    let back_click_focus = back_focus.clone();
    let back = div()
        .id("back")
        .relative()
        .size(px(m::SPACE_8))
        .rounded(px(m::RADIUS_MD))
        .flex()
        .items_center()
        .justify_center()
        .text_color(c.text_muted)
        .when(active, |d| {
            d.track_focus(&back_focus)
                .cursor_pointer()
                .on_click(cx.listener(move |_, _, window, cx| {
                    window.focus(&back_click_focus, cx);
                    cx.emit(super::super::ProviderEvent::Back);
                }))
        })
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
            if this.active && !this.saving && super::activate(event) {
                cx.emit(super::super::ProviderEvent::Back);
                cx.stop_propagation();
            }
        }))
        .child(icon(IconName::ArrowLeft, px(m::FONT_SIZE_MD)))
        .child(
            gpui::canvas(
                |_, _, _| {},
                move |rect, _, _, _| {
                    back_map.borrow_mut().insert("back".into(), rect);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
    div()
        .flex_none()
        .h(px(m::SPACE_12 + m::SPACE_2))
        .px(px(m::SPACE_4))
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .child(back)
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(m::FONT_SIZE_LG))
                        .font_weight(FontWeight(600.0))
                        .child("添加模型"),
                )
                .child(
                    div()
                        .text_size(px(m::FONT_SIZE_XS))
                        .text_color(c.text_tertiary)
                        .child("连接模型服务并选择可用模型"),
                ),
        )
        .into_any_element()
}

pub(super) fn content(
    this: &mut AddProviderPanel,
    cx: &mut Context<AddProviderPanel>,
) -> AnyElement {
    let c = cx.buddy_theme().colors;
    let active = this.active && !this.saving;
    let custom = presets::custom_link(this, cx);
    let cards = presets::preset_grid(this, cx);
    let custom_mode = this.form.preset.as_deref() == Some("custom");
    let url = fields::field_row(this, cx, "url", "Base URL", this.url_field.clone());
    let key = fields::key_row(this, cx);
    let fetch_map = this.bounds.clone();
    let fetch_focus = this.focus_handle("fetch").expect("provider fetch focus");
    let fetch_click_focus = fetch_focus.clone();
    let fetch = controls::button(
        "fetch",
        if this.form.busy == super::super::Busy::Fetch {
            "获取中…"
        } else {
            "获取模型列表"
        },
        cx,
    )
    .relative()
    .text_size(px(m::FONT_SIZE_MD))
    .flex_1()
    .bg(c.buddy_primary)
    .text_color(c.text_on_primary)
    .border_color(c.buddy_primary)
    .when(!active || this.form.busy != super::super::Busy::Idle, |d| {
        d.opacity(0.45)
    })
    .when(active, |d| {
        d.track_focus(&fetch_focus)
            .on_click(cx.listener(move |this, _, window, cx| {
                window.focus(&fetch_click_focus, cx);
                this.begin_fetch(cx);
            }))
    })
    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
        if this.active && !this.saving && super::activate(event) {
            this.begin_fetch(cx);
            cx.stop_propagation();
        }
    }))
    .child(
        gpui::canvas(
            |_, _, _| {},
            move |rect, _, _, _| {
                fetch_map.borrow_mut().insert("fetch".into(), rect);
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full(),
    );
    let latency_map = this.bounds.clone();
    let latency_focus = this
        .focus_handle("latency")
        .expect("provider latency focus");
    let latency_click_focus = latency_focus.clone();
    let latency = controls::button(
        "latency",
        if this.form.busy == super::super::Busy::Latency {
            "测速中…"
        } else {
            "测速"
        },
        cx,
    )
    .when(!active || this.form.models.is_empty(), |d| d.opacity(0.45))
    .text_size(px(m::FONT_SIZE_MD))
    .when(active, |d| {
        d.track_focus(&latency_focus)
            .on_click(cx.listener(move |this, _, window, cx| {
                window.focus(&latency_click_focus, cx);
                this.begin_latency(cx);
            }))
    })
    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
        if this.active && !this.saving && super::activate(event) {
            this.begin_latency(cx);
            cx.stop_propagation();
        }
    }))
    .relative()
    .child(
        gpui::canvas(
            |_, _, _| {},
            move |rect, _, _, _| {
                latency_map.borrow_mut().insert("latency".into(), rect);
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full(),
    );
    let error = this.form.error.clone().map(|error| {
        div()
            .id("provider-error")
            .text_color(c.state_error)
            .text_size(px(m::FONT_SIZE_XS))
            .child(error)
    });
    let mut body = div()
        .id("provider-content")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&this.focus_nav.scroll)
        .p(px(m::SPACE_4))
        .gap(px(m::SPACE_3))
        .flex()
        .flex_col()
        .child(cards)
        .child(custom);
    if custom_mode {
        body = body
            .child(presets::protocol(this, cx))
            .child(presets::compat(this, cx));
    }
    body.child(url)
        .child(key)
        .child(div().flex().gap(px(m::SPACE_2)).child(fetch).child(latency))
        .children(error)
        .child(render_models(this, cx))
        .into_any_element()
}
