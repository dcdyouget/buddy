//! AddProviderPanel 的网络请求和动态子控件。

use super::panel::{AddProviderPanel, ProviderEvent};
use crate::theme_system::tokens::metrics as m;
use crate::{
    chat_bridge::spawn_engine,
    settings::select::{SettingsSelect, SettingsSelectChanged},
};
use gpui::{px, AppContext, Context};
use std::time::{SystemTime, UNIX_EPOCH};

impl AddProviderPanel {
    /// 发起 fetch_models；只在 engine future 中进行网络访问。
    pub fn begin_fetch(&mut self, cx: &mut Context<Self>) {
        if !self.active || self.saving {
            return;
        }
        let Some(request) = self.form.begin_fetch() else {
            return;
        };
        let engine = self.engine.clone();
        let task = spawn_engine(cx, async move {
            engine
                .fetch_models(request.url, request.key, Some(request.protocol))
                .await
        });
        let entity = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = entity.update(cx, |this, cx| {
                this.form.receive_fetch(request.revision, result);
                this.sync_children(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// 发起 test_latency。
    pub fn begin_latency(&mut self, cx: &mut Context<Self>) {
        if !self.active || self.saving {
            return;
        }
        let Some((request, model_id)) = self.form.begin_latency() else {
            return;
        };
        let engine = self.engine.clone();
        let model_for_call = model_id.clone();
        let task = spawn_engine(cx, async move {
            engine
                .test_latency(
                    request.url,
                    request.key,
                    model_for_call,
                    Some(request.protocol),
                )
                .await
        });
        let entity = cx.entity().downgrade();
        cx.spawn(async move |_, cx| {
            let result = task.await;
            let _ = entity.update(cx, |this, cx| {
                this.form
                    .receive_latency(request.revision, &model_id, result);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// 提交当前 draft；保存成功与否由 Router 决定。
    pub fn submit(&mut self, cx: &mut Context<Self>) {
        if !self.active || self.saving {
            return;
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        if let Some(submission) = self.form.submission(format!("custom-{stamp}")) {
            cx.emit(ProviderEvent::Submit(submission));
        }
    }

    pub(super) fn sync_children(&mut self, cx: &mut Context<Self>) {
        self.sync_compat_selects(cx);
        self.model_context.clear();
        self.subscriptions.truncate(self.model_subscription_start);
        let ids: Vec<String> = self
            .form
            .models
            .iter()
            .map(|model| model.id.clone())
            .collect();
        self.focus_nav.sync_models(ids.clone(), cx);
        let enabled = self.active && !self.saving;
        for id in ids {
            if self.model_context.iter().any(|(known, _)| known == &id) {
                continue;
            }
            let Some(model) = self
                .form
                .models
                .iter()
                .find(|model| model.id == id)
                .cloned()
            else {
                continue;
            };
            let options = context_options(model.context_window);
            let select = cx.new(|cx| {
                SettingsSelect::new(
                    format!("context-{id}"),
                    options.iter().map(|value| format_context(*value)),
                    options
                        .iter()
                        .position(|value| *value == model.context_window)
                        .unwrap_or(0),
                    cx,
                )
            });
            select.update(cx, |select, cx| {
                select.set_active(enabled, cx);
                select.set_width(px(m::SPACE_8 * 2.0), cx);
            });
            let model_id = id.clone();
            self.subscriptions.push(cx.subscribe(
                &select,
                move |this, _, _: &SettingsSelectChanged, cx| {
                    let value = this
                        .model_context
                        .iter()
                        .find(|(known, _)| known == &model_id)
                        .map(|(_, select)| select.read(cx).selected_index())
                        .and_then(|index| options.get(index).copied());
                    if let Some(value) = value {
                        this.form.set_context_window(&model_id, value);
                    }
                    cx.notify();
                },
            ));
            self.model_context.push((id, select));
        }
    }

    pub(super) fn sync_compat_selects(&mut self, cx: &mut Context<Self>) {
        let thinking_index = super::panel_init::THINKING_OPTIONS
            .iter()
            .position(|(value, _)| *value == self.form.thinking_format)
            .unwrap_or(0);
        let max_index = ["max_tokens", "max_completion_tokens"]
            .iter()
            .position(|item| *item == self.form.max_tokens_field)
            .unwrap_or(0);
        self.thinking_select
            .update(cx, |select, cx| select.set_selected(thinking_index, cx));
        self.max_tokens_select
            .update(cx, |select, cx| select.set_selected(max_index, cx));
    }
}

fn context_options(value: u32) -> Vec<u32> {
    let mut options = vec![128_000, 256_000, 512_000, 1_000_000];
    if !options.contains(&value) {
        options.push(value);
    }
    options.sort_unstable();
    options
}
fn format_context(value: u32) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f32 / 1_000_000.0)
    } else {
        format!("{}K", (value as f64 / 1_000.0).round() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn context_options_keep_exact_nonpreset_value() {
        let options = context_options(32_768);
        assert_eq!(options, vec![32_768, 128_000, 256_000, 512_000, 1_000_000]);
        assert_eq!(format_context(options[0]), "33K");
        assert_eq!(format_context(options[4]), "1.0M");
    }
}
