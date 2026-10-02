use super::*;

impl Render for HotkeyRecorder {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.buddy_theme();
        let c = theme.colors;
        let active = self.active;
        let recording = self.state.recording;
        let saving = self.saving;
        let labels = self.display_keys_for_test();
        let button_bounds = self.button_bounds.clone();
        let button_label = if recording {
            "按下新快捷键..."
        } else {
            "重新录制"
        };
        let error = self.error.clone();
        let focus = self.focus.clone();

        let keys = div().flex().items_center().gap(px(m::SPACE_1)).children(
            labels.into_iter().enumerate().flat_map(|(index, label)| {
                let separator = (index > 0).then(|| {
                    div()
                        .text_color(c.text_tertiary)
                        .text_size(px(m::FONT_SIZE_SM))
                        .child("+")
                });
                [separator, Some(keycap(label, c))].into_iter().flatten()
            }),
        );

        let button = div()
            .id("settings-hotkey-record")
            .debug_selector(|| "settings-hotkey-record".to_string())
            .track_focus(&focus)
            .relative()
            .h(px(m::SPACE_6 + m::SPACE_1))
            .px(px(m::SPACE_2))
            .rounded(px(m::RADIUS_MD))
            .flex()
            .items_center()
            .justify_center()
            .border_1()
            .border_color(c.border_subtle)
            .bg(c.bg_sunken)
            .text_color(c.text_muted)
            .text_size(px(m::FONT_SIZE_XS))
            .font_weight(FontWeight(500.0))
            .when(active && !saving && !recording, |d| {
                d.cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| this.start_recording(window, cx)))
            })
            .child(button_label)
            .child(
                canvas(
                    |_, _, _| {},
                    move |area, _, _, _| button_bounds.set(Some(area)),
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );

        div()
            .id("settings-hotkey-recorder")
            .debug_selector(|| "settings-hotkey-recorder".to_string())
            .flex()
            .flex_col()
            .items_end()
            .gap(px(m::SPACE_1))
            .when(active, |d| {
                d.capture_key_down(cx.listener(Self::key_down))
                    .capture_key_up(cx.listener(Self::key_up))
                    .on_modifiers_changed(cx.listener(Self::modifiers_changed))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(m::SPACE_2))
                    .child(keys)
                    .child(button),
            )
            .when_some(error, |d, message| {
                d.child(
                    div()
                        .text_color(c.state_error)
                        .text_size(px(m::FONT_SIZE_XS))
                        .child(message),
                )
            })
    }
}

fn keycap(label: String, colors: &crate::theme_system::tokens::Palette) -> gpui::Div {
    div()
        .min_w(px(m::SPACE_6))
        .h(px(22.0))
        .px(px(m::SPACE_1))
        .rounded(px(m::RADIUS_SM))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(colors.border_default)
        .bg(colors.bg_sunken)
        .text_color(colors.text_primary)
        .text_size(px(m::FONT_SIZE_SM))
        .font_weight(FontWeight(500.0))
        .child(label)
}
