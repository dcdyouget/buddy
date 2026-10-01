#[path = "helpers.rs"]
mod helpers;
#[path = "row.rs"]
mod row;

use super::ModelListView;
use crate::theme_system::{BuddyTheme, tokens::metrics as m};
use gpui::{Context, FontWeight, Window, div, prelude::*, px};

impl ModelListView {
    pub(super) fn render_list(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let c = cx.buddy_theme().colors;
        let interactive = self.active && !self.saving;
        let bounds = self.bounds();
        // Bounds are frame-local: controls hidden by a changed config must not
        // remain clickable through a stale test coordinate.
        bounds.borrow_mut().clear();
        let mut list = div()
            .id("settings-model-list")
            .w_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(px(m::FONT_SIZE_BASE))
                    .font_weight(FontWeight(600.))
                    .child("模型"),
            )
            .child(
                div()
                    .text_size(px(m::FONT_SIZE_XS))
                    .text_color(c.text_tertiary)
                    .mb(px(m::SPACE_2))
                    .child("选择可用模型与默认模型"),
            );
        if self.config.models.is_empty() {
            list = list.child(
                div()
                    .py(px(m::SPACE_4))
                    .text_color(c.text_tertiary)
                    .child("暂无模型，请点击下方按钮添加"),
            );
        } else {
            for model in self.config.models.clone() {
                list = list.child(row::render_row(
                    self,
                    model,
                    interactive,
                    bounds.clone(),
                    cx,
                ));
            }
        }
        if let Some(error) = self.error.clone() {
            list = list.child(
                div()
                    .id("settings-model-error")
                    .mt(px(m::SPACE_2))
                    .text_color(c.state_error)
                    .text_size(px(m::FONT_SIZE_XS))
                    .child(error),
            );
        }
        if self.saving {
            list = list.child(
                div()
                    .id("settings-model-saving")
                    .mt(px(m::SPACE_2))
                    .text_color(c.text_tertiary)
                    .text_size(px(m::FONT_SIZE_XS))
                    .child("保存中…"),
            );
        }
        list
    }
}
