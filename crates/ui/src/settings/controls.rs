//! 设置页共用控件。
//!
//! 这些构造器对应 v1 的 `.settings-section`、`.settings-row`、
//! `.segmented-control` 和设置表单按钮。控件只读取 [`BuddyTheme`]，因此设置页
//! 不需要复制颜色、间距或圆角常量。

use crate::text_area::{EnterMode, TextArea, TextAreaEvent, TextAreaStyle};
use crate::theme_system::{Appearance, BuddyTheme, Theme, tokens::metrics as m};
use gpui::{
    App, Context, ElementId, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, IntoElement,
    Render, SharedString, Stateful, Styled, Subscription, Window, div, prelude::*, px,
};

/// 设置区：标题与说明在左侧，传入的控制器在右侧。
pub fn section(
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    control: impl IntoElement,
    cx: &App,
) -> gpui::Div {
    let c = cx.buddy_theme().colors;
    div()
        .rounded(px(m::RADIUS_LG))
        // v1 settings-section 使用 inset 1px，不占布局尺寸。
        .shadow(vec![gpui::BoxShadow {
            color: c.border_subtle.into(),
            offset: gpui::point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(1.0),
            inset: true,
        }])
        .bg(c.bg_elevated)
        .px(px(m::SPACE_4))
        .py(px(m::SPACE_3))
        .child(
            div()
                .min_h(px(m::SPACE_10))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(m::SPACE_4))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .text_color(c.text_primary)
                                .text_size(px(m::FONT_SIZE_BASE))
                                .line_height(px(m::FONT_SIZE_BASE * m::LINE_HEIGHT_BASE))
                                .font_weight(FontWeight(600.0))
                                .child(title.into()),
                        )
                        .child(
                            div()
                                .text_color(c.text_tertiary)
                                .text_size(px(m::FONT_SIZE_XS))
                                .line_height(px(m::FONT_SIZE_XS * m::LINE_HEIGHT_BASE))
                                .child(description.into()),
                        ),
                )
                .child(control),
        )
}

/// 设置页的普通操作按钮。
pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    cx: &App,
) -> Stateful<gpui::Div> {
    let c = cx.buddy_theme().colors;
    let id = id.into();
    let debug_id = format!("settings-button-{}", id);
    div()
        .id(id)
        .debug_selector(move || debug_id.clone())
        .min_h(px(m::SPACE_8))
        .px(px(m::SPACE_3))
        .rounded(px(m::RADIUS_MD))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(m::SPACE_1))
        .border_1()
        .border_color(c.border_default)
        .bg(c.bg_sunken)
        .text_color(c.text_muted)
        .text_size(px(m::FONT_SIZE_XS))
        .font_weight(FontWeight(500.0))
        .cursor_pointer()
        .hover(|s| s.text_color(c.text_primary).border_color(c.border_strong))
        .child(label.into())
}

/// 设置页的受控开关。
///
/// `checked` 由页面状态传入，页面负责在返回的元素上绑定点击处理器并更新状态。
pub fn toggle(id: impl Into<ElementId>, checked: bool, cx: &App) -> Stateful<gpui::Div> {
    let c = cx.buddy_theme().colors;
    let id = id.into();
    let debug_id = format!("settings-toggle-{}", id);
    let track = if checked {
        c.buddy_primary
    } else {
        c.bg_sunken
    };
    let knob = div()
        .absolute()
        .top(px(m::SPACE_1))
        .size(px(m::SPACE_4))
        .rounded(px(m::RADIUS_FULL))
        .bg(c.text_on_primary)
        .shadow(crate::theme_system::box_shadows(
            cx.buddy_theme().shadows.shadow_static,
        ))
        .when(checked, |d| d.right(px(m::SPACE_1)))
        .when(!checked, |d| d.left(px(m::SPACE_1)));
    div()
        .id(id)
        .debug_selector(move || debug_id.clone())
        .relative()
        .w(px(m::SPACE_10))
        .h(px(m::SPACE_6))
        .rounded(px(m::RADIUS_FULL))
        .border_1()
        .border_color(if checked {
            c.buddy_primary
        } else {
            c.border_default
        })
        .bg(track)
        .cursor_pointer()
        .child(knob)
}

/// 单行设置输入框发出的事件。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsFieldEvent {
    /// 文本发生变化。
    Changed,
}

