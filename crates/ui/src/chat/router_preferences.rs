//! 外观与热键按字段增量复用配置队列，保存成功后才发布（与当前 v1 一致）。
use super::*;
use buddy_engine::models::Theme;

#[derive(Clone)]
pub(super) enum Preference {
    Hotkey(String),
    Theme(Theme),
}

impl Preference {
    fn is_hotkey(&self) -> bool {
        matches!(self, Self::Hotkey(_))
    }

    fn apply(&self, config: &AppConfig) -> AppConfig {
        let mut next = config.clone();
        match self {
            Self::Hotkey(value) => next.hotkey = value.clone(),
            Self::Theme(value) => next.theme = value.clone(),
        }
        next
    }
}

impl PageRouter {
    pub(super) fn save_preference(&mut self, edit: Preference, cx: &mut Context<Self>) {
        let settings = self.settings.clone();
        settings.update(cx, |view, cx| {
            view.set_preference_saving(edit.is_hotkey(), true, cx)
        });
        let previous = self.config_save.take();
        let state = self.config_save_state.clone();
        let sequence = state.borrow_mut().begin(None);
        let engine = self.engine.clone();
        let (completed, completion) = tokio::sync::oneshot::channel();
        cx.spawn(async move |router, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            // 在前项完成后才取基底，避免主题、热键与模型保存互相覆盖。
            let candidate = edit.apply(&state.borrow().config());
            let to_save = candidate.clone();
            let work =
                cx.update(|cx| spawn_engine(cx, async move { engine.save_config(to_save).await }));
            match work.await {
                Ok(()) => {
                    let visible = state.borrow_mut().complete(sequence, candidate);
                    let _ = router.update(cx, |router, cx| router.publish_config(visible, cx));
                    let _ = settings.update(cx, |view, cx| {
                        view.set_preference_saving(edit.is_hotkey(), false, cx)
                    });
                }
                Err(error) => {
                    let _ = settings.update(cx, |view, cx| {
                        view.preference_failed(edit.is_hotkey(), error, cx)
                    });
                }
            }
            let _ = completed.send(());
        })
        .detach();
        self.config_save = Some(cx.background_spawn(async move {
            let _ = completion.await;
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_preserve_other_fields_and_each_other() {
        let mut config = AppConfig::default();
        config.allowed_paths = vec!["/tmp/test".into()];
        config.selected_model_id = "test::model".into();
        config.auto_start = true;
        let dark = Preference::Theme(Theme::Dark).apply(&config);
        let hotkey = Preference::Hotkey("CmdOrCtrl+Shift+K".into()).apply(&dark);
        let mut expected = config;
        expected.theme = Theme::Dark;
        expected.hotkey = "CmdOrCtrl+Shift+K".into();
        assert_eq!(
            serde_json::to_value(hotkey).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }
}
