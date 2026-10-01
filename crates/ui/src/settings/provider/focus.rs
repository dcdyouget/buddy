//! Add Provider 面板的稳定焦点注册表与 Tab 导航。

use super::panel::AddProviderPanel;
use crate::settings::provider_presets::PRESETS;
use gpui::{px, App, Context, FocusHandle, Focusable, ScrollHandle, Window};
use std::collections::BTreeMap;

/// 动态模型行上的键盘控件。
#[derive(Clone)]
pub(super) struct ModelFocus {
    pub checkbox: FocusHandle,
    pub vision: FocusHandle,
    pub image: FocusHandle,
}

/// Provider 面板自身的焦点注册表。
///
/// 控件实体会随着 render 重建，但这里的句柄只在 panel 生命周期内创建一次，
/// 因而鼠标点击、Tab 导航和退出动画不会丢失焦点身份。
pub(super) struct ProviderFocus {
    pub back: FocusHandle,
    pub presets: BTreeMap<String, FocusHandle>,
    pub custom: FocusHandle,
    pub show_key: FocusHandle,
    pub get_key: FocusHandle,
    pub compat: FocusHandle,
    pub fetch: FocusHandle,
    pub latency: FocusHandle,
    pub cancel: FocusHandle,
    pub add: FocusHandle,
    pub models: BTreeMap<String, ModelFocus>,
    pub scroll: ScrollHandle,
    pending: bool,
}

impl ProviderFocus {
    pub(super) fn new(cx: &mut Context<AddProviderPanel>) -> Self {
        let presets = PRESETS
            .iter()
            .map(|preset| (preset.id.to_owned(), cx.focus_handle()))
            .collect();
        Self {
            back: cx.focus_handle(),
            presets,
            custom: cx.focus_handle(),
            show_key: cx.focus_handle(),
            get_key: cx.focus_handle(),
            compat: cx.focus_handle(),
            fetch: cx.focus_handle(),
            latency: cx.focus_handle(),
            cancel: cx.focus_handle(),
            add: cx.focus_handle(),
            models: BTreeMap::new(),
            scroll: ScrollHandle::new(),
            pending: true,
        }
    }

    pub(super) fn reset(&mut self) {
        self.pending = true;
    }

    pub(super) fn take_pending(&mut self) -> bool {
        std::mem::take(&mut self.pending)
    }

    pub(super) fn sync_models(
        &mut self,
        ids: impl IntoIterator<Item = String>,
        cx: &mut Context<AddProviderPanel>,
    ) {
        let ids: Vec<String> = ids.into_iter().collect();
        self.models
            .retain(|id, _| ids.iter().any(|known| known == id));
        for id in ids {
            self.models.entry(id).or_insert_with(|| ModelFocus {
                checkbox: cx.focus_handle(),
                vision: cx.focus_handle(),
                image: cx.focus_handle(),
            });
        }
    }

    pub(super) fn handle(&self, id: &str) -> Option<FocusHandle> {
        match id {
            "back" => Some(self.back.clone()),
            "custom" => Some(self.custom.clone()),
            "show-key" => Some(self.show_key.clone()),
            "get-key" => Some(self.get_key.clone()),
            "compat-toggle" => Some(self.compat.clone()),
            "fetch" => Some(self.fetch.clone()),
            "latency" => Some(self.latency.clone()),
            "cancel" => Some(self.cancel.clone()),
            "add" => Some(self.add.clone()),
            id if id.starts_with("preset-") => self.presets.get(&id[7..]).cloned(),
            id if id.starts_with("model-") => self
                .models
                .get(&id[6..])
                .map(|model| model.checkbox.clone()),
            id if id.starts_with("vision-") => {
                self.models.get(&id[7..]).map(|model| model.vision.clone())
            }
            id if id.starts_with("image-") => {
                self.models.get(&id[6..]).map(|model| model.image.clone())
            }
            _ => None,
        }
    }

