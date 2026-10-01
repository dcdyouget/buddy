//! 设置页固定选项选择器。

use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use gpui::{
    App, Bounds, Context, ElementId, EventEmitter, FocusHandle, Focusable, FontWeight,
    KeyDownEvent, MouseDownEvent, Pixels, Render, SharedString, Window, canvas, div, prelude::*,
    px,
};
use std::cell::Cell;
use std::rc::Rc;

/// 选择值变化事件；当前值通过 [`SettingsSelect::selected_value`] 读取。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsSelectChanged;

/// 可复用的设置选择器，支持点击展开、点击选项和方向键选择。
pub struct SettingsSelect {
    id: ElementId,
    options: Vec<SharedString>,
    selected: usize,
    open: bool,
    focus: FocusHandle,
    last_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    menu_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl EventEmitter<SettingsSelectChanged> for SettingsSelect {}

impl SettingsSelect {
    /// 创建选择器。`selected` 超出选项范围时回退到第一项。
    pub fn new(
        id: impl Into<ElementId>,
        options: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: usize,
        cx: &mut Context<Self>,
    ) -> Self {
        let options: Vec<SharedString> = options.into_iter().map(Into::into).collect();
        let selected = selected.min(options.len().saturating_sub(1));
        Self {
            id: id.into(),
            options,
            selected,
            open: false,
            focus: cx.focus_handle(),
            last_bounds: Rc::new(Cell::new(None)),
            menu_bounds: Rc::new(Cell::new(None)),
        }
    }

    /// 当前选项下标。
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// 选项菜单是否打开（真实交互自检用）。
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 当前选项文本。
    pub fn selected_value(&self) -> Option<SharedString> {
        self.options.get(self.selected).cloned()
    }

    /// 设置当前选项并发出变化事件。
    pub fn set_selected(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.options.len() {
            return;
        }
        let changed = index != self.selected;
        self.selected = index;
        self.open = false;
        if changed {
            cx.emit(SettingsSelectChanged);
        }
        cx.notify();
    }

    /// 当前帧记录的选择器边界，供预览自测定位真实点击点。
    pub fn painted_bounds_for_test(&self) -> Option<Bounds<Pixels>> {
        self.last_bounds.get()
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        if self.options.is_empty() {
            return;
        }
        self.open = !self.open;
        cx.notify();
    }

    fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        self.set_selected(index, cx);
    }

    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        match key {
            "escape" if self.open => {
                self.open = false;
                cx.stop_propagation();
                cx.notify();
            }
            "down" | "arrowdown" | "right" | "arrowright" => {
                if self.options.is_empty() {
                    return;
                }
                if self.open {
                    let next = (self.selected + 1).min(self.options.len().saturating_sub(1));
                    self.set_selected(next, cx);
                } else {
                    self.open = true;
                    cx.notify();
                }
                cx.stop_propagation();
            }
            "up" | "arrowup" | "left" | "arrowleft" => {
                if self.options.is_empty() {
                    return;
                }
                if self.open {
                    self.set_selected(self.selected.saturating_sub(1), cx);
                } else {
                    self.open = true;
                    cx.notify();
                }
                cx.stop_propagation();
            }
            "enter" | "space" => {
                if self.options.is_empty() {
                    return;
                }
                self.open = !self.open;
                cx.stop_propagation();
                cx.notify();
            }
            _ => {}
        }
    }
}

impl Focusable for SettingsSelect {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for SettingsSelect {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.buddy_theme().colors;
        let selected = self.selected_value().unwrap_or_default();
        let options = self.options.clone();
        let selected_index = self.selected;
        let open = self.open;
        let trigger_bounds = self.last_bounds.clone();
        let menu_bounds = self.menu_bounds.clone();
        let trigger = div()
            .id("settings-select-trigger")
            .min_w(px(m::SPACE_8 * 6.0))
            .min_h(px(m::SPACE_8))
            .px(px(m::SPACE_2))
            .rounded(px(m::RADIUS_MD))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(m::SPACE_2))
            .border_1()
            .border_color(if open {
                c.buddy_primary
            } else {
                c.border_default
            })
            .bg(c.field_surface)
            .text_color(c.text_primary)
            .text_size(px(m::FONT_SIZE_SM))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, window, cx| {
                window.focus(&this.focus, cx);
                this.toggle(cx);
            }))
            .child(selected)
            .child(
                div()
                    .text_color(c.text_muted)
                    .child(icon(IconName::ChevronDown, px(m::FONT_SIZE_MD))),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, _, _| {
                        trigger_bounds.set(Some(bounds));
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        let menu = div()
            .absolute()
            .top(px(m::SPACE_8 + m::SPACE_1))
            .left_0()
            .min_w(px(m::SPACE_8 * 6.0))
            .rounded(px(m::RADIUS_MD))
            .border_1()
            .border_color(c.border_default)
            .bg(c.bg_elevated)
            .occlude()
            .shadow(crate::theme_system::box_shadows(
                cx.buddy_theme().shadows.shadow_floating_sm,
            ))
            .children(options.into_iter().enumerate().map(|(index, option)| {
                let selected = index == selected_index;
                div()
                    .id(ElementId::named_usize("settings-select-option", index))
                    .min_h(px(m::SPACE_8))
                    .px(px(m::SPACE_2))
                    .flex()
                    .items_center()
                    .gap(px(m::SPACE_1))
                    .cursor_pointer()
                    .text_color(c.text_primary)
                    .when(selected, |d| d.bg(c.primary_tint_soft))
                    .when(!selected, |d| d.hover(|s| s.bg(c.bg_sunken)))
                    .on_click(cx.listener(move |this, _, _, cx| this.choose(index, cx)))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(m::FONT_SIZE_SM))
                            .font_weight(FontWeight(500.0))
                            .text_color(c.text_primary)
                            .child(option),
                    )
                    .when(selected, |d| d.child(icon(IconName::Check, px(14.0))))
            }))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, _, _| {
                        menu_bounds.set(Some(bounds));
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        let debug_id = format!("settings-select-{}", self.id);
        div()
            .id(self.id.clone())
            .debug_selector(move || debug_id.clone())
            .relative()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .when(open, |d| {
                d.on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    let inside_menu = this
                        .menu_bounds
                        .get()
                        .is_some_and(|bounds| bounds.contains(&event.position));
                    if !inside_menu {
                        this.open = false;
                        cx.notify();
                    }
                }))
            })
            .child(trigger)
            .when(open, |d| d.child(gpui::deferred(menu).with_priority(2)))
    }
}
