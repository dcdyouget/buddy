//! 添加 Provider 面板实体：事件、输入同步与 engine 请求桥接。

use super::focus::ProviderFocus;
use crate::settings::provider_form::{Busy, ProviderForm, ProviderSubmission};
use crate::settings::{controls::SettingsField, select::SettingsSelect};
use buddy_engine::chat::ChatEngine;
use gpui::{
    App, Bounds, Context, Entity, EventEmitter, FocusHandle, Focusable, Pixels, Subscription,
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

/// 添加 Provider 面板向 SettingsView 发出的事件。
#[derive(Clone, Debug)]
pub enum ProviderEvent {
    /// 返回设置页。
    Back,
    /// 提交完整配置快照；保存由 Router 负责。
    Submit(ProviderSubmission),
}

impl EventEmitter<ProviderEvent> for AddProviderPanel {}

/// v1 AddProviderPanel 的 GPUI 实体。
pub struct AddProviderPanel {
    pub(super) engine: Arc<ChatEngine>,
    pub(super) form: ProviderForm,
    pub(super) active: bool,
    pub(super) saving: bool,
    pub(super) focus: FocusHandle,
    pub(super) focus_nav: ProviderFocus,
    pub(super) url_field: Entity<SettingsField>,
    pub(super) key_field: Entity<SettingsField>,
    pub(super) protocol: Entity<SettingsSelect>,
    pub(super) thinking_select: Entity<SettingsSelect>,
    pub(super) max_tokens_select: Entity<SettingsSelect>,
    pub(super) compat_open: bool,
    pub(super) model_context: Vec<(String, Entity<SettingsSelect>)>,
    pub(super) subscriptions: Vec<Subscription>,
    pub(super) model_subscription_start: usize,
    pub(super) bounds: Rc<RefCell<BTreeMap<String, Bounds<Pixels>>>>,
}

impl Focusable for AddProviderPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl AddProviderPanel {
    /// 每次打开从空白状态开始，并递增 revision 使旧请求失效。
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        let next_revision = self.form.revision.wrapping_add(1).max(1);
        self.form = ProviderForm::new();
        self.form.revision = next_revision;
        self.focus_nav.reset();
        self.saving = false;
        self.url_field.update(cx, |field, cx| {
            field.set_text("", cx);
            field.set_masked(false, cx);
        });
        self.key_field.update(cx, |field, cx| {
            field.set_text("", cx);
            field.set_masked(true, cx);
        });
        self.protocol
            .update(cx, |select, cx| select.set_selected(0, cx));
        self.thinking_select
            .update(cx, |select, cx| select.set_selected(0, cx));
        self.max_tokens_select
            .update(cx, |select, cx| select.set_selected(0, cx));
        self.compat_open = false;
        self.sync_children(cx);
        cx.notify();
    }

    /// 设置覆盖层是否接收输入；退出时仍保留绘制。
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if active && !self.active {
            self.focus_nav.reset();
        }
        self.active = active;
        let enabled = active && !self.saving;
        self.url_field.update(cx, |field, cx| {
            field.set_active(enabled, cx);
            field.set_masked(false, cx);
        });
        let show_key = self.form.show_key;
        self.key_field.update(cx, |field, cx| {
            field.set_active(enabled, cx);
            field.set_masked(!show_key, cx);
        });
        self.protocol
            .update(cx, |select, cx| select.set_active(enabled, cx));
        self.thinking_select
            .update(cx, |select, cx| select.set_active(enabled, cx));
        self.max_tokens_select
            .update(cx, |select, cx| select.set_active(enabled, cx));
        for (_, select) in &self.model_context {
            select.update(cx, |select, cx| select.set_active(enabled, cx));
        }
        cx.notify();
    }

    /// Router 开始或结束保存。
    pub fn set_saving(&mut self, saving: bool, cx: &mut Context<Self>) {
        self.saving = saving;
        self.form.busy = if saving { Busy::Save } else { Busy::Idle };
        self.set_active(self.active, cx);
    }

    /// 保存失败时回到可编辑状态并保留 draft。
    pub fn save_failed(&mut self, error: String, cx: &mut Context<Self>) {
        self.form.receive_save(Err(error));
        self.set_saving(false, cx);
    }

    /// 当前表单只读快照。
    pub fn form(&self) -> &ProviderForm {
        &self.form
    }
    /// URL 字段实体（自测与外层焦点编排使用）。
    pub fn url_field(&self) -> Entity<SettingsField> {
        self.url_field.clone()
    }
    /// Key 字段实体（自测与外层焦点编排使用）。
    pub fn key_field(&self) -> Entity<SettingsField> {
        self.key_field.clone()
    }
    /// 稳定控件的上一帧真实 bounds。
    pub fn control_bounds(&self, id: &str) -> Option<Bounds<Pixels>> {
        self.bounds.borrow().get(id).copied()
    }
    /// 兼容早期预览命名。
    pub fn get_control_bounds(&self, id: &str) -> Option<Bounds<Pixels>> {
        self.control_bounds(id)
    }

    /// 选择预设并同步 draft 字段。
    pub fn select_preset(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.form.select_preset(id);
        let index = usize::from(self.form.protocol == "anthropic");
        self.protocol
            .update(cx, |select, cx| select.set_selected(index, cx));
        let url = self.form.url.clone();
        self.url_field
            .update(cx, |field, cx| field.set_text(&url, cx));
        self.sync_children(cx);
        cx.notify();
    }

    /// 显示或隐藏 API Key。
    pub fn toggle_key_visibility(&mut self, cx: &mut Context<Self>) {
        if !self.active || self.saving {
            return;
        }
        self.form.show_key = !self.form.show_key;
        self.key_field
            .update(cx, |field, cx| field.set_masked(!self.form.show_key, cx));
        cx.notify();
    }

    /// 展开或收起 Compat 区域。
    pub fn toggle_compat(&mut self, cx: &mut Context<Self>) {
        if !self.active || self.saving {
            return;
        }
        self.compat_open = !self.compat_open;
        cx.notify();
    }

    /// 供 provider 根节点在下一次绘制时把焦点放到返回按钮。
    pub(super) fn take_focus_pending(&mut self) -> bool {
        self.focus_nav.take_pending()
    }

    /// 按可见控件顺序循环焦点；方向由 Shift+Tab 决定。
    pub(super) fn focus_next(&self, window: &mut gpui::Window, cx: &mut gpui::App, reverse: bool) {
        self.focus_nav.focus_next(self, window, cx, reverse);
    }

    /// 激活当前焦点上的按钮类控件；输入框和 Select 留给自身处理 Enter/Space。
    pub(super) fn activate_focused(
        &mut self,
        window: &gpui::Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.active || self.saving {
            return false;
        }
        let Some(id) = self.focus_nav.focused_id(self, window, cx) else {
            return false;
        };
        match id.as_str() {
            "back" | "cancel" => cx.emit(ProviderEvent::Back),
            "add" => self.submit(cx),
            "custom" => self.select_preset("custom", cx),
            "show-key" => self.toggle_key_visibility(cx),
            "get-key" => {} // v1 href="#" / preventDefault，无跳转。
            "compat-toggle" => self.toggle_compat(cx),
            "fetch" => self.begin_fetch(cx),
            "latency" => self.begin_latency(cx),
            id if id.starts_with("preset-") => self.select_preset(&id[7..], cx),
            id if id.starts_with("model-") => self.form.toggle_model(&id[6..]),
            id if id.starts_with("vision-") => {
                let model_id = &id[7..];
                if let Some(model) = self.form.models.iter().find(|model| model.id == model_id) {
                    self.form
                        .set_vision_support(model_id, !model.supports_vision);
                }
                cx.notify();
            }
            id if id.starts_with("image-") => {
                let model_id = &id[6..];
                if let Some(model) = self.form.models.iter().find(|model| model.id == model_id) {
                    self.form
                        .set_image_generation_support(model_id, !model.supports_image_generation);
                }
                cx.notify();
            }
            _ => return false,
        }
        cx.notify();
        true
    }

    /// 稳定焦点句柄，供 provider 各渲染模块给控件绑定 track_focus。
    pub(super) fn focus_handle(&self, id: &str) -> Option<FocusHandle> {
        self.focus_nav.handle(id)
    }

    /// 动态模型行的稳定焦点句柄。
    pub(super) fn model_focus(&self, id: &str, kind: &str) -> Option<FocusHandle> {
        let key = format!("{kind}-{id}");
        self.focus_nav.handle(&key)
    }

    /// provider-content 使用的 ScrollHandle，供预览/外层自测观察滚动位置。
    pub fn content_scroll(&self) -> gpui::ScrollHandle {
        self.focus_nav.scroll.clone()
    }

    /// 模型上下文菜单的实际边界。
    pub fn context_menu_bounds(&self, model_id: &str, cx: &App) -> Option<Bounds<Pixels>> {
        self.model_context
            .iter()
            .find(|(id, _)| id == model_id)
            .and_then(|(_, select)| select.read(cx).painted_menu_bounds_for_test())
    }

    /// 当前焦点控件的稳定 ID，供真实键盘自测确认 Tab 顺序。
    pub fn focused_control(&self, window: &gpui::Window, cx: &gpui::App) -> Option<String> {
        self.focus_nav.focused_id(self, window, cx)
    }
}
