//! T37 provider input, fetch and stale-response scenarios.

use super::*;

pub(super) async fn run(
    handle: WindowHandle<PageRouter>,
    server: &ProviderMock,
    cx: &mut AsyncApp,
) -> bool {
    let Some(panel) = open_panel(handle, cx).await else {
        return false;
    };
    let before = server.request_count();
    let _blank_clicked = click_id(handle, &panel, "fetch", cx).await;
    settle(handle, cx).await;
    let after_blank = server.request_count();
    let blank_silent = _blank_clicked
        && after_blank == before
        && panel.read_with(cx, |panel, _| panel.form().models.is_empty());
    let blank_snapshot = panel.read_with(cx, |panel, _| {
        (
            panel.form().url.clone(),
            panel.form().key.clone(),
            panel.form().models.len(),
        )
    });
    let filled = fill_connection(handle, &panel, &server.url(), cx).await;
    let key_initial_masked =
        panel.read_with(cx, |panel, app| panel.key_field().read(app).masked(app));
    let key_unmasked = click_id(handle, &panel, "show-key", cx).await
        && panel.read_with(cx, |panel, app| !panel.key_field().read(app).masked(app));
    let key_masked_again = click_id(handle, &panel, "show-key", cx).await
        && panel.read_with(cx, |panel, app| panel.key_field().read(app).masked(app));
    let fill_snapshot = panel.read_with(cx, |panel, app| {
        (
            panel.url_field().read(app).text(app),
            panel.key_field().read(app).text(app),
        )
    });
    let (success_requests, success_clicked) =
        fetch_case(handle, &panel, server, MockMode::Success, cx).await;
    let success = panel.read_with(cx, |panel, _| {
        panel.form().models.len() == 2
            && panel.form().selected.len() == 2
            && panel
                .form()
                .models
                .iter()
                .all(|model| model.provider_id == "custom")
    });
    let success_snapshot = panel.read_with(cx, |panel, app| {
        let form = panel.form();
        (
            form.models.len(),
            form.selected.len(),
            form.busy,
            form.error.clone(),
            form.url.clone(),
            form.key.clone(),
            panel.key_field().read(app).masked(app),
        )
    });
    let latency_clicked = click_id(handle, &panel, "latency", cx).await;
    wait_idle(handle, &panel, cx).await;
    let latency = panel.read_with(cx, |panel, _| {
        panel
            .form()
            .models
            .first()
            .is_some_and(|model| model.latency_ms.is_some())
            && panel
                .form()
                .models
                .iter()
                .skip(1)
                .all(|model| model.latency_ms.is_none())
    });
    server.set_mode(MockMode::LatencyError);
    let latency_error_clicked = click_id(handle, &panel, "latency", cx).await;
    wait_idle(handle, &panel, cx).await;
    let latency_error = panel.read_with(cx, |panel, _| panel.form().error.is_some());

    close_panel(handle, &panel, cx).await;
    let Some(panel) = open_panel(handle, cx).await else {
        return false;
    };
    let filled_empty = fill_connection(handle, &panel, &server.url(), cx).await;
    let (empty_requests, empty_clicked) =
        fetch_case(handle, &panel, server, MockMode::Empty, cx).await;
    let empty = panel.read_with(cx, |panel, _| {
        panel.form().models.is_empty() && panel.form().error.is_some()
    });

    close_panel(handle, &panel, cx).await;
    let Some(panel) = open_panel(handle, cx).await else {
        return false;
    };
    let filled_401 = fill_connection(handle, &panel, &server.url(), cx).await;
    let (unauth_requests, unauth_clicked) =
        fetch_case(handle, &panel, server, MockMode::Unauthorized, cx).await;
    let unauthorized = panel.read_with(cx, |panel, _| {
        panel.form().models.is_empty()
            && panel
                .form()
                .error
                .as_deref()
                .is_some_and(|e| e.contains("401"))
    });

    close_panel(handle, &panel, cx).await;
    let Some(panel) = open_panel(handle, cx).await else {
        return false;
    };
    let filled_json = fill_connection(handle, &panel, &server.url(), cx).await;
    let (json_requests, json_clicked) =
        fetch_case(handle, &panel, server, MockMode::InvalidJson, cx).await;
    let invalid_json = panel.read_with(cx, |panel, _| {
        panel.form().models.is_empty()
            && panel
                .form()
                .error
                .as_deref()
                .is_some_and(|e| e.contains("解析"))
    });

    close_panel(handle, &panel, cx).await;
    let Some(panel) = open_panel(handle, cx).await else {
        return false;
    };
    let filled_stale = fill_connection(handle, &panel, &server.url(), cx).await;
    server.set_mode(MockMode::DelayedSuccess);
    let request_before_stale = server.request_count();
    let stale_fetch = click_id(handle, &panel, "fetch", cx).await;
    let delivered = stale_fetch && wait_request(server, request_before_stale, cx).await;
    let stale_revision = panel.read_with(cx, |panel, _| panel.form().revision);
    let changed_target = delivered && click_id(handle, &panel, "custom", cx).await;
    settle(handle, cx).await;
    let stale_ignored = panel.read_with(cx, |panel, _| {
        panel.form().revision > stale_revision && panel.form().models.is_empty()
    });
    close_panel(handle, &panel, cx).await;
    let ok = blank_silent
        && filled
        && key_initial_masked
        && key_unmasked
        && key_masked_again
        && success_clicked
        && success_requests > 0
        && success
        && latency_clicked
        && latency
        && latency_error_clicked
        && latency_error
        && filled_empty
        && empty_clicked
        && empty_requests > 0
        && empty
        && filled_401
        && unauth_clicked
        && unauth_requests > 0
        && unauthorized
        && filled_json
        && json_clicked
        && json_requests > 0
        && invalid_json
        && filled_stale
        && stale_fetch
        && changed_target
        && stale_ignored;
    println!(
        "T37: 空字段静默 {blank_silent} (clicked={_blank_clicked}, before={before}, after_blank={after_blank}, now={}, form={blank_snapshot:?})；成功拉取 {success} {success_snapshot:?}；填写结果 {filled} {fill_snapshot:?}；Key 遮罩初始/显示/恢复 {key_initial_masked}/{key_unmasked}/{key_masked_again}；测速 {latency} / 失败 {latency_error}；空响应 {empty}；401 {unauthorized}；非法 JSON {invalid_json}；旧响应丢弃 {stale_ignored}",
        server.request_count()
    );
    ok
}
