//! 输入区的更多菜单，使用 Buddy 自己的菜单令牌。

use super::*;

impl Composer {
    pub(super) fn more_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.buddy_theme().colors;
        let recorded = self.more_bounds.clone();
        div()
            .id("composer-more")
            .debug_selector(|| "composer-more".into())
            .relative()
            .size(px(m::SPACE_6))
            .flex_none()
            .rounded(px(m::RADIUS_FULL))
            .flex()
            .items_center()
            .justify_center()
            .text_color(c.text_muted)
            .when(self.more_open, |d| {
                d.bg(c.bg_sunken).text_color(c.text_primary)
            })
            .when(self.enabled, |d| {
                d.track_focus(&self.more_focus)
                    .focus_visible(|s| s.bg(c.primary_tint_soft))
                    .hover(|s| s.bg(c.bg_sunken).text_color(c.text_primary))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.more_open = !this.more_open;
                        window.focus(&this.more_focus, cx);
                        cx.notify();
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        match event.keystroke.key.as_str() {
                            "escape" if this.more_open => {
                                this.more_open = false;
                                window.focus(&this.text.focus_handle(cx), cx);
                            }
                            "enter" | "space" if this.more_open => {
                                this.more_open = false;
                                cx.emit(ComposerEvent::SearchHistory);
                            }
                            "enter" | "space" | "down" | "up" => this.more_open = true,
                            _ => return,
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
            })
            .tooltip(|_, cx| TextTooltip::view("更多", cx))
            .child("⋯")
            .child(
                gpui::canvas(
                    |_, _, _| {},
                    move |bounds, _, _, _| recorded.set(Some(bounds)),
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
    }

    pub(super) fn more_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.buddy_theme().colors;
        div()
            .id("composer-more-menu")
            .absolute()
            .right(px(m::SPACE_2))
            .bottom_full()
            .mb(px(m::SPACE_1))
            .p(px(m::SPACE_1))
            .min_w(px(m::SPACE_12 * 4.0))
            .rounded(px(m::RADIUS_MD))
            .border_1()
            .border_color(c.border_default)
            .bg(c.bg_elevated)
            .shadow(box_shadows(cx.buddy_theme().shadows.shadow_floating_sm))
            .occlude()
            .on_mouse_down_out(
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    // 触发按钮的点击由它自身完成开关，不能先关闭后又打开。
                    if this
                        .more_bounds
                        .get()
                        .is_some_and(|bounds| bounds.contains(&event.position))
                    {
                        return;
                    }
                    this.more_open = false;
                    window.focus(&this.text.focus_handle(cx), cx);
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("search-history-command")
                    .debug_selector(|| "search-history-command".into())
                    .flex()
                    .items_center()
                    .gap(px(m::SPACE_2))
                    .px(px(m::SPACE_2))
                    .py(px(m::SPACE_2))
                    .rounded(px(m::RADIUS_SM))
                    .bg(c.primary_tint_soft)
                    .text_color(c.text_primary)
                    .text_size(px(m::FONT_SIZE_SM))
                    .hover(|s| s.bg(c.bg_sunken))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.more_open = false;
                        cx.emit(ComposerEvent::SearchHistory);
                        cx.notify();
                    }))
                    .child(icon(IconName::Search, px(m::FONT_SIZE_MD)))
                    .child(div().flex_1().child("搜索历史"))
                    .child(
                        div()
                            .text_color(c.text_tertiary)
                            .text_size(px(m::FONT_SIZE_XS))
                            .child(crate::chat::history_search::SHORTCUT_LABEL),
                    ),
            )
    }
}
