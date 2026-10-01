use super::super::provider_presets::PRESETS;
use super::*;

fn model(id: &str) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        provider_id: String::new(),
        api_model_id: Some(format!("raw-{id}")),
        display_name: id.to_string(),
        context_window: 128_000,
        latency_ms: None,
        supports_vision: false,
        supports_image_generation: true,
    }
}

#[test]
fn custom_protocol_survives_preset_round_trip() {
    let mut form = ProviderForm::new();
    form.select_preset("custom");
    form.set_protocol("anthropic".into());
    form.select_preset("openai");
    assert_eq!(form.protocol, "openai_compatible");
    form.select_preset("custom");
    assert_eq!(form.protocol, "anthropic");
}

#[test]
fn preset_and_connection_changes_invalidate_old_results() {
    let mut form = ProviderForm::new();
    form.select_preset("deepseek");
    form.set_key("key".to_string());
    let request = form.begin_fetch().unwrap();
    form.set_url("https://other.example".to_string());
    form.receive_fetch(request.revision, Ok(vec![model("one")]));
    assert!(form.models.is_empty());
    assert_eq!(form.busy, Busy::Idle);
    assert_eq!(form.url, "https://other.example");
}

#[test]
fn key_and_protocol_changes_invalidate_old_results() {
    let mut form = ProviderForm::new();
    form.select_preset("openai");
    form.set_key("old-key".to_string());
    let request = form.begin_fetch().unwrap();
    form.receive_fetch(request.revision, Ok(vec![model("one")]));
    assert_eq!(form.models.len(), 1);

    let old_revision = form.revision;
    form.set_key("new-key".to_string());
    assert!(form.models.is_empty());
    assert!(form.selected.is_empty());
    assert_eq!(form.busy, Busy::Idle);
    assert!(form.revision > old_revision);
    form.receive_fetch(old_revision, Ok(vec![model("stale-key")]));
    assert!(form.models.is_empty());

    let request = form.begin_fetch().unwrap();
    form.receive_fetch(request.revision, Ok(vec![model("two")]));
    let old_revision = form.revision;
    form.set_protocol("anthropic".to_string());
    assert!(form.models.is_empty());
    assert!(form.selected.is_empty());
    form.receive_fetch(old_revision, Ok(vec![model("stale-protocol")]));
    assert!(form.models.is_empty());
}

#[test]
fn empty_and_failed_fetches_preserve_last_successful_models() {
    let mut form = ProviderForm::new();
    form.select_preset("openai");
    form.set_key("key".to_string());
    let request = form.begin_fetch().unwrap();
    form.receive_fetch(request.revision, Ok(vec![model("one")]));
    let previous_models = form.models.clone();
    let previous_selected = form.selected.clone();
    let previous_url = form.url.clone();
    let previous_key = form.key.clone();

    let request = form.begin_fetch().unwrap();
    form.receive_fetch(request.revision, Ok(Vec::new()));
    assert_eq!(form.models.len(), previous_models.len());
    assert_eq!(form.models[0].id, previous_models[0].id);
    assert_eq!(form.models[0].latency_ms, previous_models[0].latency_ms);
    assert_eq!(form.selected, previous_selected);
    assert_eq!(form.url, previous_url);
    assert_eq!(form.key, previous_key);
    assert_eq!(form.error.as_deref(), Some(EMPTY_MODELS_ERROR));
    assert_eq!(form.busy, Busy::Idle);

    let request = form.begin_fetch().unwrap();
    form.receive_fetch(request.revision, Err("连接失败".to_string()));
    assert_eq!(form.models.len(), previous_models.len());
    assert_eq!(form.models[0].id, previous_models[0].id);
    assert_eq!(form.models[0].latency_ms, previous_models[0].latency_ms);
    assert_eq!(form.selected, previous_selected);
    assert_eq!(form.error.as_deref(), Some("连接失败"));
    assert_eq!(form.busy, Busy::Idle);
}

