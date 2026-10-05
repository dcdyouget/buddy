//! Provider 提交数据与现有配置的纯合并逻辑。

use buddy_engine::models::{
    normalize_model_ids, raw_model_id, scoped_model_id, AppConfig,
};
use std::collections::HashSet;

use super::provider_form::ProviderSubmission;

/// 合并一次 Provider 提交。
///
/// 同 ID Provider 被新配置覆盖；同 Provider 的旧模型保留，新的模型按作用域 ID
/// 去重追加。模型的显示名、能力和上下文等旧字段不会因重复 ID 被覆盖。
pub fn merge_provider(config: &AppConfig, submission: &ProviderSubmission) -> AppConfig {
    let mut next = config.clone();
    normalize_model_ids(&mut next);

    let provider_id = submission.provider.id.clone();
    let mut provider = submission.provider.clone();
    let mut normalized_models = Vec::with_capacity(submission.models.len());
    let mut submitted_ids = HashSet::new();

    for model in &submission.models {
        let raw_id = raw_model_id(model).to_string();
        let scoped_id = scoped_model_id(&provider_id, &raw_id);
        if submitted_ids.insert(scoped_id.clone()) {
            let mut normalized = model.clone();
            normalized.provider_id = provider_id.clone();
            normalized.id = scoped_id.clone();
            normalized.api_model_id = Some(raw_id);
            normalized_models.push(normalized);
        }
    }

    provider.enabled_model_ids = normalized_models
        .iter()
        .map(|model| model.id.clone())
        .collect();
    next.providers.retain(|item| item.id != provider_id);
    next.providers.push(provider);

    let existing_ids: HashSet<String> = next.models.iter().map(|model| model.id.clone()).collect();
    for model in normalized_models {
        if !existing_ids.contains(&model.id) {
            next.models.push(model);
        }
    }

    if let Some(first) = submission.models.first() {
        let raw_id = raw_model_id(first);
        next.selected_model_id = scoped_model_id(&provider_id, raw_id);
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use buddy_engine::models::{CompatConfig, ModelInfo, ProviderConfig, Theme};

    fn model(provider_id: &str, id: &str, raw: Option<&str>) -> ModelInfo {
        ModelInfo {
            id: id.to_string(),
            provider_id: provider_id.to_string(),
            api_model_id: raw.map(str::to_string),
            display_name: format!("display-{id}"),
            context_window: 128_000,
            latency_ms: None,
            supports_vision: false,
            supports_image_generation: false,
        }
    }

    fn config() -> AppConfig {
        AppConfig {
            font_size: 14,
            theme: Theme::Light,
            hotkey: "CmdOrCtrl+J".to_string(),
            providers: vec![ProviderConfig {
                id: "old".to_string(),
                name: "旧服务".to_string(),
                base_url: "https://old".to_string(),
                api_key: "old-key".to_string(),
                enabled_model_ids: vec!["old-model".to_string()],
                provider_type: "openai_compatible".to_string(),
                compat: None,
            }],
            models: vec![model("old", "old-model", None)],
            selected_model_id: "old-model".to_string(),
            auto_start: true,
            allowed_paths: vec!["/tmp".to_string()],
            mcp_servers: Vec::new(),
        }
    }

    fn submission(models: Vec<ModelInfo>) -> ProviderSubmission {
        ProviderSubmission {
            provider: ProviderConfig {
                id: "new".to_string(),
                name: "新服务".to_string(),
                base_url: "https://new".to_string(),
                api_key: "new-key".to_string(),
                enabled_model_ids: vec!["same".to_string(), "raw::id".to_string()],
                provider_type: "openai_compatible".to_string(),
                compat: Some(CompatConfig::default()),
            },
            models,
        }
    }

    #[test]
    fn merge_preserves_existing_config_and_scopes_raw_ids_without_splitting() {
        let result = merge_provider(
            &config(),
            &submission(vec![
                model("ignored", "same", Some("same")),
                model("ignored", "raw::id", Some("raw::id")),
            ]),
        );
        assert!(result.auto_start);
        assert_eq!(result.allowed_paths, vec!["/tmp".to_string()]);
        assert_eq!(result.models.len(), 3);
        assert!(result.models.iter().any(|model| model.id == "new::raw::id"));
        assert_eq!(result.selected_model_id, "new::same");
        assert_eq!(
            result.providers[1].enabled_model_ids,
            vec!["new::same".to_string(), "new::raw::id".to_string()]
        );
    }

    #[test]
    fn duplicate_submission_models_are_deduplicated_and_old_provider_models_remain() {
        let mut current = config();
        current.providers.push(ProviderConfig {
            id: "new".to_string(),
            name: "旧新服务".to_string(),
            base_url: "https://old-new".to_string(),
            api_key: "old-key".to_string(),
            enabled_model_ids: vec!["new::kept".to_string()],
            provider_type: "openai_compatible".to_string(),
            compat: None,
        });
        current.models.push(model("new", "new::kept", Some("kept")));
        let result = merge_provider(
            &current,
            &submission(vec![
                model("ignored", "kept", Some("kept")),
                model("ignored", "kept", Some("kept")),
            ]),
        );
        assert_eq!(
            result
                .models
                .iter()
                .filter(|model| model.id == "new::kept")
                .count(),
            1
        );
        assert_eq!(result.models.len(), 2);
        assert_eq!(
            result
                .providers
                .iter()
                .filter(|provider| provider.id == "new")
                .count(),
            1
        );
    }
}
