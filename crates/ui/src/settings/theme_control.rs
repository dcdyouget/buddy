//! 设置页外观分段控件。
//!
//! The router owns persistence and global theme mutation via [`ThemeChanged`].

use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use buddy_engine::models::Theme;
use gpui::{
    App, Bounds, Context, EventEmitter, FocusHandle, Focusable, FontWeight, KeyDownEvent, Pixels,
    Render, Window, canvas, div, prelude::*, px,
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

/// A user initiated appearance choice.
#[derive(Clone, Debug)]
pub struct ThemeChanged(pub Theme);

impl EventEmitter<ThemeChanged> for ThemeControl {}

/// 浅色 / 深色设置控件。
pub struct ThemeControl {
    theme: Theme,
    active: bool,
    saving: bool,
    error: Option<String>,
    light_focus: FocusHandle,
    dark_focus: FocusHandle,
    bounds: Rc<RefCell<BTreeMap<String, Bounds<Pixels>>>>,
}

impl Focusable for ThemeControl {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.light_focus.clone()
    }
}

impl ThemeControl {
    /// 创建受控外观选择器。
    pub fn new(theme: Theme, cx: &mut Context<Self>) -> Self {
        Self {
            theme,
            active: true,
            saving: false,
            error: None,
            light_focus: cx.focus_handle(),
            dark_focus: cx.focus_handle(),
            bounds: Rc::new(RefCell::new(BTreeMap::new())),
        }
    }

    /// 将配置中的主题同步到控件；外部同步不会发出编辑事件。
    pub fn set_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        if same_theme(&self.theme, &theme) {
            return;
        }
        self.theme = theme;
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

    /// 保存失败后恢复可编辑状态，同时保留当前选择和错误信息。
    pub fn save_failed(&mut self, error: impl Into<String>, cx: &mut Context<Self>) {
        self.error = Some(format!("保存外观失败：{}", error.into()));
        self.saving = false;
        cx.notify();
    }

    /// 当前控件选择的主题。
    pub fn theme(&self) -> Theme {
        self.theme.clone()
    }

    /// 当前保存错误（若有）。
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// 是否接受本页输入。
    pub fn active(&self) -> bool {
        self.active
    }

    /// 是否正在等待写盘完成。
    pub fn saving(&self) -> bool {
        self.saving
    }

    /// 设置页 Tab 导航使用的稳定顺序。
    pub fn focus_controls(&self) -> Vec<(String, FocusHandle)> {
        if !self.active || self.saving {
            return Vec::new();
        }
        vec![
            ("theme-light".to_owned(), self.light_focus.clone()),
            ("theme-dark".to_owned(), self.dark_focus.clone()),
        ]
    }

    /// 读取上一帧两个分段按钮的真实窗口边界。
    pub fn control_bounds(&self, id: &str) -> Option<Bounds<Pixels>> {
        self.bounds.borrow().get(id).copied()
    }

    fn choose(&mut self, theme: Theme, cx: &mut Context<Self>) {
        if !self.active || self.saving || same_theme(&self.theme, &theme) {
            return;
        }
        self.error = None;
        cx.emit(ThemeChanged(theme));
        cx.notify();
    }

    fn key_down(
        &mut self,
        theme: Theme,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.active || self.saving {
            return;
        }
        if matches!(event.keystroke.key.as_str(), "enter" | "space" | " ") {
            cx.stop_propagation();
            self.choose(theme, cx);
        }
    }
}

