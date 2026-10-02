//! T42: save failures, retry, queued updates and returning from Settings.

use super::model_test_input::{
    choose_context_128000, choose_context_256000, choose_context_256000_focused, click_id,
    focused_model_control, persisted_config, same_config, wait_config_save,
};
use super::*;
use buddy_ui::settings::SettingsView;
use buddy_ui::settings::model_list::ModelListView;
use std::fs;

fn read_disk(path: &std::path::Path) -> Option<Vec<u8>> {
    fs::read(path).ok()
}

fn check(ok: &mut bool, label: &str, value: bool) {
    println!("S06-03 T42 {label}: {value}");
    *ok &= value;
}

pub(crate) async fn run(
    handle: WindowHandle<PageRouter>,
    _view: &Entity<SettingsView>,
    _list: &Entity<ModelListView>,
    cx: &mut AsyncApp,
) -> bool {
    // T41 deliberately returned to the conversation.  Re-open the same
    // settings entity through the router before exercising retry and queue
    // behavior, matching the user path after a failed save.
    let Some((view, list)) = super::open_settings(handle, cx).await else {
        println!("FAIL S06-03 T42：无法重新打开设置");
        return false;
    };
    let mut ok = true;
    let Some(reopened) = super::router_config(handle, cx).await else {
        println!("FAIL S06-03 T42 reopened config read");
        return false;
    };
    let rendered = list.read_with(cx, |list, _| list.config().clone());
    check(
        &mut ok,
        "reopened-config",
        same_config(&reopened, &rendered),
    );
    // Exercise the model-list focus order without changing any state.  Each
    // assertion reads the actual FocusHandle state after the event.
    let first_focus = format!("enable-{}", fixture::OPENAI_ALPHA);
    let second_focus = format!("context-{}", fixture::OPENAI_ALPHA);
    let third_focus = format!("vision-{}", fixture::OPENAI_ALPHA);
    let Some(alpha_context) =
        list.read_with(cx, |list, _| list.model_context(fixture::OPENAI_ALPHA))
    else {
        println!("FAIL S06-03 T42 context focus missing");
        return false;
    };
    // S06-05/06 在模型前新增两个外观按钮和录制入口。
    for _ in 0..3 {
        input::press(handle, "tab", cx).await;
    }
    input::press(handle, "tab", cx).await;
    check(
        &mut ok,
        "tab-first-control",
        focused_model_control(handle, cx).as_deref() == Some(first_focus.as_str()),
    );
    input::press(handle, "tab", cx).await;
    check(
        &mut ok,
        "tab-second-control",
        focused_model_control(handle, cx).as_deref() == Some(second_focus.as_str()),
    );
    let Some(after_tab) = super::router_config(handle, cx).await else {
        println!("FAIL S06-03 T42 post-tab config read");
        return false;
    };
    check(
        &mut ok,
        "tab-does-not-change-config",
        same_config(&reopened, &after_tab),
    );
    let focused_context_changed = choose_context_256000_focused(handle, &alpha_context, cx).await;
    wait_config_save(handle, cx).await;
    check(
        &mut ok,
        "context-enter-down-selection",
        focused_context_changed,
    );
    input::press(handle, "tab", cx).await;
    check(
        &mut ok,
        "tab-third-control",
        focused_model_control(handle, cx).as_deref() == Some(third_focus.as_str()),
    );
    input::press(handle, "shift-tab", cx).await;
    check(
        &mut ok,
        "shift-tab-previous-control",
        focused_model_control(handle, cx).as_deref() == Some(second_focus.as_str()),
    );
    let Some(after_context_edit) = super::router_config(handle, cx).await else {
        println!("FAIL S06-03 T42 post-context config read");
        return false;
    };
    check(
        &mut ok,
        "context-edit-memory-256000",
        after_context_edit
            .models
            .iter()
            .find(|model| model.id == fixture::OPENAI_ALPHA)
            .is_some_and(|model| model.context_window == 256_000),
    );
    let Some(after_context_disk) = persisted_config(cx, fixture::sandbox()).await else {
        println!("FAIL S06-03 T42 post-context disk read");
        return false;
    };
    check(
        &mut ok,
        "context-edit-disk-256000",
        after_context_disk
            .models
            .iter()
            .find(|model| model.id == fixture::OPENAI_ALPHA)
            .is_some_and(|model| model.context_window == 256_000),
    );
    // Navigate from Alpha's context through both capabilities to its default
    // action; activation must be a real key event, not a direct config edit.
    for _ in 0..3 {
        input::press(handle, "tab", cx).await;
    }
    check(
        &mut ok,
        "default-keyboard-focus",
        focused_model_control(handle, cx).as_deref()
            == Some(format!("default-{}", fixture::OPENAI_ALPHA).as_str()),
    );
    input::press(handle, "enter", cx).await;
    wait_config_save(handle, cx).await;
    check(
        &mut ok,
        "default-keyboard-persisted",
        persisted_config(cx, fixture::sandbox())
            .await
            .is_some_and(|config| config.selected_model_id == fixture::OPENAI_ALPHA),
    );
    let sandbox = fixture::sandbox();
    let config_path = sandbox.join("config.json");
    let backup_path = sandbox.join("config.s06-03-before-failure.json");

    // Make the exact config target a directory.  A real model interaction now
    // must report a save error while preserving the on-disk bytes.
    let before_bytes = read_disk(&config_path);
    if config_path.is_file() {
        let _ = fs::copy(&config_path, &backup_path);
        let _ = fs::remove_file(&config_path);
    }
    let _ = fs::create_dir(&config_path);
    let Some(before_config) = super::router_config(handle, cx).await else {
        println!("FAIL S06-03 T42 failure baseline read");
        return false;
    };
    let clicked = choose_context_128000(handle, &view, &list, fixture::OPENAI_ALPHA, cx).await;
    wait_config_save(handle, cx).await;
    check(&mut ok, "failure-control-clicked", clicked);
    check(
        &mut ok,
        "failure-error-visible",
        list.read_with(cx, |list, _| list.error().is_some()),
    );
    let Some(alpha_context_after_failure) =
        list.read_with(cx, |list, _| list.model_context(fixture::OPENAI_ALPHA))
    else {
        println!("FAIL S06-03 T42 failed context selector missing");
        return false;
    };
    check(
        &mut ok,
        "failure-selector-restored-256000",
        alpha_context_after_failure
            .read_with(cx, |select, _| select.selected_value())
            .is_some_and(|value| value.as_ref() == "256K"),
    );
    check(
        &mut ok,
        "failure-target-remains-directory",
        read_disk(&config_path).is_none(),
    );
    check(
        &mut ok,
        "failure-backup-unchanged",
        before_bytes == read_disk(&backup_path),
    );
    let Some(failed_config) = super::router_config(handle, cx).await else {
        println!("FAIL S06-03 T42 failure config read");
        return false;
    };
    check(
        &mut ok,
        "failure-memory-unchanged",
        same_config(&before_config, &failed_config),
    );

    // Remove only the test directory and retry the same draft through the same
    // control.  This is a real retry, not a new router or direct setter.
    let _ = fs::remove_dir(&config_path);
    if backup_path.is_file() {
        let _ = fs::rename(&backup_path, &config_path);
    }
    let retry_clicked =
        choose_context_128000(handle, &view, &list, fixture::OPENAI_ALPHA, cx).await;
    check(&mut ok, "retry-control-clicked", retry_clicked);
    wait_config_save(handle, cx).await;
    let Some(saved) = persisted_config(cx, sandbox.clone()).await else {
        println!("FAIL S06-03 T42：重试后无法读取配置");
        return false;
    };
    check(
        &mut ok,
        "retry-persists-context",
        saved
            .models
            .iter()
            .find(|model| model.id == fixture::OPENAI_ALPHA)
            .is_some_and(|model| model.context_window == 128_000),
    );

    // Exercise several real edits in sequence.  Each save is awaited before
    // the next control is touched; ordering/interleaving is covered by the
    // deterministic Router/model-config unit seam, not this GUI path.
    let first = click_id(
        handle,
        &view,
        &list,
        &format!("vision-{}", fixture::OPENAI_ALPHA),
        cx,
    )
    .await;
    // The first save must be awaited before the second control is touched;
    // ModelListView intentionally disables its controls while saving.
    wait_config_save(handle, cx).await;
    let second = choose_context_128000(handle, &view, &list, fixture::OPENAI_BETA, cx).await;
    wait_config_save(handle, cx).await;
    let third = choose_context_256000(handle, &view, &list, fixture::OPENAI_BETA, cx).await;
    wait_config_save(handle, cx).await;
    check(&mut ok, "sequential-first-action", first);
    check(&mut ok, "sequential-second-action", second);
    check(&mut ok, "sequential-third-action", third);
    let Some(queued) = persisted_config(cx, sandbox.clone()).await else {
        return false;
    };
    check(
        &mut ok,
        "sequential-context-256000",
        queued
            .models
            .iter()
            .find(|model| model.id == fixture::OPENAI_BETA)
            .is_some_and(|model| model.context_window == 256_000),
    );
    check(
        &mut ok,
        "sequential-vision-state",
        queued
            .models
            .iter()
            .find(|model| model.id == fixture::OPENAI_ALPHA)
            .is_some_and(|model| !model.supports_vision),
    );

    println!("{} S06-03 T42", if ok { "PASS" } else { "FAIL" });
    ok
}
