use super::*;

impl SettingsView {
    pub(super) fn render_models(&self, interactive: bool, cx: &mut Context<Self>) -> gpui::Div {
        let c = cx.buddy_theme().colors;
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(m::SPACE_2))
            .rounded(px(m::RADIUS_LG))
            .bg(c.bg_elevated)
            .shadow(vec![gpui::BoxShadow {
                color: c.border_subtle.into(),
                offset: gpui::point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: px(1.),
                inset: true,
            }])
            .px(px(m::SPACE_4))
            .py(px(m::SPACE_3))
            .child(self.model_list.clone())
            .child(
                div()
                    .id("settings-add-provider")
                    .relative()
                    .w_full()
                    .p(px(m::SPACE_2))
                    .rounded(px(m::RADIUS_MD))
                    .bg(c.bg_sunken)
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(m::SPACE_1))
                    .text_size(px(m::FONT_SIZE_XS))
                    .text_color(c.text_muted)
                    .when(interactive, |d| {
                        d.track_focus(&self.add)
                            .cursor_pointer()
                            .hover(|s| s.text_color(c.text_primary).border_color(c.border_strong))
                            .on_click(cx.listener(|this, _, _, cx| this.open_provider(cx)))
                    })
                    .child(icon(IconName::Plus, px(m::FONT_SIZE_SM)))
                    .child("添加模型")
                    .child(
                        canvas(|_, _, _| {}, {
                            let recorded = self.add_bounds.clone();
                            move |bounds, _, _, _| recorded.set(Some(bounds))
                        })
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    ),
            )
    }
}
