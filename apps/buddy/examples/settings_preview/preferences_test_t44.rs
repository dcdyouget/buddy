//! T44：主题即时切换、持久化、失败保留与重新打开恢复。

use super::*;
use buddy_engine::models::Theme;
use buddy_ui::theme_system::{Appearance, BuddyTheme, Theme as UiTheme};
use std::fs;

fn config_path() -> std::path::PathBuf {
    fixture::sandbox().join("config.json")
}

async fn focus_theme(
    handle: WindowHandle<PageRouter>,
    control: &Entity<buddy_ui::settings::theme_control::ThemeControl>,
    id: &str,
    cx: &mut AsyncApp,
) -> bool {
    let Some(focus) = control.read_with(cx, |control, _| {
        control
            .focus_controls()
            .into_iter()
            .find(|(name, _)| name == id)
            .map(|(_, focus)| focus)
    }) else {
        return false;
    };
    for _ in 0..6 {
        if cx
            .update_window(handle.into(), |_, window, _| focus.is_focused(window))
            .unwrap_or(false)
        {
            return true;
        }
        super::input::press(handle, "shift-tab", cx).await;
    }
    false
}

pub(crate) async fn run(handle: WindowHandle<PageRouter>, cx: &mut AsyncApp) -> bool {
    macro_rules! fail {
        () => {{
            println!("FAIL T44: 主题控件不可用");
            return false;
        }};
    }
    let before_size = preferences_test_input::viewport_size(handle, cx);
    let Some(control) = theme(handle, cx) else {
        fail!();
    };
    let Some(dark) = control.read_with(cx, |control, _| control.control_bounds("theme-dark"))
    else {
        fail!();
    };
    preferences_test_input::click(handle, dark, cx).await;
    wait_save(handle, cx).await;
    let immediate = cx.update(|app| app.buddy_theme().appearance == Appearance::Dark);
    let selected_and_saved = control
        .read_with(cx, |control, _| matches!(control.theme(), Theme::Dark))
        && config(handle, cx).is_some_and(|config| matches!(config.theme, Theme::Dark))
        && persisted_config(cx, fixture::sandbox())
            .await
            .is_some_and(|config| matches!(config.theme, Theme::Dark));

    // 保存失败时旧配置、全局外观与控件选择都必须保持深色。
    let path = config_path();
    let backup = path.with_extension("t44-backup");
    let had_file = path.exists();
    if had_file {
        let _ = fs::rename(&path, &backup);
    }
    let _ = fs::create_dir(&path);
    let enter_focused = focus_theme(handle, &control, "theme-light", cx).await;
    preferences_test_input::key_down(handle, "enter", buddy_ui::gpui::Modifiers::none(), cx).await;
    wait_save(handle, cx).await;
    let failure_kept = control.read_with(cx, |control, _| {
        matches!(control.theme(), Theme::Dark)
            && !control.saving()
            && control
                .error()
                .is_some_and(|message| message.contains("保存外观失败"))
    }) && config(handle, cx)
        .is_some_and(|config| matches!(config.theme, Theme::Dark))
        && cx.update(|app| app.buddy_theme().appearance == Appearance::Dark);
    let _ = fs::remove_dir(&path);
    if had_file {
        let _ = fs::rename(&backup, &path);
    }

    // 用 Space 重试保存浅色，再点回深色，留下可供重启读取的配置。
    let space_focused = focus_theme(handle, &control, "theme-light", cx).await;
    preferences_test_input::key_down(handle, "space", buddy_ui::gpui::Modifiers::none(), cx).await;
    wait_save(handle, cx).await;
    let light_retry = control.read_with(cx, |control, _| matches!(control.theme(), Theme::Light))
        && config(handle, cx).is_some_and(|config| matches!(config.theme, Theme::Light));
    let Some(dark) = control.read_with(cx, |control, _| control.control_bounds("theme-dark"))
    else {
        fail!();
    };
    preferences_test_input::click(handle, dark, cx).await;
    wait_save(handle, cx).await;
    let dark_again = config(handle, cx).is_some_and(|config| matches!(config.theme, Theme::Dark));

    // 新建真实 PageRouter 读取 engine 配置，模拟重启；窗口大小不能改变。
    let disk = persisted_config(cx, fixture::sandbox()).await;
    let _ = handle.update(cx, |router, _, cx| router.close_settings(cx));
    preferences_test_input::settle(handle, cx).await;
    cx.update(|app| UiTheme::install(Appearance::Light, app));
    let Some(disk) = disk else {
        fail!();
    };
    let restarted = cx
        .update(|app| {
            let bounds = Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), app);
            let engine = ChatEngine::new(fixture::sandbox());
            let loaded = Loaded {
                config: disk,
                history: Vec::new(),
                offset: 0,
            };
            app.open_window(fixture::options(bounds), |window, cx| {
                cx.new(|cx| PageRouter::new(engine, loaded, window, cx))
            })
        })
        .ok();
    let Some(restarted) = restarted else {
        fail!();
    };
    let reopened = open_settings(restarted, cx).await;
    let restored = reopened
        .and_then(|_| theme(restarted, cx))
        .is_some_and(|control| {
            control.read_with(cx, |control, _| matches!(control.theme(), Theme::Dark))
                && cx.update(|app| app.buddy_theme().appearance == Appearance::Dark)
        });
    let size_kept = before_size == preferences_test_input::viewport_size(restarted, cx);
    let ok = immediate
        && selected_and_saved
        && enter_focused
        && failure_kept
        && space_focused
        && light_retry
        && dark_again
        && restored
        && size_kept;
    println!(
        "T44: 即时深色 {immediate}，选择/内存/读盘 {selected_and_saved}，Enter 聚焦/失败保留 {enter_focused}/{failure_kept}，Space 重试浅色 {space_focused}/{light_retry}，再次深色 {dark_again}，重启读盘恢复 {restored}，窗口尺寸 {size_kept}"
    );
    ok
}
