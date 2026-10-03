//! 无 Key 页（S05-16）—— 对应 v1 `src/pages/NoApiKeyPage.tsx` 与 `.brand-mark`
//!
//! | v1 | 本模块 |
//! |----|------|
//! | 整块可点击面板（`role=button`、`tabIndex=0`）：最小高 60、内边距 space-3 / space-4、间距 space-3、`--bg-surface`、圆角 xl | 同，发出 [`NoKeyPageEvent::OpenSettings`] |
//! | `.brand-mark`：32×32、圆角 md、`--primary-tint-soft` 底、品牌色 `KeyRound` 14 | 同 |
//! | 「请先设置 API Key」14px / 500 / `--state-error` / `flex: 1`；「设置」14px / 600；`ChevronRight` 16 | 同 |
//! | Enter / 空格触发（`preventDefault`） | 聚焦时按键触发 |
//! | 面板描边 `--glass-outline` | 改用 `--border-default`（`--glass-outline` 已按用户决定不迁移，见 `tokens.rs` 的 `EXCLUDED`） |
//! | 窗口拖拽（`useDragHandle`，面板 `data-no-window-drag`）、紧凑窗口尺寸 | 面板外空白可拖，面板自身保留点击；尺寸由 shell 控制 |

use super::drag::{self, DragSource};
use crate::icons::{IconName, icon};
use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use gpui::{Context, EventEmitter, FocusHandle, Focusable, FontWeight, KeyDownEvent, Window, div, prelude::*, px};

/// 提示文案（v1 原文）
pub const MESSAGE: &str = "请先设置 API Key";
/// 入口文案（v1 原文）
pub const ACTION: &str = "设置";
/// 面板最小高度（v1 `minHeight: 60`）
const MIN_HEIGHT: f32 = 60.0;

/// 无 Key 页事件
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoKeyPageEvent {
    /// 前往设置页（v1 `setPage('settings')`）
    OpenSettings,
}

/// 无 Key 页
pub struct NoKeyPage {
    focus: FocusHandle,
    drag_source: DragSource,
}

impl EventEmitter<NoKeyPageEvent> for NoKeyPage {}

impl Focusable for NoKeyPage {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl NoKeyPage {
    /// 新建
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self { focus: cx.focus_handle(), drag_source: drag::default_drag_source() }
    }

    /// Replace the outer blank-area drag callback; the panel itself remains
    /// intentionally non-draggable, matching v1 `data-no-window-drag`.
    pub fn set_drag_source(&mut self, source: DragSource) {
        self.drag_source = source;
    }
}

/// v1 `onKeyDown`：Enter 或空格触发
pub fn is_activation_key(key: &str) -> bool {
    key == "enter" || key == "space" || key == " "
}

impl Render for NoKeyPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.buddy_theme().colors;
        div().size_full().relative().flex().items_center().justify_center()
            .child(
                drag::region(&self.drag_source)
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
            )
            .child(
            div()
                .id("no-key-panel")
                .track_focus(&self.focus)
                .w_full()
                .min_h(px(MIN_HEIGHT))
                .px(px(m::SPACE_4))
                .py(px(m::SPACE_3))
                .flex()
                .items_center()
                .gap(px(m::SPACE_3))
                .rounded(px(m::RADIUS_XL))
                .border_1()
                .border_color(c.border_default)
                .bg(c.bg_surface)
                .cursor_pointer()
                .on_click(cx.listener(|_, _, _, cx| cx.emit(NoKeyPageEvent::OpenSettings)))
                .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                    if is_activation_key(&event.keystroke.key) {
                        cx.stop_propagation();
                        cx.emit(NoKeyPageEvent::OpenSettings);
                    }
                }))
                .child(
                    div()
                        .flex_none()
                        .size(px(m::SPACE_8))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(m::RADIUS_MD))
                        .text_color(c.buddy_primary)
                        .bg(c.primary_tint_soft)
                        .child(icon(IconName::KeyRound, px(14.0))),
                )
                .child(div().flex_1().text_color(c.state_error).text_size(px(m::FONT_SIZE_MD)).font_weight(FontWeight(500.0)).child(MESSAGE))
                .child(div().flex_none().text_color(c.state_error).text_size(px(m::FONT_SIZE_MD)).font_weight(FontWeight(600.0)).child(ACTION))
                .child(div().text_color(c.state_error).child(icon(IconName::ChevronRight, px(16.0)))),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_and_space_activate() {
        assert!(is_activation_key("enter"));
        assert!(is_activation_key("space"));
        assert!(!is_activation_key("a"));
        assert!(!is_activation_key("tab"));
    }
}
