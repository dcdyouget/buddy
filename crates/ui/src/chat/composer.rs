//! 输入区（S05-06）—— 对应 v1 `src/components/chat/InputDock.tsx` 与 `global.css` 的 `.input-dock`
//!
//! | v1 | 本模块 |
//! |----|------|
//! | Enter 发送；**Cmd / Ctrl + Enter 换行**；组字中 Enter 不发送 | [`TextArea`] 的 `Submit` / `Newline`（注意：v1 中 Shift+Enter 也是发送） |
//! | 输入框自动撑高，最高 120px 后滚动；行高 20px（`--space-5`）；最小高 32px | `TextAreaStyle` |
//! | 有文字时右侧显示清除按钮（20px 圆，`--bg-sunken`，X 12px） | [`Composer`] |
//! | 可发送：有文字，或（模型支持图片且有图片）；图片不被支持时不可发送 | [`can_send`] |
//! | 流式中：「模型名 · 生成中...」+ 红色停止按钮 | 同 |
//! | 容器：圆角 lg、上亮下透渐变叠 `--composer-surface`、双层描边 + `--shadow-composer` | 同（外描边改用 `--border-default`，见决策记录） |
//!
//! | 空态页的独立气泡（`hideBorder` → `.is-standalone`）：无外边距、圆角 xl、`--window-outline` 描边 +
//! 内侧 `--window-inner-highlight` + `--shadow-floating-md`；输入框聚焦也不加底色；不自动撑高（`disableAutoResize`）；
//! 紧凑窗口中撑满高度 | [`Composer::set_standalone`] |
//!
//! 模型选择（S05-15，菜单见 [`super::model_menu`]）、图片附件（S05-07）、设置入口（S05-18）只提供回调位置。

use crate::components::{IconButtonVariant, icon_button};
use crate::icons::{IconName, icon};
use crate::text_area::{TextArea, TextAreaEvent, TextAreaStyle};
use crate::theme_system::{BuddyTheme, Theme, box_shadows, tokens::metrics as m};
use gpui::{
    App, Bounds, BoxShadow, Context, Entity, EventEmitter, FocusHandle, Focusable, Hsla, Render, SharedString, Subscription, Window, div,
    linear_color_stop, linear_gradient, point, prelude::*, px, Pixels,
};
use std::cell::Cell;
use std::rc::Rc;

/// 占位文字（v1 原文）
pub const PLACEHOLDER: &str = "问点什么…";
/// 输入框最大高度（v1 `Math.min(scrollHeight, 120)`）
pub const MAX_TEXT_HEIGHT: f32 = 120.0;

/// v1 `@media (max-height: 180px)` 的阈值：不高于它即为紧凑窗口
const COMPACT_MAX_HEIGHT: f32 = 180.0;

/// v1 `canSend`
pub fn can_send(text: &str, image_count: usize, supports_vision: bool, saving_images: bool) -> bool {
    let has_text = !text.trim().is_empty();
    let unsupported_images = image_count > 0 && !supports_vision;
    !saving_images && !unsupported_images && (has_text || (supports_vision && image_count > 0))
}

/// 输入区发出的事件
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposerEvent {
    /// 发送当前草稿
    Send(String),
    /// 停止生成
    Stop,
    /// 打开设置
    OpenSettings,
    /// 打开模型选择
    PickModel,
}

/// 输入区
pub struct Composer {
    text: Entity<TextArea>,
    /// 是否在流式中（由会话决定）
    streaming: bool,
    /// 流式中显示的模型名
    streaming_model: Option<SharedString>,
    /// 当前模型是否支持图片
    supports_vision: bool,
    focused: bool,
    hovered: bool,
    /// 空态页的独立气泡（S05-16）
    standalone: bool,
    /// 上一帧绘制的模型按钮边界（窗口坐标；模型菜单据此定位，流式中按钮不存在时为 `None`）
    model_button: Rc<Cell<Option<Bounds<Pixels>>>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ComposerEvent> for Composer {}

impl Focusable for Composer {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.text.focus_handle(cx)
    }
}

fn text_style(theme: &Theme, standalone: bool) -> TextAreaStyle {
    let c = theme.colors;
    TextAreaStyle {
        font_size: px(m::FONT_SIZE_MD),
        line_height: px(m::SPACE_5),
        // 独立气泡不撑高（v1 `disableAutoResize`：保持一行高，多行在框内滚动）
        max_height: Some(px(if standalone { m::SPACE_8 - 2.0 * m::SPACE_1 } else { MAX_TEXT_HEIGHT - 2.0 * m::SPACE_1 })),
        min_height: px(m::SPACE_8 - 2.0 * m::SPACE_1),
        text_color: c.text_primary.into(),
        placeholder_color: c.text_tertiary.into(),
        caret_color: c.text_primary.into(),
        selection_color: Hsla::from(c.buddy_primary).opacity(crate::markdown::SELECTION_ALPHA),
    }
}