#[test]
fn latency_uses_raw_model_id_and_anthropic_disables_image_generation() {
    let mut form = ProviderForm::new();
    form.select_preset("anthropic");
    form.set_key("key".to_string());
    let request = form.begin_fetch().unwrap();
    form.receive_fetch(request.revision, Ok(vec![model("claude")]));
    assert!(!form.models[0].supports_image_generation);
    assert_eq!(form.models[0].provider_id, "anthropic");
    assert!(form.selected.contains("claude"));
    form.set_image_generation_support("claude", true);
    assert!(!form.models[0].supports_image_generation);
    let (request, raw) = form.begin_latency().unwrap();
    assert_eq!(raw, "raw-claude");
    form.receive_latency(request.revision, &raw, Ok(42));
    assert_eq!(form.models[0].latency_ms, Some(42));
}

#[test]
fn stale_latency_result_is_ignored_after_connection_change() {
    let mut form = ProviderForm::new();
    form.select_preset("openai");
    form.set_key("old-key".to_string());
    let request = form.begin_fetch().unwrap();
    form.receive_fetch(request.revision, Ok(vec![model("one")]));
    let (request, raw_id) = form.begin_latency().unwrap();
    form.set_key("new-key".to_string());
    assert!(form.models.is_empty());
    let fresh = form.begin_fetch().unwrap();
    form.receive_fetch(fresh.revision, Ok(vec![model("one")]));
    form.receive_latency(request.revision, &raw_id, Ok(99));
    assert_eq!(form.models[0].latency_ms, None);
    assert_eq!(form.busy, Busy::Idle);
    assert!(form.error.is_none());
}

#[test]
fn submission_requires_a_selected_model() {
    let mut form = ProviderForm::new();
    form.select_preset("openai");
    form.set_key("key".to_string());
    let request = form.begin_fetch().unwrap();
    form.receive_fetch(request.revision, Ok(vec![model("one")]));
    form.toggle_model("one");
    assert!(form.selected.is_empty());
    assert!(form.submission("ignored".to_string()).is_none());
    assert_eq!(form.busy, Busy::Idle);
}

#[test]
fn custom_compat_is_none_until_edited_and_save_failure_restores_idle() {
    let mut form = ProviderForm::new();
    form.select_preset("custom");
    form.set_url("https://example".to_string());
    form.set_key("key".to_string());
    let request = form.begin_fetch().unwrap();
    form.receive_fetch(request.revision, Ok(vec![model("one")]));
    let submission = form.submission("custom-1".to_string()).unwrap();
    assert!(submission.provider.compat.is_none());
    assert_eq!(form.busy, Busy::Save);

    let saved_url = form.url.clone();
    let saved_key = form.key.clone();
    form.receive_save(Err("保存失败".to_string()));
    assert_eq!(form.busy, Busy::Idle);
    assert_eq!(form.url, saved_url);
    assert_eq!(form.key, saved_key);
    assert_eq!(form.error.as_deref(), Some("保存失败"));

    form.set_thinking_format("deepseek".to_string());
    let submission = form.submission("custom-2".to_string()).unwrap();
    assert_eq!(
        submission
            .provider
            .compat
            .unwrap()
            .thinking_format
            .as_deref(),
        Some("deepseek")
    );
}

#[test]
fn built_in_submission_compat_matches_every_preset() {
    for preset in PRESETS {
        let mut form = ProviderForm::new();
        form.select_preset(preset.id);
        form.set_key("key".to_string());
        let request = form.begin_fetch().unwrap();
        form.receive_fetch(request.revision, Ok(vec![model("one")]));
        let submission = form.submission("ignored-custom-id".to_string()).unwrap();
        let compat = submission.provider.compat.expect("built-in compat");
        assert_eq!(submission.provider.id, preset.id);
        assert_eq!(submission.provider.name, preset.name);
        assert_eq!(submission.provider.provider_type, preset.protocol);
        assert_eq!(
            compat.thinking_format.as_deref(),
            (!preset.thinking_format.is_empty()).then_some(preset.thinking_format)
        );
        assert_eq!(
            compat.max_tokens_field.as_deref(),
            (!preset.max_tokens_field.is_empty()).then_some(preset.max_tokens_field)
        );
        form.receive_save(Ok(()));
    }
}
