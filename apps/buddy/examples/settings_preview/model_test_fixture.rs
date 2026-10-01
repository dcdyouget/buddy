//! S06-03 模型列表自测夹具。
//!
//! The fixture deliberately gives both Providers a `::` in their identifier and
//! gives them the same raw `alpha` model.  The test can therefore catch any UI
//! code that derives ownership by splitting a scoped model id.

use super::*;
use buddy_engine::models::{AppConfig, ModelInfo, ProviderConfig, Theme};
use std::path::PathBuf;

pub(crate) const OPENAI_PROVIDER: &str = "openai::sandbox";
pub(crate) const ANTHROPIC_PROVIDER: &str = "anthropic::sandbox";
pub(crate) const OPENAI_ALPHA: &str = "openai::sandbox::alpha";
pub(crate) const OPENAI_BETA: &str = "openai::sandbox::beta";
pub(crate) const ANTHROPIC_ALPHA: &str = "anthropic::sandbox::alpha";

pub(crate) fn model(
    id: &str,
    provider_id: &str,
    raw_id: &str,
    context_window: u32,
    latency_ms: u32,
) -> ModelInfo {
    ModelInfo {
        id: id.to_owned(),
        provider_id: provider_id.to_owned(),
        api_model_id: Some(raw_id.to_owned()),
        display_name: raw_id.to_owned(),
        context_window,
        latency_ms: Some(latency_ms),
        supports_vision: false,
        supports_image_generation: false,
    }
}

pub(crate) fn config() -> AppConfig {
    AppConfig {
        theme: Theme::Light,
        hotkey: "CmdOrCtrl+J".to_owned(),
        providers: vec![
            ProviderConfig {
                id: OPENAI_PROVIDER.to_owned(),
                name: "OpenAI 沙盒".to_owned(),
                base_url: "http://127.0.0.1/models".to_owned(),
                api_key: "openai-sandbox-key".to_owned(),
                enabled_model_ids: vec![OPENAI_ALPHA.to_owned(), OPENAI_BETA.to_owned()],
                provider_type: "openai_compatible".to_owned(),
                compat: None,
            },
            ProviderConfig {
                id: ANTHROPIC_PROVIDER.to_owned(),
                name: "Anthropic 沙盒".to_owned(),
                base_url: "http://127.0.0.1/anthropic".to_owned(),
                api_key: "anthropic-sandbox-key".to_owned(),
                enabled_model_ids: vec![ANTHROPIC_ALPHA.to_owned()],
                provider_type: "anthropic".to_owned(),
                compat: None,
            },
        ],
        models: vec![
            model(OPENAI_ALPHA, OPENAI_PROVIDER, "alpha", 128_000, 499),
            // This model starts at the non-preset 32768 value so the test can
            // verify the current-value option disappears after choosing a preset.
            model(OPENAI_BETA, OPENAI_PROVIDER, "beta", 32_768, 500),
            model(
                &format!("{OPENAI_PROVIDER}::latency-1500"),
                OPENAI_PROVIDER,
                "latency-1500",
                128_000,
                1500,
            ),
            model(ANTHROPIC_ALPHA, ANTHROPIC_PROVIDER, "alpha", 128_000, 499),
        ],
        selected_model_id: OPENAI_ALPHA.to_owned(),
        auto_start: false,
        allowed_paths: vec!["/tmp/buddy-s06-03".to_owned()],
        mcp_servers: Vec::new(),
    }
}

/// A process-local engine directory.  It is intentionally independent from
/// `fixture::sandbox()` so model tests can inspect the exact JSON they wrote.
pub(crate) fn sandbox() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/buddy-settings-model-preview")
        .join(std::process::id().to_string());
    let _ = std::fs::create_dir_all(&path);
    path
}

pub(crate) fn options(bounds: Bounds<buddy_ui::gpui::Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    }
}
