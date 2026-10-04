//! 「软件更新」区域绘制，对应 v1 `.update-setting*` / `.update-release-card` / `.update-progress-card`。

use super::update::{UpdateControl, UpdatePhase};
use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use gpui::{
    AnyElement, Bounds, Context, FocusHandle, FontWeight, Hsla, KeyDownEvent, Pixels, Render,
    Rgba, SharedString, Window, canvas, div, prelude::*, px,
};
use std::{cell::Cell, rc::Rc, sync::atomic::Ordering};

impl Render for UpdateControl {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.buddy_theme().colors;
        let busy = self.phase.busy();
        let label = match self.phase {
            UpdatePhase::Checking => "正在检查…",
            UpdatePhase::Downloading
            | UpdatePhase::Installing
            | UpdatePhase::WaitingIdle
            | UpdatePhase::Restarting => "正在更新…",
            UpdatePhase::Idle => "检查更新",
            _ => "重新检查",
        };
        let mut refresh = icon(IconName::RefreshCw, px(m::FONT_SIZE_SM));
        if self.phase == UpdatePhase::Checking && !crate::accessibility::prefers_reduced_motion() {
            // v1 `.buddy-spin`：1.2s 匀速旋转
            window.request_animation_frame();
            let ms = self.started.elapsed().as_millis() as f32;
            refresh = refresh.with_transformation(gpui::Transformation::rotate(gpui::radians(
                (ms % 1200.0) / 1200.0 * std::f32::consts::TAU,
            )));
        }
        let check = action(
            "update-check",
            &self.check_focus,
            self.active && !busy,
            false,
            self.check_bounds.clone(),
            cx,
        )
        .on_click(cx.listener(|this, _, _, cx| this.check(cx)))
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
            if activates(event) {
                this.check(cx);
                cx.stop_propagation();
            }
        }))
        .child(refresh)
        .child(label);
        let row = div()
            .min_h(px(m::SPACE_10))
            .flex()
            .items_start()
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
                            .child("软件更新"),
                    )
                    .child(
                        div()
                            .text_color(c.text_tertiary)
                            .text_size(px(m::FONT_SIZE_XS))
                            .line_height(px(m::FONT_SIZE_XS * m::LINE_HEIGHT_BASE))
                            .child(format!("当前版本 v{}", buddy_update::current_version())),
                    ),
            )
            .child(check);
        let feedback = self.feedback(cx);
        div()
            .rounded(px(m::RADIUS_LG))
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
            .flex()
            .flex_col()
            .gap(px(m::SPACE_3))
            .child(row)
            .children(feedback)
    }
}

impl UpdateControl {
    fn feedback(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = cx.buddy_theme().colors;
        match &self.phase {
            UpdatePhase::Latest => Some(status(c.state_success, IconName::CircleCheck, "当前已是最新版本".into())),
            UpdatePhase::Failed(message) => {
                Some(status(c.state_error, IconName::CircleAlert, message.clone().into()))
            }
            UpdatePhase::Available => {
                let release = self.release.as_ref()?;
                let notes = release.notes.trim();
                let notes = if notes.is_empty() { "此版本未提供更新说明。" } else { notes };
                let install = action(
                    "update-install",
                    &self.install_focus,
                    self.active,
                    true,
                    self.install_bounds.clone(),
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.install(cx)))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if activates(event) {
                        this.install(cx);
                        cx.stop_propagation();
                    }
                }))
                .child(icon(IconName::Download, px(m::FONT_SIZE_SM)))
                .child("立即更新");
                Some(
                    card(cx)
                        .child(
                            div()
                                .text_color(c.text_primary)
                                .text_size(px(m::FONT_SIZE_SM))
                                .font_weight(FontWeight(600.0))
                                .child(format!("发现新版本 v{}", release.version)),
                        )
                        .child(
                            div()
                                .id("update-notes")
                                .max_h(px(m::SPACE_12 * 2.0))
                                .overflow_y_scroll()
                                .text_color(c.text_muted)
                                .text_size(px(m::FONT_SIZE_XS))
                                .line_height(px(m::FONT_SIZE_XS * m::LINE_HEIGHT_BASE))
                                .child(notes.to_owned()),
                        )
                        .child(div().flex().child(install))
                        .into_any_element(),
                )
            }
            UpdatePhase::Downloading
            | UpdatePhase::Installing
            | UpdatePhase::WaitingIdle
            | UpdatePhase::Restarting => {
                let total = self.release.as_ref().map_or(0, |r| r.update.size);
                let (copy, percent) = match self.phase {
                    UpdatePhase::Downloading => {
                        let done = self.received.load(Ordering::Relaxed);
                        let percent = if total == 0 { 0 } else { (done * 100 / total).min(100) };
                        ("正在下载更新…", Some(percent))
                    }
                    UpdatePhase::Installing => ("正在安装更新…", None),
                    UpdatePhase::WaitingIdle => ("安装完成，将在当前回复结束后重启…", None),
                    _ => ("安装完成，正在重启…", None),
                };
                let width = percent.unwrap_or(100) as f32 / 100.0;
                Some(
                    card(cx)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(m::SPACE_2))
                                .text_color(c.text_muted)
                                .text_size(px(m::FONT_SIZE_XS))
                                .child(copy)
                                .children(percent.map(|p| format!("{p}%"))),
                        )
                        .child(
                            div()
                                .h(px(m::SPACE_1))
                                .overflow_hidden()
                                .rounded(px(m::RADIUS_FULL))
                                .bg(c.bg_elevated)
                                .child(
                                    div()
                                        .h_full()
                                        .w(gpui::relative(width))
                                        .rounded(px(m::RADIUS_FULL))
                                        .bg(c.buddy_primary),
                                ),
                        )
                        .into_any_element(),
                )
            }
            UpdatePhase::Idle | UpdatePhase::Checking => None,
        }
    }
}