impl Composer {
    /// 新建
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let theme = *cx.buddy_theme();
        let text = cx.new(|cx| TextArea::new(PLACEHOLDER, text_style(&theme, false), cx));
        let subscriptions = vec![
            cx.subscribe(&text, |this: &mut Self, _, event: &TextAreaEvent, cx| match event {
                TextAreaEvent::Submit => this.send(cx),
                TextAreaEvent::Changed => cx.notify(),
            }),
            cx.on_focus_in(&text.focus_handle(cx), window, |this: &mut Self, _, cx| {
                this.focused = true;
                cx.notify();
            }),
            cx.on_focus_out(&text.focus_handle(cx), window, |this: &mut Self, _, _, cx| {
                this.focused = false;
                cx.notify();
            }),
        ];
        Self { text, streaming: false, streaming_model: None, supports_vision: false, focused: false, hovered: false, standalone: false, model_button: Rc::default(), _subscriptions: subscriptions }
    }

    /// 模型按钮在窗口中的边界（上一帧；流式中为 `None`）
    pub fn model_button_bounds(&self) -> Option<Bounds<Pixels>> {
        self.model_button.get()
    }

    /// 输入框实体
    pub fn text_area(&self) -> &Entity<TextArea> {
        &self.text
    }

    /// 当前草稿
    pub fn draft(&self, cx: &App) -> String {
        self.text.read(cx).text().to_string()
    }

    /// 设置草稿（v1 `draftInput`；切页后恢复）
    pub fn set_draft(&mut self, text: &str, cx: &mut Context<Self>) {
        self.text.update(cx, |t, cx| t.set_text(text, cx));
    }

    /// 同步流式状态（流式中切换为「生成中」+ 停止按钮）
    pub fn set_streaming(&mut self, streaming: bool, model: Option<SharedString>, window: &mut Window, cx: &mut Context<Self>) {
        let ended = self.streaming && !streaming;
        self.streaming = streaming;
        self.streaming_model = model;
        // v1：流式结束后自动聚焦输入框
        if ended {
            window.focus(&self.text.focus_handle(cx), cx);
        }
        cx.notify();
    }

    /// 切换为空态页的独立气泡（v1 `hideBorder`）
    pub fn set_standalone(&mut self, standalone: bool, cx: &mut Context<Self>) {
        self.standalone = standalone;
        cx.notify();
    }

    /// 当前模型是否支持图片
    pub fn set_supports_vision(&mut self, supports: bool, cx: &mut Context<Self>) {
        self.supports_vision = supports;
        cx.notify();
    }

    fn can_send(&self, cx: &App) -> bool {
        can_send(self.text.read(cx).text(), 0, self.supports_vision, false)
    }

    /// v1 `handleSend`：流式中或不可发送时忽略；发送后由上层清空草稿
    fn send(&mut self, cx: &mut Context<Self>) {
        if self.streaming || !self.can_send(cx) {
            return;
        }
        let text = self.draft(cx);
        cx.emit(ComposerEvent::Send(text));
    }
}

