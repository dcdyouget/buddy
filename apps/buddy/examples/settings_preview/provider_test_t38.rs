//! T38 persistence, model identity and save-failure scenarios.

use super::*;
use std::{fs, path::PathBuf};

fn config_path() -> PathBuf {
    fixture::sandbox().join("config.json")
}

pub(super) async fn run(
    handle: WindowHandle<PageRouter>,
    server: &ProviderMock,
    cx: &mut AsyncApp,
) -> bool {
    let Some(panel) = open_panel(handle, cx).await else {
        return false;
    };
    let filled = fill_connection(handle, &panel, &server.url(), cx).await;
    let (_, fetched) = fetch_case(handle, &panel, server, MockMode::Success, cx).await;
    let selected_all = panel.read_with(cx, |panel, _| panel.form().selected.len() == 2);
    let deselect_one = click_id(handle, &panel, "model-shared", cx).await;
    let one_selected = panel.read_with(cx, |panel, _| panel.form().selected.len() == 1);
    let deselect_two = click_id(handle, &panel, "model-alpha", cx).await;
    let none_selected = panel.read_with(cx, |panel, _| {
        panel.form().selected.is_empty() && !panel.form().can_add()
    });
    let restore_one = click_id(handle, &panel, "model-alpha", cx).await;
    let restore_two = click_id(handle, &panel, "model-shared", cx).await;
    let restored = panel.read_with(cx, |panel, _| panel.form().selected.len() == 2);
    let vision = click_id(handle, &panel, "vision-alpha", cx).await;
    let image = click_id(handle, &panel, "image-alpha", cx).await;
    let context = click_id(handle, &panel, "context-alpha", cx).await;
    let menu = panel.read_with(cx, |panel, app| panel.context_menu_bounds("alpha", app));
    let viewport = input::viewport_size(handle, cx).unwrap();
    let menu_visible = menu.is_some_and(|bounds| bounds.top() >= px(0.0) && bounds.bottom() <= viewport.height);
    if let Some(bounds) = menu {
        input::click(handle, f32::from(bounds.center().x), f32::from(bounds.top() + bounds.size.height * 0.375), cx).await;
    }
    let context_choice = context && menu_visible
        && panel.read_with(cx, |panel, _| {
            panel
                .form()
                .models
                .iter()
                .find(|model| model.id == "alpha")
                .is_some_and(|model| model.context_window != 128_000)
        });
    let add = click_id(handle, &panel, "add", cx).await;
    settle(handle, cx).await;
    let saved = handle
        .read_with(cx, |router, _| {
            let config = router.config();
            config.providers.len() == 1
                && config.models.len() == 2
                && config.selected_model_id == config.models[0].id
                && config.models[0].id.ends_with("::alpha")
                && config.models[0].api_model_id.as_deref() == Some("alpha")
                && config.providers[0].api_key == "loopback-key"
                && config.models[0].supports_vision
                && config.models[0].supports_image_generation
                && config.models[0].context_window != 128_000
                && router.page() == Page::Conversation
        })
        .unwrap_or(false);
    wait_config_save(handle, cx).await;
    let memory_first = handle
        .read_with(cx, |router, _| router.config().clone())
        .ok();
    let disk_first = persisted_config(cx).await;
    let persisted_first = memory_first
        .as_ref()
        .zip(disk_first.as_ref())
        .is_some_and(|(memory, disk)| same_config(memory, disk));

    let Some(second_panel) = open_panel(handle, cx).await else {
        return false;
    };
    let second_filled =
        fill_preset(handle, &second_panel, "preset-openai", &server.url(), cx).await;
    let (_, second_fetched) =
        fetch_case(handle, &second_panel, server, MockMode::Success, cx).await;
    let second_add = click_id(handle, &second_panel, "add", cx).await;
    settle(handle, cx).await;
    wait_config_save(handle, cx).await;
    let scoped = handle
        .read_with(cx, |router, _| {
            let config = router.config();
            config.providers.len() == 2
                && config
                    .models
                    .iter()
                    .any(|model| model.id.starts_with("custom-") && model.id.ends_with("::alpha"))
                && config.models.iter().any(|model| {
                    model.id == "openai::alpha" && model.api_model_id.as_deref() == Some("alpha")
                })
                && config.selected_model_id == "openai::alpha"
        })
        .unwrap_or(false);

    let Some(panel) = open_panel(handle, cx).await else {
        return false;
    };
    let before_form = fill_connection(handle, &panel, &server.url(), cx).await;
    let (_, before_fetch) = fetch_case(handle, &panel, server, MockMode::Success, cx).await;
    let before_revision = panel.read_with(cx, |panel, _| panel.form().revision);
    let before_config = handle
        .read_with(cx, |router, _| router.config().clone())
        .ok();
    let disk_before_failure = persisted_config(cx).await;
    let path = config_path();
    let backup = path.with_extension("provider-test-backup");
    let had_file = path.exists();
    if had_file {
        let _ = fs::rename(&path, &backup);
    }
    let _ = fs::create_dir(&path);
    let failed_add = click_id(handle, &panel, "add", cx).await;
    settle(handle, cx).await;
    let failure_kept = panel.read_with(cx, |panel, _| {
        panel.form().revision >= before_revision
            && panel.form().key == "loopback-key"
            && panel.form().error.is_some()
    });
    let failure_snapshot = panel.read_with(cx, |panel, _| {
        (
            panel.form().busy,
            panel.form().error.clone(),
            panel.form().key.clone(),
            panel.form().models.len(),
        )
    });
    let failure_page_and_config = handle
        .read_with(cx, |router, _| {
            router.page() == Page::Settings
                && before_config
                    .as_ref()
                    .is_some_and(|before| same_config(before, router.config()))
        })
        .unwrap_or(false);
    let _ = fs::remove_dir(&path);
    if had_file {
        let _ = fs::rename(&backup, &path);
    }
    let disk_after_failure = persisted_config(cx).await;
    let failure_disk_unchanged = disk_before_failure
        .as_ref()
        .zip(disk_after_failure.as_ref())
        .is_some_and(|(before, after)| same_config(before, after));
    let retried = click_id(handle, &panel, "add", cx).await;
    wait_config_save(handle, cx).await;
    settle(handle, cx).await;
    let retry_saved = handle
        .read_with(cx, |router, _| {
            router.page() == Page::Conversation && router.config().providers.len() == 3
        })
        .unwrap_or(false)
        && persisted_config(cx)
            .await
            .is_some_and(|config| config.providers.len() == 3);
    let Some(panel) = open_panel(handle, cx).await else {
        return false;
    };
    let _ = click_id(handle, &panel, "cancel", cx).await;
    let provider_closed = handle
        .read_with(cx, |router, app| {
            !router.settings_view().read(app).provider_open() && router.page() == Page::Settings
        })
        .unwrap_or(false);
    let key_before_exit = panel.read_with(cx, |panel, app| panel.key_field().read(app).text(app));
    let draft_before_exit = handle
        .read_with(cx, |router, app| router.composer().read(app).draft(app))
        .unwrap_or_default();
    let pasted_after_exit = paste_field(handle, &panel, true, "after-exit", cx).await;
    let draft_after_exit = handle
        .read_with(cx, |router, app| router.composer().read(app).draft(app))
        .unwrap_or_default();
    let key_after_exit = panel.read_with(cx, |panel, app| panel.key_field().read(app).text(app));
    let exit_input_blocked = provider_closed
        && !pasted_after_exit
        && key_before_exit == key_after_exit
        && draft_before_exit == draft_after_exit;
    let ok = filled
        && fetched
        && selected_all
        && deselect_one
        && one_selected
        && deselect_two
        && none_selected
        && restore_one
        && restore_two
        && restored
        && vision
        && image
        && context_choice
        && add
        && saved
        && persisted_first
        && second_filled
        && second_fetched
        && second_add
        && scoped
        && before_form
        && before_fetch
        && failed_add
        && failure_kept
        && failure_page_and_config
        && failure_disk_unchanged
        && retried
        && retry_saved
        && exit_input_blocked;
    println!(
        "T38: 多选 {selected_all}/{one_selected}/{none_selected}/{restored}；保存往返 {saved}（落盘 {persisted_first}）；能力/上下文 {vision}/{image}/{context_choice}；第二 Provider {scoped}；保存失败保留草稿 {failure_kept} {failure_snapshot:?} / 页面配置不变 {failure_page_and_config} / 落盘不变 {failure_disk_unchanged}；原草稿重试 {retry_saved}；退出输入隔离 {exit_input_blocked}"
    );
    ok
}