fn card(cx: &Context<UpdateControl>) -> gpui::Div {
    let c = cx.buddy_theme().colors;
    div()
        .p(px(m::SPACE_3))
        .rounded(px(m::RADIUS_MD))
        .border_1()
        .border_color(c.border_subtle)
        .bg(c.bg_sunken)
        .flex()
        .flex_col()
        .gap(px(m::SPACE_2))
}

fn status(tone: Rgba, name: IconName, message: SharedString) -> AnyElement {
    div()
        .p(px(m::SPACE_3))
        .rounded(px(m::RADIUS_MD))
        .border_1()
        .border_color(mix(tone, 0.22))
        .bg(mix(tone, 0.08))
        .flex()
        .items_start()
        .gap(px(m::SPACE_2))
        .text_color(tone)
        .text_size(px(m::FONT_SIZE_XS))
        .line_height(px(m::FONT_SIZE_XS * m::LINE_HEIGHT_BASE))
        .child(div().flex_none().mt(px(m::SPACE_1 / 2.0)).child(icon(name, px(m::FONT_SIZE_SM))))
        .child(div().min_w_0().flex_1().child(message))
        .into_any_element()
}

/// v1 `.update-setting-action`；`primary` 对应 `.is-primary`。
fn action(
    id: &'static str,
    focus: &FocusHandle,
    enabled: bool,
    primary: bool,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    cx: &Context<UpdateControl>,
) -> gpui::Stateful<gpui::Div> {
    let c = cx.buddy_theme().colors;
    let (fg, bg, border) = if primary {
        (c.text_on_primary, c.buddy_primary, c.buddy_primary)
    } else {
        (c.text_muted, c.bg_sunken, c.border_default)
    };
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .relative()
        .flex_none()
        .min_h(px(m::SPACE_8))
        .px(px(m::SPACE_3))
        .rounded(px(m::RADIUS_MD))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(m::SPACE_1))
        .border_1()
        .border_color(border)
        .bg(bg)
        .text_color(if enabled || primary { fg } else { c.text_tertiary })
        .text_size(px(m::FONT_SIZE_XS))
        .font_weight(FontWeight(500.0))
        .when(enabled, |d| {
            d.track_focus(focus).cursor_pointer().map(|d| {
                if primary {
                    d.hover(|s| s.bg(c.buddy_primary_600).border_color(c.buddy_primary_600))
                        .focus(|s| s.bg(c.buddy_primary_600).border_color(c.buddy_primary_600))
                } else {
                    d.hover(|s| s.text_color(c.text_primary).border_color(c.border_strong))
                        .focus(|s| s.text_color(c.buddy_primary).border_color(c.buddy_primary))
                }
            })
        })
        .when(!enabled, |d| d.cursor_not_allowed())
        .child(
            canvas(|_, _, _| {}, move |area, _, _, _| bounds.set(Some(area)))
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
        )
}

fn activates(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
}

fn mix(c: Rgba, alpha: f32) -> Hsla {
    Hsla::from(c).opacity(alpha)
}