impl Render for Composer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.buddy_theme();
        let c = theme.colors;
        let standalone = self.standalone;
        self.text.update(cx, |t, cx| t.set_style(text_style(&theme, standalone), cx));
        let has_text = !self.text.read(cx).text().trim().is_empty();
        let can_send = self.can_send(cx);
        let active = self.focused || self.hovered;

        // v1 `.input-dock`：外描边 + 内描边（inset 1px `--border-subtle`）+ `--shadow-composer`；
        // 悬停 / 聚焦时 v1 的品牌色渐变描边加粗到 2px → 以品牌色内描边近似（目检项）
        // 独立气泡：内描边换成 `--window-inner-highlight`，外投影换成 `--shadow-floating-md`
        let rest_inset = if standalone { c.window_inner_highlight } else { c.border_subtle };
        let mut shadows = vec![BoxShadow {
            color: if active { Hsla::from(c.buddy_primary).opacity(0.45) } else { rest_inset.into() },
            offset: point(px(0.), px(0.)),
            blur_radius: px(0.),
            spread_radius: px(if active { 2.0 } else { 1.0 }),
            inset: true,
        }];
        shadows.extend(box_shadows(if standalone { theme.shadows.shadow_floating_md } else { theme.shadows.shadow_composer }));

        if self.streaming {
            self.model_button.set(None);
        }
        let body = if self.streaming {
            let label = format!("{} · 生成中...", self.streaming_model.clone().unwrap_or_else(|| "AI".into()));
            div()
                .flex()
                .items_center()
                .gap(px(m::SPACE_1))
                .w_full()
                .child(div().flex_1().px(px(m::SPACE_3)).py(px(m::SPACE_2)).text_size(px(m::FONT_SIZE_BASE)).text_color(c.text_muted).child(label))
                .child(
                    icon_button("composer-stop", IconName::Square, 28.0, 14.0, IconButtonVariant::Danger, false, cx)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(ComposerEvent::Stop))),
                )
        } else {
            let text_box = div()
                .relative()
                .flex_1()
                .min_w_0()
                .flex()
                .child(
                    div()
                        .w_full()
                        .py(px(m::SPACE_1))
                        .pl(px(m::SPACE_2))
                        // 有文字时给清除按钮留位（v1 paddingRight 28px）
                        .pr(px(if has_text { 28.0 } else { m::SPACE_2 }))
                        .rounded(px(m::RADIUS_MD))
                        .when(self.focused && !standalone, |d| d.bg(c.field_surface))
                        .child(self.text.clone()),
                )
                .when(has_text, |d| {
                    d.child(
                        div().absolute().right(px(6.0)).top_0().bottom_0().flex().items_center().child(
                            div()
                                .id("composer-clear")
                                .size(px(m::SPACE_5))
                                .rounded(px(m::RADIUS_FULL))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(c.bg_sunken)
                                .text_color(c.text_muted)
                                .cursor_pointer()
                                .hover(|s| s.text_color(c.text_primary))
                                .child(icon(IconName::Close, px(12.0)))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.set_draft("", cx);
                                    window.focus(&this.text.focus_handle(cx), cx);
                                })),
                        ),
                    )
                });
            div()
                .flex()
                .items_center()
                .gap(px(m::SPACE_1))
                .w_full()
                .child(text_box)
                .child(
                    icon_button("composer-settings", IconName::Settings, 24.0, 13.0, IconButtonVariant::Default, false, cx)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(ComposerEvent::OpenSettings))),
                )
                .child({
                    // v1 `.model-picker-trigger`：24px 圆，Bot 14px；悬停凹陷底。
                    // 绝对定位的 canvas 记录按钮边界（须显式 top/left，见 Transcript::render_row）
                    let recorded = self.model_button.clone();
                    div()
                        .relative()
                        .flex_none()
                        .child(
                            icon_button("composer-model", IconName::Bot, m::SPACE_6, 14.0, IconButtonVariant::Default, false, cx)
                                .on_click(cx.listener(|_, _, _, cx| cx.emit(ComposerEvent::PickModel))),
                        )
                        .child(gpui::canvas(|_, _, _| {}, move |bounds, _, _, _| recorded.set(Some(bounds))).absolute().top_0().left_0().size_full())
                })
                .child(
                    icon_button(
                        "composer-send",
                        IconName::Send,
                        28.0,
                        14.0,
                        if can_send { IconButtonVariant::Primary } else { IconButtonVariant::Default },
                        !can_send,
                        cx,
                    )
                    .when(can_send, |b| b.hover(|s| s.bg(c.buddy_primary_600)))
                    .on_click(cx.listener(|this, _, _, cx| this.send(cx))),
                )
        };

        // v1 `@media (max-height: 180px)`：紧凑窗口中独立气泡撑满高度（视口高度取自窗口，与媒体查询同义）
        let compact = window.viewport_size().height <= px(COMPACT_MAX_HEIGHT);
        div()
            .id("input-dock")
            .when(!standalone, |d| d.mx(px(m::SPACE_2)).mb(px(m::SPACE_2)).rounded(px(m::RADIUS_LG)).border_color(c.border_default))
            .when(standalone, |d| d.w_full().flex().flex_col().justify_center().rounded(px(m::RADIUS_XL)).border_color(c.window_outline))
            .when(standalone && compact, |d| d.h_full())
            .p(px(m::SPACE_1))
            // 内层渐变由外层圆角裁切（不另设 11px 圆角：硬约束 3 只允许刻度内的值）
            .overflow_hidden()
            .border_1()
            .bg(c.composer_surface)
            .shadow(shadows)
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.hovered = *hovered;
                cx.notify();
            }))
            .child(
                // v1 背景：`linear-gradient(180deg, --surface-highlight, transparent)` 叠在表面色上
                div()
                    .bg(linear_gradient(180., linear_color_stop(c.surface_highlight, 0.), linear_color_stop(Hsla::from(c.surface_highlight).opacity(0.), 1.)))
                    .child(body),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_send_matches_v1() {
        assert!(!can_send("", 0, false, false));
        assert!(!can_send("   \n", 0, true, false));
        assert!(can_send("你好", 0, false, false));
        // 仅图片：模型须支持
        assert!(can_send("", 1, true, false));
        assert!(!can_send("", 1, false, false));
        // 有文字但图片不被支持：不可发送（v1 提示「当前模型不支持图片」）
        assert!(!can_send("你好", 1, false, false));
        // 图片保存中不可发送
        assert!(!can_send("你好", 0, false, true));
    }
}
