//! 已保存模型列表（S06-03）。
//!
//! 该实体只负责设置页中的编辑交互；配置写盘由 SettingsView/Router 串行完成。

mod render;

use crate::settings::{
    model_config::{ModelEdit, context_options, model_enabled},
    select::{SettingsSelect, SettingsSelectChanged},
};
use crate::theme_system::tokens::metrics as m;
use buddy_engine::models::{AppConfig, ModelInfo};
use gpui::{
    App, AppContext, Bounds, Context, Entity, EventEmitter, FocusHandle, Focusable, Pixels, Render,
    Subscription, Window,
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

#[derive(Clone)]
struct ModelFocus {
    enabled: FocusHandle,
    default: FocusHandle,
    vision: FocusHandle,
    image: FocusHandle,
}

/// 已保存模型的设置列表。
pub struct ModelListView {
    config: AppConfig,
    active: bool,
    saving: bool,
    error: Option<String>,
    focus: BTreeMap<String, ModelFocus>,
    contexts: BTreeMap<String, Entity<SettingsSelect>>,
    context_values: BTreeMap<String, Vec<u32>>,
    bounds: Rc<RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ModelEdit> for ModelListView {}

impl Focusable for ModelListView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.focus
            .values()
            .next()
            .map(|focus| focus.enabled.clone())
            .unwrap_or_else(|| cx.focus_handle())
    }
}

