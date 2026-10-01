//! 配置写入以最后成功版本为基底；菜单的即时选中反馈不进入后继写入快照。
use buddy_engine::models::AppConfig;

pub(super) struct ConfigSaveState {
    committed: AppConfig,
    sequence: u64,
    optimistic_selection: Option<(u64, String)>,
}

impl ConfigSaveState {
    pub(super) fn new(config: AppConfig) -> Self {
        Self {
            committed: config,
            sequence: 0,
            optimistic_selection: None,
        }
    }

    pub(super) fn reset(&mut self, config: AppConfig) {
        self.committed = config;
        self.optimistic_selection = None;
    }

    pub(super) fn begin(&mut self, selected: Option<String>) -> u64 {
        self.sequence += 1;
        if let Some(id) = selected {
            self.optimistic_selection = Some((self.sequence, id));
        }
        self.sequence
    }

    pub(super) fn config(&self) -> AppConfig {
        self.committed.clone()
    }

    /// 每个成功写入都更新基底；仅把更晚的菜单选中覆盖到可见配置上。
    pub(super) fn complete(&mut self, sequence: u64, config: AppConfig) -> AppConfig {
        self.committed = config.clone();
        let mut visible = config;
        if let Some((pending, id)) = &self.optimistic_selection {
            if *pending > sequence {
                visible.selected_model_id = id.clone();
            } else {
                self.optimistic_selection = None;
            }
        }
        visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::model_config::{ModelEdit, apply_model_edit};
    use buddy_engine::models::{ModelInfo, ProviderConfig};

    fn config() -> AppConfig {
        let mut config = AppConfig::default();
        config.providers.push(ProviderConfig {
            id: "p".into(),
            name: "测试".into(),
            base_url: "https://example.invalid".into(),
            api_key: "test".into(),
            enabled_model_ids: vec!["p::a".into(), "p::b".into()],
            provider_type: "openai_compatible".into(),
            compat: None,
        });
        config.models = ["p::a", "p::b"]
            .into_iter()
            .map(|id| ModelInfo {
                id: id.into(),
                provider_id: "p".into(),
                api_model_id: Some(id.into()),
                display_name: id.into(),
                context_window: 128_000,
                latency_ms: None,
                supports_vision: false,
                supports_image_generation: false,
            })
            .collect();
        config.selected_model_id = "p::a".into();
        config
    }

    #[test]
    fn later_menu_feedback_cannot_erase_committed_capability() {
        let mut state = ConfigSaveState::new(config());
        let edit = state.begin(None);
        let candidate =
            apply_model_edit(&state.config(), &ModelEdit::SetVision("p::a".into(), true));
        let select = state.begin(Some("p::b".into()));
        let visible = state.complete(edit, candidate);
        assert_eq!(visible.selected_model_id, "p::b");
        assert!(visible.models[0].supports_vision);
        assert_eq!(state.config().selected_model_id, "p::a");
        assert!(state.config().models[0].supports_vision);
        let candidate = apply_model_edit(&state.config(), &ModelEdit::SetDefault("p::b".into()));
        let visible = state.complete(select, candidate);
        assert_eq!(visible.selected_model_id, "p::b");
        assert!(state.config().models[0].supports_vision);
        assert!(state.optimistic_selection.is_none());
    }

    #[test]
    fn provider_addition_survives_later_model_selection() {
        let mut state = ConfigSaveState::new(config());
        let add = state.begin(None);
        let mut candidate = state.config();
        let mut extra = candidate.providers[0].clone();
        extra.id = "new".into();
        candidate.providers.push(extra);
        let select = state.begin(Some("p::b".into()));
        state.complete(add, candidate);
        let candidate = apply_model_edit(&state.config(), &ModelEdit::SetDefault("p::b".into()));
        state.complete(select, candidate);
        assert_eq!(state.config().providers.len(), 2);
        assert_eq!(state.config().selected_model_id, "p::b");
    }

    #[test]
    fn queued_selection_feedback_stays_at_latest_item() {
        let mut state = ConfigSaveState::new(config());
        let first = state.begin(Some("p::b".into()));
        let last = state.begin(Some("p::a".into()));
        let first_config = apply_model_edit(&state.config(), &ModelEdit::SetDefault("p::b".into()));
        assert_eq!(
            state.complete(first, first_config).selected_model_id,
            "p::a"
        );
        let last_config = apply_model_edit(&state.config(), &ModelEdit::SetDefault("p::a".into()));
        assert_eq!(state.complete(last, last_config).selected_model_id, "p::a");
        assert_eq!(state.config().selected_model_id, "p::a");
    }
}
