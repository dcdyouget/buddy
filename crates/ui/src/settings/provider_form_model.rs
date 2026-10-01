//! ProviderForm 的模型、异步结果与提交扩展。

use buddy_engine::models::{raw_model_id, CompatConfig, ModelInfo, ProviderConfig};

use super::super::provider_presets::{preset, ProviderPreset, CUSTOM_PRESET_ID};
use super::{Busy, ProviderForm, ProviderRequest, ProviderSubmission, EMPTY_MODELS_ERROR};

impl ProviderForm {
    /// 切换一个已获取模型的选中状态。
    pub fn toggle_model(&mut self, model_id: &str) {
        if !self.models.iter().any(|model| model.id == model_id) {
            return;
        }
        if !self.selected.insert(model_id.to_string()) {
            self.selected.remove(model_id);
        }
    }

    /// 覆盖模型上下文窗口大小。
    pub fn set_context_window(&mut self, model_id: &str, context_window: u32) {
        if let Some(model) = self.models.iter_mut().find(|model| model.id == model_id) {
            model.context_window = context_window;
        }
    }

    /// 覆盖模型是否支持视觉输入。
    pub fn set_vision_support(&mut self, model_id: &str, supports_vision: bool) {
        if let Some(model) = self.models.iter_mut().find(|model| model.id == model_id) {
            model.supports_vision = supports_vision;
        }
    }

    /// 覆盖模型是否支持生图；Anthropic 协议始终禁用生图。
    pub fn set_image_generation_support(&mut self, model_id: &str, enabled: bool) {
        if self.protocol == "anthropic" {
            return;
        }
        if let Some(model) = self.models.iter_mut().find(|model| model.id == model_id) {
            model.supports_image_generation = enabled;
        }
    }

    /// 判断当前表单是否具备可提交的连接和模型选择。
    pub fn can_add(&self) -> bool {
        self.preset.is_some()
            && !self.url.trim().is_empty()
            && !self.key.is_empty()
            && !self.models.is_empty()
            && self
                .models
                .iter()
                .any(|model| self.selected.contains(&model.id))
            && self.busy == Busy::Idle
    }

    /// 开始获取模型列表并返回请求快照。
    pub fn begin_fetch(&mut self) -> Option<ProviderRequest> {
        if self.url.trim().is_empty() || self.key.is_empty() || self.busy != Busy::Idle {
            return None;
        }
        self.error = None;
        self.busy = Busy::Fetch;
        Some(self.request())
    }

    /// 接收模型列表结果；revision 不匹配的旧结果会被丢弃。
    pub fn receive_fetch(&mut self, revision: u64, result: Result<Vec<ModelInfo>, String>) {
        if revision != self.revision {
            return;
        }
        self.busy = Busy::Idle;
        match result {
            // v1 在空结果时只更新错误，保留同一连接上次成功的模型列表。
            Ok(models) if models.is_empty() => {
                self.error = Some(EMPTY_MODELS_ERROR.to_string());
            }
            Ok(mut models) => {
                let provider_id = self
                    .preset
                    .as_deref()
                    .filter(|id| *id != CUSTOM_PRESET_ID)
                    .unwrap_or(CUSTOM_PRESET_ID);
                for model in &mut models {
                    model.provider_id = provider_id.to_string();
                    if self.protocol == "anthropic" {
                        model.supports_image_generation = false;
                    }
                }
                self.selected = models.iter().map(|model| model.id.clone()).collect();
                self.models = models;
                self.error = None;
            }
            // v1 的 catch 也保留此前成功列表；连接字段变化会先 invalidate。
            Err(error) => self.error = Some(error),
        }
    }

    /// 开始测试当前第一个模型，并返回原始模型 ID。
    pub fn begin_latency(&mut self) -> Option<(ProviderRequest, String)> {
        if self.busy != Busy::Idle || self.url.trim().is_empty() || self.key.is_empty() {
            return None;
        }
        let model = self.models.first()?;
        let raw_id = raw_model_id(model).to_string();
        self.error = None;
        self.busy = Busy::Latency;
        Some((self.request(), raw_id))
    }

