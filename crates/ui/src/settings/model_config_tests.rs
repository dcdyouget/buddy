use super::model_config::{
    apply_model_edit, context_options, format_context, model_enabled, ModelEdit,
};
use buddy_engine::models::{AppConfig, ModelInfo, ProviderConfig, Theme};

fn model(provider_id: &str, id: &str, context_window: u32) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        provider_id: provider_id.to_string(),
        api_model_id: Some(id.to_string()),
        display_name: id.to_string(),
        context_window,
        latency_ms: Some(321),
        supports_vision: false,
        supports_image_generation: false,
    }
}

fn provider(id: &str, provider_type: &str, enabled_model_ids: &[&str]) -> ProviderConfig {
    ProviderConfig {
        id: id.to_string(),
        name: id.to_string(),
        base_url: format!("https://{id}.example"),
        api_key: format!("{id}-secret"),
        enabled_model_ids: enabled_model_ids.iter().map(|id| (*id).to_string()).collect(),
        provider_type: provider_type.to_string(),
        compat: None,
    }
}

fn config() -> AppConfig {
    AppConfig {
        theme: Theme::Dark,
        hotkey: "CmdOrCtrl+J".to_string(),
        providers: vec![
            provider("openai", "openai_compatible", &["openai::alpha"]),
            provider("anthropic", "anthropic", &["anthropic::alpha"]),
        ],
        models: vec![
            model("openai", "openai::alpha", 128_000),
            model("openai", "openai::raw::id", 128_000),
            model("anthropic", "anthropic::alpha", 128_000),
        ],
        selected_model_id: "openai::alpha".to_string(),
        auto_start: true,
        allowed_paths: vec!["/tmp".to_string()],
        mcp_servers: Vec::new(),
    }
}

#[test]
fn enabled_lookup_uses_provider_id_and_full_model_id() {
    let config = config();
    assert!(model_enabled(&config, &config.models[0]));
    assert!(!model_enabled(&config, &config.models[1]));
    assert!(model_enabled(&config, &config.models[2]));

    let mut copied = config.models[0].clone();
    copied.provider_id = "anthropic".to_string();
    assert!(!model_enabled(&config, &copied));
}

#[test]
fn same_raw_id_remains_scoped_to_each_provider() {
    let config = config();
    let next = apply_model_edit(
        &config,
        &ModelEdit::ToggleEnabled("openai::raw::id".to_string()),
    );
    assert!(model_enabled(&next, &next.models[1]));
    assert!(model_enabled(&next, &next.models[2]));
    assert_eq!(
        next.providers[1].enabled_model_ids,
        vec!["anthropic::alpha".to_string()]
    );
    assert_eq!(next.selected_model_id, "openai::alpha");
}

#[test]
fn provider_ids_with_scope_separator_are_not_split() {
    let mut config = config();
    let provider_id = "relay::openai";
    let model_id = "relay::openai::alpha";
    config
        .providers
        .push(provider(provider_id, "openai_compatible", &[model_id]));
    config.models.push(model(provider_id, model_id, 128_000));

    assert!(model_enabled(&config, &config.models[3]));
    let next = apply_model_edit(&config, &ModelEdit::SetVision(model_id.to_string(), true));
    assert!(next.models[3].supports_vision);
}

#[test]
fn disabling_non_default_model_keeps_current_default() {
    let mut config = config();
    config.providers[0]
        .enabled_model_ids
        .push("openai::raw::id".to_string());
    let next = apply_model_edit(
        &config,
        &ModelEdit::ToggleEnabled("openai::raw::id".to_string()),
    );
    assert_eq!(next.selected_model_id, "openai::alpha");
    assert!(!model_enabled(&next, &next.models[1]));
}

#[test]
fn toggle_falls_back_to_first_enabled_and_can_clear_and_reenable() {
    let config = config();
    let disabled_first = apply_model_edit(
        &config,
        &ModelEdit::ToggleEnabled("openai::alpha".to_string()),
    );
    assert_eq!(disabled_first.selected_model_id, "anthropic::alpha");

    let all_disabled = apply_model_edit(
        &disabled_first,
        &ModelEdit::ToggleEnabled("anthropic::alpha".to_string()),
    );
    assert_eq!(all_disabled.selected_model_id, "");

    let reenabled = apply_model_edit(
        &all_disabled,
        &ModelEdit::ToggleEnabled("openai::alpha".to_string()),
    );
    assert_eq!(reenabled.selected_model_id, "openai::alpha");
}

