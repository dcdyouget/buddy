use super::*;
use crate::chat::drag;

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_pending) {
            window.focus(&self.back, cx);
        }
        let c = cx.buddy_theme().colors;
        let interactive = self.active && !self.provider_motion.interactive();
        let header = div()
            .relative()
            .flex_none()
            .h(px(m::SPACE_12 + m::SPACE_2))
            .flex()
            .items_center()
            .gap(px(m::SPACE_2))
            .px(px(m::SPACE_4))
            .py(px(m::SPACE_3))
            .children([
                drag::region_if(&self.drag_source, interactive)
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(m::SPACE_3)),
                drag::region_if(&self.drag_source, interactive)
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(m::SPACE_3)),
            ])
            .child(
                div()
                    .id("settings-back")
                    .size(px(18.0 + m::SPACE_2))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(m::RADIUS_SM))
                    .text_color(c.text_muted)
                    .when(interactive, |d| {
                        d.track_focus(&self.back)
                            .cursor_pointer()
                            .focus(|s| s.bg(c.bg_sunken).text_color(c.buddy_primary))
                            .hover(|s| s.bg(c.bg_sunken).text_color(c.text_primary))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Back)))
                    })
                    .child(icon(IconName::ArrowLeft, px(18.0))),
            )
            .child(
                div()
                    .text_size(px(m::FONT_SIZE_LG))
                    .font_weight(FontWeight(600.0))
                    .text_color(c.text_primary)
                    .child("设置"),
            );
        let models = self.render_models(interactive, cx);
        let content = div()
            .id("settings-content")
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .when(interactive, |d| {
                d.overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            })
            .when(!interactive, |d| d.overflow_hidden())
            .child(
                div()
                    .relative()
                    .w_full()
                    .flex()
                    .flex_col()
                    .p(px(m::SPACE_4))
                    .children([
                        drag::region_if(&self.drag_source, interactive)
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .h(px(m::SPACE_4)),
                        drag::region_if(&self.drag_source, interactive)
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .right_0()
                            .h(px(m::SPACE_4)),
                        drag::region_if(&self.drag_source, interactive)
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left_0()
                            .w(px(m::SPACE_4)),
                        drag::region_if(&self.drag_source, interactive)
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right_0()
                            .w(px(m::SPACE_4)),
                    ])
                    .child(crate::settings::controls::section(
                        "外观",
                        "选择窗口的显示模式",
                        self.theme.clone(),
                        cx,
                    ))
                    .child(
                        drag::region_if(&self.drag_source, interactive)
                            .flex_none()
                            .w_full()
                            .h(px(m::SPACE_3)),
                    )
                    .child(crate::settings::controls::section(
                        "呼出快捷键",
                        "在任意应用中快速打开 Buddy",
                        self.hotkey.clone(),
                        cx,
                    ))
                    .child(
                        drag::region_if(&self.drag_source, interactive)
                            .flex_none()
                            .w_full()
                            .h(px(m::SPACE_3)),
                    )
                    // v1 顺序：外观 → 快捷键 → 软件更新 → 模型
                    .child(self.update.clone())
                    .child(
                        drag::region_if(&self.drag_source, interactive)
                            .flex_none()
                            .w_full()
                            .h(px(m::SPACE_3)),
                    )
                    .child(models),
            );
        let provider = self.provider_motion.present().then(|| {
            let amount = self.provider_motion.amount();
            if self.provider_motion.animating() {
                window.request_animation_frame();
            }
            div()
                .absolute()
                .top_0()
                .left(gpui::relative(1.0 - amount))
                .size_full()
                .opacity(amount)
                .child(self.provider.clone())
        });
        let bounds = self.bounds.clone();
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .rounded(px(m::RADIUS_XL))
            .border_1()
            .border_color(c.border_default)
            .bg(c.bg_surface)
            .shadow(vec![gpui::BoxShadow {
                color: c.window_inner_highlight.into(),
                offset: gpui::point(px(0.0), px(0.0)),
                blur_radius: px(0.0),
                spread_radius: px(1.0),
                inset: true,
            }])
            .text_color(c.text_primary)
            .text_size(px(m::FONT_SIZE_BASE))
            .overflow_hidden()
            .when(interactive, |d| {
                d.occlude().capture_key_down(cx.listener(
                    |this, event: &KeyDownEvent, window, cx| {
                        if this.hotkey.read(cx).recording() {
                            return;
                        }
                        match event.keystroke.key.as_str() {
                            "tab" => {
                                // 与可见页面顺序一致；录制期间 Tab 本身是候选主键。
                                let mut order = vec![("back".to_string(), this.back.clone())];
                                order.extend(this.theme.read(cx).focus_controls());
                                if let Some(focus) = this.hotkey.read(cx).focus_control(cx) {
                                    order.push(("hotkey".to_string(), focus));
                                }
                                order.extend(this.update.read(cx).focus_controls());
                                order.extend(this.model_list.read(cx).focus_order(cx).into_iter());
                                order.push(("add".to_string(), this.add.clone()));
                                let current =
                                    order.iter().position(|(_, focus)| focus.is_focused(window));
                                let next = match current {
                                    Some(i) if event.keystroke.modifiers.shift => {
                                        (i + order.len() - 1) % order.len()
                                    }
                                    Some(i) => (i + 1) % order.len(),
                                    None if event.keystroke.modifiers.shift => order.len() - 1,
                                    None => 0,
                                };
                                let (id, focus) = &order[next];
                                window.focus(focus, cx);
                                if id != "back" {
                                    this.scroll_to_model_control(id, cx);
                                }
                                cx.stop_propagation();
                            }
                            "enter" | "space" if this.back.is_focused(window) => {
                                cx.emit(SettingsEvent::Back);
                                cx.stop_propagation();
                            }
                            "enter" | "space" if this.add.is_focused(window) => {
                                this.open_provider(cx);
                                cx.stop_propagation();
                            }
                            _ => {}
                        }
                    },
                ))
            })
            // v1 buddy-shell：实色底上的表面高光，不影响子项布局。
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(gpui::linear_gradient(
                        145.0,
                        gpui::linear_color_stop(c.surface_highlight, 0.0),
                        gpui::linear_color_stop(
                            gpui::Hsla::from(c.surface_highlight).opacity(0.0),
                            0.38,
                        ),
                    )),
            )
            .child(header)
            .child(content)
            .children(provider)
            .child(
                canvas(|_, _, _| {}, move |area, _, _, _| bounds.set(Some(area)))
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
            )
    }
}