    /// 接收延迟测试结果；revision 不匹配的旧结果会被丢弃。
    pub fn receive_latency(&mut self, revision: u64, model_id: &str, result: Result<u32, String>) {
        if revision != self.revision {
            return;
        }
        self.busy = Busy::Idle;
        let Some(model) = self
            .models
            .iter_mut()
            .find(|model| raw_model_id(model) == model_id || model.id == model_id)
        else {
            return;
        };
        match result {
            Ok(latency) => {
                model.latency_ms = Some(latency);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    /// 构造提交数据并进入保存状态；默认模型由 `merge_provider` 取第一个模型。
    pub fn submission(&mut self, custom_id: String) -> Option<ProviderSubmission> {
        if !self.can_add() {
            return None;
        }
        let is_custom = self.preset.as_deref() == Some(CUSTOM_PRESET_ID);
        let provider_id = if is_custom {
            if custom_id.trim().is_empty() {
                return None;
            }
            custom_id
        } else {
            self.preset.clone()?
        };
        let selected_preset = if is_custom {
            None
        } else {
            Some(preset(&provider_id)?)
        };
        let provider_name = selected_preset
            .map(|item| item.name.to_string())
            .unwrap_or_else(|| "自定义服务".to_string());
        let protocol = selected_preset
            .map(|item| item.protocol.to_string())
            .unwrap_or_else(|| self.protocol.clone());
        let models = self
            .models
            .iter()
            .filter(|model| self.selected.contains(&model.id))
            .cloned()
            .map(|mut model| {
                model.provider_id = provider_id.clone();
                if protocol == "anthropic" {
                    model.supports_image_generation = false;
                }
                model
            })
            .collect::<Vec<_>>();
        if models.is_empty() {
            return None;
        }
        let enabled_model_ids = models
            .iter()
            .map(|model| raw_model_id(model).to_string())
            .collect();
        let compat = if is_custom {
            self.compat_edited.then(|| self.compat_config())
        } else {
            Some(self.compat_config_for_preset(selected_preset?))
        };
        let provider = ProviderConfig {
            id: provider_id,
            name: provider_name,
            base_url: self.url.clone(),
            api_key: self.key.clone(),
            enabled_model_ids,
            provider_type: protocol,
            compat,
        };
        self.busy = Busy::Save;
        Some(ProviderSubmission { provider, models })
    }

    /// 保存失败时恢复可编辑状态并保留全部表单输入。
    pub fn receive_save(&mut self, result: Result<(), String>) {
        self.busy = Busy::Idle;
        if let Err(error) = result {
            self.error = Some(error);
        }
    }

    fn compat_config(&self) -> CompatConfig {
        CompatConfig {
            thinking_format: (!self.thinking_format.is_empty())
                .then(|| self.thinking_format.clone()),
            max_tokens_field: (!self.max_tokens_field.is_empty())
                .then(|| self.max_tokens_field.clone()),
            supports_stream_options_usage: None,
            supports_reasoning_effort: None,
            supports_store: None,
            supports_developer_role: None,
            supports_temperature: None,
            supports_long_cache_retention: None,
            supports_tools: None,
        }
    }

    fn compat_config_for_preset(&self, preset: &ProviderPreset) -> CompatConfig {
        CompatConfig {
            thinking_format: (!preset.thinking_format.is_empty())
                .then(|| preset.thinking_format.to_string()),
            max_tokens_field: (!preset.max_tokens_field.is_empty())
                .then(|| preset.max_tokens_field.to_string()),
            supports_stream_options_usage: None,
            supports_reasoning_effort: None,
            supports_store: None,
            supports_developer_role: None,
            supports_temperature: None,
            supports_long_cache_retention: None,
            supports_tools: None,
        }
    }
}