#[test]
fn edits_preserve_unrelated_configuration_and_model_metadata() {
    let config = config();
    let next = apply_model_edit(
        &config,
        &ModelEdit::SetContext("openai::alpha".to_string(), 32_768),
    );
    assert_eq!(next.models[0].context_window, 32_768);
    assert_eq!(next.models[0].latency_ms, Some(321));
    assert_eq!(next.models[0].api_model_id.as_deref(), Some("openai::alpha"));
    assert!(matches!(next.theme, Theme::Dark));
    assert!(next.auto_start);
    assert_eq!(next.allowed_paths, vec!["/tmp".to_string()]);
    assert_eq!(next.providers[0].api_key, "openai-secret");
    assert_eq!(config.models[0].context_window, 128_000);
}

#[test]
fn context_options_keep_exact_values_and_format_like_v1() {
    assert_eq!(
        context_options(32_768),
        vec![32_768, 128_000, 256_000, 512_000, 1_000_000]
    );
    assert_eq!(
        context_options(128_000),
        vec![128_000, 256_000, 512_000, 1_000_000]
    );
    assert_eq!(format_context(32_768), "33K");
    assert_eq!(format_context(500), "1K");
    assert_eq!(format_context(1_500), "2K");
    assert_eq!(format_context(1_000_000), "1.0M");
}

#[test]
fn capability_edits_follow_provider_protocol() {
    let config = config();
    let next = apply_model_edit(
        &config,
        &ModelEdit::SetImageGeneration("openai::alpha".to_string(), true),
    );
    assert!(next.models[0].supports_image_generation);

    let anthropic = apply_model_edit(
        &next,
        &ModelEdit::SetImageGeneration("anthropic::alpha".to_string(), true),
    );
    assert!(!anthropic.models[2].supports_image_generation);
    let disabled = apply_model_edit(
        &anthropic,
        &ModelEdit::SetImageGeneration("anthropic::alpha".to_string(), false),
    );
    assert!(!disabled.models[2].supports_image_generation);

    let vision = apply_model_edit(
        &disabled,
        &ModelEdit::SetVision("anthropic::alpha".to_string(), true),
    );
    assert!(vision.models[2].supports_vision);
}

#[test]
fn unknown_model_edits_are_lossless_no_ops() {
    let config = config();
    let edits = [
        ModelEdit::SetDefault("missing::model".to_string()),
        ModelEdit::ToggleEnabled("missing::model".to_string()),
        ModelEdit::SetContext("missing::model".to_string(), 500),
        ModelEdit::SetVision("missing::model".to_string(), true),
        ModelEdit::SetImageGeneration("missing::model".to_string(), true),
    ];
    for edit in edits {
        let next = apply_model_edit(&config, &edit);
        assert_eq!(
            serde_json::to_value(&next).unwrap(),
            serde_json::to_value(&config).unwrap()
        );
        assert_eq!(next.selected_model_id, config.selected_model_id);
        assert_eq!(next.models.len(), config.models.len());
        assert_eq!(next.providers.len(), config.providers.len());
        assert_eq!(next.models[0].context_window, config.models[0].context_window);
    }

    let mut unknown_provider = config.clone();
    unknown_provider
        .models
        .push(model("missing::provider", "missing::provider::model", 128_000));
    let next = apply_model_edit(
        &unknown_provider,
        &ModelEdit::SetContext("missing::provider::model".to_string(), 500),
    );
    assert_eq!(
        serde_json::to_value(&next).unwrap(),
        serde_json::to_value(&unknown_provider).unwrap()
    );
}

#[test]
fn default_requires_existing_model_but_need_not_be_enabled() {
    let config = config();
    let changed = apply_model_edit(
        &config,
        &ModelEdit::SetDefault("openai::raw::id".to_string()),
    );
    assert_eq!(changed.selected_model_id, "openai::raw::id");

    let enabled = apply_model_edit(
        &config,
        &ModelEdit::ToggleEnabled("openai::raw::id".to_string()),
    );
    let changed = apply_model_edit(
        &enabled,
        &ModelEdit::SetDefault("openai::raw::id".to_string()),
    );
    assert_eq!(changed.selected_model_id, "openai::raw::id");
}