impl ModelListView {
    /// 创建已保存模型列表实体。
    pub fn new(config: AppConfig, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            config,
            active: false,
            saving: false,
            error: None,
            focus: BTreeMap::new(),
            contexts: BTreeMap::new(),
            context_values: BTreeMap::new(),
            bounds: Rc::new(RefCell::new(BTreeMap::new())),
            _subscriptions: Vec::new(),
        };
        this.sync_models(cx);
        this
    }

    /// 同步 Router 发布的新配置，同时保留已有控件实体与焦点。
    pub fn set_config(&mut self, config: AppConfig, cx: &mut Context<Self>) {
        self.config = config;
        self.sync_models(cx);
        self.error = None;
        cx.notify();
    }

    /// 设置列表是否接受输入；关闭时同步释放子选择器。
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.active = active;
        let enabled = active && !self.saving;
        for select in self.contexts.values() {
            select.update(cx, |select, cx| select.set_active(enabled, cx));
        }
        if !active {
            self.error = None;
        }
        cx.notify();
    }

    /// 切换配置保存中的交互锁定状态。
    pub fn set_saving(&mut self, saving: bool, cx: &mut Context<Self>) {
        self.saving = saving;
        if saving {
            self.error = None;
        }
        let enabled = self.active && !saving;
        for select in self.contexts.values() {
            select.update(cx, |select, cx| select.set_active(enabled, cx));
        }
        cx.notify();
    }

    /// 保存失败后恢复交互并显示错误信息。
    pub fn model_save_failed(&mut self, message: String, cx: &mut Context<Self>) {
        self.saving = false;
        self.error = Some(message);
        // Selects update their displayed selection on user input. Restore the
        // committed value after a failed write, just as controlled v1 inputs do.
        self.sync_models(cx);
        let enabled = self.active;
        for select in self.contexts.values() {
            select.update(cx, |select, cx| select.set_active(enabled, cx));
        }
        cx.notify();
    }

    /// 当前是否正在等待配置写盘。
    pub fn saving(&self) -> bool {
        self.saving
    }

    /// 当前保存错误（若有）。
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// 读取上一帧控件的真实窗口边界。
    pub fn control_bounds(&self, id: &str) -> Option<Bounds<Pixels>> {
        self.bounds.borrow().get(id).copied()
    }

    /// 读取模型的上下文选择器实体，供真实输入自测使用。
    pub fn model_context(&self, model_id: &str) -> Option<Entity<SettingsSelect>> {
        self.contexts.get(model_id).cloned()
    }

    /// 当前列表显示的配置快照。
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// 设置页外层 Tab 导航使用的稳定顺序；模型列表不接管父级滚动。
    pub fn focus_order(&self, cx: &App) -> Vec<(String, FocusHandle)> {
        if !self.active || self.saving {
            return Vec::new();
        }
        let mut order = Vec::new();
        for model in &self.config.models {
            let Some(focus) = self.focus.get(&model.id) else {
                continue;
            };
            order.push((format!("enable-{}", model.id), focus.enabled.clone()));
            if let Some(select) = self.contexts.get(&model.id) {
                order.push((
                    format!("context-{}", model.id),
                    select.read(cx).focus_handle(cx),
                ));
            }
            order.push((format!("vision-{}", model.id), focus.vision.clone()));
            if self.provider_type(&model.id) == Some("openai_compatible") {
                order.push((format!("image-{}", model.id), focus.image.clone()));
            }
            if model.id != self.config.selected_model_id && self.model_enabled(&model.id) {
                order.push((format!("default-{}", model.id), focus.default.clone()));
            }
        }
        order
    }

    fn sync_models(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<String> = self
            .config
            .models
            .iter()
            .map(|model| model.id.clone())
            .collect();
        self.focus
            .retain(|id, _| ids.iter().any(|known| known == id));
        self.contexts
            .retain(|id, _| ids.iter().any(|known| known == id));
        self.context_values
            .retain(|id, _| ids.iter().any(|known| known == id));

        for id in ids {
            self.focus.entry(id.clone()).or_insert_with(|| ModelFocus {
                enabled: cx.focus_handle(),
                default: cx.focus_handle(),
                vision: cx.focus_handle(),
                image: cx.focus_handle(),
            });
            let Some(model) = self.config.models.iter().find(|model| model.id == id) else {
                continue;
            };
            let options = context_options(model.context_window);
            let selected = options
                .iter()
                .position(|value| *value == model.context_window)
                .unwrap_or(0);
            self.context_values.insert(id.clone(), options);
            if let Some(select) = self.contexts.get(&id) {
                let labels = self.context_values[&id]
                    .iter()
                    .map(|value| crate::settings::model_config::format_context(*value))
                    .collect::<Vec<_>>();
                select.update(cx, |select, cx| {
                    select.reset_options(labels, selected, cx);
                    select.set_active(self.active && !self.saving, cx);
                });
            } else {
                let select_id = format!("context-{id}");
                let select = cx.new(|cx| {
                    SettingsSelect::new(
                        select_id,
                        self.context_values[&id]
                            .iter()
                            .map(|value| crate::settings::model_config::format_context(*value)),
                        selected,
                        cx,
                    )
                });
                select.update(cx, |select, cx| {
                    select.set_width(gpui::px(m::SPACE_8 * 2.0), cx);
                    select.set_active(self.active && !self.saving, cx);
                });
                let model_id = id.clone();
                let subscription = cx.subscribe(
                    &select,
                    move |this, entity, _: &SettingsSelectChanged, cx| {
                        let index = entity.read(cx).selected_index();
                        if let Some(value) = this
                            .context_values
                            .get(&model_id)
                            .and_then(|values| values.get(index))
                            .copied()
                        {
                            if this.active && !this.saving {
                                cx.emit(ModelEdit::SetContext(model_id.clone(), value));
                            }
                        }
                    },
                );
                self._subscriptions.push(subscription);
                self.contexts.insert(id, select);
            }
        }
    }

    fn model_focus(&self, id: &str) -> Option<&ModelFocus> {
        self.focus.get(id)
    }

    pub(super) fn model_enabled(&self, id: &str) -> bool {
        self.config
            .models
            .iter()
            .find(|model| model.id == id)
            .is_some_and(|model| model_enabled(&self.config, model))
    }

    pub(super) fn provider_type(&self, id: &str) -> Option<&str> {
        self.config
            .models
            .iter()
            .find(|model| model.id == id)
            .and_then(|model| {
                self.config
                    .providers
                    .iter()
                    .find(|provider| provider.id == model.provider_id)
            })
            .map(|provider| provider.provider_type.as_str())
    }

    pub(super) fn model(&self, id: &str) -> Option<&ModelInfo> {
        self.config.models.iter().find(|model| model.id == id)
    }

    pub(super) fn bounds(&self) -> Rc<RefCell<BTreeMap<String, Bounds<Pixels>>>> {
        self.bounds.clone()
    }
}

impl Render for ModelListView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        self.render_list(window, cx)
    }
}
