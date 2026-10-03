//! Provider 输入实体与事件订阅初始化。
use super::{focus::ProviderFocus, panel::AddProviderPanel};
use crate::settings::{
    controls::{SettingsField, SettingsFieldEvent},
    provider_form::ProviderForm,
    select::{SettingsSelect, SettingsSelectChanged},
};
use crate::theme_system::tokens::metrics as m;
use buddy_engine::chat::ChatEngine;
use gpui::{px, AppContext, Context};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

pub(super) const THINKING_OPTIONS: [(&str, &str); 6] = [
    ("openai", "openai (reasoning_effort)"),
    ("deepseek", "deepseek (thinking type)"),
    ("openrouter", "openrouter (reasoning effort)"),
    ("qwen", "qwen (enable_thinking)"),
    ("together", "together (reasoning enabled)"),
    ("zai", "zai (thinking type)"),
];

impl AddProviderPanel {
    /// 构建面板；调用方可在每次进入时调用 [`Self::reset`]。
    pub fn new(engine: Arc<ChatEngine>, cx: &mut Context<Self>) -> Self {
        let url_field =
            cx.new(|cx| SettingsField::with_id("provider-url", "https://api.example.com/v1", cx));
        let key_field = cx.new(|cx| SettingsField::with_id("provider-key", "sk-...", cx));
        let protocol = cx.new(|cx| {
            SettingsSelect::new(
                "provider-protocol",
                ["OpenAI 兼容 (chat/completions)", "Anthropic (messages API)"],
                0,
                cx,
            )
        });
        let thinking_select = cx.new(|cx| {
            SettingsSelect::new(
                "compat-thinking",
                THINKING_OPTIONS.iter().map(|(_, label)| *label),
                0,
                cx,
            )
        });
        let max_tokens_select = cx.new(|cx| {
            SettingsSelect::new(
                "compat-max-tokens",
                ["max_tokens", "max_completion_tokens"],
                0,
                cx,
            )
        });
        url_field.update(cx, |field, cx| field.full_width(true, cx));
        key_field.update(cx, |field, cx| field.full_width(true, cx));
        protocol.update(cx, |select, cx| select.full_width(true, cx));
        thinking_select.update(cx, |select, cx| select.set_width(px(m::SPACE_8 * 8.0), cx));
        max_tokens_select.update(cx, |select, cx| select.set_width(px(m::SPACE_8 * 8.0), cx));
        let mut panel = Self {
            engine,
            form: ProviderForm::new(),
            active: true,
            saving: false,
            focus: cx.focus_handle(),
            focus_nav: ProviderFocus::new(cx),
            url_field,
            key_field,
            protocol,
            thinking_select,
            max_tokens_select,
            compat_open: false,
            model_context: vec![],
            subscriptions: vec![],
            model_subscription_start: 0,
            bounds: Rc::new(RefCell::new(BTreeMap::new())),
            drag_source: crate::chat::drag::default_drag_source(),
        };
        panel.subscriptions.push(cx.subscribe(
            &panel.url_field,
            |this, _, event: &SettingsFieldEvent, cx| {
                if *event == SettingsFieldEvent::Changed {
                    let value = this.url_field.read(cx).text(cx);
                    this.form.set_url(value);
                    cx.notify();
                }
            },
        ));
        panel.subscriptions.push(cx.subscribe(
            &panel.key_field,
            |this, _, event: &SettingsFieldEvent, cx| {
                if *event == SettingsFieldEvent::Changed {
                    let value = this.key_field.read(cx).text(cx);
                    this.form.set_key(value);
                    cx.notify();
                }
            },
        ));
        panel.subscriptions.push(cx.subscribe(
            &panel.protocol,
            |this, _, _: &SettingsSelectChanged, cx| {
                let protocol = if this.protocol.read(cx).selected_index() == 1 {
                    "anthropic"
                } else {
                    "openai_compatible"
                };
                if this.form.preset.as_deref() == Some("custom") {
                    this.form.set_protocol(protocol.into());
                }
                cx.notify();
            },
        ));
        panel.subscriptions.push(cx.subscribe(
            &panel.thinking_select,
            |this, _, _: &SettingsSelectChanged, cx| {
                let value = THINKING_OPTIONS
                    .get(this.thinking_select.read(cx).selected_index())
                    .map(|(value, _)| value.to_string());
                if let Some(value) = value {
                    this.form.set_thinking_format(value);
                }
                cx.notify();
            },
        ));
        panel.subscriptions.push(cx.subscribe(
            &panel.max_tokens_select,
            |this, _, _: &SettingsSelectChanged, cx| {
                let value = this
                    .max_tokens_select
                    .read(cx)
                    .selected_value()
                    .map(|value| value.to_string());
                if let Some(value) = value {
                    this.form.set_max_tokens_field(value);
                }
                cx.notify();
            },
        ));
        panel.model_subscription_start = panel.subscriptions.len();
        panel.sync_children(cx);
        panel
    }
}
