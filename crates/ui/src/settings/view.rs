//! SettingsPage 编排与布局。子项的编辑交互由对应 S06 spec 接入，未实现部分只读显示。
//!
//! 设置层始终复用同一个实体；退出绘制不再注册鼠标 / 键盘处理，避免透明层吞掉底层点击。

use crate::{
    icons::{IconName, icon},
    theme_system::{BuddyTheme, tokens::metrics as m},
};
use buddy_engine::models::{AppConfig, Theme};
use gpui::{
    App, Bounds, Context, EventEmitter, FocusHandle, Focusable, FontWeight, KeyDownEvent, Pixels,
    Render, ScrollHandle, Window, canvas, div, prelude::*, px,
};
use std::{cell::Cell, rc::Rc};

/// 设置页向路由器发出的事件。
#[derive(Clone, Debug)]
pub enum SettingsEvent {
    /// 返回设置前的页面。
    Back,
}

/// 作为原页面上的全尺寸覆盖层，保留底层输入草稿和生成任务。
pub struct SettingsView {
    config: AppConfig,
    back: FocusHandle,
    active: bool,
    focus_pending: bool,
    scroll: ScrollHandle,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

impl Focusable for SettingsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.back.clone()
    }
}

impl SettingsView {
    /// 构建设置覆盖层。
    pub fn new(config: AppConfig, cx: &mut Context<Self>) -> Self {
        Self {
            config,
            back: cx.focus_handle(),
            active: false,
            focus_pending: false,
            scroll: ScrollHandle::new(),
            bounds: Rc::new(Cell::new(None)),
        }
    }

    /// 路由器外部变更配置时同步显示。
    pub fn set_config(&mut self, config: AppConfig, cx: &mut Context<Self>) {
        self.config = config;
        cx.notify();
    }

    /// 打开时将焦点移到返回按钮；关闭立即释放事件，退出动画仅绘制。
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.active = active;
        self.focus_pending = active;
        cx.notify();
    }

    /// 内容滚动位置（真实滚轮自检）。
    pub fn scroll_offset(&self) -> gpui::Point<gpui::Pixels> {
        self.scroll.offset()
    }

    /// 是否接受输入。
    pub fn active(&self) -> bool {
        self.active
    }

    /// 最后一帧实际覆盖层边界（动画位移 / 布局自检用）。
    pub fn painted_bounds(&self) -> Option<Bounds<Pixels>> {
        self.bounds.get()
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_pending) {
            window.focus(&self.back, cx);
        }
        let c = cx.buddy_theme().colors;
        let theme_name = match self.config.theme {
            Theme::Light => "浅色",
            Theme::Dark => "深色",
        };
        let header = div()
            .flex_none()
            .h(px(m::SPACE_12 + m::SPACE_2))
            .flex()
            .items_center()
            .gap(px(m::SPACE_2))
            .px(px(m::SPACE_4))
            .py(px(m::SPACE_3))
            .child(
                div()
                    .id("settings-back")
                    .size(px(18.0 + m::SPACE_2))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(m::RADIUS_SM))
                    .text_color(c.text_muted)
                    .when(self.active, |d| {
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
        let models = div()
            .px(px(m::SPACE_4))
            .py(px(m::SPACE_3))
            .rounded(px(m::RADIUS_LG))
            .bg(c.bg_elevated)
            .shadow(vec![gpui::BoxShadow {
                color: c.border_subtle.into(),
                offset: gpui::point(px(0.0), px(0.0)),
                blur_radius: px(0.0),
                spread_radius: px(1.0),
                inset: true,
            }])
            .child(
                div()
                    .text_size(px(m::FONT_SIZE_BASE))
                    .font_weight(FontWeight(600.0))
                    .child("模型"),
            )
            .child(
                div()
                    .text_size(px(m::FONT_SIZE_XS))
                    .text_color(c.text_tertiary)
                    .mb(px(m::SPACE_2))
                    .child("选择可用模型与默认模型"),
            )
            .when(self.config.models.is_empty(), |d| {
                d.child(
                    div()
                        .py(px(m::SPACE_4))
                        .text_color(c.text_muted)
                        .child("暂无模型"),
                )
            })
            .children(self.config.models.iter().map(|model| {
                div()
                    .py(px(m::SPACE_2))
                    .border_t_1()
                    .border_color(c.border_subtle)
                    .flex()
                    .justify_between()
                    .gap(px(m::SPACE_2))
                    .child(div().child(model.display_name.clone()))
                    .children(
                        (model.id == self.config.selected_model_id)
                            .then(|| div().text_color(c.buddy_primary).child("默认")),
                    )
            }));
        let content = div()
            .id("settings-content")
            .flex_1()
            .min_h_0()
            .w_full()
            .when(self.active, |d| {
                d.overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            })
            .when(!self.active, |d| d.overflow_hidden())
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .p(px(m::SPACE_4))
                    .gap(px(m::SPACE_3))
                    .child(super::controls::section(
                        "外观",
                        "选择窗口的显示模式",
                        div()
                            .text_color(c.text_muted)
                            .text_size(px(m::FONT_SIZE_XS))
                            .child(theme_name),
                        cx,
                    ))
                    .child(super::controls::section(
                        "呼出快捷键",
                        "在任意应用中快速打开 Buddy",
                        div()
                            .text_color(c.text_muted)
                            .text_size(px(m::FONT_SIZE_XS))
                            .child(self.config.hotkey.clone()),
                        cx,
                    ))
                    .child(models),
            );
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
            .when(self.active, |d| {
                d.occlude().capture_key_down(cx.listener(
                    |this, event: &KeyDownEvent, window, cx| {
                        match event.keystroke.key.as_str() {
                            "tab" => {
                                // 骨架目前仅返回按钮可聚焦；各设置控件接入时在此列表按视觉顺序追加。
                                let order = [this.back.clone()];
                                let current =
                                    order.iter().position(|focus| focus.is_focused(window));
                                let next = match current {
                                    Some(i) if event.keystroke.modifiers.shift => {
                                        (i + order.len() - 1) % order.len()
                                    }
                                    Some(i) => (i + 1) % order.len(),
                                    None if event.keystroke.modifiers.shift => order.len() - 1,
                                    None => 0,
                                };
                                window.focus(&order[next], cx);
                                cx.stop_propagation();
                            }
                            "enter" | "space" if this.back.is_focused(window) => {
                                cx.emit(SettingsEvent::Back);
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
            .child(
                canvas(|_, _, _| {}, move |area, _, _, _| bounds.set(Some(area)))
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
            )
    }
}
