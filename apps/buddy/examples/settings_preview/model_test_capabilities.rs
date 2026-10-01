//! T41 capability and scoped-provider checks.

use super::fixture;
use super::model_test_input::{click_id, same_config, wait_config_save};
use super::model_test_t41::{check, model, saved};
use super::*;
use buddy_ui::settings::SettingsView;
use buddy_ui::settings::model_list::ModelListView;

pub(crate) async fn run(
    handle: WindowHandle<PageRouter>,
    view: &Entity<SettingsView>,
    list: &Entity<ModelListView>,
    cx: &mut AsyncApp,
) -> bool {
    let mut ok = true;

    // Vision and image-generation are independent toggles. Anthropic image
    // generation stays disabled even when the same raw model name is present.
    ok &= saved(
        handle,
        view,
        list,
        &format!("vision-{}", fixture::OPENAI_ALPHA),
        cx,
    )
    .await;
    ok &= saved(
        handle,
        view,
        list,
        &format!("image-{}", fixture::OPENAI_ALPHA),
        cx,
    )
    .await;
    let Some(before_anthropic) = super::router_config(handle, cx).await else {
        println!("FAIL S06-03 T41 before-capability read");
        return false;
    };
    let disabled_click = click_id(
        handle,
        view,
        list,
        &format!("image-{}", fixture::ANTHROPIC_ALPHA),
        cx,
    )
    .await;
    wait_config_save(handle, cx).await;
    let Some(after_anthropic) = super::router_config(handle, cx).await else {
        println!("FAIL S06-03 T41 after-capability read");
        return false;
    };
    ok &= disabled_click;
    check(
        &mut ok,
        "openai-capabilities",
        model(&after_anthropic, fixture::OPENAI_ALPHA)
            .is_some_and(|item| item.supports_vision && item.supports_image_generation),
    );
    check(
        &mut ok,
        "anthropic-image-disabled",
        model(&after_anthropic, fixture::ANTHROPIC_ALPHA)
            .is_some_and(|item| !item.supports_image_generation),
    );
    check(
        &mut ok,
        "disabled-image-no-config-change",
        same_config(&before_anthropic, &after_anthropic),
    );

    // Re-enable the Anthropic alpha via its own scoped checkbox and verify the
    // OpenAI alpha stays enabled; same raw name must never cross Providers.
    ok &= saved(
        handle,
        view,
        list,
        &format!("enable-{}", fixture::ANTHROPIC_ALPHA),
        cx,
    )
    .await;
    let Some(final_config) = super::router_config(handle, cx).await else {
        println!("FAIL S06-03 T41 final config read");
        return false;
    };
    check(
        &mut ok,
        "openai-alpha-scope-isolated",
        enabled(
            &final_config,
            fixture::OPENAI_PROVIDER,
            fixture::OPENAI_ALPHA,
        ),
    );
    check(
        &mut ok,
        "anthropic-alpha-scope-isolated",
        enabled(
            &final_config,
            fixture::ANTHROPIC_PROVIDER,
            fixture::ANTHROPIC_ALPHA,
        ),
    );

    println!(
        "{} S06-03 T41 capabilities",
        if ok { "PASS" } else { "FAIL" }
    );
    ok
}

fn enabled(config: &AppConfig, provider: &str, model_id: &str) -> bool {
    config
        .providers
        .iter()
        .find(|item| item.id == provider)
        .is_some_and(|item| item.enabled_model_ids.iter().any(|id| id == model_id))
}
