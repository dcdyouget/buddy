use super::model_config::{apply_model_edit, ModelEdit};
use super::provider_form::ProviderSubmission;
use super::provider_merge::merge_provider;
use buddy_engine::models::mcp::McpServerConfig;
use buddy_engine::models::{AppConfig, ModelInfo, ProviderConfig, Theme};
use serde_json::{json, Value};

fn mcp_fixture_value() -> Value {
    json!([
        {
            "id": "stdio-files",
            "name": "本地文件工具",
            "enabled": false,
            "transport": "stdio",
            "command": "/usr/local/bin/mcp-files",
            "args": ["--root", "/tmp/workspace", "--readonly"],
            "env": {
                "MCP_ROOT": "/tmp/workspace",
                "MCP_TOKEN": "stdio-token"
            },
            "url": null,
            "headers": {},
            "timeout_secs": 7,
            "auto_reconnect": false
        },
        {
            "id": "sse-search",
            "name": "远程搜索工具",
            "enabled": true,
            "transport": "sse",
            "command": null,
            "args": [],
            "env": {},
            "url": "https://mcp.example.invalid/events",
            "headers": {
                "Authorization": "Bearer sse-token",
                "X-Client": "buddy-test"
            },
            "timeout_secs": 1500,
            "auto_reconnect": true
        }
    ])
}

fn mcp_fixture() -> Vec<McpServerConfig> {
    serde_json::from_value(mcp_fixture_value()).expect("populated MCP fixture must decode")
}

fn model(provider_id: &str, id: &str) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        provider_id: provider_id.to_string(),
        api_model_id: Some(id.to_string()),
        display_name: id.to_string(),
        context_window: 128_000,
        latency_ms: Some(321),
        supports_vision: false,
        supports_image_generation: false,
    }
}

fn provider(id: &str, enabled_model_ids: &[&str]) -> ProviderConfig {
    ProviderConfig {
        id: id.to_string(),
        name: format!("服务 {id}"),
        base_url: format!("https://{id}.example"),
        api_key: format!("{id}-secret"),
        enabled_model_ids: enabled_model_ids
            .iter()
            .map(|id| (*id).to_string())
            .collect(),
        provider_type: "openai_compatible".to_string(),
        compat: None,
    }
}

fn config_with_mcp() -> AppConfig {
    AppConfig {
        font_size: 14,
        theme: Theme::Dark,
        hotkey: "CmdOrCtrl+J".to_string(),
        providers: vec![provider("openai", &["openai::alpha"])],
        models: vec![model("openai", "openai::alpha")],
        selected_model_id: "openai::alpha".to_string(),
        auto_start: true,
        allowed_paths: vec!["/tmp/workspace".to_string()],
        mcp_servers: mcp_fixture(),
    }
}

#[test]
fn populated_stdio_and_sse_fixture_round_trips_semantically() {
    let decoded: Vec<McpServerConfig> = serde_json::from_value(mcp_fixture_value()).unwrap();

    assert_eq!(serde_json::to_value(decoded).unwrap(), mcp_fixture_value());
}

#[test]
fn model_edits_preserve_every_populated_mcp_field() {
    let config = config_with_mcp();
    let expected = serde_json::to_value(&config.mcp_servers).unwrap();
    let edits = [
        ModelEdit::SetDefault("openai::alpha".to_string()),
        ModelEdit::ToggleEnabled("openai::alpha".to_string()),
        ModelEdit::SetContext("openai::alpha".to_string(), 256_000),
        ModelEdit::SetVision("openai::alpha".to_string(), true),
        ModelEdit::SetImageGeneration("openai::alpha".to_string(), true),
    ];

    for edit in edits {
        let next = apply_model_edit(&config, &edit);
        assert_eq!(serde_json::to_value(&next.mcp_servers).unwrap(), expected);
    }
}

#[test]
fn merging_provider_preserves_populated_mcp_and_unrelated_root_settings() {
    let config = config_with_mcp();
    let expected_mcp = serde_json::to_value(&config.mcp_servers).unwrap();
    let submission = ProviderSubmission {
        provider: provider("anthropic", &[]),
        models: vec![model("ignored-provider", "claude-3-7-sonnet")],
    };

    let next = merge_provider(&config, &submission);

    assert_eq!(
        serde_json::to_value(&next.mcp_servers).unwrap(),
        expected_mcp
    );
    assert!(next.auto_start);
    assert_eq!(next.allowed_paths, config.allowed_paths);
    assert!(matches!(next.theme, Theme::Dark));
    assert_eq!(next.hotkey, config.hotkey);
}

#[test]
fn malformed_mcp_fixture_is_rejected_without_relaxing_required_fields() {
    assert!(serde_json::from_value::<Vec<McpServerConfig>>(json!([
        {
            "id": "missing-transport",
            "name": "坏配置"
        }
    ]))
    .is_err());
    assert!(serde_json::from_value::<Vec<McpServerConfig>>(json!([
        {
            "id": "unknown-transport",
            "name": "坏配置",
            "transport": "websocket"
        }
    ]))
    .is_err());
}
