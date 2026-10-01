//! T41: defaults, enablement, capabilities, context values and scoped IDs.

use super::model_test_input::{
    choose_context_128000, choose_context_256000, click_id, persisted_config, wait_config_save,
};
use super::*;
use buddy_ui::settings::SettingsView;
use buddy_ui::settings::model_list::ModelListView;

pub(crate) async fn saved(
    handle: WindowHandle<PageRouter>,
    view: &Entity<SettingsView>,
    list: &Entity<ModelListView>,
    id: &str,
    cx: &mut AsyncApp,
) -> bool {
    if !click_id(handle, view, list, id, cx).await {
        return false;
    }
    wait_config_save(handle, cx).await;
    let result = !list.read_with(cx, |list, _| list.saving());
    println!("S06-03 T41 action-{id}: {result}");
    result
}

fn enabled(config: &AppConfig, provider: &str, model: &str) -> bool {
    config
        .providers
        .iter()
        .find(|item| item.id == provider)
        .is_some_and(|item| item.enabled_model_ids.iter().any(|id| id == model))
}

pub(crate) fn model<'a>(
    config: &'a AppConfig,
    id: &str,
) -> Option<&'a buddy_engine::models::ModelInfo> {
    config.models.iter().find(|item| item.id == id)
}

pub(crate) fn check(ok: &mut bool, label: &str, value: bool) {
    println!("S06-03 T41 {label}: {value}");
    *ok &= value;
}

pub(crate) async fn run(
    handle: WindowHandle<PageRouter>,
    view: &Entity<SettingsView>,
    list: &Entity<ModelListView>,
    cx: &mut AsyncApp,
) -> bool {
    let mut ok = true;

    // Disable each model through the actual checkbox.  The current default
    // follows the first remaining enabled model and becomes empty only after
    // the final Provider model is disabled.
    ok &= saved(
        handle,
        view,
        list,
        &format!("enable-{}", fixture::OPENAI_BETA),
        cx,
    )
    .await;
    ok &= saved(
        handle,
        view,
        list,
        &format!("enable-{}", fixture::OPENAI_ALPHA),
        cx,
    )
    .await;
    ok &= saved(
        handle,
        view,
        list,
        &format!("enable-{}", fixture::ANTHROPIC_ALPHA),
        cx,
    )
    .await;
    let Some(config) = super::router_config(handle, cx).await else {
        return false;
    };
    check(
        &mut ok,
        "all-disabled-default-empty",
        config.selected_model_id.is_empty(),
    );
    check(
        &mut ok,
        "openai-alpha-disabled",
        !enabled(&config, fixture::OPENAI_PROVIDER, fixture::OPENAI_ALPHA),
    );
    check(
        &mut ok,
        "anthropic-alpha-disabled",
        !enabled(
            &config,
            fixture::ANTHROPIC_PROVIDER,
            fixture::ANTHROPIC_ALPHA,
        ),
    );

    // Re-enable both OpenAI entries, then set beta as the default.  The model
    // IDs contain repeated separators; the second Provider must remain off.
    ok &= saved(
        handle,
        view,
        list,
        &format!("enable-{}", fixture::OPENAI_ALPHA),
        cx,
    )
    .await;
    ok &= saved(
        handle,
        view,
        list,
        &format!("enable-{}", fixture::OPENAI_BETA),
        cx,
    )
    .await;
    ok &= saved(
        handle,
        view,
        list,
        &format!("default-{}", fixture::OPENAI_BETA),
        cx,
    )
    .await;
    let Some(config) = super::router_config(handle, cx).await else {
        return false;
    };
    check(
        &mut ok,
        "default-beta",
        config.selected_model_id == fixture::OPENAI_BETA,
    );
    check(
        &mut ok,
        "openai-alpha-reenabled",
        enabled(&config, fixture::OPENAI_PROVIDER, fixture::OPENAI_ALPHA),
    );
    check(
        &mut ok,
        "openai-beta-enabled",
        enabled(&config, fixture::OPENAI_PROVIDER, fixture::OPENAI_BETA),
    );
    check(
        &mut ok,
        "anthropic-still-disabled",
        !enabled(
            &config,
            fixture::ANTHROPIC_PROVIDER,
            fixture::ANTHROPIC_ALPHA,
        ),
    );

    let context = list.read_with(cx, |list, _| list.model_context(fixture::OPENAI_BETA));
    let Some(context) = context else {
        println!("FAIL S06-03 T41 context selector missing");
        return false;
    };
    check(
        &mut ok,
        "initial-context-label-33K",
        context
            .read_with(cx, |select, _| select.selected_value())
            .is_some_and(|value| value.as_ref() == "33K"),
    );
    let Some(initial_context_config) = super::router_config(handle, cx).await else {
        println!("FAIL S06-03 T41 initial context read");
        return false;
    };
    check(
        &mut ok,
        "initial-context-32768",
        model(&initial_context_config, fixture::OPENAI_BETA)
            .is_some_and(|item| item.context_window == 32_768),
    );
    // A non-context model edit must preserve the exact non-preset value.
    ok &= saved(
        handle,
        view,
        list,
        &format!("vision-{}", fixture::ANTHROPIC_ALPHA),
        cx,
    )
    .await;
    let Some(after_vision_config) = super::router_config(handle, cx).await else {
        println!("FAIL S06-03 T41 after-vision context read");
        return false;
    };
    check(
        &mut ok,
        "vision-preserves-context-32768",
        model(&after_vision_config, fixture::OPENAI_BETA)
            .is_some_and(|item| item.context_window == 32_768),
    );
    let Some(disk_after_vision) = persisted_config(cx, fixture::sandbox()).await else {
        println!("FAIL S06-03 T41 after-vision disk read");
        return false;
    };
    check(
        &mut ok,
        "disk-context-32768",
        model(&disk_after_vision, fixture::OPENAI_BETA)
            .is_some_and(|item| item.context_window == 32_768),
    );

    // v1 keeps only presets plus the current value.  After changing the
    // non-preset 32768 value to 128K, 33K must disappear and index 0 must be
    // 128K; a stale selector would incorrectly display 256K here.
    ok &= choose_context_128000(handle, view, list, fixture::OPENAI_BETA, cx).await;
    wait_config_save(handle, cx).await;
    check(
        &mut ok,
        "context-reset-index-zero-128K",
        context.read_with(cx, |select, _| {
            select.selected_index() == 0
                && select
                    .selected_value()
                    .is_some_and(|value| value.as_ref() == "128K")
        }),
    );
    let Some(after_128_config) = super::router_config(handle, cx).await else {
        println!("FAIL S06-03 T41 after-128K read");
        return false;
    };
    check(
        &mut ok,
        "context-128000",
        model(&after_128_config, fixture::OPENAI_BETA)
            .is_some_and(|item| item.context_window == 128_000),
    );
    ok &= choose_context_256000(handle, view, list, fixture::OPENAI_BETA, cx).await;
    wait_config_save(handle, cx).await;
    let Some(context_config) = super::router_config(handle, cx).await else {
        println!("FAIL S06-03 T41 after-256K context read");
        return false;
    };
    check(
        &mut ok,
        "context-256000",
        model(&context_config, fixture::OPENAI_BETA)
            .is_some_and(|item| item.context_window == 256_000),
    );

    ok &= super::model_test_capabilities::run(handle, view, list, cx).await;

    println!("{} S06-03 T41", if ok { "PASS" } else { "FAIL" });
    ok
}
