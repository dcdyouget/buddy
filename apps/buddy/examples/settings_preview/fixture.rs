use super::*;

pub(crate) fn message(id: &str, role: MessageRole, content: &str) -> Message {
    Message {
        id: id.into(),
        role,
        content: content.into(),
        images: Vec::new(),
        blocks: None,
        model_id: Some("p1::settings".into()),
        created_at: 0,
        tool_calls: None,
        tool_call_id: None,
        tool_name: None,
        is_error: None,
        parent_message_id: None,
    }
}

pub(crate) fn config() -> AppConfig {
    let models = (0..120)
        .map(|index| ModelInfo {
            id: format!("p1::settings-{index}"),
            provider_id: "p1".into(),
            api_model_id: Some(format!("settings-{index}")),
            display_name: format!("设置预览模型 {index:02}"),
            context_window: 128_000,
            latency_ms: Some(120),
            supports_vision: false,
            supports_image_generation: false,
        })
        .collect::<Vec<_>>();
    AppConfig {
        theme: Theme::Light,
        hotkey: "CmdOrCtrl+J".into(),
        providers: vec![ProviderConfig {
            id: "p1".into(),
            name: "设置预览服务".into(),
            base_url: "https://example.invalid".into(),
            api_key: "sandbox-key".into(),
            enabled_model_ids: vec!["p1::settings-0".into()],
            provider_type: "openai_compatible".into(),
            compat: None,
        }],
        models,
        selected_model_id: "p1::settings-0".into(),
        auto_start: false,
        allowed_paths: Vec::new(),
        mcp_servers: Vec::new(),
    }
}

pub(crate) fn sandbox() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/buddy-settings-preview")
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

pub(crate) fn open_router(cx: &mut App) -> WindowHandle<PageRouter> {
    let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
    let engine = ChatEngine::new(sandbox());
    let history = (0..30)
        .map(|index| {
            let role = if index % 2 == 0 {
                MessageRole::User
            } else {
                MessageRole::Assistant
            };
            message(
                &format!("history-{index}"),
                role,
                &format!("设置页下方的长会话样本 {index:02}"),
            )
        })
        .collect();
    let mut config = config();
    config.theme = match cx.buddy_theme().appearance {
        Appearance::Light => Theme::Light,
        Appearance::Dark => Theme::Dark,
    };
    let loaded = Loaded {
        config,
        history,
        offset: 0,
    };
    cx.open_window(options(bounds), |window, cx| {
        let router = cx.new(|cx| PageRouter::new(engine, loaded, window, cx));
        router
    })
    .expect("打开设置预览窗口")
}
