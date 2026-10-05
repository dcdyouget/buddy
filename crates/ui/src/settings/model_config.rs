//! 已保存模型的纯配置变换。
//!
//! 模型 ID 是配置中的完整作用域 ID。这里始终按 `ModelInfo.provider_id` 找到
//! Provider，再比较完整 ID；原始模型 ID 可能包含 `::`，不能从字符串猜测归属。

use buddy_engine::models::{AppConfig, ModelInfo};

/// 支持的模型编辑操作。
#[derive(Clone, Debug)]
pub enum ModelEdit {
    /// 将一个已存在的模型设为默认模型。
    SetDefault(String),
    /// 切换模型在所属 Provider 下的启用状态。
    ToggleEnabled(String),
    /// 更新上下文窗口 token 数。
    SetContext(String, u32),
    /// 更新图片输入能力。
    SetVision(String, bool),
    /// 更新图片生成能力；只有 OpenAI 兼容 Provider 可以开启。
    SetImageGeneration(String, bool),
}

/// 应用一次模型编辑，返回新的配置，不修改传入的配置。
///
/// 找不到模型、所属 Provider 或不允许的能力变更时返回内容相同的克隆，
/// 这样调用方可以把未知事件安全地当作 no-op 处理。
pub fn apply_model_edit(config: &AppConfig, edit: &ModelEdit) -> AppConfig {
    let mut next = config.clone();
    let model_id = match edit {
        ModelEdit::SetDefault(id)
        | ModelEdit::ToggleEnabled(id)
        | ModelEdit::SetContext(id, _)
        | ModelEdit::SetVision(id, _)
        | ModelEdit::SetImageGeneration(id, _) => id,
    };

    let Some(model_index) = next.models.iter().position(|model| model.id == *model_id) else {
        return next;
    };
    let provider_id = next.models[model_index].provider_id.clone();
    let Some(provider_index) = next
        .providers
        .iter()
        .position(|provider| provider.id == provider_id)
    else {
        return next;
    };

    match edit {
        ModelEdit::SetDefault(_) => {
            next.selected_model_id = model_id.clone();
        }
        ModelEdit::ToggleEnabled(_) => {
            let provider = &mut next.providers[provider_index];
            if let Some(index) = provider
                .enabled_model_ids
                .iter()
                .position(|id| id == model_id)
            {
                provider.enabled_model_ids.remove(index);
            } else {
                provider.enabled_model_ids.push(model_id.clone());
            }

            let selected_is_enabled = next
                .models
                .iter()
                .find(|model| model.id == next.selected_model_id)
                .is_some_and(|model| model_enabled(&next, model));
            if !selected_is_enabled {
                next.selected_model_id = next
                    .models
                    .iter()
                    .find(|model| model_enabled(&next, model))
                    .map(|model| model.id.clone())
                    .unwrap_or_default();
            }
        }
        ModelEdit::SetContext(_, value) => {
            next.models[model_index].context_window = *value;
        }
        ModelEdit::SetVision(_, value) => {
            next.models[model_index].supports_vision = *value;
        }
        ModelEdit::SetImageGeneration(_, value) => {
            if next.providers[provider_index].provider_type == "openai_compatible" {
                next.models[model_index].supports_image_generation = *value;
            } else if !*value {
                // Disabling remains safe for a Provider that cannot enable this feature.
                next.models[model_index].supports_image_generation = false;
            }
        }
    }

    next
}

/// 判断模型是否在其所属 Provider 的启用列表中。
pub fn model_enabled(config: &AppConfig, model: &ModelInfo) -> bool {
    config
        .providers
        .iter()
        .find(|provider| provider.id == model.provider_id)
        .is_some_and(|provider| provider_enabled(provider, &model.id))
}

fn provider_enabled(provider: &buddy_engine::models::ProviderConfig, model_id: &str) -> bool {
    provider.enabled_model_ids.iter().any(|id| id == model_id)
}

/// 返回 v1 的上下文窗口选项，并保留当前值的精确 token 数。
pub fn context_options(value: u32) -> Vec<u32> {
    let mut options = vec![128_000, 256_000, 512_000, 1_000_000];
    if !options.contains(&value) {
        options.push(value);
    }
    options.sort_unstable();
    options
}

/// 按 v1 的标签格式展示上下文窗口大小。
pub fn format_context(value: u32) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f32 / 1_000_000.0)
    } else {
        format!("{}K", (value as f64 / 1_000.0).round() as u32)
    }
}