impl Render for ThemeControl {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.buddy_theme().colors;
        let interactive = self.active && !self.saving;
        let selected_light = is_light(&self.theme);
        let bounds = self.bounds.clone();
        let light_focus = self.light_focus.clone();
        let dark_focus = self.dark_focus.clone();
        let light_bounds = bounds.clone();
        let dark_bounds = bounds.clone();
        let light_theme = Theme::Light;
        let dark_theme = Theme::Dark;
        let error = self.error.clone();
        let light_button = div()
            .id("theme-light")
            .relative()
            .when(interactive, |d| {
                d.track_focus(&light_focus)
                    .cursor_pointer()
                    .focus(|s| s.bg(c.primary_tint_soft).text_color(c.buddy_primary))
                    .hover(|s| s.bg(c.bg_elevated).text_color(c.text_primary))
            })
            .when(interactive, |d| {
                d.on_click(cx.listener(|this, _, window, cx| {
                    window.focus(&this.light_focus, cx);
                    this.choose(Theme::Light, cx);
                }))
            })
            .on_key_down(cx.listener(move |this, event, window, cx| {
                this.key_down(light_theme.clone(), event, window, cx)
            }))
            .min_h(px(m::SPACE_6 + m::SPACE_1))
            .px(px(m::SPACE_2))
            .rounded(px(m::RADIUS_SM))
            .flex()
            .items_center()
            .gap(px(m::SPACE_1))
            .text_size(px(m::FONT_SIZE_XS))
            .font_weight(FontWeight(500.0))
            .text_color(if selected_light {
                c.text_primary
            } else {
                c.text_muted
            })
            .when(selected_light, |d| {
                d.bg(c.bg_elevated).shadow(crate::theme_system::box_shadows(
                    cx.buddy_theme().shadows.shadow_static,
                ))
            })
            .child(icon(IconName::Sun, px(m::FONT_SIZE_SM)))
            .child("浅色")
            .child(
                canvas(
                    |_, _, _| {},
                    move |area, _, _, _| {
                        light_bounds.borrow_mut().insert("theme-light".into(), area);
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        let dark_button = div()
            .id("theme-dark")
            .relative()
            .when(interactive, |d| {
                d.track_focus(&dark_focus)
                    .cursor_pointer()
                    .focus(|s| s.bg(c.primary_tint_soft).text_color(c.buddy_primary))
                    .hover(|s| s.bg(c.bg_elevated).text_color(c.text_primary))
            })
            .when(interactive, |d| {
                d.on_click(cx.listener(|this, _, window, cx| {
                    window.focus(&this.dark_focus, cx);
                    this.choose(Theme::Dark, cx);
                }))
            })
            .on_key_down(cx.listener(move |this, event, window, cx| {
                this.key_down(dark_theme.clone(), event, window, cx)
            }))
            .min_h(px(m::SPACE_6 + m::SPACE_1))
            .px(px(m::SPACE_2))
            .rounded(px(m::RADIUS_SM))
            .flex()
            .items_center()
            .gap(px(m::SPACE_1))
            .text_size(px(m::FONT_SIZE_XS))
            .font_weight(FontWeight(500.0))
            .text_color(if selected_light {
                c.text_muted
            } else {
                c.text_primary
            })
            .when(!selected_light, |d| {
                d.bg(c.bg_elevated).shadow(crate::theme_system::box_shadows(
                    cx.buddy_theme().shadows.shadow_static,
                ))
            })
            .child(icon(IconName::Moon, px(m::FONT_SIZE_SM)))
            .child("深色")
            .child(
                canvas(
                    |_, _, _| {},
                    move |area, _, _, _| {
                        dark_bounds.borrow_mut().insert("theme-dark".into(), area);
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        div()
            .id("theme-control")
            .flex()
            .flex_col()
            .items_end()
            .gap(px(m::SPACE_1))
            .max_w(px(m::SPACE_12 * 4.0))
            .child(
                div()
                    .flex()
                    .gap(px(m::SPACE_1))
                    .p(px(m::SPACE_1))
                    .rounded(px(m::RADIUS_MD))
                    .bg(c.bg_sunken)
                    .opacity(if self.saving { 0.6 } else { 1.0 })
                    .children([light_button, dark_button]),
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

fn is_light(theme: &Theme) -> bool {
    matches!(theme, Theme::Light)
}

fn same_theme(left: &Theme, right: &Theme) -> bool {
    is_light(left) == is_light(right)
}