impl EventEmitter<SettingsFieldEvent> for SettingsField {}

fn field_style(theme: &Theme) -> TextAreaStyle {
    let c = theme.colors;
    TextAreaStyle {
        font_size: px(m::FONT_SIZE_SM),
        line_height: px(m::FONT_SIZE_SM * m::LINE_HEIGHT_BASE),
        max_height: Some(px(m::SPACE_8 - 2.0 * m::SPACE_1)),
        min_height: px(m::SPACE_8 - 2.0 * m::SPACE_1),
        text_color: c.text_primary.into(),
        placeholder_color: c.text_tertiary.into(),
        caret_color: c.text_primary.into(),
        selection_color: gpui::Hsla::from(c.buddy_primary)
            .opacity(crate::markdown::SELECTION_ALPHA),
    }
}

/// 设置页单行文本输入框。
///
/// 内部复用 Buddy 的 [`TextArea`]，但固定为 [`EnterMode::SingleLine`]。密码遮挡
/// 暂不提供，避免改变全局 `TextArea` 的文本绘制语义；API Key 等字段由后续设置
/// spec 单独设计遮挡策略。
pub struct SettingsField {
    id: ElementId,
    input: Entity<TextArea>,
    _subscriptions: Vec<Subscription>,
    appearance: Appearance,
}

impl SettingsField {
    /// 在实体构造闭包中创建字段：
    /// `let field = cx.new(|cx| SettingsField::new("占位", cx));`
    pub fn new(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self::with_id("settings-field", placeholder, cx)
    }

    /// 带稳定元素 ID 的字段构造器；多个字段同时存在时应使用此方法。
    pub fn with_id(
        id: impl Into<ElementId>,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Self {
        let placeholder = placeholder.into();
        let theme = *cx.buddy_theme();
        let style = field_style(&theme);
        let input = cx.new(|cx| {
            let mut input = TextArea::new(placeholder.clone(), style, cx);
            input.set_enter_mode(EnterMode::SingleLine);
            input
        });
        let subscriptions = vec![cx.subscribe(&input, |this, _, event: &TextAreaEvent, cx| {
            if *event == TextAreaEvent::Changed {
                let current = this.input.read(cx).text().to_owned();
                let normalized = current
                    .replace("\r\n", "")
                    .replace('\n', "")
                    .replace('\r', "");
                if normalized != current {
                    this.input
                        .update(cx, |input, cx| input.set_text(&normalized, cx));
                    return;
                }
                cx.emit(SettingsFieldEvent::Changed);
            }
        })];
        Self {
            id: id.into(),
            input,
            _subscriptions: subscriptions,
            appearance: theme.appearance,
        }
    }

    /// 返回内部输入实体，供需要订阅或读取焦点的页面使用。
    pub fn input(&self) -> &Entity<TextArea> {
        &self.input
    }

    /// 获取当前文本的快照。
    pub fn text(&self, cx: &App) -> String {
        self.input.read(cx).text().to_owned()
    }

    /// 设置文本并发出一次 [`SettingsFieldEvent::Changed`]。
    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_text(text, cx));
    }

    /// 将键盘焦点放入单行输入框。
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.input.focus_handle(cx), cx);
    }

    /// 获取内部 `TextArea` 上一帧绘制的文字区域，供预览自测定位真实点击点。
    pub fn painted_bounds_for_test(&self, cx: &App) -> Option<gpui::Bounds<gpui::Pixels>> {
        self.input.read(cx).painted_bounds_for_test()
    }
}

impl Focusable for SettingsField {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for SettingsField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.buddy_theme();
        if self.appearance != theme.appearance {
            self.appearance = theme.appearance;
            self.input
                .update(cx, |input, cx| input.set_style(field_style(&theme), cx));
        }
        let c = theme.colors;
        let input = self.input.clone();
        let debug_id = format!("settings-field-{}", self.id);
        div()
            .id(self.id.clone())
            .debug_selector(move || debug_id.clone())
            .w(px(m::SPACE_8 * 6.0))
            .flex_none()
            .h(px(m::SPACE_8))
            .px(px(m::SPACE_2))
            .py(px(m::SPACE_1))
            .rounded(px(m::RADIUS_MD))
            .border_1()
            .border_color(c.border_subtle)
            .bg(c.field_surface)
            .child(input)
    }
}