    /// 取出当前状态下可见、可交互的焦点顺序。
    pub(super) fn order(&self, panel: &AddProviderPanel, cx: &App) -> Vec<(String, FocusHandle)> {
        if !panel.active || panel.saving {
            return Vec::new();
        }
        let mut order = vec![("back".to_owned(), self.back.clone())];
        for preset in PRESETS {
            if let Some(handle) = self.presets.get(preset.id) {
                order.push((format!("preset-{}", preset.id), handle.clone()));
            }
        }
        order.push(("custom".to_owned(), self.custom.clone()));

        let custom = panel.form.preset.as_deref() == Some("custom");
        if custom {
            order.push((
                "protocol".to_owned(),
                panel.protocol.read(cx).focus_handle(cx),
            ));
            order.push(("compat-toggle".to_owned(), self.compat.clone()));
            if panel.compat_open && panel.form.protocol == "openai_compatible" {
                order.push((
                    "thinking".to_owned(),
                    panel.thinking_select.read(cx).focus_handle(cx),
                ));
                order.push((
                    "max-tokens".to_owned(),
                    panel.max_tokens_select.read(cx).focus_handle(cx),
                ));
            }
        }

        order.push(("url".to_owned(), panel.url_field.read(cx).focus_handle(cx)));
        order.push(("key".to_owned(), panel.key_field.read(cx).focus_handle(cx)));
        order.push(("show-key".to_owned(), self.show_key.clone()));
        order.push(("get-key".to_owned(), self.get_key.clone()));
        if panel.form.busy == crate::settings::provider_form::Busy::Idle {
            order.push(("fetch".to_owned(), self.fetch.clone()));
        }
        if !panel.form.models.is_empty() {
            order.push(("latency".to_owned(), self.latency.clone()));
        }

        for model in &panel.form.models {
            let Some(handles) = self.models.get(&model.id) else {
                continue;
            };
            order.push((format!("model-{}", model.id), handles.checkbox.clone()));
            order.push((format!("vision-{}", model.id), handles.vision.clone()));
            if panel.form.protocol != "anthropic" {
                order.push((format!("image-{}", model.id), handles.image.clone()));
            }
            if let Some(select) = panel
                .model_context
                .iter()
                .find(|(id, _)| id == &model.id)
                .map(|(_, select)| select.read(cx).focus_handle(cx))
            {
                order.push((format!("context-{}", model.id), select));
            }
        }

        order.push(("cancel".to_owned(), self.cancel.clone()));
        if panel.form.can_add() {
            order.push(("add".to_owned(), self.add.clone()));
        }
        order
    }

    pub(super) fn focus_next(
        &self,
        panel: &AddProviderPanel,
        window: &mut Window,
        cx: &mut App,
        reverse: bool,
    ) {
        let order = self.order(panel, cx);
        if order.is_empty() {
            return;
        }
        let current = order
            .iter()
            .position(|(_, handle)| handle.is_focused(window));
        let next = match (current, reverse) {
            (Some(index), false) => (index + 1) % order.len(),
            (Some(index), true) => (index + order.len() - 1) % order.len(),
            (None, false) => 0,
            (None, true) => order.len() - 1,
        };
        let (id, handle) = &order[next];
        window.focus(handle, cx);
        self.scroll_to(id, panel);
    }

    pub(super) fn focused_id(
        &self,
        panel: &AddProviderPanel,
        window: &Window,
        cx: &App,
    ) -> Option<String> {
        self.order(panel, cx)
            .into_iter()
            .find(|(_, handle)| handle.is_focused(window))
            .map(|(id, _)| id)
    }

    /// 用已记录的真实 bounds 把焦点控件滚入 provider-content 视口。
    fn scroll_to(&self, id: &str, panel: &AddProviderPanel) {
        if matches!(id, "back" | "cancel" | "add") {
            return;
        }
        let bounds_id = if id == "compat-toggle" {
            "compat"
        } else if id == "get-key" {
            "key"
        } else {
            id
        };
        let Some(target) = panel.bounds.borrow().get(bounds_id).copied() else {
            return;
        };
        let viewport = self.scroll.bounds();
        if viewport.size.height <= px(0.) {
            return;
        }
        let offset = self.scroll.offset();
        let mut next = offset;
        if target.top() < viewport.top() {
            next.y += viewport.top() - target.top();
        } else if target.bottom() > viewport.bottom() {
            next.y -= target.bottom() - viewport.bottom();
        }
        let max = self.scroll.max_offset();
        next.y = next.y.max(-max.y).min(px(0.));
        if next != offset {
            self.scroll.set_offset(next);
        }
    }
}
