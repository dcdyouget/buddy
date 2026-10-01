//! AddProviderPanel 根元素。

use super::panel::AddProviderPanel;
use crate::theme_system::{tokens::metrics as m, BuddyTheme};
use gpui::{div, prelude::*, px, Context, KeyDownEvent, Render, Window};

mod fields;
mod layout;
mod models;
mod presets;

impl Render for AddProviderPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.active && self.take_focus_pending() {
            if let Some(back) = self.focus_handle("back") {
                window.focus(&back, cx);
            }
        }
        let c = cx.buddy_theme().colors;
        let active = self.active && !self.saving;
        let cancel_map = self.bounds.clone();
        let cancel_focus = self.focus_handle("cancel").expect("provider cancel focus");
        let cancel_click_focus = cancel_focus.clone();
        let cancel = crate::settings::controls::button("cancel", "取消", cx)
            .relative()
            .text_size(px(m::FONT_SIZE_BASE))
            .bg(c.bg_elevated)
            .text_color(c.text_primary)
            .when(active, |d| {
                d.track_focus(&cancel_focus)
                    .on_click(cx.listener(move |_, _, window, cx| {
                        window.focus(&cancel_click_focus, cx);
                        cx.emit(super::ProviderEvent::Back);
                    }))
            })
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if this.active && !this.saving && activate(event) {
                    cx.emit(super::ProviderEvent::Back);
                    cx.stop_propagation();
                }
            }))
            .child(
                gpui::canvas(
                    |_, _, _| {},
                    move |rect, _, _, _| {
                        cancel_map.borrow_mut().insert("cancel".into(), rect);
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        let add_map = self.bounds.clone();
        let add_focus = self.focus_handle("add").expect("provider add focus");
        let add = crate::settings::controls::button(
            "add",
            if self.saving {
                "保存中…"
            } else {
                "添加"
            },
            cx,
        )
        .relative()
        .text_size(px(m::FONT_SIZE_BASE))
        .bg(c.buddy_primary)
        .text_color(c.text_on_primary)
        .border_color(c.buddy_primary)
        .when(!self.form.can_add() || !active, |d| {
            d.bg(c.border_default)
                .border_color(c.border_default)
                .opacity(0.5)
        })
        .when(active, |d| {
            d.track_focus(&add_focus)
                .on_click(cx.listener(|this, _, window, cx| {
                    window.focus(&this.focus_handle("add").expect("provider add focus"), cx);
                    this.submit(cx);
                }))
        })
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
            if this.active && !this.saving && activate(event) {
                this.submit(cx);
                cx.stop_propagation();
            }
        }))
        .child(
            gpui::canvas(
                |_, _, _| {},
                move |rect, _, _, _| {
                    add_map.borrow_mut().insert("add".into(), rect);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
        let panel_focus = self.focus.clone();
        div()
            .id("provider-panel")
            .track_focus(&panel_focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(c.bg_surface)
            .text_color(c.text_primary)
            .when(self.active, |d| {
                d.occlude()
                    .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key.as_str() == "tab" {
                            this.focus_next(window, cx, event.keystroke.modifiers.shift);
                            cx.notify();
                            cx.stop_propagation();
                        } else if activate(event) && this.activate_focused(window, cx) {
                            cx.stop_propagation();
                        }
                    }))
                    .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                        if event.keystroke.key.as_str() == "escape" {
                            cx.emit(super::ProviderEvent::Back);
                            cx.stop_propagation();
                        }
                    }))
            })
            .child(layout::header(self, cx))
            .child(layout::content(self, cx))
            .child(
                div()
                    .flex_none()
                    .px(px(m::SPACE_4))
                    .py(px(m::SPACE_3))
                    .flex()
                    .justify_end()
                    .gap(px(m::SPACE_3))
                    .border_t_1()
                    .border_color(c.border_subtle)
                    .child(cancel)
                    .child(add),
            )
    }
}

pub(super) fn activate(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
}
