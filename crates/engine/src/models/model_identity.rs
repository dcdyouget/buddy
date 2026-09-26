//! 配置内模型 ID 的唯一性与兼容迁移。
//!
//! 同一模型名称可以由不同 Provider 提供。配置和 UI 因此使用
//! `<provider_id>::<raw_model_id>` 作为模型 ID；`api_model_id` 保存真正发往 API 的原始值。

use super::{AppConfig, ModelInfo};

pub const MODEL_ID_SEPARATOR: &str = "::";

/// 返回存储在配置和前端状态中的模型 ID。
///
/// 此函数只接收 API 原始模型 ID；归一化的幂等性由 `api_model_id` 保证，
/// 不通过字符串前缀猜测，避免原始 ID 本身含有 `::` 时误判。
pub fn scoped_model_id(provider_id: &str, raw_model_id: &str) -> String {
    format!("{provider_id}{MODEL_ID_SEPARATOR}{raw_model_id}")
}

/// 获取发送给 Provider API 的原始模型 ID。
///
/// 旧配置中的裸模型 ID 原样返回，保证迁移前的配置仍可调用。
pub fn raw_model_id(model: &ModelInfo) -> &str {
    model.api_model_id.as_deref().unwrap_or(&model.id)
}

/// 将旧配置的裸模型 ID 迁移为 Provider 作用域 ID。
///
/// 同时迁移各 Provider 的启用列表与当前选中模型。函数不丢弃任何模型、能力或
/// Provider 配置，并且可在每次读写配置时安全调用。
pub fn normalize_model_ids(config: &mut AppConfig) {
    let previous_selected_id = config.selected_model_id.clone();
    let previous_models: Vec<(String, String, String)> = config
        .models
        .iter()
        .map(|model| {
            (
                model.provider_id.clone(),
                model.id.clone(),
                raw_model_id(model).to_string(),
            )
        })
        .collect();

    for model in &mut config.models {
        let api_model_id = model
            .api_model_id
            .clone()
            .unwrap_or_else(|| model.id.clone());
        model.id = scoped_model_id(&model.provider_id, &api_model_id);
        model.api_model_id = Some(api_model_id);
    }

    for provider in &mut config.providers {
        provider.enabled_model_ids = provider
            .enabled_model_ids
            .iter()
            .map(|model_id| {
                previous_models
                    .iter()
                    .find(|(provider_id, previous_id, raw_id)| {
                        provider_id == &provider.id
                            && (previous_id == model_id || raw_id == model_id)
                    })
                    .map(|(_, _, raw_id)| scoped_model_id(&provider.id, raw_id))
                    .unwrap_or_else(|| scoped_model_id(&provider.id, model_id))
            })
            .collect();
    }

    if previous_selected_id.is_empty() {
        return;
    }

    // 优先使用迁移前已存在的模型记录。这样同名模型来自多个 Provider 时，
    // 保留旧版本按 models 顺序解析 selected_model_id 的行为。
    if let Some((provider_id, _, raw_id)) = previous_models.iter().find(|(_, model_id, raw_id)| {
        model_id == &previous_selected_id || raw_id == &previous_selected_id
    }) {
        config.selected_model_id = scoped_model_id(provider_id, raw_id);
        return;
    }

    // selected_model_id 已是新格式时，确认它仍指向实际模型；无效值交给命令层校验。
    if config
        .models
        .iter()
        .any(|model| model.id == previous_selected_id)
    {
        config.selected_model_id = previous_selected_id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ProviderConfig, Theme};

    fn model(provider_id: &str, id: &str) -> ModelInfo {
        ModelInfo {
            id: id.to_string(),
            provider_id: provider_id.to_string(),
            api_model_id: None,
            display_name: id.to_string(),
            context_window: 128_000,
            latency_ms: None,
            supports_vision: false,
            supports_image_generation: false,
        }
    }

    fn provider(id: &str, enabled_model_ids: Vec<&str>) -> ProviderConfig {
        ProviderConfig {
            id: id.to_string(),
            name: id.to_string(),
            base_url: "https://example.com".to_string(),
            api_key: "key".to_string(),
            enabled_model_ids: enabled_model_ids.into_iter().map(str::to_string).collect(),
            provider_type: "openai_compatible".to_string(),
            compat: None,
        }
    }

    #[test]
    fn scopes_same_raw_model_for_each_provider_and_keeps_settings() {
        let mut config = AppConfig {
            theme: Theme::Light,
            hotkey: "CmdOrCtrl+J".to_string(),
            providers: vec![
                provider("openai", vec!["gpt-4o"]),
                provider("relay", vec!["gpt-4o"]),
            ],
            models: vec![model("openai", "gpt-4o"), model("relay", "gpt-4o")],
            selected_model_id: "gpt-4o".to_string(),
            auto_start: false,
            allowed_paths: vec![],
            mcp_servers: vec![],
        };

        normalize_model_ids(&mut config);

        assert_eq!(config.models[0].id, "openai::gpt-4o");
        assert_eq!(config.models[1].id, "relay::gpt-4o");
        assert_eq!(config.providers[0].enabled_model_ids, ["openai::gpt-4o"]);
        assert_eq!(config.providers[1].enabled_model_ids, ["relay::gpt-4o"]);
        assert_eq!(config.selected_model_id, "openai::gpt-4o");
        assert_eq!(raw_model_id(&config.models[1]), "gpt-4o");
        assert_eq!(config.models[0].api_model_id.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn normalization_is_idempotent() {
        let mut config = AppConfig {
            theme: Theme::Light,
            hotkey: "CmdOrCtrl+J".to_string(),
            providers: vec![provider("openai", vec!["openai::gpt-4o"])],
            models: vec![model("openai", "openai::gpt-4o")],
            selected_model_id: "openai::gpt-4o".to_string(),
            auto_start: false,
            allowed_paths: vec![],
            mcp_servers: vec![],
        };

        normalize_model_ids(&mut config);
        let once = serde_json::to_value(&config).unwrap();
        normalize_model_ids(&mut config);

        assert_eq!(serde_json::to_value(&config).unwrap(), once);
    }

    #[test]
    fn preserves_raw_model_ids_that_look_like_scoped_ids() {
        let raw_id = "openai::special";
        let mut config = AppConfig {
            theme: Theme::Light,
            hotkey: "CmdOrCtrl+J".to_string(),
            providers: vec![provider("openai", vec![raw_id])],
            models: vec![model("openai", raw_id)],
            selected_model_id: raw_id.to_string(),
            auto_start: false,
            allowed_paths: vec![],
            mcp_servers: vec![],
        };

        normalize_model_ids(&mut config);

        assert_eq!(config.models[0].id, "openai::openai::special");
        assert_eq!(raw_model_id(&config.models[0]), raw_id);
        assert_eq!(
            config.providers[0].enabled_model_ids,
            ["openai::openai::special"]
        );
        assert_eq!(config.selected_model_id, "openai::openai::special");
        let once = serde_json::to_value(&config).unwrap();
        normalize_model_ids(&mut config);
        assert_eq!(serde_json::to_value(&config).unwrap(), once);
    }
}
