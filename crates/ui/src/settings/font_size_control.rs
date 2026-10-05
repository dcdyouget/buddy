//! 设置页字体大小步进控件。
//!
//! 与外观控件相同：控件只发出 [`FontSizeChanged`]，写盘与全局字号由 Router 负责。

use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use buddy_engine::models::{DEFAULT_FONT_SIZE, FONT_SIZE_RANGE};
use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, FontWeight, KeyDownEvent, Render,
    SharedString, Window, div, prelude::*, px,
};

/// 用户发起的字号调整（px）。
#[derive(Clone, Debug)]
pub struct FontSizeChanged(pub u32);

impl EventEmitter<FontSizeChanged> for FontSizeControl {}

/// 对话正文字号步进器：「A−  14px  A+」。
pub struct FontSizeControl {
    size: u32,
    active: bool,
    saving: bool,
    error: Option<String>,
    smaller_focus: FocusHandle,
    larger_focus: FocusHandle,
}

impl Focusable for FontSizeControl {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.smaller_focus.clone()
    }
}

impl FontSizeControl {
    /// 创建受控字号步进器。
    pub fn new(size: u32, cx: &mut Context<Self>) -> Self {
        Self {
            size,
            active: true,
            saving: false,
            error: None,
            smaller_focus: cx.focus_handle(),
            larger_focus: cx.focus_handle(),
        }
    }

    /// 将配置中的字号同步到控件；外部同步不会发出编辑事件。
    pub fn set_size(&mut self, size: u32, cx: &mut Context<Self>) {
        if self.size == size {
            return;
        }
        self.size = size;
        self.error = None;
        cx.notify();
    }

    /// 设置页退出或子面板打开时释放交互。
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.active == active {
            return;
        }
        self.active = active;
        cx.notify();
    }

    /// Router 开始或结束配置保存。
    pub fn set_saving(&mut self, saving: bool, cx: &mut Context<Self>) {
        if self.saving == saving {
            return;
        }
        self.saving = saving;
        if saving {
            self.error = None;
        }
        cx.notify();
    }

    /// 保存失败后恢复可编辑状态并显示错误。
    pub fn save_failed(&mut self, error: impl Into<String>, cx: &mut Context<Self>) {
        self.error = Some(format!("保存字体大小失败：{}", error.into()));
        self.saving = false;
        cx.notify();
    }

    /// 当前控件显示的字号。
    pub fn size(&self) -> u32 {
        self.size
    }

    /// 设置页 Tab 导航使用的稳定顺序。
    pub fn focus_controls(&self) -> Vec<(String, FocusHandle)> {
        if !self.active || self.saving {
            return Vec::new();
        }
        vec![
            ("font-size-smaller".to_owned(), self.smaller_focus.clone()),
            ("font-size-larger".to_owned(), self.larger_focus.clone()),
        ]
    }

    fn step(&mut self, delta: i32, cx: &mut Context<Self>) {
        if !self.active || self.saving {
            return;
        }
        let Some(next) = stepped(self.size, delta) else {
            return;
        };
        self.error = None;
        cx.emit(FontSizeChanged(next));
        cx.notify();
    }

    fn key_down(&mut self, delta: i32, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if matches!(event.keystroke.key.as_str(), "enter" | "space" | " ") {
            cx.stop_propagation();
            self.step(delta, cx);
        }
    }

    fn button(
        &self,
        id: &'static str,
        label: &'static str,
        delta: i32,
        focus: &FocusHandle,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let c = cx.buddy_theme().colors;
        let enabled = self.active && !self.saving && stepped(self.size, delta).is_some();
        div()
            .id(id)
            .size(px(m::SPACE_6 + m::SPACE_1))
            .rounded(px(m::RADIUS_SM))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(m::FONT_SIZE_XS))
            .font_weight(FontWeight(600.0))
            .text_color(if enabled { c.text_primary } else { c.text_tertiary })
            .when(enabled, |d| {
                d.track_focus(focus)
                    .cursor_pointer()
                    .focus(|s| s.bg(c.primary_tint_soft).text_color(c.buddy_primary))
                    .hover(|s| s.bg(c.bg_elevated))
                    .on_click(cx.listener(move |this, _, _, cx| this.step(delta, cx)))
                    .on_key_down(cx.listener(move |this, event, _, cx| this.key_down(delta, event, cx)))
            })
            .child(label)
    }
}

/// 按步长调整后的字号；越界时返回 `None`。
fn stepped(size: u32, delta: i32) -> Option<u32> {
    let next = size.checked_add_signed(delta)?;
    FONT_SIZE_RANGE.contains(&next).then_some(next)
}

impl Render for FontSizeControl {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.buddy_theme().colors;
        let smaller_focus = self.smaller_focus.clone();
        let larger_focus = self.larger_focus.clone();
        let label: SharedString = if self.size == DEFAULT_FONT_SIZE {
            format!("{}px（默认）", self.size).into()
        } else {
            format!("{}px", self.size).into()
        };
        let error = self.error.clone();
        div()
            .id("font-size-control")
            .flex()
            .flex_col()
            .items_end()
            .gap(px(m::SPACE_1))
            .max_w(px(m::SPACE_12 * 4.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(m::SPACE_1))
                    .p(px(m::SPACE_1))
                    .rounded(px(m::RADIUS_MD))
                    .bg(c.bg_sunken)
                    .opacity(if self.saving { 0.6 } else { 1.0 })
                    .child(self.button("font-size-smaller", "A−", -1, &smaller_focus, cx))
                    .child(
                        div()
                            .min_w(px(m::SPACE_12 + m::SPACE_6))
                            .flex()
                            .justify_center()
                            .text_size(px(m::FONT_SIZE_XS))
                            .font_weight(FontWeight(500.0))
                            .text_color(c.text_primary)
                            .child(label),
                    )
                    .child(self.button("font-size-larger", "A+", 1, &larger_focus, cx)),
            )
            .when_some(error, |d, message| {
                d.child(
                    div()
                        .max_w(px(m::SPACE_12 * 4.0))
                        .text_color(c.state_error)
                        .text_size(px(m::FONT_SIZE_XS))
                        .child(message),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_stay_within_range() {
        assert_eq!(stepped(DEFAULT_FONT_SIZE, 1), Some(DEFAULT_FONT_SIZE + 1));
        assert_eq!(stepped(DEFAULT_FONT_SIZE, -1), Some(DEFAULT_FONT_SIZE - 1));
        assert_eq!(stepped(*FONT_SIZE_RANGE.end(), 1), None);
        assert_eq!(stepped(*FONT_SIZE_RANGE.start(), -1), None);
        assert_eq!(stepped(0, -1), None);
    }
}
