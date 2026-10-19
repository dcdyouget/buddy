//! 网络搜索后端选择；配置写入由 Router 负责。

use crate::{
    settings::select::{SettingsSelect, SettingsSelectChanged},
    theme_system::{BuddyTheme, tokens::metrics as m},
};
use buddy_engine::models::WebSearchBackend;
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, Render, Subscription, Window, div,
    prelude::*, px,
};

const BACKEND_OPTIONS: [&str; 2] = ["原生实现", "DeepSeek"];

/// 用户发起的网络搜索后端选择。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WebSearchBackendChanged(pub WebSearchBackend);

impl EventEmitter<WebSearchBackendChanged> for WebSearchControl {}

/// 网络搜索后端设置控件。
pub struct WebSearchControl {
    select: Entity<SettingsSelect>,
    active: bool,
    saving: bool,
    error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl Focusable for WebSearchControl {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.select.read(cx).focus_handle(cx)
    }
}

impl WebSearchControl {
    /// 使用配置中的后端创建设置控件。
    pub fn new(backend: WebSearchBackend, cx: &mut Context<Self>) -> Self {
        let select = cx.new(|cx| {
            SettingsSelect::new(
                "web-search-backend",
                BACKEND_OPTIONS,
                backend_index(backend),
                cx,
            )
        });
        let subscriptions =
            vec![
                cx.subscribe(&select, |this, _, _: &SettingsSelectChanged, cx| {
                    if !this.active || this.saving {
                        return;
                    }
                    let Some(backend) = backend_at(this.select.read(cx).selected_index()) else {
                        return;
                    };
                    this.error = None;
                    cx.emit(WebSearchBackendChanged(backend));
                    cx.notify();
                }),
            ];

        Self {
            select,
            active: true,
            saving: false,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    /// 将已保存配置同步到选择器；同步不会发出用户编辑事件。
    pub fn set_backend(&mut self, backend: WebSearchBackend, cx: &mut Context<Self>) {
        let selected = self.select.read(cx).selected_index();
        let index = backend_index(backend);
        let changed = selected != index || self.error.is_some();
        if !changed {
            return;
        }
        self.select.update(cx, |select, cx| {
            select.reset_options(BACKEND_OPTIONS, index, cx)
        });
        self.error = None;
        cx.notify();
    }

    /// 设置页退出或子面板打开时释放选择器交互。
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.active == active {
            return;
        }
        self.active = active;
        self.sync_select_active(cx);
        cx.notify();
    }

    /// Router 保存期间禁用后端选择。
    pub fn set_saving(&mut self, saving: bool, cx: &mut Context<Self>) {
        if self.saving == saving {
            return;
        }
        self.saving = saving;
        if saving {
            self.error = None;
        }
        self.sync_select_active(cx);
        cx.notify();
    }

    /// 保存失败时恢复已保存值，并显示可恢复的错误。
    pub fn save_failed(
        &mut self,
        backend: WebSearchBackend,
        error: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.select.update(cx, |select, cx| {
            select.reset_options(BACKEND_OPTIONS, backend_index(backend), cx)
        });
        self.saving = false;
        self.error = Some(format!("保存网络搜索设置失败：{}", error.into()));
        self.sync_select_active(cx);
        cx.notify();
    }

    /// Tab 导航使用的稳定焦点位置。
    pub fn focus_controls(&self, cx: &App) -> Vec<(String, FocusHandle)> {
        if !self.active || self.saving {
            return Vec::new();
        }
        vec![(
            "web-search-backend".to_owned(),
            self.select.read(cx).focus_handle(cx),
        )]
    }

    /// 供设置页滚动定位到选择器使用。
    pub fn control_bounds_for_test(&self, cx: &App) -> Option<gpui::Bounds<gpui::Pixels>> {
        self.select.read(cx).painted_bounds_for_test()
    }

    fn sync_select_active(&self, cx: &mut Context<Self>) {
        let active = self.active && !self.saving;
        self.select
            .update(cx, |select, cx| select.set_active(active, cx));
    }
}

impl Render for WebSearchControl {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.buddy_theme().colors;
        let error = self.error.clone();
        div()
            .flex()
            .flex_col()
            .items_end()
            .gap(px(m::SPACE_1))
            .max_w(px(m::SPACE_12 * 4.0))
            .opacity(if self.saving { 0.65 } else { 1.0 })
            .child(self.select.clone())
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

fn backend_index(backend: WebSearchBackend) -> usize {
    match backend {
        WebSearchBackend::Native => 0,
        WebSearchBackend::DeepSeek => 1,
    }
}

fn backend_at(index: usize) -> Option<WebSearchBackend> {
    match index {
        0 => Some(WebSearchBackend::Native),
        1 => Some(WebSearchBackend::DeepSeek),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_selection_maps_native_and_deepseek() {
        assert_eq!(backend_index(WebSearchBackend::Native), 0);
        assert_eq!(backend_index(WebSearchBackend::DeepSeek), 1);
        assert_eq!(backend_at(0), Some(WebSearchBackend::Native));
        assert_eq!(backend_at(1), Some(WebSearchBackend::DeepSeek));
        assert_eq!(backend_at(2), None);
    }
}
